//! Worker-only receiver observation: current interface -> source capability ->
//! shared read protocol -> portable Feature transport. No device write-back.
use super::hid_transport as transport;

use anyhow::{Context as _, bail, ensure};
use razer_device::receiver_capabilities::ReceiverCapability;
use razer_device::receiver_capabilities::capability;
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
// worker separately at 15s, including identity enumeration and transport I/O.
const APPLICATION_QUERY_BUDGET: Duration = Duration::from_secs(10);

pub(super) fn next_transaction(
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

fn wait_for_query_lease(
    validate: &dyn Fn() -> anyhow::Result<()>,
    mut wait: impl FnMut() -> anyhow::Result<bool>,
) -> anyhow::Result<()> {
    loop {
        validate()?;
        if wait()? {
            return Ok(());
        }
    }
}

impl ReceiverLock {
    pub(super) fn lighting_owner(
        container: &str,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        let name: Vec<u16> = format!(
            "Local\\RazerUi-LightingOwner-{}",
            container.to_ascii_lowercase()
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        ensure!(
            !handle.is_null(),
            "Cannot create the physical lighting owner lease: {}",
            std::io::Error::last_os_error()
        );
        let result = (|| {
            loop {
                validate()?;
                match unsafe { WaitForSingleObject(handle, 40) } {
                    WAIT_OBJECT_0 | WAIT_ABANDONED => return Ok(()),
                    windows_sys::Win32::Foundation::WAIT_TIMEOUT => {}
                    status => bail!("Physical lighting owner lease failed: {status}"),
                }
            }
        })();
        if let Err(error) = result {
            unsafe {
                CloseHandle(handle);
            };
            return Err(error);
        }
        Ok(Self(handle))
    }
    pub(super) fn acquire(
        container: &str,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
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
        let started = Instant::now();
        let result = wait_for_query_lease(
            &|| {
                check_deadline(started)?;
                validate()
            },
            || match unsafe { WaitForSingleObject(handle, 40) } {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(true),
                windows_sys::Win32::Foundation::WAIT_TIMEOUT => Ok(false),
                status => bail!("Physical device query lease failed: {status}"),
            },
        );
        if let Err(error) = result {
            unsafe {
                CloseHandle(handle);
            }
            return Err(error);
        }
        let owned = Self(handle);
        validate()?;
        Ok(owned)
    }
}

#[cfg(test)]
mod lease_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn query_lease_contention_uses_owner_budget_instead_of_500ms_failure() {
        let elapsed_ms = Cell::new(0);
        let checks = Cell::new(0);
        wait_for_query_lease(
            &|| {
                checks.set(checks.get() + 1);
                ensure!(elapsed_ms.get() < 2_000, "owner deadline elapsed");
                Ok(())
            },
            || {
                elapsed_ms.set(elapsed_ms.get() + 40);
                Ok(elapsed_ms.get() >= 800)
            },
        )
        .unwrap();
        assert_eq!(elapsed_ms.get(), 800);
        assert_eq!(checks.get(), 20);
    }

    #[test]
    fn query_lease_cancellation_prevents_the_next_wait() {
        let waits = Cell::new(0);
        let result = wait_for_query_lease(
            &|| {
                ensure!(waits.get() < 2, "owner canceled");
                Ok(())
            },
            || {
                waits.set(waits.get() + 1);
                Ok(false)
            },
        );
        assert_eq!(result.unwrap_err().to_string(), "owner canceled");
        assert_eq!(waits.get(), 2);
    }

    #[test]
    fn query_lease_wait_failure_is_preserved_without_retry() {
        let waits = Cell::new(0);
        let result = wait_for_query_lease(&|| Ok(()), || {
            waits.set(waits.get() + 1);
            bail!("simulated OS wait failure")
        });
        assert_eq!(result.unwrap_err().to_string(), "simulated OS wait failure");
        assert_eq!(waits.get(), 1);
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

pub(super) fn select_target(
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
    // Source `connectHidDevice` matches VID/PID/container/interface only; the
    // feature length is an observation of the interface we are about to use, not
    // a selection rule. Keep it present and truthful instead of requiring a value.
    ensure!(
        target["vendor_id"] == selected.vendor_id
            && target["product_id"] == selected.product_id
            && target["device_container_id"]
                .as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(container))
            && target["claim_interface"] == selected.claim_interface
            && target["feature_report_bytes"].is_u64(),
        "请求路径不是源匹配接口：VID {:04X}/PID {:04X}、接口 {}（Feature 长度按实际枚举记录）",
        selected.vendor_id,
        selected.product_id,
        selected.claim_interface,
    );
    ensure!(
        target["device_instance_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "接收器缺少真实设备实例 ID"
    );
    Ok((target.clone(), selected))
}

pub(super) fn check_deadline(started: Instant) -> anyhow::Result<()> {
    ensure!(
        started.elapsed() < APPLICATION_QUERY_BUDGET,
        "接收器查询已过期，未将结果作为设备观察"
    );
    Ok(())
}

pub(super) fn dock_mode(path: &str, device_container_id: &str) -> anyhow::Result<Value> {
    let started = Instant::now();
    let (before, selected) = select_target(
        &super::hid::enumerate_metadata()?,
        path,
        device_container_id,
    )?;
    razer_device::receiver_dock_mode::capability(selected.product_id)?;
    let guard = || -> anyhow::Result<()> {
        check_deadline(started)?;
        let (current, _) = select_target(
            &super::hid::enumerate_collection(selected.vendor_id, selected.product_id, path)?,
            path,
            device_container_id,
        )?;
        ensure!(
            current["device_instance_id"] == before["device_instance_id"],
            "Dock mode device instance changed"
        );
        Ok(())
    };
    let _receiver_lock = ReceiverLock::acquire(device_container_id, &guard)?;
    let bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("Dock mode target lacks its Feature Report length")?;
    let device = transport::open(path, bytes)?;
    guard()?;
    let transaction = next_transaction(path, device_container_id, selected)?;
    let data =
        razer_device::receiver_dock_mode::read(&device, selected.product_id, transaction, &guard)?;
    guard()?;
    Ok(
        json!({"path":path,"device_container_id":device_container_id,
        "product_id":selected.product_id,"device_instance_id":before["device_instance_id"],
        "is_dongle":true,"method":"getDockExtendedFirmwareVersion","data":data,
        "transaction_id":transaction,"transport_metadata":device.metadata()}),
    )
}

pub(super) fn query(
    path: &str,
    device_container_id: &str,
    source_product_id: Option<u16>,
) -> anyhow::Result<Value> {
    let started = Instant::now();
    let (before, selected) = select_target(
        &super::hid::enumerate_metadata()?,
        path,
        device_container_id,
    )?;
    let selected =
        razer_device::receiver_capabilities::for_source(selected.product_id, source_product_id)?;
    let revalidate = || -> anyhow::Result<Value> {
        check_deadline(started)?;
        let (current, current_capability) = select_target(
            &super::hid::enumerate_collection(selected.vendor_id, selected.product_id, path)?,
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
    let _receiver_lock =
        ReceiverLock::acquire(device_container_id, &|| revalidate().map(|_| ()))?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("缺少接收器实际 Feature Report 长度")?;
    let device = transport::open(path, feature_bytes)?;
    revalidate()?;
    let transaction = next_transaction(path, device_container_id, &selected)?;
    let devices =
        razer_device::device_query::read_receiver(&device, &selected, transaction, || {
            revalidate().map(|_| ())
        })?;
    revalidate()?;
    let rows: Vec<_> = devices.iter().map(|(product_id, status)| {
        let (connected, state) = match status { 0 => (Some(false), "disconnected"), 1 => (Some(true), "connected"), _ => (None, "unknown") };
        json!({"product_id":product_id,"status":status,"connected":connected,"connection_state":state})
    }).collect();
    let mut result = json!({
        "query":"receiver_wireless_status_v2", "path":path, "device_container_id":device_container_id,
        "device_instance_id":before["device_instance_id"], "vendor_id":selected.vendor_id, "product_id":selected.product_id,
        "claim_interface":selected.claim_interface, "report_bytes":selected.report_bytes, "transaction_id":transaction,
        "source_product_id":selected.source_product_id, "source_class":selected.source_class,
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
