//! Retained 1342 hotkey owner; registration, callbacks and cleanup are real IPC.
use super::super::*;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{PresetShortcutBinding, PresetShortcutRequest};
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
    client: &mut ServiceClient,
    current: &mut BTreeMap<(u32, u32), PresetShortcutBinding>,
    request: &PresetShortcutRequest,
) -> anyhow::Result<()> {
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
        client.request(ServiceRequest::UnregisterShortcut {
            vkey_code: key,
            modifiers,
        })?;
        current.remove(&(key, modifiers));
    }
    for (identity, binding) in desired {
        if current.get(&identity) == Some(&binding) {
            continue;
        }
        client.request(ServiceRequest::RegisterShortcut {
            vkey_code: identity.0,
            modifiers: identity.1,
            argument: String::new(),
        })?;
        current.insert(identity, binding);
    }
    if !current.is_empty() {
        client.request(ServiceRequest::EnableGlobalShortcuts {
            enable: !request.capturing,
        })?;
    }
    Ok(())
}
fn run(receiver: Receiver<Command>, events: Sender<Message>) -> anyhow::Result<()> {
    let mut client = ServiceClient::spawn()?;
    let mut current = BTreeMap::<(u32, u32), PresetShortcutBinding>::new();
    let mut generation = 0;
    let mut capturing = false;
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
                    let result = synchronize(&mut client, &mut current, &request)
                        .map_err(|e| format!("{e:#}"));
                    let _ = events.send(Message::Completion(request.generation, result.err()));
                }
                None => {}
            }
            if current.is_empty() {
                continue;
            }
            let observed = client.request(ServiceRequest::ShortcutEvents)?;
            for event in observed
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Shortcut owner returned an invalid event array"))?
            {
                let payload: serde_json::Value = serde_json::from_str(
                    event["event"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Shortcut event JSON missing"))?,
                )?;
                let key = u32::try_from(
                    payload["virtualKey"]
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("Shortcut key missing"))?,
                )?;
                let modifiers = u32::try_from(
                    payload["modifiers"]
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("Shortcut modifiers missing"))?,
                )?;
                if !capturing {
                    if let Some(binding) = current.get(&(key, modifiers)) {
                        let _ = events.send(Message::Trigger(generation, binding.clone()));
                    }
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    // Attempt every removal even if another removal fails. Final worker exit
    // drops its direct platform owner and releases its thread registrations.
    for ((key, modifiers), _) in current {
        if let Err(error) = client.request(ServiceRequest::UnregisterShortcut {
            vkey_code: key,
            modifiers,
        }) {
            let _ = events.send(Message::Error(format!(
                "Shortcut cleanup failed: {error:#}"
            )));
        }
    }
    if let Err(error) = client.request(ServiceRequest::Shutdown) {
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
        if request.bindings.is_empty() && !self.audio_preset_shortcuts.contains_key(&identity) {
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
                window.push_notification(format!("Preset shortcut thread failed: {error}"), cx);
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
                                Message::Error(error) => window.push_notification(error, cx),
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
        cx: &mut Context<Self>,
    ) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
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
        }));
    }
}
