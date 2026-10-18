use super::*;
use std::{collections::VecDeque, sync::atomic::Ordering};

fn owner() -> Owner {
    Owner::new(23, 1422, Arc::new(AtomicBool::new(false))).unwrap()
}
fn poll(terminal: bool) -> serde_json::Value {
    serde_json::to_value(PlaybackPoll {
        session_id: 23,
        product_id: 1422,
        started: !terminal,
        terminal,
        canceled: false,
        error: None,
    })
    .unwrap()
}
fn clip() -> PcmPlaybackFile {
    PcmPlaybackFile::new("simulated-recording".into(), 48_000, 2).unwrap()
}

#[derive(Default)]
struct Simulated {
    calls: Vec<&'static str>,
    prepare_error: bool,
    enable_error: bool,
    restore_error: bool,
    stop_error: bool,
    stopped: bool,
    fatal_at: Option<&'static str>,
    terminate_error: bool,
    reconnect_error: bool,
    start_reply: Option<serde_json::Value>,
    polls: VecDeque<serde_json::Value>,
    cancel_at: Option<(&'static str, Arc<AtomicBool>)>,
    panic_at: Option<&'static str>,
}
impl Simulated {
    fn call(&mut self, name: &'static str) {
        self.calls.push(name);
        if let Some((at, canceled)) = &self.cancel_at {
            if *at == name {
                canceled.store(true, Ordering::Release);
            }
        }
        assert_ne!(self.panic_at, Some(name), "simulated dispatch unwind");
    }

    fn dispatch(&mut self, name: &'static str) -> anyhow::Result<()> {
        self.call(name);
        ensure!(!self.stopped, "service already stopped");
        if self.fatal_at == Some(name) {
            self.stopped = true;
            anyhow::bail!("fatal IPC failure during {name}");
        }
        Ok(())
    }
}
impl Transport for Simulated {
    fn prepare(&mut self) -> anyhow::Result<()> {
        self.call("prepare");
        ensure!(!self.prepare_error, "observed device changed");
        Ok(())
    }
    fn prepare_restore(&mut self) -> anyhow::Result<()> {
        if self.stopped {
            self.call("reap");
            ensure!(!self.terminate_error, "old service process not released");
            self.call("reconnect");
            ensure!(!self.reconnect_error, "restore service connection failed");
            self.stopped = false;
        }
        Ok(())
    }
    fn return_switch(&mut self, enabled: bool) -> anyhow::Result<()> {
        self.dispatch(if enabled { "enable" } else { "restore" })?;
        ensure!(!(enabled && self.enable_error), "negative enable ACK");
        ensure!(!(!enabled && self.restore_error), "negative restore ACK");
        Ok(())
    }
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<serde_json::Value> {
        Ok(match request {
            ServiceRequest::SeirenAudioPlaybackStart {
                session_id,
                product_id,
                ..
            } => {
                assert_eq!((session_id, product_id), (23, 1422));
                self.dispatch("start")?;
                self.start_reply.take().unwrap_or_else(|| poll(false))
            }
            ServiceRequest::SeirenAudioPlaybackPoll { session_id } => {
                assert_eq!(session_id, 23);
                self.dispatch("poll")?;
                self.polls.pop_front().unwrap_or_else(|| poll(true))
            }
            ServiceRequest::SeirenAudioPlaybackStop { session_id } => {
                assert_eq!(session_id, 23);
                self.dispatch("stop")?;
                let mut reply = poll(true);
                if self.stop_error {
                    reply["error"] = "playback release failed".into();
                }
                reply
            }
            ServiceRequest::Shutdown => {
                self.dispatch("shutdown")?;
                serde_json::json!({"stopped":true})
            }
            _ => panic!("unexpected service operation"),
        })
    }
}

#[test]
fn natural_completion_releases_route_after_terminal_join_and_reports_one_start() {
    let owner = owner();
    let mut transport = Simulated {
        polls: [poll(false), poll(false), poll(true)].into(),
        ..Default::default()
    };
    let (updates, received) = mpsc::channel();
    let mut waits = Vec::new();
    run_with_transport(&owner, clip(), &updates, &mut transport, |wait| {
        waits.push(wait)
    })
    .unwrap();
    assert_eq!(
        transport.calls,
        [
            "prepare", "enable", "start", "poll", "poll", "poll", "restore", "shutdown"
        ]
    );
    assert_eq!(
        waits,
        [
            Duration::from_millis(500),
            Duration::from_millis(16),
            Duration::from_millis(16)
        ]
    );
    assert_eq!(
        received
            .try_iter()
            .filter(|update| matches!(update, Update::Started))
            .count(),
        1
    );
}

#[test]
fn cancellation_during_return_wait_restores_without_starting_playback() {
    let owner = owner();
    let mut transport = Simulated::default();
    let (updates, _) = mpsc::channel();
    run_with_transport(&owner, clip(), &updates, &mut transport, |wait| {
        assert_eq!(wait, Duration::from_millis(500));
        owner.canceled.store(true, Ordering::Release);
    })
    .unwrap();
    assert_eq!(
        transport.calls,
        ["prepare", "enable", "restore", "shutdown"]
    );
}

#[test]
fn malformed_start_still_stops_original_owner_restores_and_shuts_down() {
    let owner = owner();
    let mut transport = Simulated {
        start_reply: Some(serde_json::json!({"session_id":999})),
        ..Default::default()
    };
    let (updates, _) = mpsc::channel();
    assert!(run_with_transport(&owner, clip(), &updates, &mut transport, |_| {}).is_err());
    assert_eq!(
        transport.calls,
        ["prepare", "enable", "start", "stop", "restore", "shutdown"]
    );
}

#[test]
fn negative_enable_ack_preserves_source_playback_and_diagnostic() {
    let owner = owner();
    let mut transport = Simulated {
        enable_error: true,
        ..Default::default()
    };
    let (updates, _) = mpsc::channel();
    let error = run_with_transport(&owner, clip(), &updates, &mut transport, |_| {}).unwrap_err();
    assert!(error.to_string().contains("negative enable ACK"));
    assert_eq!(
        transport.calls,
        ["prepare", "enable", "start", "poll", "restore", "shutdown"]
    );
}

#[test]
fn cancellation_after_start_keeps_independent_stop_restore_errors() {
    let owner = owner();
    let mut transport = Simulated {
        stop_error: true,
        restore_error: true,
        cancel_at: Some(("start", owner.canceled.clone())),
        ..Default::default()
    };
    let (updates, _) = mpsc::channel();
    let error = run_with_transport(&owner, clip(), &updates, &mut transport, |_| {})
        .unwrap_err()
        .to_string();
    assert!(error.contains("playback release failed"));
    assert!(error.contains("negative restore ACK"));
    assert_eq!(
        transport.calls,
        ["prepare", "enable", "start", "stop", "restore", "shutdown"]
    );
}

#[test]
fn changed_physical_device_never_receives_temporary_routing_or_playback() {
    let owner = owner();
    let mut transport = Simulated {
        prepare_error: true,
        ..Default::default()
    };
    let (updates, _) = mpsc::channel();
    assert!(
        run_with_transport(&owner, clip(), &updates, &mut transport, |_| {})
            .unwrap_err()
            .to_string()
            .contains("observed device changed")
    );
    assert_eq!(transport.calls, ["prepare", "shutdown"]);
}

#[test]
fn dispatch_unwind_still_releases_all_later_stages() {
    for stage in ["start", "stop", "restore"] {
        let owner = owner();
        let mut transport = Simulated {
            panic_at: Some(stage),
            cancel_at: Some(("start", owner.canceled.clone())),
            ..Default::default()
        };
        let (updates, _) = mpsc::channel();
        assert!(run_with_transport(&owner, clip(), &updates, &mut transport, |_| {}).is_err());
        assert_eq!(
            transport.calls,
            ["prepare", "enable", "start", "stop", "restore", "shutdown"]
        );
    }
}

#[test]
fn fatal_ipc_releases_old_process_before_reconnecting_for_original_route_restore() {
    for stage in ["enable", "start", "poll", "stop"] {
        let owner = owner();
        let mut transport = Simulated {
            fatal_at: Some(stage),
            cancel_at: (stage == "stop").then(|| ("start", owner.canceled.clone())),
            ..Default::default()
        };
        let (updates, _received) = mpsc::channel();
        let error = run_with_transport(&owner, clip(), &updates, &mut transport, |_| {})
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("fatal IPC failure during {stage}")));
        assert!(
            transport
                .calls
                .ends_with(&["reap", "reconnect", "restore", "shutdown"])
        );
        assert!(!transport.stopped);
    }
}

#[test]
fn failed_original_process_release_does_not_reconnect_or_claim_route_restore() {
    let owner = owner();
    let mut transport = Simulated {
        fatal_at: Some("start"),
        terminate_error: true,
        ..Default::default()
    };
    let (updates, _received) = mpsc::channel();
    let error = run_with_transport(&owner, clip(), &updates, &mut transport, |_| {})
        .unwrap_err()
        .to_string();
    assert!(error.contains("fatal IPC failure during start"));
    assert!(error.contains("old service process not released"));
    assert!(error.contains("service already stopped"));
    assert_eq!(
        transport.calls,
        ["prepare", "enable", "start", "stop", "reap", "shutdown"]
    );
    assert!(transport.stopped);
}

#[test]
fn failed_restore_connection_preserves_original_failure_and_shutdown_attempt() {
    let owner = owner();
    let mut transport = Simulated {
        fatal_at: Some("poll"),
        reconnect_error: true,
        ..Default::default()
    };
    let (updates, _received) = mpsc::channel();
    let error = run_with_transport(&owner, clip(), &updates, &mut transport, |_| {})
        .unwrap_err()
        .to_string();
    assert!(error.contains("fatal IPC failure during poll"));
    assert!(error.contains("restore service connection failed"));
    assert!(
        transport
            .calls
            .ends_with(&["stop", "reap", "reconnect", "shutdown"])
    );
    assert!(transport.stopped);
}
