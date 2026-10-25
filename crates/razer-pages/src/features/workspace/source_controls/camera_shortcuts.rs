//! Current camera capture -> ON_SET_VIEW_PRESET_MODE -> MW registration/event.
//! Complete 3594/3595/3596 owners are retained by camera-local-pages. Registration is
//! local OS state; its receipt is neither a profile save nor a camera write.
use super::*;
use razer_device::camera_submission::CameraScope;
use razer_service::mapping::hotkeys::{Session as Hotkeys, Shortcut};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

const SOURCE_KEYS: &[(&str, u32)] = include!("camera_shortcut_keys.rs");

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    mode: u64,
    key: u32,
    modifiers: u32,
}

fn chord(keys: &Value) -> anyhow::Result<Option<(u32, u32)>> {
    let keys = keys
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("camera shortcut is not an array"))?;
    if keys.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(
        matches!(keys.len(), 2 | 3),
        "camera shortcut requires two or three keys"
    );
    let last = keys.last().and_then(Value::as_str);
    let key = SOURCE_KEYS
        .iter()
        .find(|(id, _)| Some(*id) == last)
        .map(|(_, key)| *key)
        .ok_or_else(|| anyhow::anyhow!("unknown camera shortcut key"))?;
    // Current MW te deliberately treats LEFT as generic, RIGHT as side-specific.
    let modifiers = keys[..keys.len() - 1].iter().fold(0, |bits, key| {
        bits | match key.as_str() {
            Some("KEY_LEFT_ALT") => 1,
            Some("KEY_LEFT_CTRL") => 2,
            Some("KEY_LEFT_SHIFT") => 4,
            Some("KEY_RIGHT_ALT") => 16,
            Some("KEY_RIGHT_CTRL") => 32,
            Some("KEY_RIGHT_SHIFT") => 64,
            _ => 0,
        }
    });
    Ok(Some((key, modifiers)))
}

fn bindings(profile: &Value) -> anyhow::Result<Vec<Binding>> {
    let presets = profile
        .pointer("/camera/viewPresets/data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("camera view presets are absent"))?;
    let mut result = Vec::new();
    for preset in presets {
        if let Some((key, modifiers)) = chord(&preset["shortcutKey"])? {
            result.push(Binding {
                mode: preset["mode"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("invalid camera preset mode"))?,
                key,
                modifiers,
            });
        }
    }
    Ok(result)
}

enum Command {
    Sync {
        generation: u64,
        bindings: Vec<Binding>,
        enabled: bool,
    },
    Invalid {
        generation: u64,
        mode: u64,
        error: String,
        enabled: bool,
    },
}
enum Event {
    Pressed {
        generation: u64,
        key: u32,
        modifiers: u32,
    },
    Failed {
        generation: u64,
        error: String,
    },
}
struct Session {
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    canceled: Arc<AtomicBool>,
    completed: mpsc::Receiver<()>,
}
impl Session {
    fn new(jobs: &razer_service::camera::lifecycle::Owner) -> anyhow::Result<Self> {
        let (commands, incoming) = mpsc::channel();
        let (outgoing, events) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let completed = jobs.spawn("camera-preset-shortcuts", canceled.clone(), move || {
            let mut os = match Hotkeys::start() {
                Ok(owner) => owner,
                Err(error) => {
                    let _ = outgoing.send(Event::Failed {
                        generation: 0,
                        error: format!("{error:#}"),
                    });
                    return;
                }
            };
            let mut generation = 0;
            let mut enabled = true;
            while !signal.load(Ordering::Acquire) {
                match incoming.recv_timeout(Duration::from_millis(25)) {
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Ok(Command::Invalid {
                        generation: next,
                        mode,
                        error,
                        enabled: active,
                    }) => {
                        generation = next;
                        enabled = active;
                        // Source se unregisters this preset's old tag before
                        // te rejects a malformed chord. Other tags stay active.
                        let cleanup = (|| -> anyhow::Result<()> {
                            for old in os.registered()? {
                                if old.argument == mode.to_string() {
                                    os.unregister(old.virtual_key, old.modifiers)?;
                                }
                            }
                            if !os.registered()?.is_empty() {
                                os.enable(active)?;
                            }
                            Ok(())
                        })();
                        let error = match cleanup {
                            Ok(()) => error,
                            Err(e) => format!("{error}; shortcut cleanup: {e:#}"),
                        };
                        let _ = outgoing.send(Event::Failed { generation, error });
                    }
                    Ok(Command::Sync {
                        generation: next,
                        bindings,
                        enabled: active,
                    }) => {
                        generation = next;
                        enabled = active;
                        let result = (|| -> anyhow::Result<()> {
                            // Remove obsolete identities before registering replacements,
                            // just as se unregisters the old tag before the new chord.
                            let registered = os.registered()?;
                            for old in registered {
                                if !bindings.iter().any(|b| {
                                    b.key == old.virtual_key && b.modifiers == old.modifiers
                                }) {
                                    os.unregister(old.virtual_key, old.modifiers)?;
                                }
                            }
                            for binding in &bindings {
                                if !os.registered()?.iter().any(|old| {
                                    old.virtual_key == binding.key
                                        && old.modifiers == binding.modifiers
                                        && old.argument == binding.mode.to_string()
                                }) {
                                    os.register(Shortcut {
                                        virtual_key: binding.key,
                                        modifiers: binding.modifiers,
                                        argument: binding.mode.to_string(),
                                    })?;
                                }
                            }
                            if !os.registered()?.is_empty() {
                                os.enable(active)?;
                            }
                            Ok(())
                        })();
                        if let Err(error) = result {
                            let _ = outgoing.send(Event::Failed {
                                generation,
                                error: format!("{error:#}"),
                            });
                        }
                    }
                }
                match os.drain() {
                    Ok(events) if enabled => {
                        for event in events {
                            let _ = outgoing.send(Event::Pressed {
                                generation,
                                key: event.shortcut.virtual_key,
                                modifiers: event.shortcut.modifiers,
                            });
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let _ = outgoing.send(Event::Failed {
                            generation,
                            error: format!("{error:#}"),
                        });
                        break;
                    }
                }
            }
            if let Err(error) = os.close() {
                let _ = outgoing.send(Event::Failed {
                    generation,
                    error: format!("Shortcut cleanup: {error:#}"),
                });
            }
        })?;
        Ok(Self {
            commands,
            events,
            canceled,
            completed,
        })
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct State {
    session: Option<Session>,
    task: Option<Task<()>>,
    requested: Option<(Option<CameraScope>, Value, bool)>,
    generation: u64,
    error: Option<String>,
}

fn restore_keys(draft: &mut Value, saved: &Value) {
    let Some(presets) = draft
        .pointer_mut("/camera/viewPresets/data")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(previous) = saved
        .pointer("/camera/viewPresets/data")
        .and_then(Value::as_array)
    else {
        return;
    };
    for preset in presets {
        if let Some(keys) = previous
            .iter()
            .find(|p| p["mode"] == preset["mode"])
            .and_then(|p| p["shortcutKey"].as_array())
            .filter(|keys| keys.iter().all(Value::is_string))
        {
            preset["shortcutKey"] = Value::Array(keys.clone());
        }
    }
}

impl SourceControls {
    pub(super) fn prepare_camera_shortcuts_exit(&mut self) {
        if let Some(session) = &self.camera_shortcuts.session {
            session.canceled.store(true, Ordering::Release);
        }
        self.camera_shortcuts.requested = None;
        self.camera_shortcuts.generation = self.camera_shortcuts.generation.wrapping_add(1);
    }
    pub(super) fn restore_camera_shortcuts(&mut self, saved: &Value) {
        if !matches!(self.spec.product_id, 3594..=3596) {
            return;
        }
        restore_keys(&mut self.draft, saved);
    }
    pub(super) fn init_camera_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.spec.product_id, 3594..=3596) {
            return;
        }
        // useFm retains its active capture border on window blur; focus only
        // suspends/re-enables input redirection. Outside click and unmount own
        // exit. GPUI routes key events only to the focused capture field.
        self.camera_shortcuts.task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                if view
                    .update_in(cx, |this, window, cx| {
                        this.poll_camera_shortcuts(window, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn poll_camera_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = &self.camera_shortcuts.session
            && session.canceled.load(Ordering::Acquire)
        {
            if matches!(session.completed.try_recv(), Err(mpsc::TryRecvError::Empty)) {
                return;
            }
            self.camera_shortcuts.session.take();
            self.camera_shortcuts.requested = None;
        }
        let scope = self.camera_runtime.queue.scope().cloned();
        // Zoom/pan edits do not affect registration. Keep the comparison small
        // and preserve domain preset modes rather than mutable list positions.
        let keys = self.draft.pointer("/camera/viewPresets/data").and_then(Value::as_array)
            .map(|presets| Value::Array(presets.iter().map(|p| serde_json::json!({"mode": p["mode"], "shortcutKey": p["shortcutKey"]})).collect()))
            .unwrap_or(Value::Null);
        let requested = (scope.clone(), keys, self.listening.is_none());
        if self.camera_shortcuts.requested.as_ref() != Some(&requested) {
            self.camera_shortcuts.generation = self.camera_shortcuts.generation.wrapping_add(1);
            self.camera_shortcuts.requested = Some(requested);
            self.camera_shortcuts.error = None;
            if scope.is_none() {
                // Keep one OS registration worker. Its ordered empty update
                // unregisters the old scope before any replacement registers.
                if let Some(session) = &self.camera_shortcuts.session {
                    let _ = session.commands.send(Command::Sync {
                        generation: self.camera_shortcuts.generation,
                        bindings: Vec::new(),
                        enabled: false,
                    });
                }
                return;
            }
            let result = (|| -> anyhow::Result<()> {
                if self.camera_shortcuts.session.is_none() {
                    self.camera_shortcuts.session = Some(Session::new(&self.camera_runtime.jobs)?);
                }
                let generation = self.camera_shortcuts.generation;
                let enabled = self.listening.is_none();
                let command = match bindings(&self.draft) {
                    Ok(bindings) => Command::Sync {
                        generation,
                        bindings,
                        enabled,
                    },
                    Err(error) => {
                        let mode = self
                            .draft
                            .pointer("/camera/viewPresets/data")
                            .and_then(Value::as_array)
                            .and_then(|presets| {
                                presets.iter().find(|p| chord(&p["shortcutKey"]).is_err())
                            })
                            .and_then(|p| p["mode"].as_u64())
                            .ok_or_else(|| anyhow::anyhow!("{error:#}"))?;
                        Command::Invalid {
                            generation,
                            mode,
                            error: format!("{error:#}"),
                            enabled,
                        }
                    }
                };
                self.camera_shortcuts
                    .session
                    .as_ref()
                    .unwrap()
                    .commands
                    .send(command)?;
                Ok(())
            })();
            if let Err(error) = result {
                self.camera_shortcuts.error = Some(format!("{error:#}"));
                eprintln!("camera shortcut registration: {error:#}");
            }
        }
        let events = self
            .camera_shortcuts
            .session
            .as_ref()
            .map(|session| session.events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Failed { generation, error }
                    if generation == self.camera_shortcuts.generation =>
                {
                    eprintln!("camera shortcut diagnostic: {error}");
                    self.camera_shortcuts.error = Some(error);
                }
                Event::Pressed {
                    generation,
                    key,
                    modifiers,
                } if generation == self.camera_shortcuts.generation
                    && self.listening.is_none()
                    && scope.is_some() =>
                {
                    // MW pe checks every preset, including duplicate chords.
                    let modes = bindings(&self.draft)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|b| b.key == key && b.modifiers == modifiers)
                        .map(|b| b.mode)
                        .collect::<Vec<_>>();
                    for mode in modes {
                        if self
                            .draft
                            .pointer("/camera/viewPresets/viewMode")
                            .and_then(Value::as_u64)
                            == Some(mode)
                        {
                            continue;
                        }
                        self.draft["camera"]["viewPresets"]["viewMode"] = mode.into();
                        self.camera_footer.changed = true;
                        // Current pe -> MW yFn only publishes viewMode. The
                        // actual zoom/pan/tilt request is Sm's viewMode effect,
                        // mounted by xm only while the CAMERA tab is visible.
                        // GU renders only the active tab; the other tabs still
                        // receive the selected mode without that device write.
                        if self.page == "CAMERA" {
                            self.request_camera_edit(
                                &format!("{}:view-preset", self.spec.product_id),
                                window,
                                cx,
                            );
                        }
                        self.sync(window, cx);
                        cx.emit(SourceControlsChanged);
                        cx.notify();
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{chord, restore_keys};
    use serde_json::json;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn current_camera_modifiers_and_key_codes_match_middleware() {
        assert_eq!(
            chord(&json!(["KEY_LEFT_CTRL", "KEY_LEFT_SHIFT", "KEY_A"])).unwrap(),
            Some((65, 6))
        );
        assert_eq!(
            chord(&json!(["KEY_RIGHT_CTRL", "KEY_A"])).unwrap(),
            Some((65, 32))
        );
        assert_eq!(chord(&json!([])).unwrap(), None);
        assert!(chord(&json!(["KEY_A"])).is_err());
        assert!(
            chord(&json!([
                "KEY_LEFT_CTRL",
                "KEY_LEFT_ALT",
                "KEY_LEFT_SHIFT",
                "KEY_A"
            ]))
            .is_err()
        );
    }
    #[test]
    fn empty_default_chords_restore_by_preset_mode_not_array_position() {
        let mut draft = json!({"camera":{"viewPresets":{"data":[{"mode":1,"shortcutKey":[]},{"mode":2,"shortcutKey":[]}]}}});
        let saved = json!({"camera":{"viewPresets":{"data":[{"mode":2,"shortcutKey":["KEY_RIGHT_CTRL","KEY_A"]},{"mode":1,"shortcutKey":["KEY_LEFT_CTRL","KEY_B"]}]}}});
        restore_keys(&mut draft, &saved);
        assert_eq!(
            draft["camera"]["viewPresets"]["data"][0]["shortcutKey"],
            json!(["KEY_LEFT_CTRL", "KEY_B"])
        );
        assert_eq!(
            draft["camera"]["viewPresets"]["data"][1]["shortcutKey"],
            json!(["KEY_RIGHT_CTRL", "KEY_A"])
        );
    }
}
