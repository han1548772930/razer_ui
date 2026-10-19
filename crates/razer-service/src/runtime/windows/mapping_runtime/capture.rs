//! Source-defined recording and focused capture scopes on the shared mapping
//! actor. Device registrations and redirects belong to the retained driver
//! registry; the recorder owns its actual Windows input hooks.
#[path = "capture/recorder.rs"]
mod recorder;
#[path = "capture/targets.rs"]
mod targets;
#[cfg(test)]
#[path = "capture/tests.rs"]
mod tests;

use anyhow::{Context, Result, ensure};
use razer_device::mapping_native_session::hooks::RedirectKind;
use razer_ipc::{ServiceRequest, macro_recording::ActionReply};
use serde_json::{Value, json};
use std::collections::VecDeque;
use uuid::Uuid;

use recorder::{InputRecorder, Recorder};
pub(super) use targets::Target;

/// Implemented by the actor owning mapping execution, hotkeys and driver
/// registrations. An error may follow partial work: release must still be
/// able to find that scope's resource. Removing one scope preserves peers.
pub(super) trait Host {
    fn set_mappings(&mut self, scope: Uuid, enabled: bool) -> Result<ActionReply>;
    fn set_shortcuts(&mut self, scope: Uuid, enabled: bool) -> Result<ActionReply>;
    fn add_target(&mut self, scope: Uuid, target: &Target) -> Result<()>;
    fn redirect(
        &mut self,
        scope: Uuid,
        target: &Target,
        kind: RedirectKind,
        enabled: bool,
    ) -> Result<()>;
    fn remove_target(&mut self, scope: Uuid, target: &Target) -> Result<()>;
    fn subscribe_modes(&mut self, scope: Uuid, enabled: bool) -> Result<()>;
    fn global_mode(&mut self) -> Result<Value>;
    fn publish_storage(&mut self, scope: Uuid, key: &str, value: &Value) -> Result<()>;
}

fn confirmed(reply: ActionReply) -> Result<()> {
    ensure!(reply.result(), "{}", reply.reason());
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Keys,
    Global,
    Mode,
}

struct Registration {
    target: Target,
    redirects: Vec<RedirectKind>,
}

enum Event {
    Input(Value),
    Mode(Value),
}

struct Capture {
    scope: Uuid,
    kind: Kind,
    suspended: bool,
    mode_subscription: bool,
    registrations: Vec<Registration>,
    events: VecDeque<Event>,
    failure: Option<String>,
}
impl Capture {
    fn push(&mut self, event: Event) {
        if self.events.len() >= 512 {
            self.failure
                .get_or_insert_with(|| "Capture event buffer overflowed".into());
        } else {
            self.events.push_back(event);
        }
    }
    fn active(&self) -> bool {
        self.suspended || self.mode_subscription || !self.registrations.is_empty()
    }
}

pub(super) struct State<R: InputRecorder = razer_platform::macro_recorder::MacroRecorder> {
    recorder: Option<Recorder<R>>,
    capture: Option<Capture>,
}
impl<R: InputRecorder> Default for State<R> {
    fn default() -> Self {
        Self {
            recorder: None,
            capture: None,
        }
    }
}

impl<R: InputRecorder> State<R> {
    pub fn handle(
        &mut self,
        scope: Uuid,
        request: &ServiceRequest,
        host: &mut impl Host,
    ) -> Option<Result<Value>> {
        use ServiceRequest::*;
        if let Some(result) = self.recorder_request(scope, request, host) {
            return Some(result);
        }
        Some(match request {
            StartMappingKeyCapture => targets::observe(None, false)
                .and_then(|selection| self.start(scope, Kind::Keys, selection, host)),
            StartGlobalShortcutCapture => targets::observe(None, true)
                .and_then(|selection| self.start(scope, Kind::Global, selection, host)),
            StartKeyboardCalibrationCapture {
                product_id,
                container_id,
            } => targets::observe(Some((*product_id, container_id)), false)
                .and_then(|selection| self.start(scope, Kind::Keys, selection, host)),
            StartShortcutCapture => {
                self.start(scope, Kind::Mode, targets::Selection::default(), host)
            }
            MappingKeyCaptureEvents => self.events(scope, Kind::Keys),
            GlobalShortcutCaptureEvents => self.events(scope, Kind::Global),
            ShortcutCaptureEvents => self.events(scope, Kind::Mode),
            StopMappingKeyCapture => self.stop_capture(scope, false, host),
            StopShortcutCapture => self.stop_capture(scope, true, host),
            _ => return None,
        })
    }

    fn vacant(&self) -> Result<()> {
        ensure!(
            self.capture.is_none() && self.recorder.is_none(),
            "Another capture or recorder owns input suppression"
        );
        Ok(())
    }

    fn start(
        &mut self,
        scope: Uuid,
        kind: Kind,
        selection: targets::Selection,
        host: &mut impl Host,
    ) -> Result<Value> {
        self.vacant()?;
        self.capture = Some(Capture {
            scope,
            kind,
            suspended: true,
            mode_subscription: false,
            registrations: Vec::new(),
            events: VecDeque::new(),
            failure: None,
        });
        let started = (|| {
            confirmed(host.set_mappings(scope, false)?)?;
            let capture = self.capture.as_mut().unwrap();
            for target in selection.targets {
                capture.registrations.push(Registration {
                    target,
                    redirects: Vec::new(),
                });
                let registration = capture.registrations.last_mut().unwrap();
                // add_target returns only after the actual driver is ready.
                host.add_target(scope, &registration.target)?;
                for &redirect in registration.target.redirects() {
                    registration.redirects.push(redirect);
                    host.redirect(scope, &registration.target, redirect, true)?;
                }
            }
            if kind != Kind::Keys {
                capture.mode_subscription = true;
                host.subscribe_modes(scope, true)?;
            }
            Ok::<(), anyhow::Error>(())
        })();
        if let Err(error) = started {
            return match self.release_capture(scope, host) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(anyhow::anyhow!("{error:#}; capture cleanup: {cleanup:#}")),
            };
        }
        let capture = self.capture.as_ref().unwrap();
        if kind == Kind::Mode {
            return Ok(
                json!({"capturing":true,"mappings_suspended":true,"mode_events_registered":true}),
            );
        }
        Ok(
            json!({"capturing":true,"redirected_devices":capture.registrations.len(),
            "devices":capture.registrations.iter().map(|row|row.target.info()).collect::<Vec<_>>(),
            "unresolved_devices":selection.omitted,"mode_events_registered":kind==Kind::Global}),
        )
    }

    /// Actor publication order is the mixed input/mode order. A retired scope
    /// or registration can never publish into a successor's capture.
    pub fn input(&mut self, scope: Uuid, guid: Uuid, input: Value, time_tick: u64) {
        let Some(capture) = self.capture.as_mut().filter(|owner| owner.scope == scope) else {
            return;
        };
        let Some(registration) = capture
            .registrations
            .iter()
            .find(|row| row.target.guid == guid)
        else {
            return;
        };
        let event = json!({"deviceInfo":registration.target.info(),"eventType":3,"input":input,"timeTick":time_tick});
        capture.push(Event::Input(event));
    }

    pub fn input_failed(&mut self, scope: Uuid, guid: Uuid, error: String) {
        if let Some(capture) = self.capture.as_mut().filter(|owner| {
            owner.scope == scope
                && owner
                    .registrations
                    .iter()
                    .any(|row| row.target.guid == guid)
        }) {
            capture.failure.get_or_insert(error);
        }
    }

    pub fn mode(&mut self, event: Value) {
        if let Some(capture) = self
            .capture
            .as_mut()
            .filter(|owner| owner.mode_subscription)
        {
            capture.push(Event::Mode(event));
        }
    }

    fn events(&mut self, scope: Uuid, kind: Kind) -> Result<Value> {
        let capture = self.capture.as_mut().context("Input capture is inactive")?;
        ensure!(
            capture.scope == scope && capture.kind == kind,
            "Input capture owner or event stream changed"
        );
        if let Some(error) = &capture.failure {
            anyhow::bail!("{error}");
        }
        if kind == Kind::Global {
            use razer_ipc::global_shortcut_capture::{Batch, Event as WireEvent};
            #[derive(serde::Deserialize)]
            struct Mode {
                #[serde(rename = "isEnabled")]
                enabled: bool,
            }
            // Validate the whole batch before consuming it. Malformed mode
            // input cannot silently discard preceding keys or invent a state.
            let events = capture
                .events
                .iter()
                .map(|event| match event {
                    Event::Input(value) => Ok(WireEvent::Input(value.clone())),
                    Event::Mode(value) => {
                        let payload = value
                            .get("event")
                            .and_then(Value::as_str)
                            .context("Mode event lacks its source payload")?;
                        Ok(WireEvent::Mode(
                            serde_json::from_str::<Mode>(payload)?.enabled,
                        ))
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            capture.events.clear();
            return Ok(serde_json::to_value(Batch::new(events))?);
        }
        let events = capture
            .events
            .drain(..)
            .map(|event| match event {
                Event::Input(value) | Event::Mode(value) => value,
            })
            .collect::<Vec<_>>();
        Ok(json!({"events":events}))
    }

    fn stop_capture(&mut self, scope: Uuid, mode: bool, host: &mut impl Host) -> Result<Value> {
        if let Some(capture) = &self.capture {
            ensure!(
                capture.scope == scope && (capture.kind == Kind::Mode) == mode,
                "Capture stop belongs to a different owner or kind"
            );
        }
        self.release_capture(scope, host)?;
        Ok(if mode {
            json!({"capturing":false,"mappings_suspended":false})
        } else {
            json!({"capturing":false})
        })
    }

    fn release_capture(&mut self, scope: Uuid, host: &mut impl Host) -> Result<()> {
        let Some(capture) = self.capture.as_mut().filter(|owner| owner.scope == scope) else {
            return Ok(());
        };
        let mut errors = Vec::new();
        if capture.suspended {
            match host.set_mappings(scope, true).and_then(confirmed) {
                Ok(()) => capture.suspended = false,
                Err(error) => errors.push(format!("Restore mappings: {error:#}")),
            }
        }
        for index in (0..capture.registrations.len()).rev() {
            let registration = &mut capture.registrations[index];
            for at in (0..registration.redirects.len()).rev() {
                match host.redirect(
                    scope,
                    &registration.target,
                    registration.redirects[at],
                    false,
                ) {
                    Ok(()) => {
                        registration.redirects.remove(at);
                    }
                    Err(error) => errors.push(format!("Disable input redirect: {error:#}")),
                }
            }
            match host.remove_target(scope, &registration.target) {
                Ok(()) => {
                    capture.registrations.remove(index);
                }
                Err(error) => errors.push(format!("Remove capture device: {error:#}")),
            }
        }
        if capture.mode_subscription {
            match host.subscribe_modes(scope, false) {
                Ok(()) => capture.mode_subscription = false,
                Err(error) => errors.push(format!("Unsubscribe capture mode events: {error:#}")),
            }
        }
        if !capture.active() {
            self.capture = None;
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub fn release(&mut self, scope: Uuid, host: &mut impl Host) -> Result<()> {
        let capture = self.release_capture(scope, host);
        let recorder = self.release_recorder(scope, host);
        match (capture, recorder) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(a), Err(b)) => Err(anyhow::anyhow!("{a:#}; {b:#}")),
        }
    }
}
