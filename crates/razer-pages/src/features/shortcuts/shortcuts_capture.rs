use super::*;
use razer_service::mapping::key_capture::{Reply, Session};

impl Shortcuts {
    pub(super) fn stop_recording(&mut self, cx: &mut Context<Self>) {
        let was_recording = self.recording;
        self.recording = false;
        self.recording_id = None;
        self.recording_ready = None;
        self.recording_task = None;
        self.recording_session = None;
        self.recording_state = capture::CaptureState::default();
        if was_recording {
            cx.emit(ShortcutsCaptureStopped);
        }
    }

    pub(super) fn start_recording(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.discard_editor(window, cx);
        let Some(generation) = self.recording_generation.checked_add(1) else {
            self.recording_diagnostic = Some("Shortcut input generation exhausted".into());
            return;
        };
        self.recording_generation = generation;
        self.menu_for = None;
        self.recording_id = Some(id);
        self.recording = true;
        self.recording_diagnostic = None;
        self.recording_ready = Some(Instant::now() + Duration::from_millis(100));
        self.recording_task = Some(cx.spawn(async move |page, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let started = page
                .update(cx, |page, _| {
                    if !page.recording || page.recording_generation != generation {
                        return false;
                    }
                    match Session::start_global_shortcut() {
                        Ok(session) => page.recording_session = Some(session),
                        Err(error) => {
                            page.capture_diagnostic(format!("Shortcut capture startup: {error:#}"));
                            return false;
                        }
                    }
                    true
                })
                .unwrap_or(false);
            if !started {
                return;
            }
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let current = page
                    .update(cx, |page, cx| {
                        if !page.recording || page.recording_generation != generation {
                            return false;
                        }
                        page.drain_recording_session(cx)
                    })
                    .unwrap_or(false);
                if !current {
                    break;
                }
            }
        }));
        window.focus(&self.recording_focus, cx);
        cx.notify();
    }

    fn capture_diagnostic(&mut self, error: String) {
        eprintln!("[shortcut capture] {error}");
        self.recording_diagnostic = Some(error);
    }

    fn accepts_recorded_input(&self) -> bool {
        self.recording && self.recording_ready.is_some_and(|at| Instant::now() >= at)
    }

    fn drain_recording_session(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(session) = &self.recording_session else {
            return false;
        };
        let replies = session.drain();
        let mut failed = false;
        for reply in replies {
            match reply {
                Reply::Ready(receipt) => {
                    if !receipt.unresolved_devices().is_empty() {
                        self.capture_diagnostic(format!(
                            "Shortcut capture has unresolved observed endpoints: {:?}",
                            receipt.unresolved_devices(),
                        ));
                    }
                }
                Reply::Mode(enabled) => self.recording_state.set_mode(enabled),
                Reply::Events(events) => {
                    for event in events {
                        match self.recording_state.redirected(event) {
                            Ok(Some(input)) => self.apply_recorded_input(input, cx),
                            Ok(None) => {}
                            Err(error) => self.capture_diagnostic(error),
                        }
                    }
                }
                Reply::Failed(error) => {
                    self.capture_diagnostic(error);
                    failed = true;
                }
            }
        }
        !failed
    }

    fn apply_recorded_input(&mut self, input: capture::RecordedInput, cx: &mut Context<Self>) {
        if !self.accepts_recorded_input() {
            return;
        }
        let Some(id) = &self.recording_id else {
            return;
        };
        let Some(item) = self.items.iter_mut().find(|item| &item.id == id) else {
            return;
        };
        input.apply_to(item);
        self.input_warnings.recorded(&self.items, id);
        self.changed(cx);
    }

    pub(super) fn record_input(
        &mut self,
        input: String,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_recorded_input() {
            return;
        }
        // Apply already queued native input and mode notifications before the
        // browser fallback shares their pressed-key and modifier state.
        self.drain_recording_session(cx);
        let modifiers = [modifiers.control, modifiers.alt, modifiers.shift]
            .into_iter()
            .zip(["KEY_LEFT_CTRL", "KEY_LEFT_ALT", "KEY_LEFT_SHIFT"])
            .filter(|(held, _)| *held)
            .map(|(_, id)| id.to_owned())
            .collect::<Vec<_>>();
        let input = if matches!(
            input.as_str(),
            "ScrollButton" | "RightClick" | "Button4" | "Button5"
        ) {
            self.recording_state.mouse(&input, &modifiers)
        } else {
            self.recording_state.browser_key_down(&input, &modifiers)
        };
        if let Some(input) = input {
            self.apply_recorded_input(input, cx);
        }
    }

    pub(super) fn capture(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.recording {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        if let Some(key) = canonical_key(&event.keystroke.key) {
            self.record_input(key, event.keystroke.modifiers, cx);
        }
    }

    pub(super) fn release_captured_input(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if !self.accepts_recorded_input() {
            return;
        }
        cx.stop_propagation();
        self.drain_recording_session(cx);
        if let Some(key) = canonical_key(&event.keystroke.key) {
            self.recording_state.browser_key_up(&key);
        }
    }
}
