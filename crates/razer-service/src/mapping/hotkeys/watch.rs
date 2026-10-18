//! A retained background scope for one replaceable registration. The mapping
//! owner remains the sole OS listener; this task only publishes its replies.
use super::{Event, Shortcut, drain, register, registered, unregister};
use crate::mapping::{Transport, WorkerGuard, guarded, shutdown};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

enum Command {
    Replace {
        generation: u64,
        shortcut: Option<Shortcut>,
    },
    Close,
}
pub enum WatchUpdate {
    Registered {
        generation: u64,
        result: Result<Option<Shortcut>, String>,
    },
    Triggered {
        generation: u64,
        event: Event,
    },
    Closed(Result<(), String>),
}
pub struct Watch {
    commands: mpsc::Sender<Command>,
    closed: Arc<AtomicBool>,
}
impl Watch {
    pub fn start(
        debounce: Duration,
    ) -> Result<(Self, async_channel::Receiver<WatchUpdate>), String> {
        Self::start_with(debounce, crate::mapping::Client::spawn)
    }
    fn start_with<T: Transport + 'static>(
        debounce: Duration,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> Result<(Self, async_channel::Receiver<WatchUpdate>), String> {
        let (commands, input) = mpsc::channel();
        let (output, updates) = async_channel::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let cancellation = closed.clone();
        let worker = WorkerGuard::start();
        std::thread::Builder::new()
            .name("macro-hotkey-registration".into())
            .spawn(move || {
                let _worker = worker;
                let result = guarded(factory)
                    .and_then(|mut session| {
                        let result =
                            guarded(|| run(&mut session, &input, &output, debounce, &cancellation));
                        let cleanup = shutdown(&mut session).map_err(anyhow::Error::msg);
                        drop(session);
                        match (result, cleanup) {
                            (Ok(()), result) | (result, Ok(())) => result,
                            (Err(error), Err(cleanup)) => {
                                Err(anyhow::anyhow!("{error:#}; hotkey cleanup: {cleanup:#}"))
                            }
                        }
                    })
                    .map_err(|error| format!("{error:#}"));
                let _ = output.send_blocking(WatchUpdate::Closed(result));
            })
            .map_err(|error| error.to_string())?;
        Ok((Self { commands, closed }, updates))
    }
    pub fn replace(&self, generation: u64, shortcut: Option<Shortcut>) -> Result<(), String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("Global shortcut registration is closing".into());
        }
        self.commands
            .send(Command::Replace {
                generation,
                shortcut,
            })
            .map_err(|_| "Global shortcut registration task has closed".into())
    }
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Close);
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Default)]
struct Pending {
    current: Option<(u64, Shortcut)>,
    event: Option<(Instant, Event)>,
}
impl Pending {
    fn replace(&mut self, generation: u64, shortcut: Option<Shortcut>) {
        self.current = shortcut.map(|shortcut| (generation, shortcut));
        self.event = None;
    }
    fn accept(&mut self, now: Instant, event: Event) {
        if self
            .current
            .as_ref()
            .is_some_and(|(_, shortcut)| *shortcut == event.shortcut)
        {
            self.event = Some((now, event));
        }
    }
    fn ready(&mut self, now: Instant, debounce: Duration) -> Option<(u64, Event)> {
        if self
            .event
            .as_ref()
            .is_some_and(|(at, _)| now.duration_since(*at) >= debounce)
        {
            let (_, event) = self.event.take()?;
            Some((self.current.as_ref()?.0, event))
        } else {
            None
        }
    }
}

fn run(
    session: &mut impl Transport,
    input: &mpsc::Receiver<Command>,
    output: &async_channel::Sender<WatchUpdate>,
    debounce: Duration,
    cancellation: &AtomicBool,
) -> anyhow::Result<()> {
    let mut pending = Pending::default();
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Ok(());
        }
        match input.recv_timeout(Duration::from_millis(16)) {
            Ok(Command::Close) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Ok(Command::Replace {
                generation,
                shortcut,
            }) => {
                if cancellation.load(Ordering::Acquire) {
                    return Ok(());
                }
                // Invalidate pending callbacks before changing the native scope.
                pending.replace(generation, None);
                let result = (|| {
                    for previous in registered(session)? {
                        unregister(session, previous.virtual_key, previous.modifiers)?;
                    }
                    if let Some(shortcut) = &shortcut {
                        register(session, shortcut.clone())?;
                        anyhow::ensure!(
                            registered(session)?.contains(shortcut),
                            "Registered shortcut did not match the requested chord"
                        );
                    }
                    // Events queued before the replacement cannot start a new recording.
                    let _ = drain(session)?;
                    Ok::<_, anyhow::Error>(shortcut)
                })();
                if let Ok(shortcut) = &result {
                    pending.replace(generation, shortcut.clone());
                }
                if output
                    .send_blocking(WatchUpdate::Registered {
                        generation,
                        result: result.map_err(|error| format!("{error:#}")),
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if cancellation.load(Ordering::Acquire) {
            return Ok(());
        }
        for event in drain(session)? {
            pending.accept(Instant::now(), event);
        }
        if let Some((generation, event)) = pending.ready(Instant::now(), debounce) {
            if registered(session)?.contains(&event.shortcut)
                && output
                    .send_blocking(WatchUpdate::Triggered { generation, event })
                    .is_err()
            {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_ipc::ServiceRequest;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    struct Fake {
        shortcut: Option<Shortcut>,
        calls: Arc<Mutex<Vec<&'static str>>>,
        reject: bool,
        cleanup_failure: bool,
    }
    impl Transport for Fake {
        fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
            match request {
                ServiceRequest::GlobalShortcuts => Ok(Value::Array(self.shortcut.iter().map(|s| json!({"virtualKey":s.virtual_key,"modifiers":s.modifiers,"argument":s.argument})).collect())),
                ServiceRequest::RegisterShortcut { vkey_code, modifiers, argument } => {
                    self.calls.lock().unwrap().push("register");
                    if !self.reject { self.shortcut=Some(Shortcut { virtual_key:vkey_code, modifiers, argument }); }
                    Ok(json!({"registered":!self.reject,"vkey_code":vkey_code,"modifiers":modifiers}))
                }
                ServiceRequest::UnregisterShortcut { vkey_code, modifiers } => {
                    self.calls.lock().unwrap().push("unregister");
                    self.shortcut=None;
                    Ok(json!({"registered":false,"vkey_code":vkey_code,"modifiers":modifiers}))
                }
                ServiceRequest::ShortcutEvents => Ok(json!([])),
                ServiceRequest::Shutdown => {
                    self.calls.lock().unwrap().push("close");
                    Ok(json!({"stopped":!self.cleanup_failure}))
                }
                _ => panic!("unexpected request"),
            }
        }
        fn is_stopped(&self) -> bool {
            false
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            self.calls.lock().unwrap().push("drop");
        }
    }
    fn next(updates: &async_channel::Receiver<WatchUpdate>) -> WatchUpdate {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(3), updates.recv())
                    .await
                    .unwrap()
                    .unwrap()
            })
    }
    #[test]
    fn replacing_and_dropping_registration_closes_scope_before_completion() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let (watch, updates) = Watch::start_with(Duration::ZERO, move || {
            Ok(Fake {
                shortcut: None,
                calls: log,
                reject: false,
                cleanup_failure: false,
            })
        })
        .unwrap();
        watch.replace(1, Some(event(65).shortcut)).unwrap();
        assert!(matches!(
            next(&updates),
            WatchUpdate::Registered {
                generation: 1,
                result: Ok(Some(_))
            }
        ));
        watch.replace(2, Some(event(66).shortcut)).unwrap();
        assert!(matches!(
            next(&updates),
            WatchUpdate::Registered {
                generation: 2,
                result: Ok(Some(_))
            }
        ));
        watch.replace(3, None).unwrap();
        assert!(matches!(
            next(&updates),
            WatchUpdate::Registered {
                generation: 3,
                result: Ok(None)
            }
        ));
        drop(watch);
        assert!(matches!(next(&updates), WatchUpdate::Closed(Ok(()))));
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "register",
                "unregister",
                "register",
                "unregister",
                "close",
                "drop"
            ]
        );
    }
    #[test]
    fn rejected_registration_and_cleanup_are_reported_without_triggering() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let (watch, updates) = Watch::start_with(Duration::ZERO, move || {
            Ok(Fake {
                shortcut: None,
                calls: log,
                reject: true,
                cleanup_failure: true,
            })
        })
        .unwrap();
        watch.replace(1, Some(event(65).shortcut)).unwrap();
        assert!(matches!(
            next(&updates),
            WatchUpdate::Registered {
                generation: 1,
                result: Err(_)
            }
        ));
        watch.close();
        assert!(matches!(next(&updates), WatchUpdate::Closed(Err(_))));
        assert_eq!(*calls.lock().unwrap(), vec!["register", "close", "drop"]);
    }
    #[test]
    fn close_discards_queued_registration_before_scope_is_ready() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let (release, wait) = mpsc::channel();
        let (watch, updates) = Watch::start_with(Duration::ZERO, move || {
            wait.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok(Fake {
                shortcut: None,
                calls: log,
                reject: false,
                cleanup_failure: false,
            })
        })
        .unwrap();
        watch.replace(1, Some(event(65).shortcut)).unwrap();
        watch.close();
        assert!(watch.replace(2, Some(event(66).shortcut)).is_err());
        release.send(()).unwrap();
        assert!(matches!(next(&updates), WatchUpdate::Closed(Ok(()))));
        assert_eq!(*calls.lock().unwrap(), vec!["close", "drop"]);
    }
    fn event(key: u32) -> Event {
        Event {
            shortcut: Shortcut {
                virtual_key: key,
                modifiers: 512,
                argument: "macro_record_shortcut".into(),
            },
            time_tick: 17,
        }
    }
    #[test]
    fn debounce_keeps_last_event_and_replacement_discards_previous_generation() {
        let now = Instant::now();
        let delay = Duration::from_millis(400);
        let mut state = Pending::default();
        state.replace(1, Some(event(65).shortcut));
        state.accept(now, event(65));
        state.accept(now + Duration::from_millis(100), event(65));
        assert!(state.ready(now + delay, delay).is_none());
        assert_eq!(
            state.ready(now + Duration::from_millis(500), delay),
            Some((1, event(65)))
        );
        state.accept(now + delay, event(65));
        state.replace(2, Some(event(66).shortcut));
        state.accept(now + delay, event(65));
        assert!(state.ready(now + Duration::from_secs(2), delay).is_none());
        state.accept(now + delay, event(66));
        assert_eq!(
            state.ready(now + Duration::from_secs(2), delay),
            Some((2, event(66)))
        );
        state.replace(3, None);
        state.accept(now, event(66));
        assert!(state.ready(now + delay, delay).is_none());
    }
}
