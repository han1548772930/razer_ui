//! Explicit hardware checks. Ordinary `cargo test` never opens a device.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{FeatureTransport, HidBackend, HidNode, ReportLengths},
    device_identity::{self, IdentityLookup},
    device_reads::{self, DeviceReadCapability},
    keyboard_analog, keyboard_gaming_mode, keyboard_metadata, keyboard_settings,
    mouse_current_dpi as dpi, mouse_current_polling as polling, mouse_current_power as power,
};
use razer_hid::NativeBackend;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::Write as _,
    time::{Duration, Instant},
};

// Current PID679 BUTTON_LIST -> NormalMode firmware IDs for observed layout 1.
// The complete current-source inventory is retained by the frontend read audit.
const KEYBOARD_679_LAYOUT_1_KEYS: &[u8] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 43, 44, 46, 47, 48, 49, 50, 51, 52, 53,
    54, 55, 57, 58, 60, 61, 62, 64, 75, 76, 79, 80, 81, 83, 84, 85, 86, 89, 110, 112, 113, 114,
    115, 116, 117, 118, 119, 120, 121, 122, 123, 129,
];

struct Traced {
    transport: Box<dyn FeatureTransport>,
    events: RefCell<Vec<Value>>,
}

fn guarded<T>(action: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action))
        .map_err(|_| anyhow::anyhow!("Connected-device operation panicked"))?
}

struct MouseSnapshot {
    serial_number: String,
    dpi: dpi::DpiReport,
    active_payload: Vec<u8>,
    polling: polling::Observation,
    polling_payload: Vec<u8>,
    idle: power::Report,
}

impl MouseSnapshot {
    fn same_configuration(&self, other: &Self) -> bool {
        self.serial_number == other.serial_number
            && self.dpi.stages == other.dpi.stages
            && self.dpi.active == other.dpi.active
            && self.active_payload == other.active_payload
            && self.polling == other.polling
            && self.polling_payload == other.polling_payload
            && self.idle.time_to_sleep == other.idle.time_to_sleep
            && self.idle.acknowledgements.first().map(|ack| &ack.data)
                == other.idle.acknowledgements.first().map(|ack| &ack.data)
    }

    fn value(&self) -> Value {
        json!({"serial_observed":!self.serial_number.is_empty(),"dpi":self.dpi,"active_payload":self.active_payload,
            "polling":self.polling,"polling_payload":self.polling_payload,"idle":self.idle})
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseChange {
    TransientDpi(dpi::DpiPair),
    Polling(u32),
    IdleMinutes(u16),
}

impl MouseChange {
    fn value(self) -> Value {
        match self {
            Self::TransientDpi(pair) => json!({"transient_dpi":pair}),
            Self::Polling(hz) => json!({"polling_hz":hz}),
            Self::IdleMinutes(minutes) => json!({"idle_minutes":minutes}),
        }
    }
}

fn transient_dpi(pair: dpi::DpiPair) -> dpi::DpiChange {
    dpi::DpiChange {
        stages: None,
        active: Some(pair),
        profile_guid: None,
        initialize: false,
    }
}

struct MouseOperation {
    query: &'static str,
    original: MouseChange,
    target: MouseChange,
}

fn mouse_restore_plan(baseline: &MouseSnapshot) -> anyhow::Result<[MouseOperation; 3]> {
    let source = dpi::capability(dpi::PRODUCT_ID).context("Current DPI source is absent")?;
    let active = baseline.dpi.active.context("Active DPI was not observed")?;
    // The production setter writes all seven fields, including Z=0. A pair
    // alone cannot prove that the original complete payload can be restored.
    ensure!(
        baseline.active_payload.len() == 7
            && baseline.active_payload[0] == 0
            && baseline.active_payload[1..3] == active.x.to_be_bytes()
            && baseline.active_payload[3..5] == active.y.to_be_bytes()
            && baseline.active_payload[5..] == [0, 0],
        "Original transient DPI has fields the audited setter cannot restore exactly"
    );
    let target_x = if active.x.saturating_add(50) <= source.maximum() {
        active.x + 50
    } else {
        active
            .x
            .checked_sub(50)
            .context("No reversible DPI target")?
    };
    let target_dpi = dpi::DpiPair {
        x: target_x,
        ..active
    };
    dpi::validate_change_for(dpi::PRODUCT_ID, &transient_dpi(active))?;
    dpi::validate_change_for(dpi::PRODUCT_ID, &transient_dpi(target_dpi))?;
    let hz = baseline
        .polling
        .hz()
        .context("Original polling code is unknown")?;
    ensure!(
        baseline.polling_payload == [1, polling::rate_code(hz)?],
        "Original polling payload cannot be restored completely"
    );
    let seconds = baseline
        .idle
        .time_to_sleep
        .context("Original idle time was not observed")?;
    ensure!(
        seconds.is_multiple_of(60)
            && baseline.idle.acknowledgements.len() == 1
            && baseline.idle.acknowledgements[0].data == seconds.to_be_bytes(),
        "Original idle seconds cannot be restored exactly through the source minutes setter"
    );
    let minutes = seconds / 60;
    power::validate_change(dpi::PRODUCT_ID, &power::Change::PowerSaving(minutes))?;
    let target_minutes = if minutes < 15 { minutes + 1 } else { minutes - 1 };
    power::validate_change(dpi::PRODUCT_ID, &power::Change::PowerSaving(target_minutes))?;
    Ok([
        MouseOperation {
            query: "mouse_transient_dpi_write_restore",
            original: MouseChange::TransientDpi(active),
            target: MouseChange::TransientDpi(target_dpi),
        },
        MouseOperation {
            query: "mouse_polling_write_restore",
            original: MouseChange::Polling(hz),
            target: MouseChange::Polling(if hz == 500 { 1000 } else { 500 }),
        },
        MouseOperation {
            query: "mouse_idle_write_restore",
            original: MouseChange::IdleMinutes(minutes),
            target: MouseChange::IdleMinutes(target_minutes),
        },
    ])
}

enum MouseMutation {
    Dpi(dpi::DpiReport),
    Polling(razer_device::device_writes::DeviceWriteResult),
    Idle(power::Report),
}

impl MouseMutation {
    fn confirmed(&self, change: MouseChange) -> bool {
        match (self, change) {
            (Self::Dpi(report), MouseChange::TransientDpi(pair)) => {
                report.verified
                    && report.active == Some(pair)
                    && report.requested.as_ref() == Some(&transient_dpi(pair))
                    && report.stages.is_none()
                    && report.profile_slots.is_empty()
                    && report.profile_slot_writes.is_empty()
            }
            (Self::Polling(report), MouseChange::Polling(hz)) => {
                report.verified
                    && matches!(report.observed.as_ref(),
                        Some(device_reads::DeviceReadValue::Polling { hz: actual }) if *actual == hz)
            }
            (Self::Idle(report), MouseChange::IdleMinutes(minutes)) => {
                report.setter_sent
                    && report.requested == power::Change::PowerSaving(minutes)
                    && report.time_to_sleep == Some(minutes * 60)
            }
            _ => false,
        }
    }

    fn changed(&self) -> bool {
        match self {
            Self::Dpi(report) => !report.acknowledgements.is_empty(),
            Self::Polling(report) => report.changed && report.acknowledged.is_some(),
            Self::Idle(report) => report.setter_sent,
        }
    }

    fn value(&self) -> Value {
        match self {
            Self::Dpi(report) => json!(report),
            Self::Polling(report) => json!(report),
            Self::Idle(report) => json!(report),
        }
    }
}

struct MouseSession<'a> {
    device: &'a Traced,
    physical_product_id: u32,
    polling: DeviceReadCapability,
    power: DeviceReadCapability,
}

impl<'a> MouseSession<'a> {
    fn new(device: &'a Traced, physical_product_id: u32) -> anyhow::Result<Self> {
        Ok(Self {
            device,
            physical_product_id,
            polling: polling::transport(physical_product_id)?,
            power: power::capability(dpi::PRODUCT_ID)
                .context("Current mouse power source is absent")?
                .transport(physical_product_id, false)?,
        })
    }

    fn online(
        &self,
        next: &mut impl FnMut() -> anyhow::Result<u8>,
        deadline: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if self.physical_product_id != dpi::PRODUCT_ID {
            dpi::verify_wireless_online_for(dpi::PRODUCT_ID, self.device, next()?, deadline)?;
        }
        deadline()
    }

    fn read(
        &self,
        next: &mut impl FnMut() -> anyhow::Result<u8>,
        deadline: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<MouseSnapshot> {
        let identity = dpi::execute_for(
            dpi::PRODUCT_ID,
            self.device,
            Some(&dpi::DpiChange {
                stages: None,
                active: None,
                profile_guid: None,
                initialize: true,
            }),
            &mut *next,
            deadline,
        )?;
        ensure!(
            identity.verified
                && identity.active.is_none()
                && identity.stages.is_none()
                && identity.acknowledgements.is_empty(),
            "mouse identity query performed an unrelated operation"
        );
        let serial_number = identity
            .serial_number
            .filter(|serial| !serial.is_empty())
            .context("actual mouse serial was not observed")?;
        let first = self.device.events.borrow().len();
        let dpi = dpi::execute_for(dpi::PRODUCT_ID, self.device, None, &mut *next, deadline)?;
        ensure!(
            dpi.verified
                && dpi.read_errors.is_empty()
                && dpi.stages.is_some()
                && dpi.active.is_some(),
            "Complete stage table and transient DPI were not observed"
        );
        // Inspect the already confirmed production GET evidence; this helper
        // neither frames nor sends a protocol request.
        let active_payload = self.device.confirmed_payload_since(first, [4, 133])?;
        let first = self.device.events.borrow().len();
        let polling = polling::read(self.device, &self.polling, &mut *next, deadline)?;
        let polling_payload = self.device.confirmed_payload_since(first, [0, 192])?;
        let idle = power::execute(
            dpi::PRODUCT_ID,
            self.device,
            &self.power,
            &power::Change::ReadPowerSaving,
            &mut *next,
            deadline,
        )?;
        Ok(MouseSnapshot {
            serial_number,
            dpi,
            active_payload,
            polling,
            polling_payload,
            idle,
        })
    }

    fn apply(
        &self,
        change: MouseChange,
        next: &mut impl FnMut() -> anyhow::Result<u8>,
        deadline: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<MouseMutation> {
        match change {
            MouseChange::TransientDpi(pair) => Ok(MouseMutation::Dpi(dpi::execute_for(
                dpi::PRODUCT_ID,
                self.device,
                Some(&transient_dpi(pair)),
                next,
                deadline,
            )?)),
            // apply changes only high-speed profile 1; request can synchronize
            // OBM slots when a GUID is supplied and is not used by this fixture.
            MouseChange::Polling(hz) => Ok(MouseMutation::Polling(polling::apply(
                self.device,
                &self.polling,
                hz,
                next,
                deadline,
            )?)),
            MouseChange::IdleMinutes(minutes) => Ok(MouseMutation::Idle(power::execute(
                dpi::PRODUCT_ID,
                self.device,
                &self.power,
                &power::Change::PowerSaving(minutes),
                next,
                deadline,
            )?)),
        }
    }

    fn write_restore(
        &self,
        baseline: &MouseSnapshot,
        operation: &MouseOperation,
        next: &mut impl FnMut() -> anyhow::Result<u8>,
    ) -> anyhow::Result<Value> {
        let started = Instant::now();
        let deadline = || {
            ensure!(
                started.elapsed() < Duration::from_secs(45),
                "Mouse write deadline expired"
            );
            Ok(())
        };
        self.online(next, &deadline)?;
        let current = self.read(next, &deadline)?;
        ensure!(
            current.same_configuration(baseline),
            "Mouse changed after its original snapshot; no SET sent"
        );
        println!(
            "RAZER_CONNECTED_BASELINE {}",
            json!({
                "product_id":dpi::PRODUCT_ID,"physical_product_id":self.physical_product_id,
                "query":operation.query,"baseline":current.value(),"target":operation.target.value()
            })
        );
        std::io::stdout().flush()?;
        let write = guarded(|| self.apply(operation.target, next, &deadline));
        // No fallible early return is allowed between the attempted SET and
        // restoration. This deadline is independent of the attempted write.
        let restoring = Instant::now();
        let restore_deadline = || {
            ensure!(
                restoring.elapsed() < Duration::from_secs(45),
                "Mouse original-value restoration deadline expired"
            );
            Ok(())
        };
        let restoration = guarded(|| self.apply(operation.original, next, &restore_deadline));
        let final_read = guarded(|| self.read(next, &restore_deadline));
        let online_after = guarded(|| self.online(next, &restore_deadline));
        let verified = write
            .as_ref()
            .is_ok_and(|report| report.confirmed(operation.target) && report.changed())
            && restoration
                .as_ref()
                .is_ok_and(|report| report.confirmed(operation.original))
            && final_read
                .as_ref()
                .is_ok_and(|value| value.same_configuration(baseline))
            && online_after.is_ok();
        Ok(json!({"query":operation.query,"error":null,"value":{
            "baseline":baseline.value(),"target":operation.target.value(),
            "write":write.as_ref().ok().map(MouseMutation::value),
            "write_error":write.as_ref().err().map(|error| format!("{error:#}")),
            "restoration":restoration.as_ref().ok().map(MouseMutation::value),
            "restore_error":restoration.as_ref().err().map(|error| format!("{error:#}")),
            "final_read":final_read.as_ref().ok().map(MouseSnapshot::value),
            "final_read_error":final_read.as_ref().err().map(|error| format!("{error:#}")),
            "receiver_online_required":self.physical_product_id != dpi::PRODUCT_ID,
            "receiver_online_after_error":online_after.as_ref().err().map(|error| format!("{error:#}")),
            "verified":verified
        }}))
    }

    fn checks(&self, next: &mut impl FnMut() -> anyhow::Result<u8>) -> anyhow::Result<Vec<Value>> {
        let started = Instant::now();
        let deadline = || {
            ensure!(
                started.elapsed() < Duration::from_secs(45),
                "Mouse preflight deadline expired"
            );
            Ok(())
        };
        self.online(next, &deadline)?;
        let baseline = self.read(next, &deadline)?;
        let operations = mouse_restore_plan(&baseline)?;
        let mut readings = Vec::new();
        for operation in operations {
            let result = self.write_restore(&baseline, &operation, next);
            let reading = result.unwrap_or_else(|error| {
                json!({
                    "query":operation.query,"value":null,"error":format!("{error:#}")
                })
            });
            let verified = connected_reading_succeeded("write-restore", &reading);
            readings.push(reading);
            if !verified {
                break;
            }
        }
        Ok(readings)
    }
}

fn write_restore_mouse(
    backend: &mut NativeBackend,
    nodes: &[HidNode],
    product_id: u32,
) -> anyhow::Result<Value> {
    ensure!(
        product_id == dpi::PRODUCT_ID,
        "Mouse fixture requires its audited PID190 source"
    );
    let source = dpi::capability(product_id).context("Current mouse DPI source is absent")?;
    let (node, cap, device) = select(backend, nodes, source.transport())?;
    let mut sequence = 0;
    let mut next = || {
        let transaction = cap.transaction_prefix | sequence;
        sequence = (sequence + 1) % cap.transaction_modulus;
        Ok(transaction)
    };
    let result =
        guarded(|| MouseSession::new(&device, u32::from(node.product_id))?.checks(&mut next));
    let readings = result.unwrap_or_else(|error| {
        vec![json!({
            "query":"mouse_reversible_preflight","value":null,"error":format!("{error:#}")
        })]
    });
    let events = device.events.take();
    let metadata = device.metadata();
    drop(device);
    Ok(
        json!({"product_id":product_id,"physical_product_id":node.product_id,
        "path":String::from_utf8(node.path)?,"interface":node.interface_number,
        "readings":readings,"transfers":events,"transport_metadata":metadata,"transport_released":true}),
    )
}

fn write_restore_continuous_rapid_trigger(
    device: &Traced,
    settings: &keyboard_settings::KeyboardSettingsCapability,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
) -> anyhow::Result<Value> {
    let query = keyboard_analog::KeyboardAnalogQuery::ContinuousRapidTrigger {
        profile_id: keyboard_analog::ACTIVE_PROFILE_ID,
    };
    let started = Instant::now();
    let deadline = || {
        ensure!(
            started.elapsed() < Duration::from_secs(45),
            "Analog verification deadline expired"
        );
        Ok(())
    };
    let baseline = keyboard_analog::read(device, settings, query.clone(), next()?, &deadline)?;
    let keyboard_analog::KeyboardAnalogObservation::ContinuousRapidTrigger {
        profile_id,
        raw_enabled,
    } = &baseline
    else {
        anyhow::bail!("Expected the observed continuous rapid trigger state");
    };
    ensure!(
        *profile_id == keyboard_analog::ACTIVE_PROFILE_ID && *raw_enabled <= 1,
        "The actual analog flag cannot be restored exactly through its source setter"
    );
    let change = |enabled| keyboard_analog::KeyboardAnalogChange::ContinuousRapidTrigger {
        profile_id: *profile_id,
        enabled,
    };
    let target = *raw_enabled == 0;
    println!(
        "RAZER_CONNECTED_BASELINE {}",
        json!({"product_id":settings.product_id,
        "query":"keyboard_continuous_rapid_trigger_write_restore","baseline":baseline,"target_enabled":target})
    );
    std::io::stdout().flush()?;
    let write = guarded(|| {
        Ok(keyboard_analog::write(
            device,
            settings,
            change(target),
            true,
            &mut *next,
            &deadline,
        )?)
    });
    let restoring = Instant::now();
    let restore_deadline = || {
        ensure!(
            restoring.elapsed() < Duration::from_secs(45),
            "Analog restoration deadline expired"
        );
        Ok(())
    };
    let restoration = guarded(|| {
        Ok(keyboard_analog::write(
            device,
            settings,
            change(*raw_enabled != 0),
            true,
            &mut *next,
            &restore_deadline,
        )?)
    });
    let final_read =
        guarded(|| keyboard_analog::read(device, settings, query, next()?, &restore_deadline));
    let verified = write.as_ref().is_ok_and(|value| value.verified())
        && restoration.as_ref().is_ok_and(|value| value.verified())
        && final_read.as_ref().is_ok_and(|value| value == &baseline);
    Ok(
        json!({"query":"keyboard_continuous_rapid_trigger_write_restore","error":null,
        "value":{"baseline":baseline,"target_enabled":target,"write":write.as_ref().ok(),
            "write_error":write.as_ref().err().map(|error| format!("{error:#}")),
            "restoration":restoration.as_ref().ok(),"restore_error":restoration.as_ref().err().map(|error| format!("{error:#}")),
            "final_read":final_read.as_ref().ok(),"final_read_error":final_read.as_ref().err().map(|error| format!("{error:#}")),
            "verified":verified}}),
    )
}

fn write_restore_keyboard(
    backend: &mut NativeBackend,
    nodes: &[HidNode],
    product_id: u32,
) -> anyhow::Result<Value> {
    let settings = keyboard_settings::capability(product_id)
        .context("No audited keyboard brightness write capability")?;
    let (node, cap, device) = select(backend, nodes, &settings.transport)?;
    let mut sequence = 0;
    let mut next = || {
        let transaction = cap.transaction_prefix | sequence;
        sequence = (sequence + 1) % cap.transaction_modulus;
        Ok(transaction)
    };
    let started = Instant::now();
    let deadline = || {
        ensure!(
            started.elapsed() < Duration::from_secs(45),
            "Write verification deadline expired"
        );
        Ok(())
    };
    let regions = keyboard_settings::read_regions(&device, settings, &mut next, &deadline)?;
    ensure!(
        regions.len() == 1,
        "All-region write requires one observed region so every changed value can be restored"
    );
    let baseline = keyboard_settings::read(&device, settings, &mut next, &deadline)?;
    ensure!(
        regions[0].region_id == baseline.region_id
            && keyboard_settings::encode_percent(baseline.percent)? == baseline.raw_value,
        "Original brightness cannot be restored byte-for-byte through the audited setter"
    );
    let target = if baseline.percent == 0 {
        1
    } else {
        baseline.percent - 1
    };
    println!(
        "RAZER_CONNECTED_BASELINE {}",
        json!({"product_id":product_id,"path":String::from_utf8(node.path.clone())?,"baseline":baseline,"target_percent":target})
    );
    std::io::stdout().flush()?;
    let write =
        guarded(|| keyboard_settings::apply(&device, settings, target, &mut next, &deadline));
    // Restoration has its own deadline and runs even when the attempted write
    // rejects, loses its acknowledgement, or panics after sending the packet.
    let restoring = Instant::now();
    let restore_deadline = || {
        ensure!(
            restoring.elapsed() < Duration::from_secs(45),
            "Original brightness restoration deadline expired"
        );
        Ok(())
    };
    let restore = guarded(|| {
        keyboard_settings::apply(
            &device,
            settings,
            baseline.percent,
            &mut next,
            &restore_deadline,
        )
    });
    let final_read =
        guarded(|| keyboard_settings::read(&device, settings, &mut next, &restore_deadline));
    let verified = write.as_ref().is_ok_and(|value| value.verified)
        && restore.as_ref().is_ok_and(|value| value.verified)
        && final_read.as_ref().is_ok_and(|value| {
            value.raw_value == baseline.raw_value
                && value.profile_id == baseline.profile_id
                && value.region_id == baseline.region_id
        });
    let mut readings = vec![
        json!({"query":"keyboard_brightness_write_restore", "error":null,
        "value":{"baseline":baseline,"target_percent":target,"write":write.as_ref().ok(),
            "write_error":write.as_ref().err().map(|error| format!("{error:#}")),
            "restoration":restore.as_ref().ok(),"restore_error":restore.as_ref().err().map(|error| format!("{error:#}")),
            "final_read":final_read.as_ref().ok(),"final_read_error":final_read.as_ref().err().map(|error| format!("{error:#}")),
            "verified":verified}}),
    ];
    if verified && product_id == 679 {
        let analog = write_restore_continuous_rapid_trigger(&device, settings, &mut next);
        readings.push(analog.unwrap_or_else(|error| json!({
            "query":"keyboard_continuous_rapid_trigger_write_restore", "value":null, "error":format!("{error:#}")
        })));
    }
    let events = device.events.take();
    drop(device);
    Ok(
        json!({"product_id":product_id,"physical_product_id":node.product_id,
        "path":String::from_utf8(node.path)?,"interface":node.interface_number,"readings":readings,
        "transfers":events,"transport_released":true}),
    )
}

impl Traced {
    fn confirmed_payload_since(&self, first: usize, command: [u8; 2]) -> anyhow::Result<Vec<u8>> {
        let events = self.events.borrow();
        for event in events[first..].iter().rev() {
            if event.get("operation").and_then(Value::as_str) != Some("get_feature")
                || event.get("count").and_then(Value::as_u64) != Some(91)
                || event.get("error") != Some(&Value::Null)
            {
                continue;
            }
            let bytes: Vec<u8> = serde_json::from_value(
                event
                    .get("bytes")
                    .context("Feature evidence has no bytes")?
                    .clone(),
            )?;
            if bytes.len() == 91 && bytes[1] == 2 && bytes[7..9] == command {
                let count = usize::from(bytes[6]);
                ensure!(count <= 80, "Confirmed Feature evidence is truncated");
                return Ok(bytes[9..9 + count].to_vec());
            }
        }
        anyhow::bail!("No complete confirmed production GET evidence for {command:?}")
    }

    fn record(&self, operation: &str, bytes: &[u8], result: &anyhow::Result<usize>) {
        self.events.borrow_mut().push(json!({
            "operation":operation, "bytes":bytes,
            "count":result.as_ref().ok(), "error":result.as_ref().err().map(|error| format!("{error:#}"))
        }));
    }
}

impl FeatureTransport for Traced {
    fn hid_node(&self) -> Option<&HidNode> {
        self.transport.hid_node()
    }
    fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.send_feature_at_collection_max(bytes)
    }
    fn send_feature_at_collection_max(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let result = self.transport.send_feature_at_collection_max(bytes);
        let counted = result.map(|()| bytes.len());
        self.record("send_feature", bytes, &counted);
        counted.map(|_| ())
    }
    fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        self.get_feature_at_collection_max(bytes)
    }
    fn get_feature_at_collection_max(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        let result = self.transport.get_feature_at_collection_max(bytes);
        self.record("get_feature", bytes, &result);
        result
    }
    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        self.transport.report_lengths()
    }
    fn metadata(&self) -> Value {
        self.transport.metadata()
    }
}

fn select(
    backend: &mut NativeBackend,
    nodes: &[HidNode],
    source: &DeviceReadCapability,
) -> anyhow::Result<(HidNode, DeviceReadCapability, Traced)> {
    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    for node in nodes {
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            continue;
        };
        if node.vendor_id != source.vendor_id
            || identity.product_id != source.product_id
            || identity.is_ble
        {
            continue;
        }
        let cap = source.for_physical_product(u32::from(node.product_id))?;
        if node.interface_number != i32::from(cap.claim_interface) {
            continue;
        }
        match backend.open(node) {
            Ok(transport) => {
                if transport.report_lengths()?.feature.values().max().copied()
                    == Some(cap.report_bytes)
                {
                    candidates.push((node.clone(), cap));
                }
                drop(transport);
            }
            Err(error) => rejected.push(format!(
                "usage {:04x}:{:04x}: {error:#}",
                node.usage_page, node.usage
            )),
        }
    }
    ensure!(
        candidates.len() == 1,
        "Expected one exact source collection, found {}; rejected: {}",
        candidates.len(),
        rejected.join("; ")
    );
    let (node, cap) = candidates.pop().expect("one observed collection");
    let transport = backend.open(&node)?;
    Ok((
        node,
        cap,
        Traced {
            transport,
            events: RefCell::new(Vec::new()),
        },
    ))
}

fn query_product(
    backend: &mut NativeBackend,
    nodes: &[HidNode],
    product_id: u32,
) -> anyhow::Result<Value> {
    let source = dpi::capability(product_id)
        .map(|cap| cap.transport())
        .or_else(|| keyboard_settings::capability(product_id).map(|cap| &cap.transport))
        .or_else(|| device_reads::capability(product_id))
        .context("No current source read capability")?;
    let (node, cap, device) = select(backend, nodes, source)?;
    let mut transaction = 0u8;
    let mut next = || {
        let current = cap.transaction_prefix | transaction;
        transaction = (transaction + 1) % cap.transaction_modulus;
        Ok(current)
    };
    let started = Instant::now();
    let deadline = || {
        ensure!(
            started.elapsed() < Duration::from_secs(45),
            "Connected query deadline expired"
        );
        Ok(())
    };
    let mut readings = Vec::new();
    let mut device_online = true;
    let mut unperformed = Vec::new();
    if product_id == dpi::PRODUCT_ID {
        let online = if u32::from(node.product_id) != product_id {
            let online = dpi::verify_wireless_online_for(product_id, &device, next()?, &deadline);
            let confirmed = online.is_ok();
            readings.push(json!({"query":"receiver_online", "value":online.as_ref().ok().map(|()| true), "error":online.err().map(|error| format!("{error:#}"))}));
            confirmed
        } else {
            true
        };
        device_online = online;
        if online {
            let actual = dpi::execute_for(product_id, &device, None, &mut next, &deadline);
            readings.push(json!({"query":"dpi", "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
            let polling_cap = polling::transport(u32::from(node.product_id))?;
            let actual = polling::read(&device, &polling_cap, &mut next, &deadline);
            readings.push(json!({"query":"polling", "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
            if u32::from(node.product_id) != product_id {
                let actual =
                    dpi::verify_wireless_online_for(product_id, &device, next()?, &deadline);
                readings.push(json!({"query":"receiver_online_after", "value":actual.as_ref().ok().map(|()| true), "error":actual.err().map(|error| format!("{error:#}"))}));
            }
        }
        if !online {
            for query in ["dpi", "polling"] {
                unperformed.push(json!({"query":query,"reason":"Receiver has not confirmed the logical mouse online"}));
            }
        }
    }
    if let Some(settings) = keyboard_settings::capability(product_id) {
        let actual = keyboard_settings::read(&device, settings, &mut next, &deadline);
        readings.push(json!({"query":"keyboard_brightness", "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
    }
    if product_id == 679 {
        let metadata_cap = keyboard_metadata::capability(product_id)
            .context("Current keyboard metadata capability is absent")?;
        let actual = keyboard_metadata::read(&device, metadata_cap, &mut next, &deadline);
        let layout = actual.as_ref().ok().map(|value| value.keyboard_layout);
        readings.push(json!({"query":"keyboard_metadata", "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
        let actual = keyboard_gaming_mode::read(&device, product_id, next()?, &deadline);
        readings.push(json!({"query":"keyboard_gaming_indicator", "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
        keyboard_analog::capability(product_id)
            .context("Current keyboard analog capability is absent")?;
        let analog = keyboard_settings::capability(product_id)
            .context("Current keyboard analog transport is absent")?;
        for (name, query) in [
            (
                "keyboard_continuous_rapid_trigger",
                keyboard_analog::KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id: 1 },
            ),
            (
                "keyboard_kill_switch",
                keyboard_analog::KeyboardAnalogQuery::KillSwitch { profile_id: 1 },
            ),
        ] {
            let actual = keyboard_analog::read(&device, analog, query, next()?, &deadline);
            readings.push(json!({"query":name, "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
        }
        if layout == Some(1) {
            for (rapid, chunk_size) in [(false, 15), (true, 13)] {
                for (index, keys) in KEYBOARD_679_LAYOUT_1_KEYS.chunks(chunk_size).enumerate() {
                    let key_ids = keys.to_vec();
                    let profile_id = keyboard_analog::ACTIVE_PROFILE_ID;
                    let (name, query) = if rapid {
                        (
                            "keyboard_rapid_trigger",
                            keyboard_analog::KeyboardAnalogQuery::RapidTrigger {
                                profile_id,
                                key_ids,
                            },
                        )
                    } else {
                        (
                            "keyboard_actuation",
                            keyboard_analog::KeyboardAnalogQuery::Actuation {
                                profile_id,
                                key_ids,
                            },
                        )
                    };
                    let actual = keyboard_analog::read(&device, analog, query, next()?, &deadline);
                    readings.push(json!({"query":format!("{name}_{index}"), "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
                }
            }
        } else {
            unperformed.push(json!({"query":"keyboard_per_key_actuation_and_rapid_trigger", "observed_layout":layout,
                "reason":"This hardware fixture requires the observed source-audited layout 1 inventory"}));
        }
    }
    let shared_reads = device_reads::capability(product_id)
        .map(|capability| capability.for_physical_product(u32::from(node.product_id)))
        .transpose()?;
    for command in shared_reads
        .iter()
        .flat_map(|capability| &capability.queries)
    {
        if !device_online {
            unperformed.push(json!({"query":command.name.name(),"reason":"Receiver has not confirmed the logical mouse online"}));
            continue;
        }
        if product_id == dpi::PRODUCT_ID
            && matches!(
                command.name,
                device_reads::DeviceReadKind::Dpi | device_reads::DeviceReadKind::Polling
            )
        {
            continue;
        }
        let actual = razer_device::device_query::read_device(
            &device,
            shared_reads
                .as_ref()
                .expect("query belongs to a bound shared capability"),
            command,
            next()?,
            &deadline,
        );
        readings.push(json!({"query":command.name.name(), "value":actual.as_ref().ok(), "error":actual.as_ref().err().map(|error| format!("{error:#}"))}));
    }
    let events = device.events.take();
    let metadata = device.metadata();
    drop(device);
    Ok(
        json!({"product_id":product_id,"physical_product_id":node.product_id,"path":String::from_utf8(node.path)?,"interface":node.interface_number,
        "usage_page":node.usage_page,"usage":node.usage,"source_class":cap.source_class,
        "readings":readings,"unperformed":unperformed,"transfers":events,"transport_metadata":metadata,"transport_released":true}),
    )
}

#[test]
#[ignore = "Requires explicit connected-device authorization and a real HID collection"]
fn connected_device_queries() {
    run_connected("read-only");
}

#[test]
#[ignore = "Explicit reversible hardware write; original value must be recorded and restored"]
fn connected_device_write_restore() {
    run_connected("write-restore");
}

fn connected_reading_succeeded(mode: &str, reading: &Value) -> bool {
    reading.get("error") == Some(&Value::Null)
        && reading.get("value").is_some_and(|value| {
            !value.is_null()
                && value.get("kind").and_then(Value::as_str) != Some("unknown")
                && if mode == "write-restore" {
                    value.get("verified").and_then(Value::as_bool) == Some(true)
                } else {
                    value.get("verified").and_then(Value::as_bool) != Some(false)
                }
        })
}

#[test]
fn connected_result_validation_rejects_unconfirmed_mutation_or_restoration() {
    for value in [
        json!({"verified":false,"restore_error":"lost acknowledgement"}),
        json!({"write_error":"device rejected"}),
        json!({"verified":true,"kind":"unknown"}),
        Value::Null,
    ] {
        assert!(!connected_reading_succeeded(
            "write-restore",
            &json!({"error":null,"value":value})
        ));
    }
    assert!(connected_reading_succeeded(
        "write-restore",
        &json!({"error":null,"value":{"verified":true}})
    ));
    assert!(!connected_reading_succeeded(
        "write-restore",
        &json!({"error":"failed after recording","value":{"verified":true}})
    ));
}

#[test]
fn connected_result_validation_preserves_real_reads_without_fabricated_values() {
    assert!(connected_reading_succeeded(
        "read-only",
        &json!({"error":null,"value":{"raw_value":255,"percent":100}})
    ));
    for reading in [
        json!({"error":null,"value":{"kind":"unknown"}}),
        json!({"error":"no response","value":null}),
        json!({"error":null}),
    ] {
        assert!(!connected_reading_succeeded("read-only", &reading));
    }
}

mod mouse_reversible_tests {
    use super::*;
    use std::rc::Rc;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Fault {
        None,
        TargetReadbackError,
        TargetPanic,
        RestoreMismatch,
        FinalStageMismatch,
        OfflineAfterRestore,
    }

    struct State {
        active: Vec<u8>,
        polling: Vec<u8>,
        idle: Vec<u8>,
        stages: Vec<u8>,
        sent: Vec<Vec<u8>>,
        reply: Vec<u8>,
        setters: usize,
        fail_next_getter: bool,
        fault: Fault,
    }

    impl State {
        fn new(fault: Fault) -> Self {
            Self {
                active: vec![0, 7, 208, 8, 52, 0, 0], // X=2000, Y=2100, Z=0.
                polling: vec![1, 8],                  // High-speed profile 1, 1000 Hz.
                idle: vec![1, 44], // Source getter seconds=300; setter minutes=5.
                // Profile, active stage, count, index, X/Y/Z. A nonzero stage Z
                // must survive a transient-only operation without a stage SET.
                stages: vec![1, 1, 1, 1, 3, 32, 6, 64, 0, 7],
                sent: Vec::new(),
                reply: Vec::new(),
                setters: 0,
                fail_next_getter: false,
                fault,
            }
        }
    }

    struct SimulatedMouse {
        node: HidNode,
        state: Rc<RefCell<State>>,
    }

    impl FeatureTransport for SimulatedMouse {
        fn hid_node(&self) -> Option<&HidNode> {
            Some(&self.node)
        }

        fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
            assert_eq!(bytes.len(), 91);
            let mut state = self.state.borrow_mut();
            state.sent.push(bytes.to_vec());
            let command = [bytes[7], bytes[8]];
            let setter = matches!(command, [4, 5] | [0, 64] | [7, 3]);
            if setter {
                state.setters += 1;
                if state.fault != Fault::RestoreMismatch || state.setters != 2 {
                    match command {
                        [4, 5] => {
                            assert_eq!((bytes[6], bytes[9]), (7, 0));
                            state.active = bytes[9..16].to_vec();
                        }
                        [0, 64] => {
                            assert_eq!((bytes[6], bytes[9]), (2, 1));
                            state.polling = bytes[9..11].to_vec();
                        }
                        [7, 3] => {
                            assert_eq!(bytes[6], 2);
                            state.idle = bytes[9..11].to_vec();
                        }
                        _ => unreachable!(),
                    }
                }
                if state.setters == 1 {
                    if state.fault == Fault::TargetPanic {
                        panic!("Simulated panic after the target SET reached the device");
                    }
                    state.fail_next_getter = state.fault == Fault::TargetReadbackError;
                }
            }
            let mut data = match command {
                [4, 133] => {
                    assert_eq!(bytes[9], 0);
                    state.active.clone()
                }
                [4, 134] => {
                    assert_eq!(bytes[9], 1);
                    let mut stages = state.stages.clone();
                    if state.fault == Fault::FinalStageMismatch && state.setters >= 2 {
                        stages[9] ^= 1;
                    }
                    stages
                }
                [0, 192] => {
                    assert_eq!(&bytes[9..11], &[1, 0]);
                    state.polling.clone()
                }
                [7, 131] => state.idle.clone(),
                [0, 184] => vec![u8::from(
                    state.fault != Fault::OfflineAfterRestore || state.setters < 2,
                )],
                [4, 5] | [0, 64] | [7, 3] => Vec::new(),
                _ => anyhow::bail!(
                    "Unexpected mutation or getter in the hardware fixture: {command:?}"
                ),
            };
            let fail = !setter && state.fail_next_getter;
            if fail {
                state.fail_next_getter = false;
                data.clear();
            }
            let mut reply = vec![0; 91];
            reply[1] = if fail { 5 } else { 2 };
            reply[2] = bytes[2];
            reply[6] = data.len() as u8;
            reply[7..9].copy_from_slice(&command);
            reply[9..9 + data.len()].copy_from_slice(&data);
            state.reply = reply;
            Ok(())
        }

        fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
            let state = self.state.borrow();
            bytes.copy_from_slice(&state.reply);
            Ok(bytes.len())
        }

        fn metadata(&self) -> Value {
            json!({"simulated":true})
        }
    }

    fn fixture(physical: u16, fault: Fault) -> (Traced, Rc<RefCell<State>>) {
        let state = Rc::new(RefCell::new(State::new(fault)));
        let device = Traced {
            transport: Box::new(SimulatedMouse {
                node: HidNode {
                    path: b"simulated-mouse-collection".to_vec(),
                    vendor_id: 0x1532,
                    product_id: physical,
                    usage_page: 0xff00,
                    usage: 1,
                    interface_number: 0,
                    release_number: 0,
                    serial_number: None,
                    manufacturer: None,
                    product: None,
                },
                state: Rc::clone(&state),
            }),
            events: RefCell::new(Vec::new()),
        };
        (device, state)
    }

    fn next(physical: u32) -> impl FnMut() -> anyhow::Result<u8> {
        let source = dpi::capability(dpi::PRODUCT_ID)
            .unwrap()
            .transport()
            .for_physical_product(physical)
            .unwrap();
        let mut counter = 0;
        move || {
            let result = source.transaction_prefix | counter;
            counter = (counter + 1) % source.transaction_modulus;
            Ok(result)
        }
    }

    fn assert_original(state: &State) {
        let original = State::new(Fault::None);
        assert_eq!(state.active, original.active);
        assert_eq!(state.polling, original.polling);
        assert_eq!(state.idle, original.idle);
        assert_eq!(state.stages, original.stages);
    }

    #[test]
    fn connected_result_validation_mouse_restores_all_three_settings_without_profile_writes() {
        for physical in [190, 191] {
            let (device, state) = fixture(physical, Fault::None);
            let session = MouseSession::new(&device, u32::from(physical)).unwrap();
            let readings = session.checks(&mut next(u32::from(physical))).unwrap();
            assert_eq!(readings.len(), 3);
            assert!(
                readings
                    .iter()
                    .all(|reading| connected_reading_succeeded("write-restore", reading))
            );
            let state = state.borrow();
            assert_original(&state);
            assert_eq!(state.setters, 6);
            let mutations = state
                .sent
                .iter()
                .filter(|report| report[8] & 0x80 == 0)
                .collect::<Vec<_>>();
            assert_eq!(mutations.len(), 6);
            assert_eq!(&mutations[0][6..16], &[7, 4, 5, 0, 8, 2, 8, 52, 0, 0]);
            assert_eq!(&mutations[1][9..16], &[0, 7, 208, 8, 52, 0, 0]);
            assert_eq!(&mutations[2][6..11], &[2, 0, 64, 1, 16]);
            assert_eq!(&mutations[3][6..11], &[2, 0, 64, 1, 8]);
            assert_eq!(&mutations[4][6..11], &[2, 7, 3, 1, 104]); // Six minutes, 360 seconds.
            assert_eq!(&mutations[5][6..11], &[2, 7, 3, 1, 44]); // Five minutes, 300 seconds.
            let online_getters = state
                .sent
                .iter()
                .filter(|report| report[7..9] == [0, 184])
                .count();
            assert_eq!(online_getters, if physical == 191 { 7 } else { 0 });
        }
    }

    #[test]
    fn connected_result_validation_mouse_restores_after_error_or_panic_and_stops() {
        for fault in [Fault::TargetReadbackError, Fault::TargetPanic] {
            for index in 0..3 {
                let (device, state) = fixture(191, fault);
                let session = MouseSession::new(&device, 191).unwrap();
                let mut next = next(191);
                let baseline = session.read(&mut next, &|| Ok(())).unwrap();
                let plan = mouse_restore_plan(&baseline).unwrap();
                let reading = session
                    .write_restore(&baseline, &plan[index], &mut next)
                    .unwrap();
                assert!(!connected_reading_succeeded("write-restore", &reading));
                assert!(reading["value"]["write_error"].is_string());
                assert!(reading["value"]["restore_error"].is_null());
                assert!(reading["value"]["final_read_error"].is_null());
                assert!(reading["value"]["receiver_online_after_error"].is_null());
                assert_eq!(state.borrow().setters, 2);
                assert_original(&state.borrow());
            }
            let (device, state) = fixture(191, fault);
            let readings = MouseSession::new(&device, 191)
                .unwrap()
                .checks(&mut next(191))
                .unwrap();
            assert_eq!(readings.len(), 1);
            assert_eq!(state.borrow().setters, 2);
            assert_original(&state.borrow());
        }
    }

    #[test]
    fn connected_result_validation_mouse_rejects_inexact_restoration_or_offline_receiver() {
        for fault in [
            Fault::RestoreMismatch,
            Fault::FinalStageMismatch,
            Fault::OfflineAfterRestore,
        ] {
            let (device, state) = fixture(191, fault);
            let readings = MouseSession::new(&device, 191)
                .unwrap()
                .checks(&mut next(191))
                .unwrap();
            assert_eq!(readings.len(), 1);
            assert!(!connected_reading_succeeded("write-restore", &readings[0]));
            assert_eq!(state.borrow().setters, 2);
            let value = &readings[0]["value"];
            assert!(value["write_error"].is_null());
            if fault == Fault::RestoreMismatch {
                assert!(value["restore_error"].is_string());
            } else {
                assert!(value["restore_error"].is_null());
                assert_original(&state.borrow());
            }
            if fault == Fault::OfflineAfterRestore {
                assert!(value["receiver_online_after_error"].is_string());
            } else {
                assert!(value["receiver_online_after_error"].is_null());
            }
        }
    }

    #[test]
    fn connected_result_validation_mouse_refuses_unrestorable_original_fields_before_any_set() {
        for case in 0..8 {
            let (device, state) = fixture(191, Fault::None);
            {
                let mut state = state.borrow_mut();
                match case {
                    0 => state.active[6] = 1,
                    1 => state.active.push(0),
                    2 => state.polling[1] = 3,
                    3 => state.polling.push(0),
                    4 => state.idle = 301_u16.to_be_bytes().to_vec(),
                    5 => state.idle = 0_u16.to_be_bytes().to_vec(),
                    6 => state.idle = 960_u16.to_be_bytes().to_vec(),
                    7 => state.idle.push(0),
                    _ => unreachable!(),
                }
            }
            let result = MouseSession::new(&device, 191)
                .unwrap()
                .checks(&mut next(191));
            assert!(
                result.is_err(),
                "Unrestorable original case {case} was admitted"
            );
            assert_eq!(state.borrow().setters, 0);
        }
    }
}

fn run_connected(mode: &str) {
    assert_eq!(
        std::env::var("RAZER_CONNECTED_DEVICE_TEST").as_deref(),
        Ok(mode)
    );
    let products = std::env::var("RAZER_CONNECTED_PRODUCTS").expect("explicit product selection");
    let products = products
        .split(',')
        .map(|product| product.parse::<u32>().expect("numeric logical product ID"))
        .collect::<Vec<_>>();
    println!(
        "RAZER_CONNECTED_STARTED {}",
        json!({"mode":mode,"products":products})
    );
    std::io::stdout()
        .flush()
        .expect("record native test admission");
    let mut backend = NativeBackend::new().expect("initialize actual HID backend");
    let nodes = backend
        .enumerate()
        .expect("enumerate actual HID collections");
    let mut results = Vec::new();
    for product_id in products {
        let result = if mode == "read-only" {
            query_product(&mut backend, &nodes, product_id)
        } else if product_id == dpi::PRODUCT_ID {
            write_restore_mouse(&mut backend, &nodes, product_id)
        } else {
            write_restore_keyboard(&mut backend, &nodes, product_id)
        };
        results.push(json!({"product_id":product_id,"result":result.as_ref().ok(),"error":result.as_ref().err().map(|error| format!("{error:#}"))}));
    }
    drop(backend);
    println!(
        "RAZER_CONNECTED_RESULT {}",
        json!({"mode":mode, "devices":results, "backend_released":true})
    );
    assert!(
        results.iter().all(|row| {
            row["error"].is_null()
                && row["result"]["transport_released"] == true
                && row["result"]["readings"]
                    .as_array()
                    .is_some_and(|readings| {
                        !readings.is_empty()
                            && readings
                                .iter()
                                .all(|reading| connected_reading_succeeded(mode, reading))
                    })
        }),
        "A device read, mutation or original-value restoration was not confirmed; inspect the recorded result"
    );
}
