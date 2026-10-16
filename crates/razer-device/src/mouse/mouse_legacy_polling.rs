//! PID 70's current rzDevice25Legacy polling and active-DPI chains.
//!
//! Evidence: mouse-mw-70-current main.4d2dda8011e526ce4d9b.js (DeviceInfo,
//! defaults and rzDeviceType), 6120.7fff31f083f844383e26.js (84816 helpers,
//! 7755 packets and task ot), and rzDevice25Legacy.0cda87c8a563a0430722.js.
//! These are retained under .analysis/inputs/frontend, not runtime inputs.
//! The source UI exposes 125/500/1000; the native parser also recognizes the
//! remaining ordinary codes. This does not enable DualLink/profile commands.
use crate::{
    backend::FeatureTransport,
    device_reads::{DeviceReadCapability, DeviceReadKind, ReadCommand},
    device_writes::{
        DeviceWriteCapability, DeviceWriteKind, DeviceWriteResult, DeviceWriteSetting, PollingCode,
        WriteCommand, WriteMethod, WriteSourceClass,
    },
};
use anyhow::{Context as _, Result, ensure};
use std::{collections::BTreeMap, sync::OnceLock};

pub const PRODUCT_ID: u32 = 70;
pub const UI_POLLING_RATES: [u32; 3] = [125, 500, 1000];
const GET_POLLING: [u8; 3] = [1, 0, 0x85];
const SET_POLLING: [u8; 3] = [1, 0, 0x05];
const GET_MODE: [u8; 3] = [2, 0, 0x84];
const SET_MODE: [u8; 3] = [2, 0, 0x04];
const GET_DPI: [u8; 3] = [7, 4, 0x85];
const SET_DPI: [u8; 3] = [7, 4, 0x05];

/// Current setupDevice -> Y/iS checks [mode,param] before selecting [3,0].
/// PID70 is a Chroma device: its source cleanup does not restore Normal mode.
pub fn initialize_driver_mode(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    next_transaction: &mut impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<()> {
    let mode = crate::feature_exchange::exchange(
        device,
        cap,
        GET_MODE,
        &[],
        next_transaction()?,
        validate,
    )?;
    if mode.get(..2) != Some(&[3, 0]) {
        crate::feature_exchange::exchange(
            device,
            cap,
            SET_MODE,
            &[3, 0],
            next_transaction()?,
            validate,
        )?;
    }
    Ok(())
}

fn codes() -> BTreeMap<String, u8> {
    [
        (1000, 1),
        (500, 2),
        (333, 3),
        (250, 4),
        (200, 5),
        (125, 8),
        (100, 10),
    ]
    .into_iter()
    .map(|(hz, code)| (format!("RATE_{hz}Hz"), code))
    .collect()
}

pub fn read_capability() -> &'static DeviceReadCapability {
    static CAP: OnceLock<DeviceReadCapability> = OnceLock::new();
    CAP.get_or_init(|| DeviceReadCapability {
        product_id: PRODUCT_ID,
        source_class: "rzDevice25Legacy".into(),
        direct_pids: vec![PRODUCT_ID],
        vendor_id: 0x1532,
        claim_interface: 0,
        claim_interfaces_by_pid: BTreeMap::new(),
        physical_transports_by_pid: Default::default(),
        report_id: 0,
        report_bytes: 91,
        transaction_prefix: 0,
        transaction_modulus: 31,
        queries: vec![
            ReadCommand {
                name: DeviceReadKind::Polling,
                method: "getPollingRate".into(),
                command: GET_POLLING,
                payload: vec![],
                min_response_bytes: 1,
            },
            ReadCommand {
                name: DeviceReadKind::Dpi,
                method: "getDpiLevel".into(),
                command: GET_DPI,
                payload: vec![1, 0, 0, 0, 0, 0, 0],
                min_response_bytes: 5,
            },
        ],
        polling_codes: codes(),
        polling_value_offset: Some(0),
        polling_default_hz: Some(100),
        polling_physical_product_id: Some(PRODUCT_ID),
        charging_codes: BTreeMap::new(),
        max_retry_in: 10,
        max_retry_out: 20,
        sleep_between_out_ms: 5,
        sleep_between_out_in_ms: 5,
        sleep_between_in_ms: 5,
    })
}

pub fn write_capability() -> &'static DeviceWriteCapability {
    static CAP: DeviceWriteCapability = DeviceWriteCapability {
        product_id: PRODUCT_ID,
        source_class: WriteSourceClass::Legacy,
        min_dpi: 100,
        max_dpi: 16000,
        polling_codes: &[
            PollingCode { hz: 1000, code: 1 },
            PollingCode { hz: 500, code: 2 },
            PollingCode { hz: 333, code: 3 },
            PollingCode { hz: 250, code: 4 },
            PollingCode { hz: 200, code: 5 },
            PollingCode { hz: 125, code: 8 },
            PollingCode { hz: 100, code: 10 },
        ],
        writes: &[
            WriteCommand {
                kind: DeviceWriteKind::Polling,
                method: WriteMethod::PollingRate,
                command: SET_POLLING,
                selector: 0,
                read_kind: DeviceReadKind::Polling,
                min_response_bytes: 0,
            },
            WriteCommand {
                kind: DeviceWriteKind::Dpi,
                method: WriteMethod::DpiLevel,
                command: SET_DPI,
                selector: 1,
                read_kind: DeviceReadKind::Dpi,
                min_response_bytes: 0,
            },
        ],
    };
    &CAP
}

/// Source ot reads first, skips equal values, writes, then confirms by GET.
/// The SET acknowledgment is not required to echo the requested data byte;
/// the original helper merely decodes it and the task uses the later GET.
/// The caller owns the connection lock and locally persisted profile. Neither
/// a failure nor a returned observation rewrites that optimistic UI profile.
pub fn apply_polling(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    hz: u32,
    mut next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> Result<DeviceWriteResult> {
    ensure!(
        cap.product_id == PRODUCT_ID && cap.source_class == "rzDevice25Legacy",
        "Legacy polling requires the current PID 70 transport"
    );
    let setting = DeviceWriteSetting::Polling { hz };
    let setter = crate::device_writes::prepare(write_capability(), &setting)?;
    let getter = &read_capability().queries[0];
    validate()?;
    // setupDevice awaits Y but does not treat its caught communication error
    // as an initialization success or a fatal polling error.
    if let Err(error) = initialize_driver_mode(device, cap, &mut next_transaction, &validate) {
        eprintln!("Legacy mouse driver-mode initialization failed: {error:#}");
    }
    validate()?;
    let previous =
        crate::device_query::read_device(device, cap, getter, next_transaction()?, &validate)
            .map_err(|error| {
                eprintln!("Legacy polling initial getter failed: {error:#}");
                error
            })
            .ok();
    validate()?;
    // Source be catches getter failures and returns a comparison sentinel 0.
    // Keep the missing observation explicit while preserving the subsequent SET.
    if previous
        .as_ref()
        .is_some_and(|value| setting.matches_value(value))
    {
        validate()?;
        return Ok(DeviceWriteResult {
            requested: setting,
            observed: previous.clone(),
            acknowledged: None,
            previous,
            changed: false,
            verified: true,
        });
    }
    validate()?;
    crate::feature_exchange::exchange(
        device,
        cap,
        setter.command,
        &setter.payload,
        next_transaction()?,
        &validate,
    )
    .context("Legacy polling write was not acknowledged; device state may have changed")?;
    validate()?;
    let observed =
        crate::device_query::read_device(device, cap, getter, next_transaction()?, &validate)
            .context("Legacy polling write acknowledged, but readback failed")?;
    validate()?;
    ensure!(
        setting.matches_value(&observed),
        "Legacy polling readback differs from the requested rate"
    );
    Ok(DeviceWriteResult {
        requested: setting,
        previous,
        observed: Some(observed),
        acknowledged: None,
        changed: true,
        verified: true,
    })
}

/// Current middleware 6120 Xe (103145) selects setDpiLevel(1, x, y).
/// Unlike its stage-table branch, it completes on the setter ACK and does not
/// read back, skip equal values, or update the UI from an observation. The
/// complete sensitivity profile is persisted by the caller before this task.
pub fn apply_dpi(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    x: u16,
    y: u16,
    mut next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> Result<DeviceWriteResult> {
    ensure!(
        cap.product_id == PRODUCT_ID && cap.source_class == "rzDevice25Legacy",
        "Legacy DPI requires the current PID 70 transport"
    );
    let requested = DeviceWriteSetting::Dpi { x, y };
    let setter = crate::device_writes::prepare(write_capability(), &requested)?;
    validate()?;
    if let Err(error) = initialize_driver_mode(device, cap, &mut next_transaction, &validate) {
        eprintln!("Legacy mouse driver-mode initialization failed: {error:#}");
    }
    validate()?;
    let data = crate::feature_exchange::exchange(
        device,
        cap,
        setter.command,
        &setter.payload,
        next_transaction()?,
        &validate,
    )
    .context("Legacy DPI setter was not acknowledged; device state may have changed")?;
    validate()?;
    Ok(DeviceWriteResult {
        requested,
        previous: None,
        observed: None,
        acknowledged: Some(crate::device_writes::DeviceWriteAck {
            command: setter.command,
            data,
        }),
        changed: true,
        verified: false,
    })
}

#[cfg(test)]
#[path = "mouse_legacy_polling_tests.rs"]
mod tests;
