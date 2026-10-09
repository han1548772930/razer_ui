//! Portable worker routes: native nodes and source-proved Razer queries.
//! No Windows ContainerId, guessed peer routing, DLL or OS API in this layer.
use super::ServiceRequest;
use anyhow::{Context as _, bail, ensure};
use razer_device::backend::HidBackend;
use razer_device::backend::HidNode;
use razer_device::device_identity;
use razer_device::device_identity::IdentityLookup;
use razer_device::device_query;
use razer_device::device_reads;
use razer_device::receiver_capabilities;
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct PortableRuntime {
    transactions: HashMap<String, u8>,
}

impl PortableRuntime {
    fn transaction(
        &mut self,
        node: &HidNode,
        class: &str,
        prefix: u8,
        modulus: u8,
    ) -> anyhow::Result<u8> {
        ensure!(modulus > 0, "源事务范围无效");
        let key = format!(
            "{:x}:{}:{}:{}:{}",
            md5::compute(&node.path),
            node.usage_page,
            node.usage,
            class,
            prefix
        );
        let next = self.transactions.entry(key).or_default();
        let result = prefix | (*next % modulus);
        *next = (*next + 1) % modulus;
        Ok(result)
    }

    fn revalidate(node: &HidNode) -> anyhow::Result<()> {
        let current = with_backend(|backend| backend.enumerate())?;
        ensure!(
            current.iter().filter(|value| *value == node).count() == 1,
            "查询期间 HID collection 身份变化或不唯一"
        );
        Ok(())
    }

    pub(super) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        match request {
            ServiceRequest::HidNodes => {
                let nodes = with_backend(|backend| backend.enumerate())?
                    .into_iter()
                    .filter(|node| matches!(node.vendor_id, 0x1532 | 0x068e))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"nodes":nodes, "transport":"hidapi", "platform":std::env::consts::OS,
                    "identity_scope":"hid_collection", "enumeration_completeness":"not_reported_by_backend"}),
                )
            }
            ServiceRequest::HidNodeRead {
                node,
                product_id,
                kind,
            } => {
                let cap =
                    device_reads::capability(product_id).context("产品没有源核实的基础查询能力")?;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && cap.direct_pids.contains(&u32::from(node.product_id))
                        && node.interface_number == i32::from(cap.claim_interface),
                    "HID 节点不符合源产品/接口选择；未知接口号不能猜测"
                );
                let IdentityLookup::Unique(identity) =
                    device_identity::lookup(u32::from(node.product_id))
                else {
                    bail!("直接设备身份映射不唯一");
                };
                ensure!(
                    identity.product_id == product_id && !identity.is_dongle,
                    "此入口只查询已核实的直接设备；接收器节点不能冒充鼠标"
                );
                let command = cap
                    .queries
                    .iter()
                    .find(|command| command.name == kind)
                    .context("产品不支持该源查询")?;
                let started = Instant::now();
                let deadline = || deadline(started);
                let device = with_backend(|backend| backend.open(&node))?;
                Self::revalidate(&node)?;
                let transaction = self.transaction(
                    &node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )?;
                let reading = device_query::read_device(
                    device.as_ref(),
                    cap,
                    command,
                    transaction,
                    deadline,
                )?;
                Self::revalidate(&node)?;
                deadline()?;
                Ok(
                    json!({"node":node,"product_id":product_id,"reading":reading,
                    "method":command.method,"source_class":cap.source_class,"transaction_id":transaction,
                    "transport_metadata":device.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64,
                    "identity_scope":"hid_collection", "evidence":"docs/re/mouse-read-capabilities-current-evidence.json"}),
                )
            }
            ServiceRequest::HidNodeWrite {
                node,
                product_id,
                setting,
            } => {
                let cap =
                    device_reads::capability(product_id).context("产品没有源核实的基础查询能力")?;
                let write_cap = razer_device::device_writes::capability(product_id)
                    .context("产品没有源核实的直接写入能力")?;
                razer_device::device_writes::prepare(write_cap, &setting)?;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && cap.direct_pids.contains(&u32::from(node.product_id))
                        && node.interface_number == i32::from(cap.claim_interface),
                    "HID 写入节点不符合源产品/接口选择"
                );
                let IdentityLookup::Unique(identity) =
                    device_identity::lookup(u32::from(node.product_id))
                else {
                    bail!("写入目标没有唯一的直接设备身份");
                };
                ensure!(
                    identity.product_id == product_id && !identity.is_dongle && !identity.is_ble,
                    "写入入口只接受已核实的直接设备，不接受接收器或 BLE 路由"
                );
                let started = Instant::now();
                let device = with_backend(|backend| backend.open(&node))?;
                let result = razer_device::device_writes::apply(
                    device.as_ref(),
                    cap,
                    &setting,
                    || {
                        self.transaction(
                            &node,
                            &cap.source_class,
                            cap.transaction_prefix,
                            cap.transaction_modulus,
                        )
                    },
                    || {
                        ensure!(
                            started.elapsed() < Duration::from_secs(20),
                            "设备写入已超过确认期限"
                        );
                        Self::revalidate(&node)
                    },
                )?;
                Ok(json!({"node":node,"product_id":product_id,"result":result,
                    "source_class":cap.source_class,"transport_metadata":device.metadata(),
                    "identity_scope":"hid_collection","elapsed_ms":started.elapsed().as_millis() as u64,
                    "evidence":"docs/re/device-write-capabilities-current-evidence.json"}))
            }
            ServiceRequest::HidNodeReceiverStatus { node } => {
                let cap = receiver_capabilities::capability(node.product_id)
                    .context("产品没有源核实的接收器查询能力")?;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && node.interface_number == i32::from(cap.claim_interface),
                    "接收器节点不符合源 VID/接口选择；未知接口号不能猜测"
                );
                let started = Instant::now();
                let device = with_backend(|backend| backend.open(&node))?;
                Self::revalidate(&node)?;
                let transaction = self.transaction(
                    &node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )?;
                let devices =
                    device_query::read_receiver(device.as_ref(), cap, transaction, || {
                        deadline(started)
                    })?;
                Self::revalidate(&node)?;
                deadline(started)?;
                let rows = devices
                    .into_iter()
                    .map(|(product_id, status)| json!({"product_id":product_id,"status":status}))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"node":node,"query":"receiver_wireless_status_v2","devices":rows,"device_count":rows.len(),
                    "transaction_id":transaction,"source_class":cap.source_class,"capability_evidence":cap.evidence_path,
                    "transport_metadata":device.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64,
                    "identity_scope":"hid_collection"}),
                )
            }
            _ => bail!("此操作需要尚未移植的平台服务或设备身份适配"),
        }
    }
}

fn deadline(started: Instant) -> anyhow::Result<()> {
    ensure!(
        started.elapsed() < Duration::from_secs(10),
        "设备查询已超过观察期限"
    );
    Ok(())
}
