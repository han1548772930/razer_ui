use super::*;
use razer_device::earbuds_system_volume::Edit;
use razer_device::simple_audio::EndpointType;
use razer_device::simple_audio_volume::AudioVolumeWriteResponse;

struct Fake {
    endpoints: Vec<SimpleAudioEndpoint>,
    muted: bool,
    volume: u8,
    writes: Vec<(String, bool, u8)>,
    partial: bool,
    reject_volume: bool,
    missing_readback: bool,
    notice: Notifications,
    cancel_after_read: Option<Arc<AtomicBool>>,
    cancel_after_write: Option<Arc<AtomicBool>>,
    replace_after_write: bool,
    close_attempts: Arc<std::sync::atomic::AtomicUsize>,
    close_failures: usize,
}
impl Fake {
    fn new() -> Self {
        Self {
            endpoints: vec![SimpleAudioEndpoint {
                id: "selected-system-id".into(),
                name: "Headphones".into(),
                container_id: "CONTAINER".into(),
                endpoint_type: EndpointType::Speaker,
            }],
            muted: false,
            volume: 37,
            writes: Vec::new(),
            partial: false,
            reject_volume: false,
            missing_readback: false,
            notice: Notifications::default(),
            cancel_after_read: None,
            cancel_after_write: None,
            replace_after_write: false,
            close_attempts: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            close_failures: 0,
        }
    }
}
impl Backend for Fake {
    fn enumerate(&mut self) -> Result<Vec<SimpleAudioEndpoint>> {
        Ok(self.endpoints.clone())
    }
    fn read(&mut self, id: &str) -> Result<AudioVolumeReadResult> {
        if let Some(active) = self.cancel_after_read.take() {
            active.store(false, Ordering::Release);
        }
        Ok(AudioVolumeReadResult {
            result: true,
            reason: String::new(),
            device_id: id.into(),
            muted: self.muted,
            volume: self.volume,
        })
    }
    fn write(&mut self, id: &str, mute: bool, volume: u8) -> Result<AudioVolumeWriteResult> {
        let previous = self.read(id)?;
        self.writes.push((id.into(), mute, volume));
        if !self.reject_volume {
            self.volume = volume;
        }
        if !self.partial && !self.reject_volume {
            self.muted = mute;
        }
        let observation = if self.missing_readback {
            None
        } else {
            Some(self.read(id)?)
        };
        if let Some(active) = self.cancel_after_write.take() {
            active.store(false, Ordering::Release);
        }
        if self.replace_after_write {
            self.endpoints[0].id = "replacement-system-id".into();
        }
        Ok(AudioVolumeWriteResult {
            source_response: AudioVolumeWriteResponse {
                result: !self.partial && !self.reject_volume,
                reason: if self.reject_volume {
                    "volume rejected".into()
                } else if self.partial {
                    "mute rejected".into()
                } else {
                    String::new()
                },
            },
            previous,
            observation,
            observation_error: self.missing_readback.then(|| "readback failed".into()),
            volume_submitted: true,
            mute_submitted: !self.reject_volume,
            hresult: if self.partial || self.reject_volume { -1 } else { 0 },
        })
    }
    fn notifications(&mut self) -> Result<Notifications> {
        Ok(std::mem::take(&mut self.notice))
    }
    fn close(&mut self) -> Result<()> {
        self.close_attempts.fetch_add(1, Ordering::AcqRel);
        if self.close_failures > 0 {
            self.close_failures -= 1;
            anyhow::bail!("unregistration rejected");
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
fn edit(session: &mut Session<Fake>, edit: Edit, active: bool) -> Update {
    session.edit(
        Command {
            scope_id: session.scope.id(),
            request_id: Uuid::new_v4(),
            setting: Setting::new(!session.backend.muted, session.backend.volume)
                .unwrap()
                .edit(edit)
                .unwrap(),
        },
        &AtomicBool::new(active),
    )
}

#[test]
fn opens_without_writing_and_preserves_separate_mute_and_zero_volume_for_all_variants() {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let mut session = session(pid);
        assert_eq!(
            session
                .refresh()
                .observation()
                .as_ref()
                .unwrap()
                .setting()
                .volume(),
            37
        );
        assert!(session.backend.writes.is_empty());
        let result = edit(&mut session, Edit::Volume(0), true);
        assert!(result.error().is_none());
        assert!(result.observation().as_ref().unwrap().setting().enabled());
        assert_eq!(
            session.backend.writes,
            [("selected-system-id".into(), false, 0)]
        );
        assert!(matches!(
            edit(&mut session, Edit::Volume(0), true).mutation(),
            Some(Mutation::Unchanged)
        ));
        assert!(
            edit(&mut session, Edit::Enabled(false), true)
                .error()
                .is_none()
        );
        let result = edit(&mut session, Edit::Volume(88), true);
        assert!(matches!(result.mutation(), Some(Mutation::Unchanged)));
        assert_eq!(result.observation().as_ref().unwrap().setting().volume(), 0);
        assert_eq!(session.backend.writes.len(), 2);
    }
}
#[test]
fn cancelled_and_wrong_session_commands_never_mutate_the_endpoint() {
    let mut session = session(2657);
    assert!(
        edit(&mut session, Edit::Volume(70), false)
            .error()
            .is_some()
    );
    let result = session.edit(
        Command {
            scope_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            setting: Setting::new(false, 37).unwrap(),
        },
        &AtomicBool::new(true),
    );
    assert!(result.error().is_some());
    assert!(session.backend.writes.is_empty());
}

#[test]
fn cancellation_during_read_prevents_write_and_late_cancellation_rejects_confirmation() {
    let mut session = session(2657);
    let active = Arc::new(AtomicBool::new(true));
    session.backend.cancel_after_read = Some(active.clone());
    let result = session.edit(
        Command::new(
            session.scope.id(),
            Uuid::new_v4(),
            Setting::new(true, 79).unwrap(),
        ),
        &active,
    );
    assert!(
        result
            .error()
            .unwrap()
            .contains("cancelled before submission")
    );
    assert!(session.backend.writes.is_empty());
    active.store(true, Ordering::Release);
    session.backend.cancel_after_write = Some(active.clone());
    let result = session.edit(
        Command::new(
            session.scope.id(),
            Uuid::new_v4(),
            Setting::new(true, 79).unwrap(),
        ),
        &active,
    );
    assert!(result.observation().is_err());
    assert!(result.error().unwrap().contains("after cancellation"));
    assert!(matches!(result.mutation(), Some(Mutation::Written(_))));
    assert_eq!(session.backend.volume, 79);
}

#[test]
fn endpoint_replacement_during_write_cannot_confirm_the_new_connection() {
    let mut session = session(2689);
    session.backend.replace_after_write = true;
    let result = edit(&mut session, Edit::Volume(62), true);
    assert!(
        result
            .error()
            .unwrap()
            .contains("changed after volume submission")
    );
    assert!(result.observation().is_err());
    assert_eq!(
        session.backend.writes,
        [("selected-system-id".into(), false, 62)]
    );
}

#[test]
fn cancelled_session_retains_its_owner_until_failed_unregistration_is_released() {
    let mut session = session(2668);
    session.backend.close_failures = 1;
    let attempts = session.backend.close_attempts.clone();
    let (_commands, receiver) = std::sync::mpsc::channel();
    let (events, incoming) = async_channel::unbounded();
    let result = run_session(session, Arc::new(AtomicBool::new(false)), receiver, events);
    assert!(result.unwrap_err().contains("cleanup is pending"));
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    let update = incoming.try_recv().unwrap();
    assert!(update.observation().is_err());
    assert!(update.error().unwrap().contains("unregistration rejected"));
    assert!(incoming.try_recv().is_err());
}
#[test]
fn partial_mutation_retains_real_readback_and_missing_confirmation_is_unobserved() {
    let mut session = session(2668);
    session.backend.partial = true;
    let result = edit(&mut session, Edit::Enabled(false), true);
    assert!(result.error().unwrap().contains("mute rejected"));
    assert!(result.observation().as_ref().unwrap().setting().enabled());
    assert!(matches!(result.mutation(), Some(Mutation::Written(_))));
    session.backend.partial = false;
    session.backend.missing_readback = true;
    let result = edit(&mut session, Edit::Volume(79), true);
    assert!(
        result.observation().is_err(),
        "The pre-write observation must not mask a missing confirmation"
    );
    assert!(result.error().unwrap().contains("readback failed"));
    assert_eq!(session.backend.volume, 79);
}

#[test]
fn mute_submits_the_entire_ui_setting_after_a_rejected_volume_edit() {
    let mut session = session(2660);
    session.refresh().observation().as_ref().unwrap();
    session.backend.reject_volume = true;
    let result = session.edit(
        Command::new(
            session.scope.id(),
            Uuid::new_v4(),
            Setting::new(true, 83).unwrap(),
        ),
        &AtomicBool::new(true),
    );
    assert!(result.error().unwrap().contains("volume rejected"));
    assert_eq!(
        result.observation().as_ref().unwrap().setting().volume(),
        37
    );
    session.backend.reject_volume = false;
    let result = session.edit(
        Command::new(
            session.scope.id(),
            Uuid::new_v4(),
            Setting::new(false, 83).unwrap(),
        ),
        &AtomicBool::new(true),
    );
    assert!(result.error().is_none());
    assert_eq!(
        session.backend.writes,
        [
            ("selected-system-id".into(), false, 83),
            ("selected-system-id".into(), true, 83),
        ]
    );
    assert_eq!(
        result.observation().as_ref().unwrap().setting(),
        Setting::new(false, 83).unwrap()
    );
}
#[test]
fn default_speaker_notifications_read_the_selected_endpoint_and_reject_endpoint_replacement() {
    let mut session = session(2689);
    session.refresh().observation().as_ref().unwrap();
    assert!(
        session.notification().unwrap().is_none(),
        "No timer-triggered Core Audio read"
    );
    session.backend.notice.default_endpoint = Some("unrelated default".into());
    assert!(session.notification().unwrap().is_none());
    session.backend.volume = 82;
    session.backend.notice.speaker = true;
    let update = session.notification().unwrap().unwrap();
    let read = update.observation().as_ref().unwrap();
    assert_eq!(read.endpoint().id, "selected-system-id");
    assert_eq!(read.setting().volume(), 82);
    session.backend.notice.default_endpoint = Some("selected-system-id".into());
    assert!(session.notification().unwrap().is_some());
    session.backend.endpoints[0].id = "replacement-system-id".into();
    session.backend.notice.topology = true;
    assert!(
        session
            .notification()
            .unwrap()
            .unwrap()
            .observation()
            .is_err()
    );
    assert!(edit(&mut session, Edit::Volume(23), true).error().is_some());
    assert!(session.backend.writes.is_empty());
}
