use super::*;
use razer_device::{simple_audio::EndpointType, simple_audio_volume::AudioVolumeWriteResponse};
use std::sync::atomic::AtomicUsize;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Enumerate,
    SystemRead,
    Hardware(Option<bool>),
    SystemWrite(bool, u8),
    Close,
}
struct Fake {
    calls: Vec<Call>,
    volume: u8,
    system_muted: bool,
    hardware_muted: bool,
    system_read_failure: bool,
    hardware_failure: bool,
    write_failure: bool,
    missing_readback: bool,
    cleanup_failure: bool,
    cancel_after_hardware: Option<Arc<AtomicBool>>,
    closes: Arc<AtomicUsize>,
    close_failures: usize,
}
impl Fake {
    fn new() -> Self {
        Self {
            calls: vec![],
            volume: 37,
            system_muted: true,
            hardware_muted: true,
            system_read_failure: false,
            hardware_failure: false,
            write_failure: false,
            missing_readback: false,
            cleanup_failure: false,
            cancel_after_hardware: None,
            closes: Arc::new(AtomicUsize::new(0)),
            close_failures: 0,
        }
    }
    fn reading(&self, id: &str) -> AudioVolumeReadResult {
        AudioVolumeReadResult {
            result: true,
            reason: String::new(),
            device_id: id.into(),
            muted: self.system_muted,
            volume: self.volume,
        }
    }
}
impl Backend for Fake {
    fn enumerate(&mut self) -> Result<Vec<SimpleAudioEndpoint>> {
        self.calls.push(Call::Enumerate);
        Ok(vec![SimpleAudioEndpoint {
            id: "capture".into(),
            name: "Headset microphone".into(),
            container_id: "CONTAINER".into(),
            endpoint_type: EndpointType::Microphone,
        }])
    }
    fn read(&mut self, id: &str) -> Result<AudioVolumeReadResult> {
        self.calls.push(Call::SystemRead);
        ensure!(!self.system_read_failure, "system read failed");
        Ok(self.reading(id))
    }
    fn write(&mut self, id: &str, muted: bool, volume: u8) -> Result<AudioVolumeWriteResult> {
        self.calls.push(Call::SystemWrite(muted, volume));
        let previous = self.reading(id);
        self.volume = volume;
        self.system_muted = muted;
        ensure!(!self.write_failure, "OS transport failed after submission");
        Ok(AudioVolumeWriteResult {
            source_response: AudioVolumeWriteResponse {
                result: true,
                reason: String::new(),
            },
            previous,
            observation: (!self.missing_readback).then(|| self.reading(id)),
            observation_error: self.missing_readback.then(|| "readback failed".into()),
            volume_submitted: true,
            mute_submitted: false,
            hresult: 0,
        })
    }
    fn hardware(&mut self, muted: Option<bool>) -> HardwareStep {
        self.calls.push(Call::Hardware(muted));
        let write = if let Some(muted) = muted {
            if self.hardware_failure {
                WriteOutcome::Failed("hardware mute failed".into())
            } else if muted == self.hardware_muted {
                WriteOutcome::Skipped
            } else {
                self.hardware_muted = muted;
                WriteOutcome::Confirmed
            }
        } else {
            WriteOutcome::Skipped
        };
        let report = serde_json::from_value(serde_json::json!({
            "observation":{"Observed":self.hardware_muted}, "write":write,
        }))
        .unwrap();
        if let Some(active) = self.cancel_after_hardware.take() {
            active.store(false, Ordering::Release);
        }
        HardwareStep {
            report: Ok(report),
            cleanup_error: self.cleanup_failure.then(|| "HID cleanup failed".into()),
        }
    }
    fn changed(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn close(&mut self) -> Result<()> {
        self.calls.push(Call::Close);
        self.closes.fetch_add(1, Ordering::AcqRel);
        if self.close_failures > 0 {
            self.close_failures -= 1;
            anyhow::bail!("COM unregistration failed");
        }
        Ok(())
    }
}
fn session(pid: u32) -> Session<Fake> {
    Session {
        scope: Scope::new(pid, "CONTAINER".into()).unwrap(),
        endpoint: None,
        backend: Fake::new(),
    }
}

#[test]
fn all_earbuds_keep_hardware_mute_and_system_mute_independent() {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let mut session = session(pid);
        session.backend.hardware_muted = false;
        let update = session.observe();
        let snapshot = update.observation().as_ref().unwrap();
        assert_eq!(snapshot.setting(), Some(Setting::new(true, 37).unwrap()));
        assert!(!snapshot.system().as_ref().unwrap().setting().enabled());
        assert_eq!(
            session.backend.calls,
            [Call::Enumerate, Call::SystemRead, Call::Hardware(None)]
        );
    }
}

#[test]
fn microphone_edit_changes_device_mute_then_preserves_os_mute_while_setting_volume() {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let mut session = session(pid);
        let update = session.edit(Setting::new(true, 83).unwrap(), &AtomicBool::new(true));
        assert!(update.error().is_none());
        assert_eq!(
            session.backend.calls,
            [
                Call::Enumerate,
                Call::Hardware(Some(false)),
                Call::SystemRead,
                Call::SystemWrite(true, 83),
                Call::Enumerate
            ]
        );
        assert_eq!(
            update.observation().as_ref().unwrap().setting(),
            Some(Setting::new(true, 83).unwrap())
        );
        assert_eq!(update.hardware_write(), Some(&WriteOutcome::Confirmed));
        assert!(session.backend.system_muted);
    }
}

#[test]
fn partial_hardware_failure_does_not_hide_successful_system_volume_change() {
    let mut session = session(2657);
    session.backend.hardware_failure = true;
    let update = session.edit(Setting::new(true, 83).unwrap(), &AtomicBool::new(true));
    let snapshot = update.observation().as_ref().unwrap();
    assert_eq!(snapshot.hardware_mute(), &ToggleObservation::Observed(true));
    assert_eq!(snapshot.system().as_ref().unwrap().setting().volume(), 83);
    assert_eq!(snapshot.setting(), Some(Setting::new(false, 83).unwrap()));
    assert!(update.error().unwrap().contains("hardware mute failed"));
}

#[test]
fn failed_or_missing_os_readback_never_reuses_prewrite_volume_as_confirmation() {
    for missing in [true, false] {
        let mut session = session(2689);
        session.backend.missing_readback = missing;
        session.backend.write_failure = !missing;
        let update = session.edit(Setting::new(true, 64).unwrap(), &AtomicBool::new(true));
        let snapshot = update.observation().as_ref().unwrap();
        assert!(snapshot.system().is_err());
        assert_eq!(
            snapshot.hardware_mute(),
            &ToggleObservation::Observed(false)
        );
        assert!(snapshot.setting().is_none());
        assert!(update.error().is_some());
        assert_eq!(session.backend.volume, 64);
    }
}

#[test]
fn read_failure_and_hid_cleanup_failure_retain_the_other_observed_field() {
    let mut session = session(2660);
    session.backend.system_read_failure = true;
    let update = session.observe();
    assert!(update.observation().as_ref().unwrap().system().is_err());
    assert_eq!(
        update.observation().as_ref().unwrap().hardware_mute(),
        &ToggleObservation::Observed(true)
    );
    session.backend.system_read_failure = false;
    session.backend.cleanup_failure = true;
    let update = session.edit(Setting::new(true, 58).unwrap(), &AtomicBool::new(true));
    assert_eq!(
        update.observation().as_ref().unwrap().setting(),
        Some(Setting::new(true, 58).unwrap())
    );
    assert!(update.error().unwrap().contains("HID cleanup failed"));
}

#[test]
fn cancellation_after_hardware_confirmation_stops_os_submission() {
    let active = Arc::new(AtomicBool::new(true));
    let mut session = session(2664);
    session.backend.cancel_after_hardware = Some(active.clone());
    let update = session.edit(Setting::new(true, 91).unwrap(), &active);
    assert_eq!(
        session.backend.calls,
        [Call::Enumerate, Call::Hardware(Some(false))]
    );
    assert_eq!(
        update.observation().as_ref().unwrap().hardware_mute(),
        &ToggleObservation::Observed(false)
    );
    assert!(update.observation().as_ref().unwrap().system().is_err());
    assert!(update.error().unwrap().contains("cancelled"));
}

#[test]
fn hardware_notice_uses_its_real_mute_and_a_fresh_system_read_without_saving() {
    let mut session = session(2668);
    session.backend.hardware_muted = true;
    let update = session.hardware_notice(0);
    assert_eq!(
        update.observation().as_ref().unwrap().setting(),
        Some(Setting::new(true, 37).unwrap())
    );
    assert_eq!(session.backend.calls, [Call::Enumerate, Call::SystemRead]);
    let update = session.hardware_notice(9);
    assert_eq!(
        update.observation().as_ref().unwrap().hardware_mute(),
        &ToggleObservation::Unknown(9)
    );
    assert!(update.observation().as_ref().unwrap().setting().is_none());
}

#[test]
fn microphone_worker_drains_com_even_when_cancelled_and_retains_cleanup_failures() {
    let mut session = session(2657);
    let closes = session.backend.closes.clone();
    session.backend.close_failures = 2;
    let (_, receiver) = std::sync::mpsc::channel();
    let (events, _) = async_channel::unbounded();
    let error =
        run_session(session, Arc::new(AtomicBool::new(false)), receiver, events).unwrap_err();
    assert_eq!(closes.load(Ordering::Acquire), 3);
    assert!(error.contains("COM unregistration failed"));
}
