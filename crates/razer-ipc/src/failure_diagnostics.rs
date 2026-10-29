//! Failed operations retain measured progress separately from successful data.
use razer_device::receiver_session::ReceiverObservationFailure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "domain",
    content = "progress",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum FailureDiagnostics {
    SeirenEq(razer_device::seiren_eq::Report),
    Leviathan(razer_device::leviathan::Report),
    MixerObservation(razer_device::audio_mixer::MixerObservationFailure),
    MixerSubmission(razer_device::audio_mixer::MixerSubmissionFailure),
    MixerMicEq(razer_device::audio_mixer::MicEqFailure),
    MixerDriverCompletion(razer_device::audio_mixer::DriverCompletionFailure),
    MixerStreamsReset(razer_device::audio_mixer::StreamsResetFailure),
    MixerCommandBatch(razer_device::audio_mixer_local::CommandBatchFailure),
    MixerHardwareEvents(razer_device::audio_mixer_hardware::HardwareEventsFailure),
    MixerLightingSample(razer_device::audio_mixer_local::lighting::SampleFailure),
    MixerExchange(razer_device::audio_mixer::MixerExchangeProgress),
    MixerTransfer(razer_device::audio_mixer_local::TransferProgress),
    MixerCommand(razer_device::audio_mixer_local::Receipt),
    MixerHardware(razer_device::audio_mixer_hardware::Snapshot),
    DeviceWrite(razer_device::device_writes::DeviceWriteFailure),
    MouseDpi(razer_device::mouse_current_dpi::WriteFailure),
    MousePower(razer_device::mouse_current_power::WriteFailure),
    MousePolling(razer_device::mouse_current_polling::RequestFailure),
    MousePollingSlots(razer_device::mouse_current_polling::SlotWriteFailure),
    MouseReceiverIndicator(razer_device::mouse_receiver_legacy::WriteFailure),
    MouseReceiverPolling(razer_device::mouse_receiver_polling::PollingWriteFailure),
    MouseDpiStages(razer_device::mouse_dpi_stages::DpiStagesWriteFailure),
    MouseDpiSlots(razer_device::mouse_dpi_stages::DpiSlotSynchronizationFailure),
    MouseDpiStageAcknowledgement(razer_device::mouse_dpi_stages::DpiStagesAcknowledgement),
    MouseScroll(razer_device::mouse_scroll::SubmissionFailure),
    MouseDynamic(razer_device::mouse_dynamic::WriteFailure),
    KeyboardBrightness(razer_device::keyboard_settings::KeyboardBrightnessWriteFailure),
    KeyboardIndicator(razer_device::keyboard_indicator::Report),
    KeyboardChunks(Vec<razer_device::keyboard_latest_protocol::ChunkReceipt>),
    ReceiverDeviceRead(ReceiverObservationFailure<razer_device::device_reads::DeviceReadValue>),
    ReceiverDeviceWrite(ReceiverObservationFailure<razer_device::device_writes::DeviceWriteResult>),
    ReceiverPollingWrite(
        ReceiverObservationFailure<razer_device::mouse_receiver_polling::PollingWriteResult>,
    ),
    ReceiverDpiStagesRead(
        ReceiverObservationFailure<razer_device::mouse_dpi_stages::DpiStagesReading>,
    ),
    ReceiverDpiStagesWrite(
        ReceiverObservationFailure<razer_device::mouse_dpi_stages::DpiStagesWriteResult>,
    ),
    ReceiverDpi(ReceiverObservationFailure<razer_device::mouse_current_dpi::DpiReport>),
    ReceiverPower(ReceiverObservationFailure<razer_device::mouse_current_power::Report>),
    ReceiverBrightness(ReceiverObservationFailure<razer_device::mouse_brightness::TaskReport>),
    ReceiverDynamic(ReceiverObservationFailure<razer_device::mouse_dynamic::Report>),
    ReceiverRotation(ReceiverObservationFailure<razer_device::mouse_rotation::Report>),
    ReceiverScroll(ReceiverObservationFailure<razer_device::mouse_scroll::SubmissionReport>),
    ReceiverKeyboardBrightnessRead(
        ReceiverObservationFailure<razer_device::keyboard_settings::KeyboardBrightness>,
    ),
    ReceiverKeyboardBrightnessWrite(
        ReceiverObservationFailure<razer_device::keyboard_settings::KeyboardBrightnessWriteResult>,
    ),
    ReceiverScope(ReceiverObservationFailure<()>),
    OnboardMacro {
        phase: razer_device::onboard_macro::MacroFailurePhase,
        failed_step: Option<usize>,
        report: razer_device::onboard_macro::MacroWriteReport,
    },
    ReceiverExchange(razer_device::ReceiverExchangeFailure),
    ReceiverFrameSubmission(razer_device::ReceiverFrameSubmissionFailure),
    HanboInput(razer_device::hanbo_cooler::CompletedHanboInput),
    ControllerFrameSubmission(razer_device::controller_lighting::software::FrameSubmissionFailure),
    FeatureExchange(razer_device::CompletedFeatureExchange),
    DeviceRead(razer_device::device_query::CompletedDeviceRead),
}

impl FailureDiagnostics {
    pub fn from_error(error: &anyhow::Error) -> Option<Self> {
        if let Some(failure) = error.downcast_ref::<ServiceFailure>() {
            return Some(failure.diagnostics.clone());
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::device_reads::DeviceReadValue>,
        >() {
            return Some(Self::ReceiverDeviceRead(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::device_writes::DeviceWriteResult>,
        >() {
            return Some(Self::ReceiverDeviceWrite(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::mouse_receiver_polling::PollingWriteResult>,
        >() {
            return Some(Self::ReceiverPollingWrite(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::mouse_dpi_stages::DpiStagesReading>,
        >() {
            return Some(Self::ReceiverDpiStagesRead(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::mouse_dpi_stages::DpiStagesWriteResult>,
        >() {
            return Some(Self::ReceiverDpiStagesWrite(failure.clone()));
        }
        if let Some(failure) = error
            .downcast_ref::<ReceiverObservationFailure<razer_device::mouse_current_dpi::DpiReport>>(
            )
        {
            return Some(Self::ReceiverDpi(failure.clone()));
        }
        if let Some(failure) = error
            .downcast_ref::<ReceiverObservationFailure<razer_device::mouse_current_power::Report>>()
        {
            return Some(Self::ReceiverPower(failure.clone()));
        }
        if let Some(failure) = error
            .downcast_ref::<ReceiverObservationFailure<razer_device::mouse_brightness::TaskReport>>(
            )
        {
            return Some(Self::ReceiverBrightness(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<ReceiverObservationFailure<razer_device::mouse_dynamic::Report>>()
        {
            return Some(Self::ReceiverDynamic(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<ReceiverObservationFailure<razer_device::mouse_rotation::Report>>()
        {
            return Some(Self::ReceiverRotation(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::mouse_scroll::SubmissionReport>,
        >() {
            return Some(Self::ReceiverScroll(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<
            ReceiverObservationFailure<razer_device::keyboard_settings::KeyboardBrightness>,
        >() {
            return Some(Self::ReceiverKeyboardBrightnessRead(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<ReceiverObservationFailure<
            razer_device::keyboard_settings::KeyboardBrightnessWriteResult,
        >>() {
            return Some(Self::ReceiverKeyboardBrightnessWrite(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<ReceiverObservationFailure<()>>() {
            return Some(Self::ReceiverScope(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_current_polling::RequestFailure>()
        {
            return Some(Self::MousePolling(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_current_polling::SlotWriteFailure>()
        {
            return Some(Self::MousePollingSlots(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::mouse_current_dpi::WriteFailure>()
        {
            return Some(Self::MouseDpi(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_current_power::WriteFailure>()
        {
            return Some(Self::MousePower(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_receiver_legacy::WriteFailure>()
        {
            return Some(Self::MouseReceiverIndicator(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_receiver_polling::PollingWriteFailure>()
        {
            return Some(Self::MouseReceiverPolling(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_dpi_stages::DpiStagesWriteFailure>()
        {
            return Some(Self::MouseDpiStages(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::mouse_dpi_stages::DpiSlotSynchronizationFailure>()
        {
            return Some(Self::MouseDpiSlots(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::mouse_scroll::SubmissionFailure>()
        {
            return Some(Self::MouseScroll(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::mouse_dynamic::WriteFailure>() {
            return Some(Self::MouseDynamic(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::device_writes::DeviceWriteFailure>()
        {
            return Some(Self::DeviceWrite(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::keyboard_settings::KeyboardBrightnessWriteFailure>()
        {
            return Some(Self::KeyboardBrightness(failure.clone()));
        }
        if let Some(report) = razer_device::keyboard_indicator::failed_report(error) {
            return Some(Self::KeyboardIndicator(report.clone()));
        }
        if let Some(chunks) = razer_device::keyboard_latest_protocol::failed_write(error) {
            return Some(Self::KeyboardChunks(chunks.to_vec()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::seiren_eq::ExecutionFailure>() {
            return Some(Self::SeirenEq(failure.progress.clone()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::leviathan::ExecutionFailure>() {
            return Some(Self::Leviathan(failure.progress.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer_hardware::HardwareEventsFailure>()
        {
            return Some(Self::MixerHardwareEvents(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer_local::CommandBatchFailure>()
        {
            return Some(Self::MixerCommandBatch(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::audio_mixer::MicEqFailure>() {
            return Some(Self::MixerMicEq(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer::DriverCompletionFailure>()
        {
            return Some(Self::MixerDriverCompletion(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer::StreamsResetFailure>()
        {
            return Some(Self::MixerStreamsReset(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer::MixerObservationFailure>()
        {
            return Some(Self::MixerObservation(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer::MixerSubmissionFailure>()
        {
            return Some(Self::MixerSubmission(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer_local::lighting::SampleFailure>()
        {
            return Some(Self::MixerLightingSample(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer::MixerExchangeFailure>()
        {
            return Some(Self::MixerExchange(failure.progress.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer_local::TransferFailure>()
        {
            return Some(Self::MixerTransfer(failure.progress.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer_local::CommandFailure>()
        {
            return Some(Self::MixerCommand(failure.completed.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::audio_mixer_hardware::InitialReadFailure>()
        {
            return Some(Self::MixerHardware(failure.snapshot.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::onboard_macro::MacroWriteFailure>()
        {
            return Some(Self::OnboardMacro {
                phase: failure.phase,
                failed_step: failure.failed_step,
                report: failure.progress.clone(),
            });
        }
        if let Some(failure) = error.downcast_ref::<razer_device::ReceiverExchangeFailure>() {
            return Some(Self::ReceiverExchange(failure.clone()));
        }
        if let Some(failure) = error.downcast_ref::<razer_device::ReceiverFrameSubmissionFailure>()
        {
            return Some(Self::ReceiverFrameSubmission(failure.clone()));
        }
        if let Some(failure) =
            error.downcast_ref::<razer_device::hanbo_cooler::CompletedHanboInput>()
        {
            return Some(Self::HanboInput(failure.clone()));
        }
        if let Some(failure) = error
            .downcast_ref::<razer_device::controller_lighting::software::FrameSubmissionFailure>(
        ) {
            return Some(Self::ControllerFrameSubmission(failure.clone()));
        }
        if let Some(acknowledged) =
            error.downcast_ref::<razer_device::mouse_dpi_stages::DpiStagesAcknowledgement>()
        {
            return Some(Self::MouseDpiStageAcknowledgement(acknowledged.clone()));
        }
        if let Some(completed) = razer_device::device_query::CompletedDeviceRead::from_error(error)
        {
            return Some(Self::DeviceRead(completed.clone()));
        }
        if let Some(completed) = razer_device::CompletedFeatureExchange::from_error(error) {
            return Some(Self::FeatureExchange(completed.clone()));
        }
        None
    }
}

/// The response failed. Its diagnostics cannot become current device state.
#[derive(Debug)]
pub struct ServiceFailure {
    pub message: String,
    pub diagnostics: FailureDiagnostics,
}

impl std::fmt::Display for ServiceFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ServiceFailure {}

pub(crate) fn retain(
    error: anyhow::Error,
    diagnostics: Option<FailureDiagnostics>,
) -> anyhow::Error {
    match diagnostics {
        Some(diagnostics) => {
            let message = format!("{error:#}");
            error.context(ServiceFailure {
                message,
                diagnostics,
            })
        }
        None => error,
    }
}

#[cfg(test)]
pub(crate) fn sample() -> FailureDiagnostics {
    FailureDiagnostics::MixerHardware(razer_device::audio_mixer_hardware::Snapshot {
        keys: [true, false, true, false, true, false],
        channel_mutes: [false, true, false, true],
        mic_muted: true,
        mic_long_press: false,
        sliders: [17, 29, 43, 61],
        phantom_power: true,
        mic_plugged: false,
        line_in_plugged: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponseEnvelope;

    fn failed_wire(bytes: &[u8]) -> ResponseEnvelope {
        let response: ResponseEnvelope = serde_json::from_slice(bytes).unwrap();
        response.validate_outcome().unwrap();
        assert!(response.data.is_none());
        assert!(response.error.is_some());
        assert_eq!(serde_json::to_vec(&response).unwrap(), bytes);
        let mut invalid_success: ResponseEnvelope = serde_json::from_slice(bytes).unwrap();
        invalid_success.error = None;
        assert!(invalid_success.validate_outcome().is_err());
        invalid_success.data = Some(serde_json::json!({"accepted":true}));
        assert!(invalid_success.validate_outcome().is_err());
        response
    }

    #[test]
    fn mouse_write_diagnostic_has_fixed_typed_bytes_and_never_becomes_success_data() {
        let bytes = br#"{"id":21,"data":null,"error":"release failed","fatal":false,"diagnostics":{"domain":"device_write","progress":{"requested":{"kind":"polling","hz":1000},"previous":null,"acknowledged":{"command":[2,0,14],"data":[7,8]},"setter_reply":null,"observed":null}}}"#;
        let response = failed_wire(bytes);
        let Some(FailureDiagnostics::DeviceWrite(progress)) = response.diagnostics else {
            panic!("mouse setter progress lost its domain type")
        };
        assert_eq!(progress.acknowledged.as_ref().unwrap().data, [7, 8]);
        assert!(progress.observed.is_none());
        let error = anyhow::anyhow!("release failed").context(progress);
        assert!(matches!(
            FailureDiagnostics::from_error(&error),
            Some(FailureDiagnostics::DeviceWrite(_))
        ));
    }

    #[test]
    fn keyboard_write_diagnostic_has_fixed_typed_bytes_and_no_successful_readback() {
        let bytes = br#"{"id":22,"data":null,"error":"release failed","fatal":false,"diagnostics":{"domain":"keyboard_brightness","progress":{"requested_percent":50,"acknowledged":{"profile_id":1,"region_id":0,"raw_value":127,"percent":50},"observed":null}}}"#;
        let response = failed_wire(bytes);
        let Some(FailureDiagnostics::KeyboardBrightness(progress)) = response.diagnostics else {
            panic!("keyboard setter progress lost its domain type")
        };
        assert_eq!(progress.acknowledged.as_ref().unwrap().raw_value, 127);
        assert!(progress.observed.is_none());
        let error = anyhow::anyhow!("release failed").context(progress);
        assert!(matches!(
            FailureDiagnostics::from_error(&error),
            Some(FailureDiagnostics::KeyboardBrightness(_))
        ));
    }

    #[test]
    fn receiver_post_peer_failure_keeps_ack_and_old_readback_only_in_typed_diagnostics() {
        let bytes = br#"{"id":23,"data":null,"error":"peer changed","fatal":false,"diagnostics":{"domain":"receiver_device_write","progress":{"reading":{"requested":{"kind":"idle","raw_time":120},"previous":null,"observed":{"kind":"idle","raw_time":120},"acknowledged":{"command":[2,7,3],"data":[7,8]},"changed":true,"verified":true},"receiver_before":[[183,1]],"receiver_after":[[183,0]]}}}"#;
        let response = failed_wire(bytes);
        let Some(FailureDiagnostics::ReceiverDeviceWrite(progress)) = response.diagnostics else {
            panic!("receiver failure lost its complete inner write progress")
        };
        assert_eq!(
            progress.receiver_after.as_deref(),
            Some([(183, 0)].as_slice())
        );
        assert_eq!(progress.reading.acknowledged.as_ref().unwrap().data, [7, 8]);
        let error = anyhow::anyhow!("peer changed").context(progress);
        assert!(matches!(
            FailureDiagnostics::from_error(&error),
            Some(FailureDiagnostics::ReceiverDeviceWrite(_))
        ));
    }

    #[test]
    fn mixer_aggregate_keeps_completed_commands_and_failed_exchange_separate_from_success() {
        let bytes = br#"{"id":24,"data":null,"error":"write failed","fatal":false,"diagnostics":{"domain":"mixer_mic_eq","progress":{"progress":{"requested_bands":[0,1,2,3,4,5,6,7,8,9],"results":[{"requested":{"kind":"boolean","enabled":true},"transport_completed":true}],"completed_bands":[]},"failed_exchange":{"outputs":[{"report":[6,1],"completed":false}],"inputs":[]}}}}"#;
        let response = failed_wire(bytes);
        let Some(FailureDiagnostics::MixerMicEq(progress)) = response.diagnostics else {
            panic!("Mixer aggregate lost its complete command progress")
        };
        assert_eq!(progress.progress.results.len(), 1);
        assert!(progress.progress.completed_bands.is_empty());
        let failed_exchange = progress.failed_exchange.as_ref().unwrap();
        assert_eq!(failed_exchange.outputs[0].report, [6, 1]);
        assert!(!failed_exchange.outputs[0].completed);
        let error = anyhow::anyhow!("write failed")
            .context(razer_device::audio_mixer::MixerExchangeFailure {
                progress: failed_exchange.clone(),
            })
            .context(progress);
        assert!(matches!(
            FailureDiagnostics::from_error(&error),
            Some(FailureDiagnostics::MixerMicEq(_))
        ));
    }

    #[test]
    fn complete_domain_progress_takes_precedence_over_an_inner_scalar_receipt() {
        let acknowledged = razer_device::mouse_dpi_stages::DpiStagesAcknowledgement {
            response_bytes: 2,
            raw_data: vec![7, 8],
        };
        let scalar_error = anyhow::anyhow!("release failed").context(acknowledged.clone());
        assert!(matches!(
            FailureDiagnostics::from_error(&scalar_error),
            Some(FailureDiagnostics::MouseDpiStageAcknowledgement(_))
        ));
        let domain = razer_device::mouse_dpi_stages::DpiStagesWriteFailure {
            requested: razer_device::mouse_dpi_stages::DpiStagesDraft {
                enabled: true,
                active_stage: 1,
                stages: vec![razer_device::mouse_dpi_stages::DpiStage {
                    x: 800,
                    y: 800,
                    independent: false,
                    visible: true,
                }],
            },
            packed: razer_device::mouse_dpi_stages::PackedDpiStages {
                active_stage: 1,
                stages: vec![razer_device::mouse_dpi_stages::PackedDpiStage {
                    index: 0,
                    x: 800,
                    y: 800,
                    z: 0,
                }],
            },
            acknowledged: Some(acknowledged),
            observed: None,
        };
        let error = scalar_error.context(domain);
        assert!(matches!(
            FailureDiagnostics::from_error(&error),
            Some(FailureDiagnostics::MouseDpiStages(_))
        ));
        let retained = retain(error, Some(sample()));
        assert!(matches!(
            FailureDiagnostics::from_error(&retained),
            Some(FailureDiagnostics::MixerHardware(_))
        ));
    }

    #[test]
    fn diagnostic_wire_bytes_keep_failure_separate_from_successful_data() {
        let bytes = br#"{"id":17,"data":null,"error":"release failed","fatal":false,"diagnostics":{"domain":"mixer_hardware","progress":{"keys":[true,false,true,false,true,false],"channel_mutes":[false,true,false,true],"mic_muted":true,"mic_long_press":false,"sliders":[17,29,43,61],"phantom_power":true,"mic_plugged":false,"line_in_plugged":true}}}"#;
        let response: ResponseEnvelope = serde_json::from_slice(bytes).unwrap();
        response.validate_outcome().unwrap();
        assert!(response.data.is_none());
        let Some(FailureDiagnostics::MixerHardware(snapshot)) = &response.diagnostics else {
            panic!("typed snapshot was not retained")
        };
        assert_eq!(snapshot.sliders, [17, 29, 43, 61]);
        assert_eq!(serde_json::to_vec(&response).unwrap(), bytes);
        let mut success = response;
        success.error = None;
        success.data = Some(serde_json::json!({"accepted":true}));
        assert!(success.validate_outcome().is_err());
        for invalid in [
            r#"{"domain":"unknown","progress":{}}"#,
            r#"{"domain":"mixer_hardware","progress":{}}"#,
            r#"{"domain":"mixer_hardware"}"#,
            r#"{"domain":"mixer_hardware","progress":null}"#,
            r#"{"progress":{"sliders":[1,2,3,4]}}"#,
        ] {
            assert!(serde_json::from_str::<FailureDiagnostics>(invalid).is_err());
        }
    }

    #[test]
    fn typed_diagnostics_survive_error_context_cleanup_and_worker_log_attachment() {
        let diagnostics = crate::worker_diagnostics::WorkerDiagnostics::default();
        diagnostics.record_stdout(b"actual worker log").unwrap();
        let error = retain(anyhow::anyhow!("actual restore failure"), Some(sample()));
        let error = crate::with_cleanup_error(error, Err(anyhow::anyhow!("close also failed")));
        let error = diagnostics.attach(error);
        assert!(format!("{error:#}").contains("actual restore failure"));
        assert!(format!("{error:#}").contains("close also failed"));
        assert!(format!("{error:#}").contains("actual worker log"));
        let failure = error.downcast_ref::<ServiceFailure>().unwrap();
        let FailureDiagnostics::MixerHardware(snapshot) = &failure.diagnostics else {
            panic!("typed snapshot was lost")
        };
        assert_eq!(snapshot.sliders, [17, 29, 43, 61]);
    }
}
