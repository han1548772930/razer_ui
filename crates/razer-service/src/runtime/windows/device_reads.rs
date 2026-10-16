//! Source-described device queries through the portable Feature transport.
//! Every request rechecks physical identity and, for a relay, the actual peer.
use super::{hid, hid_transport, receiver};
use anyhow::{Context as _, ensure};
use razer_device::device_identity;
use razer_device::device_identity::IdentityLookup;
use razer_device::device_reads;
use razer_device::device_reads::DeviceReadCapability;
use razer_device::device_reads::DeviceReadKind;
use razer_device::device_reads::DeviceReadTarget;
use serde_json::{Value, json};
#[path = "keyboard/keyboard_wristrest.rs"]
mod keyboard_wristrest_adapter;
#[path = "keyboard/keyboard_indicator.rs"]
mod keyboard_indicator_adapter;
#[path = "keyboard/keyboard_latest_restore.rs"]
mod keyboard_latest_restore_adapter;
pub(super) fn keyboard_latest_restore(
    target: &razer_device::keyboard_latest_protocol::Target,
    operation: &razer_device::keyboard_latest_protocol::Operation,
) -> anyhow::Result<Value> {
    keyboard_latest_restore_adapter::request(target, operation)
}
pub(super) fn keyboard_latest_event_source(target:&razer_device::keyboard_latest_protocol::Target)->anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>>{keyboard_latest_restore_adapter::event_source(target)}
#[path = "keyboard/keyboard_legacy_calibration.rs"]
mod keyboard_legacy_calibration_adapter;
pub(super) fn keyboard_legacy_calibration(
    target: &DeviceReadTarget,
    operation: &razer_device::keyboard_legacy_calibration::Operation,
) -> anyhow::Result<Value> {
    keyboard_legacy_calibration_adapter::request(target, operation)
}
pub(super) fn keyboard_indicator(
    target: &DeviceReadTarget,
    change: &razer_device::keyboard_indicator::Change,
) -> anyhow::Result<Value> {
    keyboard_indicator_adapter::request(target, change)
}
#[path = "mouse/mouse_current_dpi.rs"]
mod mouse_current_dpi_adapter;
#[path = "mouse/mouse_current_power.rs"]
mod mouse_current_power_adapter;
#[path = "mouse/mouse_scroll.rs"]
mod mouse_scroll_adapter;
#[path = "mouse/mouse_indicator_led.rs"]
mod mouse_indicator_led_adapter;
#[path = "mouse/mouse_receiver_legacy.rs"]
mod mouse_receiver_legacy_adapter;
pub(super) fn mouse_receiver_legacy(target: &DeviceReadTarget,
    change: &razer_device::mouse_receiver_legacy::Change) -> anyhow::Result<Value> {
    mouse_receiver_legacy_adapter::request(target, change)
}
pub(super) fn mouse_indicator_led(target: &DeviceReadTarget,
    change: &razer_device::mouse_indicator_led::Change) -> anyhow::Result<Value> {
    mouse_indicator_led_adapter::request(target, change)
}
pub(super) fn mouse_indicator_led_monitor(target: &razer_device::mouse_indicator_led::Target,
    canceled: &std::sync::atomic::AtomicBool,
    publish: &mut impl FnMut(razer_device::mouse_indicator_led::Notification) -> anyhow::Result<()>) -> anyhow::Result<()> {
    mouse_indicator_led_adapter::monitor(target, canceled, publish)
}
#[path = "audio/seiren_eq.rs"]
mod seiren_eq_adapter;
#[path = "audio/leviathan.rs"]
mod leviathan_adapter;
pub(super) fn leviathan_event_source(target:&razer_device::leviathan::EventTarget)->anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>> {
    leviathan_adapter::event_source(target)
}
pub(super) fn leviathan(target:&DeviceReadTarget,operation:razer_device::leviathan::Operation)->anyhow::Result<Value> {
    leviathan_adapter::request(target,operation)
}
pub(super) use seiren_eq_adapter::State as SeirenAudio;
pub(super) fn seiren_eq(
    target: &DeviceReadTarget,
    change: &razer_device::seiren_eq::Change,
) -> anyhow::Result<Value> {
    seiren_eq_adapter::request(target, change)
}
#[path = "mouse/mouse_current_power_ble.rs"]
mod mouse_current_power_ble_adapter;
#[path = "mouse/mouse_runtime_artwork.rs"]
mod mouse_runtime_artwork_adapter;
pub(super) fn mouse_current_power(
    target: &DeviceReadTarget,
    change: &razer_device::mouse_current_power::Change,
) -> anyhow::Result<Value> {
    mouse_current_power_adapter::request(target, change)
}
pub(super) fn mouse_scroll(target: &DeviceReadTarget, submission: &razer_device::mouse_scroll::Submission) -> anyhow::Result<Value> {
    if target.peer_product_id.is_some() {
        return receiver_setting(target, super::super::receiver_settings::Setting::Scroll(submission));
    }
    mouse_scroll_adapter::request(target, submission)
}
pub(super) fn mouse_current_power_ble(
    target: &razer_device::mouse_runtime_artwork::WindowsBleTarget,
    change: &razer_device::mouse_current_power::Change,
) -> anyhow::Result<Value> {
    mouse_current_power_ble_adapter::request(target, change)
}

pub(super) fn keyboard_wristrest(
    target: &razer_device::keyboard_wristrest::Target,
    canceled: &std::sync::atomic::AtomicBool,
    publish: &mut impl FnMut(razer_device::keyboard_wristrest::Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    keyboard_wristrest_adapter::run(target, canceled, publish)
}
pub(super) fn mouse_runtime_artwork(
    target: &razer_device::mouse_runtime_artwork::Target,
    canceled: &std::sync::atomic::AtomicBool,
    publish: &mut impl FnMut(razer_device::mouse_runtime_artwork::Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    mouse_runtime_artwork_adapter::run(target, canceled, publish)
}

pub(super) fn mouse_current_dpi(
    target: &DeviceReadTarget,
    change: Option<&razer_device::mouse_current_dpi::DpiChange>,
) -> anyhow::Result<Value> {
    if target.peer_product_id.is_some() {
        return receiver_setting(target, super::super::receiver_settings::Setting::CurrentDpi(change));
    }
    mouse_current_dpi_adapter::request(target, change)
}
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
        razer_device::receiver_capabilities::capability(pid)
            .context("物理接收器能力未核实")?
            .claim_interface
    } else {
        cap.claim_interface_for(target.physical_product_id)?
    };
    ensure!(
        observed["vendor_id"] == cap.vendor_id
            && observed["product_id"] == pid
            && observed["claim_interface"] == claim_interface
            && observed["feature_report_bytes"].is_u64()
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
            let receiver_cap = razer_device::receiver_capabilities::capability(u16::try_from(
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
            let result = receiver::query(&target.path, &target.device_container_id, None)?;
            let peers = razer_device::receiver_protocol::receiver_peers(&result)?;
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
    if target.product_id == razer_device::mouse_receiver_polling::PRODUCT_ID
        && target.peer_product_id.is_some()
    {
        return query_mouse_receiver(target, kind);
    }
    let source_cap = device_reads::capability(target.product_id)
        .context("此产品没有当前源核实的基础读取能力")?;
    let bound_cap = if target.peer_product_id.is_none() {
        source_cap.for_physical_product(target.physical_product_id)?
    } else {
        let master = razer_device::receiver_capabilities::capability(u16::try_from(target.physical_product_id)?)
            .context("物理接收器能力未核实")?;
        source_cap.for_duallink_master(target.physical_product_id, master.claim_interface)?
    };
    let cap = &bound_cap;
    let command = cap
        .queries
        .iter()
        .find(|command| command.name == kind)
        .context("此产品不支持该项源查询")?;
    if kind == DeviceReadKind::Polling {
        ensure!(
            target.peer_product_id.is_none()
                && cap.polling_physical_product_id == Some(target.physical_product_id),
            "此轮询率分支只核实了直接设备；接收器连接状态分支尚未核实"
        );
    }
    let before = current_target(target, cap)?;
    verify_routing(target, source_cap)?;
    let started = Instant::now();
    let deadline = || -> anyhow::Result<()> {
        ensure!(
            started.elapsed() < Duration::from_secs(10),
            "设备查询已超过观察期限"
        );
        Ok(())
    };
    let (reading, transport_metadata) = {
        let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        let feature_bytes = before["feature_report_bytes"]
            .as_u64()
            .and_then(|count| usize::try_from(count).ok())
            .context("缺少实际 Feature Report 长度")?;
        let device = hid_transport::open(&target.path, feature_bytes)?;
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "打开设备期间实例发生变化"
        );
        let transaction = transaction(target, cap)?;
        let reading =
            razer_device::device_query::read_device(&device, cap, command, transaction, deadline)?;
        (reading, device.metadata())
    };
    // Unlock first: the same physical receiver lock also protects peer queries.
    ensure!(
        current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
        "查询期间设备实例发生变化"
    );
    verify_routing(target, source_cap)?;
    Ok(
        json!({"target":target,"reading":reading,"method":command.method,"source_class":cap.source_class,
        "transport_metadata":transport_metadata, "elapsed_ms":started.elapsed().as_millis() as u64,"evidence":"docs/re/mouse-read-capabilities-current-evidence.json"}),
    )
}

fn query_mouse_receiver(target: &DeviceReadTarget, kind: DeviceReadKind) -> anyhow::Result<Value> {
    with_mouse_receiver(target, kind, |route, device, next, validate| {
        Ok(serde_json::to_value(route.read(device, next, validate)?)?)
    })
}

fn with_mouse_receiver(
    target: &DeviceReadTarget,
    kind: DeviceReadKind,
    operation: impl FnOnce(
        &razer_device::mouse_receiver_polling::ReceiverRead,
        &dyn razer_device::backend::FeatureTransport,
        &mut dyn FnMut() -> anyhow::Result<u8>,
        &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Value>,
) -> anyhow::Result<Value> {
    let route = razer_device::mouse_receiver_polling::ReceiverRead::new(
        target.product_id,
        target.physical_product_id,
        target.peer_product_id,
        kind,
    )?;
    let cap = route.transport();
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "接收器鼠标查询超过观察期限"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "接收器鼠标查询期间设备实例发生变化"
        );
        Ok(())
    };
    let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    validate()?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("接收器缺少实际 Feature Report 长度")?;
    ensure!(
        feature_bytes == cap.report_bytes,
        "接收器鼠标 Feature Report 长度不符合当前源码"
    );
    let device = hid_transport::open(&target.path, feature_bytes)?;
    // Status and mouse GET share this retained physical handle and lock. Do not
    // query via receiver::query here: that would reacquire the same OS lock and
    // select the master's own transaction namespace instead of the mouse's.
    let mut response = operation(&route, &device, &mut || transaction(target, cap), &validate)?;
    response["target"] = serde_json::to_value(target)?;
    if response.get("method").is_none() {
        response["method"] = json!(route.method());
    }
    response["source_class"] = json!(cap.source_class);
    response["transport_metadata"] = device.metadata();
    response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
    response["evidence"] = json!(".analysis/results/frontend/mouse-mw-audit.json");
    Ok(response)
}

pub(super) fn write(
    target: &DeviceReadTarget,
    setting: &razer_device::device_writes::DeviceWriteSetting,
) -> anyhow::Result<Value> {
    use razer_device::device_writes;
    if target.product_id == razer_device::mouse_receiver_polling::PRODUCT_ID
        && target.peer_product_id.is_some()
        && let device_writes::DeviceWriteSetting::Idle { raw_time } = setting
    {
        return with_mouse_receiver(target, DeviceReadKind::Idle, |route, device, next, validate| {
            let observed = route.apply_idle(device, *raw_time, next, validate)?;
            Ok(json!({"result":observed.reading,"receiver_before":observed.receiver_before,
                "receiver_after":observed.receiver_after,"method":"setTimeToSleep"}))
        });
    }
    if target.product_id == razer_device::mouse_receiver_polling::PRODUCT_ID
        && target.peer_product_id.is_some()
        && let device_writes::DeviceWriteSetting::Polling { hz } = setting
    {
        return with_mouse_receiver(
            target,
            DeviceReadKind::Polling,
            |route, device, next, validate| {
                let observed = route.apply_polling(device, *hz, next, validate)?;
                Ok(
                    json!({"result":observed.reading,"receiver_before":observed.receiver_before,
                "receiver_after":observed.receiver_after,"profile_scope":"current_active_profile","obm_profiles_written":false}),
                )
            },
        );
    }
    ensure!(
        target.peer_product_id.is_none(),
        "接收器转发写入尚未按产品原码核实；不会把直接设备命令发给接收器"
    );
    let cap =
        device_reads::capability(target.product_id).context("产品没有源核实的基础读取能力")?;
    let bound_cap = cap.for_physical_product(target.physical_product_id)?;
    let cap = &bound_cap;
    let write_cap =
        device_writes::capability(target.product_id).context("产品没有源核实的设备写入能力")?;
    device_writes::prepare(write_cap, setting)?;
    if setting.read_kind() == DeviceReadKind::Polling {
        ensure!(
            cap.polling_physical_product_id == Some(target.physical_product_id),
            "轮询率写入的物理连接分支尚未核实"
        );
    }
    let before = current_target(target, cap)?;
    verify_routing(target, cap)?;
    let started = Instant::now();
    let validate = || -> anyhow::Result<()> {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "设备写入已超过确认期限"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "写入期间设备实例发生变化"
        );
        Ok(())
    };
    let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("写入目标缺少实际 Feature Report 长度")?;
    let device = hid_transport::open(&target.path, feature_bytes)?;
    let result =
        device_writes::apply(&device, cap, setting, || transaction(target, cap), validate)?;
    Ok(
        json!({"target":target,"result":result,"source_class":cap.source_class,
        "transport_metadata":device.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64,
        "evidence":"docs/re/device-write-capabilities-current-evidence.json"}),
    )
}

#[path = "keyboard/keyboard_metadata_ble.rs"]
mod keyboard_metadata_ble_adapter;

pub(super) fn keyboard_metadata_ble(
    target: &razer_device::mouse_runtime_artwork::WindowsBleTarget,
    serial: bool,
) -> anyhow::Result<Value> {
    keyboard_metadata_ble_adapter::request(target, serial)
}

pub(super) fn keyboard_metadata(target: &DeviceReadTarget, serial: bool) -> anyhow::Result<Value> {
    use razer_device::keyboard_metadata;
    let selected =
        match keyboard_metadata::hid_capability(target.product_id, target.physical_product_id) {
            Some(cap) => std::borrow::Cow::Borrowed(cap),
            None if target.peer_product_id.is_some() => std::borrow::Cow::Owned(
                keyboard_metadata::relay_capability(target.product_id, target.physical_product_id)?,
            ),
            None => anyhow::bail!("Keyboard metadata transport has no current source evidence"),
        };
    let relay = matches!(selected, std::borrow::Cow::Owned(_));
    let cap = selected.as_ref();
    if !relay {
        ensure!(
            target
                .peer_product_id
                .is_none_or(|peer| peer == target.product_id || peer == target.physical_product_id),
            "Keyboard metadata target is not its audited physical radio owner"
        );
        ensure!(
            target.peer_product_id.is_none()
                || keyboard_metadata::radio_capability(target.product_id)
                    .is_some_and(|radio| radio.direct_pids.contains(&target.physical_product_id)),
            "Keyboard metadata peer requires its own source radio route"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
        else {
            anyhow::bail!("Keyboard metadata physical identity is ambiguous");
        };
        ensure!(
            identity.product_id == target.product_id && !identity.is_ble,
            "Keyboard metadata physical identity differs from source"
        );
    }
    let physical_target = DeviceReadTarget {
        peer_product_id: None,
        ..target.clone()
    };
    let before = current_target(&physical_target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Keyboard metadata read deadline expired"
        );
        ensure!(
            current_target(&physical_target, cap)?["device_instance_id"]
                == before["device_instance_id"],
            "Keyboard instance changed during metadata read"
        );
        Ok(())
    };
    let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("Keyboard collection has no observed Feature report length")?;
    ensure!(
        feature_bytes == cap.report_bytes,
        "Keyboard metadata Feature descriptor differs from source"
    );
    let device = hid_transport::open(&target.path, feature_bytes)?;
    if relay {
        let receiver_cap = razer_device::receiver_capabilities::capability(u16::try_from(
            target.physical_product_id,
        )?)
        .context("Keyboard relay has no current receiver query capability")?;
        let receiver_transaction =
            receiver::next_transaction(&target.path, &target.device_container_id, receiver_cap)?;
        let peers = razer_device::device_query::read_receiver(
            &device,
            receiver_cap,
            receiver_transaction,
            &validate,
        )?;
        keyboard_metadata::validate_peer(
            target.product_id,
            target
                .peer_product_id
                .context("Keyboard relay peer is absent")?,
            &peers,
        )?;
    }
    let mut response = json!({"target": target, "source_class": cap.source_class, "transport_metadata": device.metadata()});
    if serial {
        response["method"] = json!("getSerialNumber");
        response["serial_number"] = json!(keyboard_metadata::read_serial(
            &device,
            cap,
            || transaction(target, cap),
            &validate
        )?);
    } else {
        response["method"] = json!("getEdition");
        response["reading"] = serde_json::to_value(keyboard_metadata::read(
            &device,
            cap,
            || transaction(target, cap),
            &validate,
        )?)?;
    }
    validate()?;
    response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
    Ok(response)
}

fn receiver_setting(target: &DeviceReadTarget, setting: super::super::receiver_settings::Setting<'_>) -> anyhow::Result<Value> {
    let peer = target.peer_product_id.context("Receiver setting requires a retained peer")?;
    let session = setting.session(target.product_id, target.physical_product_id, peer)?;
    let cap = session.transport();
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(started.elapsed() < Duration::from_secs(20), "Receiver setting deadline expired");
        ensure!(current_target(target, cap)?["device_instance_id"] == before["device_instance_id"], "Receiver device instance changed during setting");
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"].as_u64().and_then(|bytes| usize::try_from(bytes).ok()).context("Receiver Feature size is absent")?;
    ensure!(feature_bytes == cap.report_bytes, "Receiver Feature size differs from source");
    let device = hid_transport::open(&target.path, feature_bytes)?;
    let mut response = setting.execute(&session, &device, &mut || transaction(target, cap), &validate)?;
    response["target"] = serde_json::to_value(target)?;
    response["peer_product_id"] = json!(peer);
    response["transport_metadata"] = device.metadata();
    response["identity_scope"] = json!("device_instance_and_live_receiver_peer");
    response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
    Ok(response)
}

pub(super) fn keyboard_brightness(
    target: &DeviceReadTarget,
    percent: Option<u8>,
) -> anyhow::Result<Value> {
    use razer_device::keyboard_settings;
    if target.peer_product_id.is_some() {
        return receiver_setting(target, super::super::receiver_settings::Setting::KeyboardBrightness(percent));
    }
    let settings = keyboard_settings::capability(target.product_id)
        .context("此产品没有当前源核实的键盘亮度能力")?;
    if let Some(percent) = percent {
        keyboard_settings::encode_percent(percent)?;
    }
    let cap = &settings.transport;
    let before = current_target(target, cap)?;
    verify_routing(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "键盘亮度操作超过确认期限"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "键盘亮度操作期间设备实例发生变化"
        );
        Ok(())
    };
    let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("键盘接口缺少实际 Feature Report 长度")?;
    ensure!(feature_bytes == cap.report_bytes, "键盘亮度接口实际 Feature Report 长度不匹配");
    let device = hid_transport::open(&target.path, feature_bytes)?;
    validate()?;
    let mut response = json!({"target":target,"source_class":settings.source_class,
        "transport_metadata":device.metadata(),"evidence":"docs/re/keyboard-brightness-expanded-current-source.json"});
    if let Some(percent) = percent {
        response["result"] = serde_json::to_value(keyboard_settings::apply(
            &device,
            settings,
            percent,
            || transaction(target, cap),
            &validate,
        )?)?;
    } else {
        response["reading"] = serde_json::to_value(keyboard_settings::read(
            &device,
            settings,
            || transaction(target, cap),
            &validate,
        )?)?;
    }
    validate()?;
    response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
    Ok(response)
}

pub(super) fn keyboard_submission(
    target: &DeviceReadTarget,
    profile_guid: &str,
    intent: razer_device::keyboard_submission::Intent,
    canceled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<Value> {
    use razer_device::keyboard_submission;
    ensure!(
        target.peer_product_id.is_none(),
        "Keyboard analog target requires a direct collection"
    );
    let cap = keyboard_submission::validate(target.product_id, &intent)?.device_read_capability();
    let before = current_target(target, &cap)?;
    verify_routing(target, &cap)?;
    let started = Instant::now();
    let guard = || -> anyhow::Result<()> {
        ensure!(
            !canceled.load(std::sync::atomic::Ordering::Acquire),
            "Keyboard task canceled"
        );
        ensure!(
            started.elapsed() < Duration::from_secs(55),
            "Keyboard task deadline expired"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Keyboard instance changed during submission"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Keyboard descriptor has no Feature report size")?
        as usize;
    let device = hid_transport::open(&target.path, feature_bytes)?;
    let receipt = keyboard_submission::submit(
        &device,
        target.product_id,
        profile_guid,
        intent,
        || transaction(target, &cap),
        &guard,
    )?;
    Ok(json!({"target":target,"receipt":receipt,"transport_metadata":device.metadata()}))
}

pub(super) fn keyboard_gaming_indicator(
    target: &DeviceReadTarget,
    enabled: bool,
) -> anyhow::Result<Value> {
    let settings = razer_device::keyboard_settings::capability(target.product_id)
        .context("No source-audited keyboard transport")?;
    let cap = &settings.transport;
    ensure!(
        matches!(target.product_id, 515 | 679) && target.peer_product_id.is_none(),
        "Gaming LED requires an audited direct keyboard route"
    );
    let before = current_target(target, cap)?;
    verify_routing(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Gaming LED operation timed out"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Gaming LED device instance changed"
        );
        Ok(())
    };
    let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Missing gaming LED feature report length")? as usize;
    ensure!(
        feature_bytes == cap.report_bytes,
        "Gaming LED feature report size differs"
    );
    let device = hid_transport::open(&target.path, feature_bytes)?;
    let report = razer_device::keyboard_gaming_mode::apply(
        &device,
        target.product_id,
        enabled,
        false,
        || transaction(target, cap),
        validate,
    )?;
    Ok(json!({"target":target,"result":report,"source_class":settings.source_class}))
}

pub(super) fn dpi_stages(
    target: &DeviceReadTarget,
    draft: Option<&razer_device::mouse_dpi_stages::DpiStagesDraft>,
) -> anyhow::Result<Value> {
    use razer_device::mouse_dpi_stages;
    if target.product_id == razer_device::mouse_receiver_polling::PRODUCT_ID
        && target.peer_product_id.is_some()
    {
        return with_mouse_receiver(
            target,
            DeviceReadKind::Dpi,
            |route, device, next, validate| {
                let mut response = if let Some(draft) = draft {
                    let mut response = serde_json::to_value(
                        route.apply_dpi_stages(device, draft, next, validate)?,
                    )?;
                    response["result"] = response["reading"].take();
                    response
                        .as_object_mut()
                        .expect("serialized receiver observation")
                        .remove("reading");
                    response
                } else {
                    serde_json::to_value(route.read_dpi_stages(device, next, validate)?)?
                };
                response["profile_scope"] = json!("current_active_table");
                response["obm_profiles_written"] = json!(false);
                response["method"] = json!(if draft.is_some() {
                    "setDPIStages"
                } else {
                    "getDPIStages"
                });
                Ok(response)
            },
        );
    }
    if target.peer_product_id.is_some() {
        return receiver_setting(target, super::super::receiver_settings::Setting::DpiStages(draft));
    }
    let settings = mouse_dpi_stages::capability(target.product_id)
        .context("Product has no current source-proven DPI stage capability")?
        .for_physical_product(target.physical_product_id)?;
    let settings = &settings;
    let cap = &settings.transport;
    if let Some(draft) = draft {
        mouse_dpi_stages::pack(settings, draft)?;
    }
    let before = current_target(target, cap)?;
    verify_routing(target, cap)?;
    let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
    else {
        anyhow::bail!("DPI stage target has no unique device identity");
    };
    ensure!(
        !identity.is_dongle && !identity.is_ble,
        "DPI stage target requires a source-proven direct device"
    );
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "DPI stage operation exceeded its confirmation deadline"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Device instance changed during DPI stage operation"
        );
        Ok(())
    };
    let _guard = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .context("DPI stage target lacks an observed Feature Report size")?;
    ensure!(
        feature_bytes == cap.report_bytes,
        "DPI stage observed report size does not match current source"
    );
    let device = hid_transport::open(&target.path, feature_bytes)?;
    validate()?;
    let mut response = json!({"target":target,"source_class":settings.source_class,
        "transport_source_class":cap.source_class,"transport_metadata":device.metadata(),
        "evidence":"docs/re/mouse-dpi-stages-current-evidence.json","profile_scope":"current_active_table","obm_profiles_written":false});
    if let Some(draft) = draft {
        response["result"] = serde_json::to_value(mouse_dpi_stages::apply_current(
            &device,
            settings,
            draft,
            || transaction(target, cap),
            &validate,
        )?)?;
    } else {
        response["reading"] = serde_json::to_value(mouse_dpi_stages::read_current(
            &device,
            settings,
            || transaction(target, cap),
            &validate,
        )?)?;
    }
    validate()?;
    response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
    Ok(response)
}
