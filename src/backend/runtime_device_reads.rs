//! Shared source-described device queries through the pinned native HID ABI.
//! Every request rechecks physical identity and, for a relay, the actual peer.
use super::{hid, hid_transport, receiver};
use crate::backend::{
    device_identity::{self, IdentityLookup},
    device_reads::{self, DeviceReadCapability, DeviceReadKind, DeviceReadTarget, ReadReply},
};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

fn current_target(target: &DeviceReadTarget, cap: &DeviceReadCapability) -> anyhow::Result<Value> {
    ensure!(
        receiver::valid_container(&target.device_container_id),
        "设备查询容器身份无效"
    );
    ensure!(
        target.path.is_ascii()
            && !target.path.contains('\0')
            && target.path.to_ascii_lowercase().starts_with(r"\\?\hid#"),
        "设备查询路径无效"
    );
    let pid = u16::try_from(target.physical_product_id)?;
    let snapshot = hid::enumerate_for_product(cap.vendor_id, pid)?;
    ensure!(snapshot["complete"] == true, "设备查询目标枚举未完成");
    let items = snapshot["interfaces"]
        .as_array()
        .context("设备查询缺少HID接口")?;
    let matches = items
        .iter()
        .filter(|item| {
            item["path"]
                .as_str()
                .is_some_and(|path| path.eq_ignore_ascii_case(&target.path))
        })
        .collect::<Vec<_>>();
    ensure!(matches.len() == 1, "设备查询路径不是唯一的当前接口");
    let observed = matches[0];
    let claim_interface = if target.peer_product_id.is_some() {
        crate::backend::receiver_capabilities::capability(pid)
            .context("物理接收器能力未核实")?
            .claim_interface
    } else {
        cap.claim_interface
    };
    ensure!(
        observed["vendor_id"] == cap.vendor_id
            && observed["product_id"] == pid
            && observed["claim_interface"] == claim_interface
            && observed["feature_report_bytes"] == cap.report_bytes
            && observed["device_container_id"]
                .as_str()
                .is_some_and(|id| id.eq_ignore_ascii_case(&target.device_container_id)),
        "设备查询路径/物理身份/接口规格已变化"
    );
    ensure!(
        observed["device_instance_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        "设备查询缺少实例身份"
    );
    Ok(observed.clone())
}

fn verify_routing(target: &DeviceReadTarget, cap: &DeviceReadCapability) -> anyhow::Result<()> {
    match target.peer_product_id {
        None => {
            ensure!(
                cap.direct_pids.contains(&target.physical_product_id),
                "源能力没有此直接连接身份"
            );
            let IdentityLookup::Unique(identity) =
                device_identity::lookup(target.physical_product_id)
            else {
                anyhow::bail!("直接设备身份映射不唯一")
            };
            ensure!(
                identity.product_id == target.product_id && !identity.is_dongle,
                "直接设备不属于请求的有线产品，接收器需要真实无线关联"
            );
        }
        Some(peer) => {
            ensure!(
                cap.direct_pids.contains(&peer),
                "无线关联PID不属于当前产品源能力"
            );
            let receiver_cap = crate::backend::receiver_capabilities::capability(u16::try_from(
                target.physical_product_id,
            )?)
            .context("当前物理接收器查询协议未核实")?;
            let IdentityLookup::Unique(identity) =
                device_identity::lookup_receiver_peer(peer, receiver_cap.peer_match_product_id)
            else {
                anyhow::bail!("无线设备身份映射不唯一")
            };
            ensure!(
                identity.product_id == target.product_id,
                "无线设备不属于请求的逻辑产品"
            );
            // A stale saved binding is never sufficient authority to route reads.
            let result = receiver::query(&target.path, &target.device_container_id)?;
            let peers = crate::backend::discovery::receiver_peers(&result)?;
            ensure!(
                peers
                    .iter()
                    .any(|(pid, status)| *pid == peer && *status == 1),
                "接收器没有确认此无线设备当前在线"
            );
        }
    }
    Ok(())
}

fn transaction(target: &DeviceReadTarget, cap: &DeviceReadCapability) -> anyhow::Result<u8> {
    static IDS: OnceLock<Mutex<HashMap<String, u8>>> = OnceLock::new();
    ensure!(cap.transaction_modulus > 0, "设备事务范围无效");
    let key = format!(
        "{}:{}:{}",
        target.path.to_ascii_lowercase(),
        cap.source_class,
        cap.transaction_prefix
    );
    let mut ids = IDS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| anyhow::anyhow!("设备事务状态不可用"))?;
    let next = ids.entry(key).or_default();
    // Source starts transactionId at 0, resets 31 to 0 before returning the
    // old value, then increments. This emits the same 0..30 repeating sequence.
    let counter = *next % cap.transaction_modulus;
    *next = (counter + 1) % cap.transaction_modulus;
    Ok(cap.transaction_prefix | counter)
}

pub(super) fn query(target: &DeviceReadTarget, kind: DeviceReadKind) -> anyhow::Result<Value> {
    let cap = device_reads::capability(target.product_id)
        .context("此产品没有当前源核实的基础读取能力")?;
    let command = cap
        .queries
        .iter()
        .find(|command| command.name == kind)
        .context("此产品不支持该项源查询")?;
    let before = current_target(target, cap)?;
    verify_routing(target, cap)?;
    let started = Instant::now();
    let deadline = || -> anyhow::Result<()> {
        ensure!(
            started.elapsed() < Duration::from_secs(10),
            "设备查询已超过观察期限"
        );
        Ok(())
    };
    let reading = {
        let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        let device = hid_transport::open(&target.path)?;
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "打开设备期间实例发生变化"
        );
        let transaction = transaction(target, cap)?;
        let outgoing = device_reads::query_report(cap, command, transaction)?;
        let mut last_error = "没有收到设备响应".to_string();
        let mut reading = None;
        'send: for _ in 0..cap.max_retry_out {
            std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
            deadline()?;
            match device.send_feature(&outgoing) {
                Ok(count) if count == cap.report_bytes => {}
                Ok(count) => {
                    last_error = format!("查询发送长度不匹配：{count}");
                    continue;
                }
                Err(error) => {
                    last_error = format!("{error:#}");
                    continue;
                }
            }
            std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
            for attempt in 0..cap.max_retry_in {
                if attempt > 0 {
                    std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
                }
                deadline()?;
                let mut report = vec![0; cap.report_bytes];
                report[0] = cap.report_id;
                let count = match device.get_feature(&mut report) {
                    Ok(count) => count,
                    Err(error) => {
                        // The current host turns a failed getFeatureReport into
                        // an empty response; middleware retries with a new OUT.
                        last_error = format!("{error:#}");
                        break;
                    }
                };
                deadline()?;
                ensure!(count == cap.report_bytes, "设备查询返回长度不匹配：{count}");
                match device_reads::decode_report(&report, cap, command, transaction)? {
                    ReadReply::Busy => last_error = "设备忙".into(),
                    ReadReply::Retry(reason) => {
                        last_error = reason;
                        break;
                    }
                    ReadReply::Complete(value) => {
                        reading = Some(value);
                        break 'send;
                    }
                }
            }
        }
        reading.with_context(|| format!("设备查询重试失败：{last_error}"))?
    };
    // Unlock first: the same physical receiver lock also protects peer queries.
    ensure!(
        current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
        "查询期间设备实例发生变化"
    );
    verify_routing(target, cap)?;
    Ok(
        json!({"target":target,"reading":reading,"method":command.method,"source_class":cap.source_class,
        "elapsed_ms":started.elapsed().as_millis() as u64,"evidence":"docs/re/mouse-read-capabilities-current-evidence.json"}),
    )
}
