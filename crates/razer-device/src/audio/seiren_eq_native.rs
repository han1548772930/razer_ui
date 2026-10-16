//! Source Audio_StreamMixer -> AudioMarie -> current Marie native exports.
//! The platform owns library lookup/FFI; this task machine owns source ordering,
//! unchecked initialization diagnostics, cancellation and unconditional cleanup.
use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    is_enabled: bool,
    value: f64,
}
impl Level {
    pub fn new(is_enabled: bool, value: f64) -> Self {
        Self { is_enabled, value }
    }
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }
    pub fn value(&self) -> f64 {
        self.value
    }
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.value.is_finite() && (0. ..=100.).contains(&self.value),
            "Seiren audio level exceeds current 0..100 choices"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NativeLevels {
    pub headphones: Level,
    pub microphone: Level,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "export", content = "arguments")]
pub enum NativeOperation {
    DeviceInitialize,
    GetMode,
    GetDeviceVolume { device_id: u32 },
    GetDeviceMute { device_id: u32 },
    SetDeviceVolume { device_id: u32, level: u32 },
    SetDeviceMute { device_id: u32, muted: bool },
    SetMicMonitorLevel { level: i32 },
    SetMicMonitorEnable { enabled: bool },
    SetMasterSampleRate { rate: u32 },
    SetMode { enabled: bool },
    SetPlaybackExternal { enabled: bool },
    SetLevelMeterEnable { enabled: bool },
    SetStreamMixMonitor { enabled: bool },
    SetMixEnable { output_id: u32, input_id: u32, enabled: bool },
    SetMixLevel { output_id: u32, input_id: u32, level: u32 },
    DeviceTerminate,
}
impl NativeOperation {
    pub fn is_setter(&self) -> bool {
        matches!(
            self,
            Self::SetDeviceVolume { .. }
                | Self::SetDeviceMute { .. }
                | Self::SetMicMonitorLevel { .. }
                | Self::SetMicMonitorEnable { .. }
                | Self::SetMasterSampleRate { .. }
                | Self::SetMode { .. }
                | Self::SetPlaybackExternal { .. }
                | Self::SetLevelMeterEnable { .. }
                | Self::SetStreamMixMonitor { .. }
                | Self::SetMixEnable { .. }
                | Self::SetMixLevel { .. }
        )
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum NativeReply {
    Boolean(bool),
    Long(i32),
}
pub trait NativeTransport {
    fn call(&self, operation: &NativeOperation) -> anyhow::Result<NativeReply>;
    /// Retained feature state from initialization and explicit mode changes.
    /// This is not an extra native getter or a UI observation.
    fn stream_mixer_enabled(&self)->bool{false}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NativeStage {
    operation: NativeOperation,
    called: bool,
    reply: Option<NativeReply>,
    error: Option<String>,
}
impl NativeStage {
    pub fn operation(&self) -> &NativeOperation {
        &self.operation
    }
    pub fn reply(&self) -> Option<&NativeReply> {
        self.reply.as_ref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub(super) fn confirmed(&self) -> bool {
        self.called && self.reply.is_some() && self.error.is_none()
    }
    pub(super) fn setter_sent(&self) -> bool {
        self.called && self.operation.is_setter()
    }
}
#[derive(Default)]
struct CallOutcome {
    called: bool,
    returned: bool,
    succeeded: bool,
}
fn call(
    report: &mut Report,
    transport: &dyn NativeTransport,
    operation: NativeOperation,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> CallOutcome {
    if let Err(error) = validate() {
        report.native_stages.push(NativeStage {
            operation,
            called: false,
            reply: None,
            error: Some(format!("{error:#}")),
        });
        return CallOutcome::default();
    }
    let mut returned = false;
    let response = transport.call(&operation);
    let reply = response.as_ref().ok().cloned();
    let result = response.and_then(|reply| {
        match (&operation, &reply) {
            (NativeOperation::GetMode, NativeReply::Long(mode)) => ensure!(
                *mode == 0 || *mode == 1,
                "Marie native returned an invalid stream-mixer mode"
            ),
            (NativeOperation::GetDeviceVolume { .. }, NativeReply::Long(level)) => ensure!(
                (0..=100).contains(level),
                "Marie native returned a volume outside 0..100"
            ),
            (NativeOperation::GetDeviceMute { .. }, NativeReply::Boolean(_)) => {}
            (
                NativeOperation::DeviceInitialize
                | NativeOperation::DeviceTerminate
                | NativeOperation::SetDeviceVolume { .. }
                | NativeOperation::SetDeviceMute { .. }
                | NativeOperation::SetMicMonitorLevel { .. }
                | NativeOperation::SetMicMonitorEnable { .. }
                | NativeOperation::SetMasterSampleRate { .. }
                | NativeOperation::SetMode { .. }
                | NativeOperation::SetPlaybackExternal { .. }
                | NativeOperation::SetLevelMeterEnable { .. }
                | NativeOperation::SetStreamMixMonitor { .. }
                | NativeOperation::SetMixEnable { .. }
                | NativeOperation::SetMixLevel { .. },
                NativeReply::Boolean(succeeded),
            ) => {
                returned = true;
                ensure!(*succeeded, "Marie native returned false for {operation:?}");
            }
            _ => anyhow::bail!("Marie native returned the wrong value type for {operation:?}"),
        }
        returned = true;
        Ok(reply)
    });
    let succeeded = result.is_ok();
    report.native_stages.push(NativeStage {
        operation,
        called: true,
        reply,
        error: result.err().map(|error| format!("{error:#}")),
    });
    CallOutcome {
        called: true,
        returned,
        succeeded,
    }
}

pub fn execute_native(
    product_id: u32,
    transport: &dyn NativeTransport,
    change: &Change,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    execute_native_inner(product_id,transport,change,validate,true)
}

/// The source AudioMarie instance initializes once and is retained until
/// unload. Reusing it must never terminate it after each volume/sample task.
pub fn execute_native_in_session(
    product_id: u32, transport: &dyn NativeTransport, change: &Change,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    execute_native_inner(product_id,transport,change,validate,false)
}
fn execute_native_inner(
    product_id: u32, transport: &dyn NativeTransport, change: &Change,
    validate: impl Fn() -> anyhow::Result<()>, lifecycle:bool,
) -> anyhow::Result<Report> {
    validate_change(product_id, change)?;
    ensure!(
        change.requires_native(),
        "Seiren HID task was sent to the native executor"
    );
    validate()?;
    let mut report = Report {
        requested: change.clone(),
        stages: Vec::new(),
        source_result: false,
        native_stages: Vec::new(),
        source_skipped: None,
        native_levels: None,
    };
    let initialized = if lifecycle {call(
        &mut report,
        transport,
        NativeOperation::DeviceInitialize,
        &validate,
    )} else {CallOutcome{called:true,returned:true,succeeded:true}};
    // AudioMarie.init sets isInit after ConfigureFFI, before awaiting
    // DeviceInitialize. Its false return or caught exception does not clear
    // isInit, so the source feature still attempts the requested operation.
    // Keep initialization failures as diagnostics; cancellation before the call
    // must still prevent the operation, and every later call checks the guard.
    if initialized.called {
        let operations = match change {
            Change::SamplingRate(rate) => vec![NativeOperation::SetMasterSampleRate{rate:*rate}],
            Change::ReadVolumes => {
                // Audio_StreamMixer.getRuntimeData publishes only the actual
                // native HP volume/mute then mic volume/mute, in this order.
                vec![
                    NativeOperation::GetDeviceVolume { device_id: 524289 },
                    NativeOperation::GetDeviceMute { device_id: 524289 },
                    NativeOperation::GetDeviceVolume { device_id: 262145 },
                    NativeOperation::GetDeviceMute { device_id: 262145 },
                ]
            }
            Change::MicVolume(level) | Change::HeadphonesVolume(level) => {
                let device_id = if matches!(change, Change::MicVolume(_)) {
                    262145
                } else {
                    524289
                };
                // libffi "long" arguments are 32-bit on Windows. JS may have a
                // fractional interpolated mic level; the source conversion truncates.
                vec![
                    NativeOperation::SetDeviceVolume {
                        device_id,
                        level: level.value as u32,
                    },
                    NativeOperation::SetDeviceMute {
                        device_id,
                        muted: !level.is_enabled,
                    },
                ]
            }
            Change::MonitorVolume(level) => {
                {
                    let enabled=transport.stream_mixer_enabled();
                    if enabled {
                        report.source_skipped = Some(
                            "Source skips mic-monitor settings while stream mixer is enabled"
                                .into(),
                        );
                        report.source_result = true;
                        Vec::new()
                    } else {
                        vec![
                            NativeOperation::SetMicMonitorLevel {
                                level: level.value as i32,
                            },
                            NativeOperation::SetMicMonitorEnable {
                                enabled: level.is_enabled,
                            },
                        ]
                    }
                }
            }
            _ => unreachable!("validated native operation"),
        };
        if !operations.is_empty() {
            let mut succeeded = true;
            for operation in operations {
                let outcome = call(&mut report, transport, operation, &validate);
                succeeded &= outcome.succeeded;
                if !outcome.returned {
                    break;
                }
                // Source tries mute/enable even if level returns false; a
                // thrown transport error enters the feature's catch block.
                // Cancellation, identity change or a deadline also stops it.
            }
            report.source_result = succeeded;
            if succeeded && matches!(change, Change::ReadVolumes) {
                let stages = &report.native_stages;
                let (
                    Some(NativeReply::Long(hp)),
                    Some(NativeReply::Boolean(hp_muted)),
                    Some(NativeReply::Long(mic)),
                    Some(NativeReply::Boolean(mic_muted)),
                ) = (
                    stages[usize::from(lifecycle)].reply(),
                    stages[usize::from(lifecycle)+1].reply(),
                    stages[usize::from(lifecycle)+2].reply(),
                    stages[usize::from(lifecycle)+3].reply(),
                )
                else {
                    unreachable!("validated native observation types");
                };
                report.native_levels = Some(NativeLevels {
                    headphones: Level::new(!*hp_muted, f64::from(*hp)),
                    microphone: Level::new(!*mic_muted, f64::from(*mic)),
                });
            }
        }
    }
    // DeviceTerminate runs even after partial initialization, failed setters,
    // cancellation or an identity change; it uses the original container owner.
    if lifecycle {call(
        &mut report,
        transport,
        NativeOperation::DeviceTerminate,
        &|| Ok(()),
    );}
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };
    struct Simulated {
        calls: Mutex<Vec<NativeOperation>>,
        fail: Option<NativeOperation>,
        throw: Option<NativeOperation>,
        wrong_reply: Option<NativeOperation>,
        cancel: AtomicBool,
        mode: i32,
    }
    impl Simulated {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                fail: None,
                throw: None,
                wrong_reply: None,
                cancel: AtomicBool::new(false),
                mode: 0,
            }
        }
    }
    impl NativeTransport for Simulated {
        fn stream_mixer_enabled(&self)->bool{self.mode==1}
        fn call(&self, operation: &NativeOperation) -> anyhow::Result<NativeReply> {
            self.calls.lock().unwrap().push(operation.clone());
            ensure!(
                self.throw.as_ref() != Some(operation),
                "Simulated native transport exception"
            );
            if self.wrong_reply.as_ref() == Some(operation) {
                return Ok(NativeReply::Boolean(true));
            }
            if *operation == NativeOperation::GetMode {
                return Ok(NativeReply::Long(self.mode));
            }
            if let NativeOperation::GetDeviceVolume { device_id } = operation {
                return Ok(NativeReply::Long(if *device_id == 524289 {
                    73
                } else {
                    41
                }));
            }
            if let NativeOperation::GetDeviceMute { device_id } = operation {
                return Ok(NativeReply::Boolean(*device_id == 262145));
            }
            Ok(NativeReply::Boolean(self.fail.as_ref() != Some(operation)))
        }
    }
    #[test]
    fn actual_native_volume_then_mute_and_cleanup_preserve_device_id_and_fractional_ffi_conversion()
    {
        for (change, device_id) in [
            (Change::MicVolume(Level::new(true, 43.9)), 262145),
            (Change::HeadphonesVolume(Level::new(false, 43.9)), 524289),
        ] {
            let device = Simulated::new();
            let report = execute_native(1422, &device, &change, || Ok(())).unwrap();
            assert!(report.confirmed());
            assert!(report.setter_sent());
            assert_eq!(
                *device.calls.lock().unwrap(),
                vec![
                    NativeOperation::DeviceInitialize,
                    NativeOperation::SetDeviceVolume {
                        device_id,
                        level: 43
                    },
                    NativeOperation::SetDeviceMute {
                        device_id,
                        muted: matches!(change, Change::HeadphonesVolume(_))
                    },
                    NativeOperation::DeviceTerminate
                ]
            );
        }
    }
    #[test]
    fn initialization_observes_actual_hp_then_mic_levels_and_accepts_false_mute_as_a_value() {
        let device = Simulated::new();
        let report = execute_native(1422, &device, &Change::ReadVolumes, || Ok(())).unwrap();
        assert!(report.confirmed());
        assert!(!report.setter_sent());
        let levels = report.native_levels().unwrap();
        assert_eq!(levels.headphones, Level::new(true, 73.));
        assert_eq!(levels.microphone, Level::new(false, 41.));
        assert_eq!(
            *device.calls.lock().unwrap(),
            vec![
                NativeOperation::DeviceInitialize,
                NativeOperation::GetDeviceVolume { device_id: 524289 },
                NativeOperation::GetDeviceMute { device_id: 524289 },
                NativeOperation::GetDeviceVolume { device_id: 262145 },
                NativeOperation::GetDeviceMute { device_id: 262145 },
                NativeOperation::DeviceTerminate
            ]
        );
        assert!(!Change::ReadVolumes.persists_local_settings());
    }
    #[test]
    fn failed_native_level_still_attempts_source_mute_and_retains_actual_failure() {
        let mut device = Simulated::new();
        device.fail = Some(NativeOperation::SetDeviceVolume {
            device_id: 262145,
            level: 50,
        });
        let report = execute_native(
            1446,
            &device,
            &Change::MicVolume(Level::new(true, 50.)),
            || Ok(()),
        )
        .unwrap();
        assert!(!report.confirmed());
        assert!(!report.source_result());
        assert_eq!(device.calls.lock().unwrap().len(), 4);
        assert!(report.native_stages()[1].error().is_some());
    }
    #[test]
    fn unchecked_native_initialization_keeps_diagnostics_and_attempts_source_volume_task() {
        for throws in [false, true] {
            let mut device = Simulated::new();
            if throws {
                device.throw = Some(NativeOperation::DeviceInitialize);
            } else {
                device.fail = Some(NativeOperation::DeviceInitialize);
            }
            let report = execute_native(
                1422,
                &device,
                &Change::MicVolume(Level::new(true, 50.)),
                || Ok(()),
            )
            .unwrap();
            assert!(report.source_result());
            assert!(!report.confirmed());
            assert!(report.setter_sent());
            assert!(report.native_stages()[0].error().is_some());
            assert_eq!(
                report.native_stages()[0].reply(),
                (!throws).then_some(&NativeReply::Boolean(false))
            );
            assert_eq!(
                *device.calls.lock().unwrap(),
                vec![
                    NativeOperation::DeviceInitialize,
                    NativeOperation::SetDeviceVolume {
                        device_id: 262145,
                        level: 50,
                    },
                    NativeOperation::SetDeviceMute {
                        device_id: 262145,
                        muted: false,
                    },
                    NativeOperation::DeviceTerminate,
                ]
            );
        }
    }
    #[test]
    fn thrown_volume_transport_error_stops_at_source_catch_and_still_cleans_up() {
        let mut device = Simulated::new();
        let volume = NativeOperation::SetDeviceVolume {
            device_id: 524289,
            level: 50,
        };
        device.throw = Some(volume.clone());
        let report = execute_native(
            1446,
            &device,
            &Change::HeadphonesVolume(Level::new(true, 50.)),
            || Ok(()),
        )
        .unwrap();
        assert!(!report.source_result());
        assert!(!report.confirmed());
        assert!(report.setter_sent());
        assert_eq!(
            *device.calls.lock().unwrap(),
            vec![
                NativeOperation::DeviceInitialize,
                volume,
                NativeOperation::DeviceTerminate,
            ]
        );
    }
    #[test]
    fn invalid_native_read_type_has_no_fabricated_observation_and_still_cleans_up() {
        let mut device = Simulated::new();
        let read = NativeOperation::GetDeviceVolume { device_id: 524289 };
        device.wrong_reply = Some(read.clone());
        let report = execute_native(1422, &device, &Change::ReadVolumes, || Ok(())).unwrap();
        assert!(!report.source_result());
        assert!(!report.confirmed());
        assert!(!report.setter_sent());
        assert!(report.native_levels().is_none());
        assert!(report.native_stages()[1].error().is_some());
        assert_eq!(
            *device.calls.lock().unwrap(),
            vec![
                NativeOperation::DeviceInitialize,
                read,
                NativeOperation::DeviceTerminate,
            ]
        );
    }
    #[test]
    fn actual_stream_mixer_mode_skips_monitor_without_claiming_device_write() {
        let mut device = Simulated::new();
        device.mode = 1;
        let report = execute_native(
            1422,
            &device,
            &Change::MonitorVolume(Level::new(true, 70.)),
            || Ok(()),
        )
        .unwrap();
        assert!(report.source_result());
        assert!(!report.confirmed());
        assert!(!report.setter_sent());
        assert!(report.source_skipped().is_some());
        assert_eq!(
            *device.calls.lock().unwrap(),
            vec![
                NativeOperation::DeviceInitialize,
                NativeOperation::DeviceTerminate
            ]
        );
    }
    #[test]
    fn cancellation_before_next_native_mutation_still_terminates_original_owner() {
        let device = Simulated::new();
        let report = execute_native(
            1422,
            &device,
            &Change::MicVolume(Level::new(true, 50.)),
            || {
                if device.calls.lock().unwrap().len() == 2 {
                    device.cancel.store(true, Ordering::Release);
                }
                ensure!(
                    !device.cancel.load(Ordering::Acquire),
                    "Canceled native task"
                );
                Ok(())
            },
        )
        .unwrap();
        assert!(!report.confirmed());
        assert!(report.setter_sent());
        assert_eq!(
            device.calls.lock().unwrap().last(),
            Some(&NativeOperation::DeviceTerminate)
        );
        assert!(
            !device
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|op| matches!(op, NativeOperation::SetDeviceMute { .. }))
        );
    }
}
