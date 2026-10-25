//! Retained 1342 hotkey scope; registration, callbacks and cleanup share mapping ownership.
use super::super::*;
use razer_pages::features::{PresetShortcutBinding, PresetShortcutRequest};
use razer_service::mapping::hotkeys::{Session as Hotkeys, Shortcut};
use razer_service::mapping::mode_capture::{self, Reply as CaptureReply};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

enum Command {
    Update(PresetShortcutRequest),
    Stop,
}
enum Message {
    Completion(u64, Option<String>),
    Trigger(u64, PresetShortcutBinding),
    Mode(u64, bool),
    Error(String),
}
pub(in crate::shell) struct Session {
    command: Sender<Command>,
    worker: Option<JoinHandle<()>>,
    _events: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.command.send(Command::Stop);
    }
}
impl Session {
    fn retire(mut self) -> Option<JoinHandle<()>> {
        let _ = self.command.send(Command::Stop);
        self.worker.take()
    }
}

fn synchronize(
    client: &mut Hotkeys,
    current: &mut BTreeMap<(u32, u32), PresetShortcutBinding>,
    request: &PresetShortcutRequest,
    capture: &mut Option<mode_capture::Session>,
) -> anyhow::Result<()> {
    let mut errors = request.registration_errors.clone();
    if request.capturing && capture.is_none() {
        match mode_capture::Session::start() {
            Ok(session) => *capture = Some(session),
            Err(error) => errors.push(format!("Shortcut capture lifecycle: {error:#}")),
        }
    } else if !request.capturing {
        if let Some(session) = capture.take() {
            if let Err(error) = session.stop() {
                errors.push(format!("Shortcut capture lifecycle: {error}"));
            }
        }
    }
    let desired = request
        .bindings
        .iter()
        .cloned()
        .map(|b| ((b.virtual_key, b.modifiers), b))
        .collect::<BTreeMap<_, _>>();
    let removed = current
        .iter()
        .filter(|(key, binding)| desired.get(key) != Some(*binding))
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    for (key, modifiers) in removed {
        match client.unregister(key, modifiers) {
            Ok(_) => {
                current.remove(&(key, modifiers));
            }
            Err(error) => errors.push(format!("Unregister shortcut {key}/{modifiers}: {error:#}")),
        }
    }
    for (identity, binding) in desired {
        if current.get(&identity) == Some(&binding) {
            continue;
        }
        match client.register(Shortcut {
            virtual_key: identity.0,
            modifiers: identity.1,
            argument: String::new(),
        }) {
            Ok(_) => {
                current.insert(identity, binding);
            }
            Err(error) => errors.push(format!("Register shortcut {identity:?}: {error:#}")),
        }
    }
    match client.enable(!request.capturing) {
        Ok(_) => {}
        Err(error) => errors.push(format!("Enable global shortcuts: {error:#}")),
    }
    anyhow::ensure!(errors.is_empty(), "{}", errors.join("\n"));
    Ok(())
}
fn run(receiver: Receiver<Command>, events: Sender<Message>) -> anyhow::Result<()> {
    let mut client = Hotkeys::start()?;
    let mut current = BTreeMap::<(u32, u32), PresetShortcutBinding>::new();
    let mut generation = 0;
    let mut capturing = false;
    let mut capture = None;
    let mut capture_ready = false;
    let mut pending_capture = None;
    let result = (|| {
        loop {
            let command = match receiver.recv_timeout(Duration::from_millis(80)) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => break,
            };
            match command {
                Some(Command::Stop) => break,
                Some(Command::Update(mut request)) => {
                    // Replace unsent capture edits with the newest complete library.
                    while let Ok(command) = receiver.try_recv() {
                        match command {
                            Command::Update(next) => request = next,
                            Command::Stop => return Ok(()),
                        }
                    }
                    generation = request.generation;
                    capturing = request.capturing;
                    if !capturing {
                        capture_ready = false;
                        pending_capture = None;
                    }
                    let result = synchronize(&mut client, &mut current, &request, &mut capture)
                        .map_err(|e| format!("{e:#}"));
                    if capturing && capture.is_some() && !capture_ready {
                        pending_capture = Some((request.generation, result.err()));
                    } else {
                        let _ = events.send(Message::Completion(request.generation, result.err()));
                    }
                }
                None => {}
            }
            if let Some(session) = &capture {
                for reply in session.drain() {
                    match reply {
                        CaptureReply::Ready => {
                            capture_ready = true;
                            if let Some((generation, error)) = pending_capture.take() {
                                let _ = events.send(Message::Completion(generation, error));
                            }
                        }
                        CaptureReply::Mode(enabled) => {
                            let _ = events.send(Message::Mode(generation, enabled));
                        }
                        CaptureReply::Failed(error) => {
                            capture_ready = false;
                            let error = match pending_capture.take().and_then(|(_, error)| error) {
                                Some(previous) => format!("{previous}; {error}"),
                                None => error,
                            };
                            let _ = events.send(Message::Completion(generation, Some(error)));
                        }
                    }
                }
            }
            if current.is_empty() {
                continue;
            }
            for event in client.drain()? {
                let key = event.shortcut.virtual_key;
                let modifiers = event.shortcut.modifiers;
                if !capturing {
                    if let Some(binding) = current.get(&(key, modifiers)) {
                        let _ = events.send(Message::Trigger(generation, binding.clone()));
                    }
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    if let Some(session) = capture {
        if let Err(error) = session.stop() {
            let _ = events.send(Message::Error(format!(
                "Shortcut capture cleanup failed: {error:#}"
            )));
        }
    }
    // Attempt every removal even if another removal fails. Releasing this
    // scope preserves registrations retained by other mapping consumers.
    for ((key, modifiers), _) in current {
        if let Err(error) = client.unregister(key, modifiers) {
            let _ = events.send(Message::Error(format!(
                "Shortcut cleanup failed: {error:#}"
            )));
        }
    }
    if let Err(error) = client.close() {
        let _ = events.send(Message::Error(format!(
            "Shortcut worker shutdown failed: {error:#}"
        )));
    }
    result
}
impl AppShell {
    pub(in crate::shell) fn request_audio_preset_shortcuts(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: PresetShortcutRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let device = workspace.read(cx).device(cx);
        if device.product_id != 1342
            || !self
                .device_observations
                .iter()
                .any(|route| route.matches(device) && route.peer_product_id().is_none())
        {
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        if let Some(session) = self.audio_preset_shortcuts.get(&identity) {
            if session
                .command
                .send(Command::Update(request.clone()))
                .is_ok()
            {
                return;
            }
        }
        if request.bindings.is_empty()
            && !request.capturing
            && request.registration_errors.is_empty()
            && !self.audio_preset_shortcuts.contains_key(&identity)
        {
            return;
        }
        let Some(body) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        let (command, receiver) = mpsc::channel();
        let (events, status) = mpsc::channel();
        let worker_events = events.clone();
        let worker = match thread::Builder::new()
            .name("razer-preset-shortcuts".into())
            .spawn(move || {
                if let Err(error) = run(receiver, worker_events) {
                    let _ = events.send(Message::Error(format!(
                        "Preset shortcut owner failed: {error:#}"
                    )));
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                body.update(cx, |body, cx| {
                    body.finish_preset_shortcuts(
                        request.generation,
                        Some(format!("Preset shortcut thread failed: {error}")),
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let captured_identity = identity.clone();
        let task = cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let mut messages = Vec::new();
                let disconnected = loop {
                    match status.try_recv() {
                        Ok(message) => messages.push(message),
                        Err(mpsc::TryRecvError::Empty) => break false,
                        Err(mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                if shell
                    .update_in(cx, |this, window, cx| {
                        if !this.devices.contains(&workspace)
                            || workspace.read(cx).identity(cx) != captured_identity
                        {
                            return false;
                        }
                        for message in messages {
                            match message {
                                Message::Completion(generation, error) => {
                                    body.update(cx, |body, cx| {
                                        body.finish_preset_shortcuts(generation, error, window, cx)
                                    })
                                }
                                Message::Trigger(generation, binding) => {
                                    body.update(cx, |body, cx| {
                                        if body.preset_shortcut_current(generation) {
                                            body.activate_preset_shortcut(&binding, window, cx)
                                        }
                                    })
                                }
                                Message::Mode(generation, enabled) => {
                                    body.update(cx, |body, cx| {
                                        if body.preset_shortcut_current(generation) {
                                            body.observe_preset_shortcut_mode(enabled, cx);
                                        }
                                    })
                                }
                                Message::Error(error) => {
                                    eprintln!("Audio Mixer preset shortcut: {error}")
                                }
                            }
                        }
                        !disconnected
                    })
                    .unwrap_or(false)
                    == false
                {
                    break;
                }
            }
        });
        let _ = command.send(Command::Update(request));
        if let Some(old) = self
            .audio_preset_shortcuts
            .insert(
                identity,
                Session {
                    command,
                    worker: Some(worker),
                    _events: task,
                },
            )
            .and_then(Session::retire)
        {
            self.audio_preset_shortcut_cleanup.push(old);
        }
    }
    pub(in crate::shell) fn sync_audio_preset_shortcut_cleanup(&mut self, cx: &mut Context<Self>) {
        let removed = self
            .audio_preset_shortcuts
            .keys()
            .filter(|key| {
                !self.devices.iter().any(|workspace| {
                    workspace.read(cx).identity(cx) == **key
                        && self.device_observations.iter().any(|route| {
                            route.matches(workspace.read(cx).device(cx))
                                && route.peer_product_id().is_none()
                        })
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for identity in removed {
            if let Some(worker) = self
                .audio_preset_shortcuts
                .remove(&identity)
                .and_then(Session::retire)
            {
                self.audio_preset_shortcut_cleanup.push(worker);
            }
        }
    }
    pub(in crate::shell) fn install_audio_preset_shortcut_cleanup(
        &mut self,
        _cx: &mut Context<Self>,
    ) {
        self.register_exit_cleanup(|this, cx| {
            let sessions = std::mem::take(&mut this.audio_preset_shortcuts);
            let retired = std::mem::take(&mut this.audio_preset_shortcut_cleanup);
            for session in sessions.values() {
                let _ = session.command.send(Command::Stop);
            }
            let task = cx.background_executor().spawn(async move {
                for (_, session) in sessions {
                    if let Some(thread) = session.retire() {
                        let _ = thread.join();
                    }
                }
                for thread in retired {
                    let _ = thread.join();
                }
            });
            async move { task.await }
        });
    }
}
