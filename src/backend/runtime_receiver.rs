//! Worker-only receiver observation: current interface -> source capability ->
//! shared read protocol -> original native HID exports. No device write-back.
use super::hid_transport as transport;

use crate::backend::{
    receiver_capabilities::{ReceiverCapability, capability},
    receiver_protocol::{self as protocol, Reply},
};
use anyhow::{Context as _, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

// Application observation budget, not a vendor protocol constant. The current
// source's maximum scheduled retry sleeps total 5.5s; the parent bounds a stuck
// worker separately at 15s, including native loading and enumeration.
const APPLICATION_QUERY_BUDGET: Duration = Duration::from_secs(10);
static QUERY_LOCK: Mutex<()> = Mutex::new(());

fn next_transaction(
    path: &str,
    container: &str,
    selected: &ReceiverCapability,
) -> anyhow::Result<u8> {
    static TRANSACTIONS: OnceLock<Mutex<HashMap<String, u8>>> = OnceLock::new();
    ensure!(selected.transaction_modulus > 0, "接收器事务范围无效");
    let key = format!(
        "{}:{}:{}:{}",
        path.to_ascii_lowercase(),
        container.to_ascii_lowercase(),
        selected.source_class,
        selected.transaction_prefix
    );
    let mut transactions = TRANSACTIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| anyhow::anyhow!("接收器事务状态不可用"))?;
    let next = transactions.entry(key).or_default();
    let counter = *next % selected.transaction_modulus;
    *next = (counter + 1) % selected.transaction_modulus;
    Ok(selected.transaction_prefix | counter)
}

pub(super) struct ReceiverLock(HANDLE);
impl ReceiverLock {
    pub(super) fn acquire(container: &str) -> anyhow::Result<Self> {
        let name: Vec<u16> = format!("Local\\RazerUi-Receiver-{}", container.to_ascii_lowercase())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        ensure!(
            !handle.is_null(),
            "无法创建接收器查询锁：{}",
            std::io::Error::last_os_error()
        );
        // Coordinates our workers only. Other applications use private locks;
        // transaction/command validation detects a conflicting device response.
        let status = unsafe { WaitForSingleObject(handle, 500) };
        if !matches!(status, WAIT_OBJECT_0 | WAIT_ABANDONED) {
            unsafe {
                CloseHandle(handle);
            }
            bail!("接收器正在被其他查询使用（锁状态 {status}）");
        }
        Ok(Self(handle))
    }
}
impl Drop for ReceiverLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

pub(super) fn valid_container(value: &str) -> bool {
    let Some(inner) = value
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return false;
    };
    inner.len() == 36
        && inner.bytes().enumerate().all(|(ix, ch)| {
            if [8, 13, 18, 23].contains(&ix) {
                ch == b'-'
            } else {
                ch.is_ascii_hexdigit()
            }
        })
        && inner.bytes().any(|ch| ch.is_ascii_hexdigit() && ch != b'0')
}

fn select_target(
    snapshot: &Value,
    path: &str,
    container: &str,
) -> anyhow::Result<(Value, &'static ReceiverCapability)> {
    ensure!(valid_container(container), "接收器 ContainerId 格式无效");
    ensure!(
        path.is_ascii()
            && !path.contains('\0')
            && path.to_ascii_lowercase().starts_with(r"\\?\hid#"),
        "接收器 HID 路径格式无效"
    );
    ensure!(
        snapshot["complete"] == true,
        "HID 枚举未完成，不能确定接收器查询目标"
    );
    let interfaces = snapshot["interfaces"]
        .as_array()
        .context("缺少 HID 接口观察")?;
    let exact: Vec<_> = interfaces
        .iter()
        .filter(|item| {
            item["path"]
                .as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(path))
        })
        .collect();
    ensure!(exact.len() == 1, "请求 HID 路径不对应唯一的当前接口");
    let product_id = exact[0]["product_id"]
        .as_u64()
        .and_then(|id| u16::try_from(id).ok())
        .context("接口缺少有效产品 ID")?;
    let selected = capability(product_id).context("当前产品没有源代码核验的接收器查询能力")?;
    let target = exact[0];
    ensure!(
        target["vendor_id"] == selected.vendor_id
            && target["product_id"] == selected.product_id
            && target["device_container_id"]
                .as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(container))
            && target["claim_interface"] == selected.claim_interface
            && target["feature_report_bytes"] == selected.report_bytes,
        "请求路径不是源匹配接口：VID {:04X}/PID {:04X}、接口 {}、{} 字节 Feature",
        selected.vendor_id,
        selected.product_id,
        selected.claim_interface,
        selected.report_bytes,
    );
    ensure!(
        target["device_instance_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "接收器缺少真实设备实例 ID"
    );
    Ok((target.clone(), selected))
}

fn check_deadline(started: Instant) -> anyhow::Result<()> {
    ensure!(
        started.elapsed() < APPLICATION_QUERY_BUDGET,
        "接收器查询已过期，未将结果作为设备观察"
    );
    Ok(())
}

pub(super) fn query(path: &str, device_container_id: &str) -> anyhow::Result<Value> {
    let _process_lock = QUERY_LOCK
        .try_lock()
        .map_err(|_| anyhow::anyhow!("已有接收器查询正在进行"))?;
    let started = Instant::now();
    let (before, selected) = select_target(
        &super::hid::enumerate_metadata()?,
        path,
        device_container_id,
    )?;
    let _receiver_lock = ReceiverLock::acquire(device_container_id)?;
    check_deadline(started)?;
    let device = transport::open(path)?;
    let revalidate = || -> anyhow::Result<Value> {
        let (current, current_capability) = select_target(
            &super::hid::enumerate_for_product(selected.vendor_id, selected.product_id)?,
            path,
            device_container_id,
        )?;
        ensure!(
            before["device_instance_id"] == current["device_instance_id"]
                && current_capability.product_id == selected.product_id,
            "查询期间接收器实例发生变化"
        );
        check_deadline(started)?;
        Ok(current)
    };
    revalidate()?;
    let transaction = next_transaction(path, device_container_id, selected)?;
    let outgoing = protocol::query_report(selected, transaction)?;
    let mut last_error = "尚未取得连接状态".to_owned();
    // Exact factory timing/retry parameters are source-derived, including the
    // factory's multiplier. Keep one transaction across retries, as source does.
    for _ in 0..selected.max_retry_out {
        std::thread::sleep(Duration::from_millis(selected.sleep_between_out_ms));
        check_deadline(started)?;
        let sent = device.send_feature(&outgoing);
        check_deadline(started)?;
        match sent {
            Ok(count) if count == selected.report_bytes => {}
            Ok(count) => {
                last_error = format!(
                    "原生 HID 发送查询返回 {count}，期望 {}",
                    selected.report_bytes
                );
                continue;
            }
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        }
        std::thread::sleep(Duration::from_millis(selected.sleep_between_out_in_ms));
        for attempt in 0..selected.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(selected.sleep_between_in_ms));
            }
            check_deadline(started)?;
            let mut report = vec![0u8; selected.report_bytes];
            report[0] = selected.report_id;
            let received = device.get_feature(&mut report);
            check_deadline(started)?;
            let received = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = error.to_string();
                    break;
                }
            };
            ensure!(
                received == report.len(),
                "原生 HID 返回 {received} 字节，期望 {}",
                report.len()
            );
            match protocol::decode_report(&report, selected, transaction)? {
                Reply::Busy => {
                    last_error = "设备忙，查询未完成".into();
                }
                Reply::Retry(reason) => {
                    last_error = reason.into();
                    break;
                }
                Reply::Complete(devices) => {
                    revalidate()?;
                    let rows: Vec<_> = devices.iter().map(|(product_id, status)| {
                        let (connected, state) = match status { 0 => (Some(false), "disconnected"), 1 => (Some(true), "connected"), _ => (None, "unknown") };
                        json!({"product_id":product_id,"status":status,"connected":connected,"connection_state":state})
                    }).collect();
                    let mut result = json!({
                        "query":"receiver_wireless_status_v2", "path":path, "device_container_id":device_container_id,
                        "device_instance_id":before["device_instance_id"], "vendor_id":selected.vendor_id, "product_id":selected.product_id,
                        "claim_interface":selected.claim_interface, "report_bytes":selected.report_bytes, "transaction_id":transaction,
                        "source_product_id":selected.source_product_id, "source_class":selected.source_class, "capability_evidence":selected.evidence_path,
                        "application_query_budget_ms":APPLICATION_QUERY_BUDGET.as_millis() as u64,
                        "device_count":rows.len(), "devices":rows,
                        "observed_at_unix_ms":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
                        "elapsed_ms":started.elapsed().as_millis() as u64,
                    });
                    result
                        .as_object_mut()
                        .unwrap()
                        .extend(device.metadata().as_object().unwrap().clone());
                    return Ok(result);
                }
            }
        }
    }
    bail!("接收器查询重试已耗尽：{last_error}")
}
