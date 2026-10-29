//! Per-client service state retained by the shared headless process.
use super::{portable, shutdown};
use razer_ipc::{RequestEnvelope, ResponseEnvelope, ServiceRequest};

pub(super) struct SessionRuntime {
    closing: bool,
    #[cfg(windows)]
    native: super::native::NativeRuntime,
    portable: portable::PortableRuntime,
    host_storage: razer_storage::host::HostStorage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    struct RetryEvents(Arc<AtomicUsize>);
    impl razer_device::receiver_pairing::HardwareEvents for RetryEvents {
        fn begin(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn receive(&mut self, _: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(None)
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            if self.0.fetch_add(1, Ordering::AcqRel) == 0 {
                anyhow::bail!("retained event cleanup failed");
            }
            Ok(())
        }
    }

    #[test]
    fn failed_operation_keeps_typed_diagnostics_out_of_success_data() {
        let snapshot = razer_device::audio_mixer_hardware::Snapshot {
            keys: [false; 6],
            channel_mutes: [true, false, true, false],
            mic_muted: true,
            mic_long_press: false,
            sliders: [17, 29, 43, 61],
            phantom_power: false,
            mic_plugged: true,
            line_in_plugged: false,
        };
        let error = anyhow::anyhow!("actual exchange release failed").context(
            razer_device::audio_mixer_hardware::InitialReadFailure {
                snapshot: snapshot.clone(),
            },
        );
        let response = response_from_result(17, Err(error));
        response.validate_outcome().unwrap();
        assert!(response.data.is_none());
        assert!(
            response
                .error
                .as_deref()
                .unwrap()
                .contains("actual exchange release failed")
        );
        let Some(razer_ipc::FailureDiagnostics::MixerHardware(retained)) = response.diagnostics
        else {
            panic!("server discarded the measured failure diagnostic")
        };
        assert_eq!(retained, snapshot);
    }

    #[test]
    fn close_stops_new_requests_while_retrying_the_same_event_owner() {
        let mut session = SessionRuntime::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        session
            .portable
            .simulated_pending_events(Box::new(RetryEvents(attempts.clone())));
        assert!(matches!(
            session.close_progress(1),
            shutdown::CleanupProgress::Pending(_)
        ));
        let rejected = session.request(RequestEnvelope {
            id: 2,
            request: ServiceRequest::HostStorageClose { view_id: 19 },
        });
        assert!(rejected.data.is_none());
        assert!(rejected.error.unwrap().contains("no new work"));
        let shutdown::CleanupProgress::Released(response) = session.close_progress(3) else {
            panic!("the same owner was released by the second finish");
        };
        assert!(
            response
                .error
                .unwrap()
                .contains("retained event cleanup failed")
        );
        assert_eq!(attempts.load(Ordering::Acquire), 2);
    }

    #[test]
    fn terminal_submission_failure_never_becomes_success_on_the_next_close() {
        let mut session = SessionRuntime::new();
        let (portable, exited) = portable::cleanup_tests::failing_submission_runtime();
        session.portable = portable;
        for id in [4, 5] {
            let shutdown::CleanupProgress::Released(response) = session.close_progress(id) else {
                panic!("the real submission worker has already been joined");
            };
            assert!(exited.load(Ordering::Acquire));
            assert!(response.data.is_none());
            assert!(
                response
                    .error
                    .unwrap()
                    .contains("submission restore confirmation failed")
            );
        }
    }
}

impl SessionRuntime {
    pub(super) fn new() -> Self {
        Self {
            closing: false,
            #[cfg(windows)]
            native: super::native::NativeRuntime::new(),
            portable: portable::PortableRuntime::default(),
            host_storage: razer_storage::host::HostStorage::default(),
        }
    }

    pub(super) fn close(&mut self, id: u64) -> ResponseEnvelope {
        self.close_progress(id).into_response()
    }

    pub(super) fn close_progress(&mut self, id: u64) -> shutdown::CleanupProgress {
        self.closing = true;
        let mut cleanup = shutdown::Cleanup::default();
        #[cfg(windows)]
        cleanup.merge(self.native.shutdown_cleanup());
        cleanup.merge(self.portable.shutdown());
        cleanup.progress(id)
    }

    pub(super) fn request(&mut self, envelope: RequestEnvelope) -> ResponseEnvelope {
        if matches!(envelope.request, ServiceRequest::Shutdown) {
            return self.close(envelope.id);
        }
        if self.closing {
            return ResponseEnvelope {
                diagnostics: None,
                id: envelope.id,
                data: None,
                error: Some("Service session is closing; no new work is admitted".to_owned()),
                fatal: false,
            };
        }
        let result = match envelope.request {
            ServiceRequest::HelpSystemInfo { product_id, query } => {
                crate::help_system_info::query(product_id, query)
                    .and_then(|response| serde_json::to_value(response).map_err(Into::into))
            }
            ServiceRequest::KeyboardLayoutRead => razer_platform::keyboard_layout::get()
                .map(|layout| serde_json::json!({"layout":layout})),
            ServiceRequest::WindowsServiceStart { name } => {
                razer_platform::windows_service_status::start(&name)
                    .map(|exit_code| serde_json::json!({"name":name,"exit_code":exit_code}))
            }
            ServiceRequest::WindowsServiceStop { name } => {
                razer_platform::windows_service_status::stop(&name)
                    .map(|exit_code| serde_json::json!({"name":name,"exit_code":exit_code}))
            }
            ServiceRequest::WindowsServiceStatus { name } => {
                razer_platform::windows_service_status::query(&name)
                    .map(|status| serde_json::json!({"name":name,"status":status}))
            }
            ServiceRequest::WheelScrollLinesRead => {
                razer_platform::wheel_scroll::get().map(|lines| serde_json::json!({"lines":lines}))
            }
            ServiceRequest::WheelScrollLinesWrite { lines } => razer_platform::wheel_scroll::set(
                lines,
            )
            .map(|accepted| serde_json::json!({"requested_lines":lines,"accepted":accepted})),
            ServiceRequest::HostStorageView { view } => {
                self.host_storage.register_view(view);
                Ok(serde_json::json!({"registered": true}))
            }
            ServiceRequest::HostStorageClose { view_id } => {
                self.host_storage.close_view(view_id);
                Ok(serde_json::json!({"closed": true}))
            }
            ServiceRequest::HostStorageCall { call } => self
                .host_storage
                .call(call)
                .and_then(|result| serde_json::to_value(result).map_err(Into::into)),
            ServiceRequest::HostStorageEvents { view_id } => self
                .host_storage
                .drain_events(view_id)
                .and_then(|events| serde_json::to_value(events).map_err(Into::into)),
            request @ (ServiceRequest::AudioDevices
            | ServiceRequest::MouseInputMonitoringStart { .. }
            | ServiceRequest::MouseInputMonitoringPoll { .. }
            | ServiceRequest::MouseInputMonitoringStop { .. }
            | ServiceRequest::AudioVolumeRead { .. }
            | ServiceRequest::AudioVolumeWrite { .. }
            | ServiceRequest::AudioEndpoints { .. }
            | ServiceRequest::AudioNotificationsEnable { .. }
            | ServiceRequest::AudioNotificationsDrain
            | ServiceRequest::AudioRoutingEnable { .. }
            | ServiceRequest::AudioRouteDevice { .. }
            | ServiceRequest::AudioRouterEvents
            | ServiceRequest::ForegroundMonitorStart { .. }
            | ServiceRequest::ForegroundMonitorStop { .. }
            | ServiceRequest::ForegroundMonitorEvents { .. }
            | ServiceRequest::HidNodes
            | ServiceRequest::CameraDiscover { .. }
            | ServiceRequest::CameraStart { .. }
            | ServiceRequest::CameraTaskStart { .. }
            | ServiceRequest::CameraPoll { .. }
            | ServiceRequest::CameraCancel { .. }
            | ServiceRequest::ThxStart { .. }
            | ServiceRequest::ThxOwnerStart { .. }
            | ServiceRequest::ThxOwnerSubmit { .. }
            | ServiceRequest::ThxOwnerRelease { .. }
            | ServiceRequest::ThxPoll { .. }
            | ServiceRequest::ThxCancel { .. }
            | ServiceRequest::HidNodeReports { .. }
            | ServiceRequest::ControllerTriggers { .. }
            | ServiceRequest::ControllerLighting { .. }
            | ServiceRequest::MarciLighting { .. }
            | ServiceRequest::ControllerSleep { .. }
            | ServiceRequest::ControllerSocd { .. }
            | ServiceRequest::ControllerInitialization { .. }
            | ServiceRequest::ControllerTriggerCalibrationStart { .. }
            | ServiceRequest::ControllerTriggerCalibrationPoll { .. }
            | ServiceRequest::ControllerTriggerCalibrationCancel { .. }
            | ServiceRequest::HidNodeDpiStagesRead { .. }
            | ServiceRequest::HidNodeDpiStagesWrite { .. }
            | ServiceRequest::HidNodeMouseCurrentDpi { .. }
            | ServiceRequest::HidNodeMouseBrightness { .. }
            | ServiceRequest::HidNodeMouseDynamic { .. }
            | ServiceRequest::HidNodeMouseCurrentPolling { .. }
            | ServiceRequest::HidNodeMouseCurrentPower { .. }
            | ServiceRequest::HidNodeMouseScroll { .. }
            | ServiceRequest::MouseTrackingStart { .. }
            | ServiceRequest::MouseTrackingPoll { .. }
            | ServiceRequest::MouseTrackingCancel { .. }
            | ServiceRequest::MousePhilipsStart { .. }
            | ServiceRequest::MousePhilipsPoll { .. }
            | ServiceRequest::MousePhilipsCancel { .. }
            | ServiceRequest::HidNodeLeviathan { .. }
            | ServiceRequest::LeviathanEventsStart { .. }
            | ServiceRequest::LeviathanEventsPoll { .. }
            | ServiceRequest::LeviathanEventsStop { .. }
            | ServiceRequest::HidNodeMouseIndicatorLed { .. }
            | ServiceRequest::MouseIndicatorLedStart { .. }
            | ServiceRequest::MouseIndicatorLedPoll { .. }
            | ServiceRequest::MouseIndicatorLedStop { .. }
            | ServiceRequest::HidNodeSeirenEq { .. }
            | ServiceRequest::KeyboardIndicator { .. }
            | ServiceRequest::KeyboardLatestRestore { .. }
            | ServiceRequest::KeyboardLegacyCalibration { .. }
            | ServiceRequest::MouseReceiverLegacy { .. }
            | ServiceRequest::HyperpollingReceiver { .. }
            | ServiceRequest::KeyboardAnalogStart { .. }
            | ServiceRequest::KeyboardAnalogPoll { .. }
            | ServiceRequest::KeyboardAnalogCancel { .. }
            | ServiceRequest::KeyboardWristrestStart { .. }
            | ServiceRequest::KeyboardWristrestPoll { .. }
            | ServiceRequest::KeyboardWristrestStop { .. }
            | ServiceRequest::MouseRuntimeArtworkStart { .. }
            | ServiceRequest::MouseRuntimeArtworkPoll { .. }
            | ServiceRequest::MouseRuntimeArtworkStop { .. }
            | ServiceRequest::SeirenAudioCaptureStart { .. }
            | ServiceRequest::SeirenAudioCapturePoll { .. }
            | ServiceRequest::SeirenAudioCaptureStop { .. }
            | ServiceRequest::SeirenAudioPlaybackStart { .. }
            | ServiceRequest::SeirenAudioPlaybackPoll { .. }
            | ServiceRequest::SeirenAudioPlaybackStop { .. }
            | ServiceRequest::HidNodeRead { .. }
            | ServiceRequest::HidNodeMouseReceiverRead { .. }
            | ServiceRequest::HidNodeMouseReceiverDpiStages { .. }
            | ServiceRequest::HidNodeMouseReceiverPolling { .. }
            | ServiceRequest::HidNodeMouseReceiverIdle { .. }
            | ServiceRequest::HidNodeWrite { .. }
            | ServiceRequest::HidNodeHanboCooler { .. }
            | ServiceRequest::HidNodeHanboCoolerRead { .. }
            | ServiceRequest::HidNodeHanboStartupRead { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessRead { .. }
            | ServiceRequest::HidNodeKeyboardReceiverBrightness { .. }
            | ServiceRequest::HidNodeKeyboardMetadataRead { .. }
            | ServiceRequest::KeyboardProfileInventoryRead { .. }
            | ServiceRequest::KeyboardOnboardProfilesRead { .. }
            | ServiceRequest::HidNodeKeyboardSerialRead { .. }
            | ServiceRequest::HidNodeKeyboardRelayMetadataRead { .. }
            | ServiceRequest::HidNodeKeyboardRelaySerialRead { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessWrite { .. }
            | ServiceRequest::HidNodeMixerRead { .. }
            | ServiceRequest::HidNodeMixerLocalWrite { .. }
            | ServiceRequest::HidNodeMixerWrite { .. }
            | ServiceRequest::HidNodeMixerEqWrite { .. }
            | ServiceRequest::HidNodeMixerRouteRead { .. }
            | ServiceRequest::HidNodeMixerRouteWrite { .. }
            | ServiceRequest::HidNodeMixerRestartStreams { .. }
            | ServiceRequest::HidNodeReceiverStatus { .. }
            | ServiceRequest::HidNodeReceiverStatusForSource { .. }
            | ServiceRequest::HidNodeReceiverDockMode { .. }) => self.portable.request(request),
            request => {
                #[cfg(windows)]
                {
                    self.native.request(request)
                }
                #[cfg(not(windows))]
                {
                    match request {
                        ServiceRequest::Shutdown => Ok(serde_json::json!({"stopped":true})),
                        _ => Err(anyhow::anyhow!(
                            "此操作需要尚未移植的平台服务或设备身份适配"
                        )),
                    }
                }
            }
        };
        response_from_result(envelope.id, result)
    }
}

fn response_from_result(id: u64, result: anyhow::Result<serde_json::Value>) -> ResponseEnvelope {
    match result {
        Ok(data) => ResponseEnvelope {
            diagnostics: None,
            id,
            data: Some(data),
            error: None,
            fatal: false,
        },
        Err(error) => ResponseEnvelope {
            id,
            data: None,
            error: Some(format!("{error:#}")),
            diagnostics: razer_ipc::FailureDiagnostics::from_error(&error),
            fatal: false,
        },
    }
}
