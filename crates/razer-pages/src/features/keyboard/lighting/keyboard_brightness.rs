//! Source brightness change/toggle intents and serialized active-device requests.
//! Draft persistence is local; only the service readback can confirm hardware.
use super::KeyboardProductWorkspace;
use gpui_kit::component::{
    ActiveTheme, ThemeStyled,
    slider::{Slider, SliderState},
};
use gpui_kit::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy)]
pub struct KeyboardBrightnessRequested {
    generation: u64,
    percent: u8,
    epoch: u64,
    edit_revision: u64,
}
impl KeyboardBrightnessRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn percent(&self) -> u8 {
        self.percent
    }
}
#[derive(Clone, Copy)]
pub struct KeyboardBrightnessReadRequested {
    generation: u64,
    epoch: u64,
}
impl KeyboardBrightnessReadRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Observation {
    #[default]
    Unobserved,
    Disconnected,
    Failed,
    Unknown(u8),
    Confirmed(u8),
}

#[derive(Clone, Copy)]
struct Edit {
    percent: u8,
    revision: u64,
}

enum Request {
    Read(KeyboardBrightnessReadRequested),
    Write(KeyboardBrightnessRequested),
}

#[derive(Default)]
pub(super) struct State {
    generation: u64,
    epoch: u64,
    pending: Option<KeyboardBrightnessRequested>,
    read_pending: Option<KeyboardBrightnessReadRequested>,
    edit_revision: u64,
    active: bool,
    read_needed: bool,
    error: Option<String>,
    queued: Option<Edit>,
    preview: Option<Edit>,
    observation: Observation,
    operation_cancel: Option<Arc<AtomicBool>>,
}
impl State {
    fn invalidate(&mut self) {
        if let Some(canceled) = &self.operation_cancel {
            canceled.store(true, Ordering::Release);
        }
        // A command already sent can still affect hardware. Keep its serial
        // slot until the owned service has completed cancellation and cleanup.
        self.epoch = self.epoch.wrapping_add(1);
        self.queued = None;
        self.preview = None;
        self.observation = Observation::Unobserved;
        self.error = None;
        self.read_needed = self.active;
    }

    fn set_active(&mut self, active: bool) {
        if self.active != active {
            let disconnected = self.observation == Observation::Disconnected;
            self.active = active;
            self.invalidate();
            if disconnected && !active {
                self.observation = Observation::Disconnected;
            }
        }
    }

    fn disconnect(&mut self) {
        self.invalidate();
        self.observation = Observation::Disconnected;
    }

    fn preview_edit(&mut self, percent: u8) -> Edit {
        self.edit_revision = self.edit_revision.wrapping_add(1);
        let edit = Edit {
            percent,
            revision: self.edit_revision,
        };
        self.preview = Some(edit);
        edit
    }

    fn begin_write(&mut self, percent: u8) -> Option<KeyboardBrightnessRequested> {
        // The current skippable task keeps only the newest desired setting.
        let edit = self.preview_edit(percent);
        self.error = None;
        self.read_needed = false;
        if self.pending.is_some() || self.read_pending.is_some() {
            self.queued = Some(edit);
            return None;
        }
        Some(self.start_write(edit))
    }

    fn start_write(&mut self, edit: Edit) -> KeyboardBrightnessRequested {
        self.generation = self.generation.wrapping_add(1);
        let request = KeyboardBrightnessRequested {
            generation: self.generation,
            percent: edit.percent,
            epoch: self.epoch,
            edit_revision: edit.revision,
        };
        self.pending = Some(request);
        self.operation_cancel = Some(Arc::new(AtomicBool::new(false)));
        request
    }

    fn begin_read(&mut self) -> Option<KeyboardBrightnessReadRequested> {
        if !self.active {
            return None;
        }
        if self.pending.is_some() || self.read_pending.is_some() {
            self.read_needed = true;
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        let request = KeyboardBrightnessReadRequested {
            generation: self.generation,
            epoch: self.epoch,
        };
        self.read_pending = Some(request);
        self.read_needed = false;
        self.error = None;
        self.operation_cancel = Some(Arc::new(AtomicBool::new(false)));
        Some(request)
    }

    fn write_matches(&self, generation: u64, percent: u8) -> bool {
        self.pending
            .is_some_and(|request| request.generation == generation && request.percent == percent)
    }

    fn write_current(&self, generation: u64, percent: u8) -> bool {
        self.write_matches(generation, percent)
            && self
                .pending
                .is_some_and(|request| request.epoch == self.epoch)
    }

    fn read_matches(&self, generation: u64) -> bool {
        self.read_pending
            .is_some_and(|request| request.generation == generation)
    }

    fn read_current(&self, generation: u64) -> bool {
        self.read_matches(generation)
            && self.active
            && self
                .read_pending
                .is_some_and(|request| request.epoch == self.epoch)
    }

    fn accept(&mut self, observed: Option<u8>, error: Option<String>) {
        self.observation = match observed {
            Some(percent @ 0..=100) => Observation::Confirmed(percent),
            Some(raw) => Observation::Unknown(raw),
            None => Observation::Failed,
        };
        // A valid read and failed cleanup are independent facts.
        self.error = error;
    }

    fn finish_read(
        &mut self,
        generation: u64,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
    ) -> Option<Request> {
        if !self.read_matches(generation) {
            return None;
        }
        if self.read_current(generation) {
            if scope_current {
                self.accept(observed, error);
            } else {
                self.disconnect();
                self.read_needed = false;
                self.error = error;
            }
        }
        self.read_pending = None;
        self.operation_cancel = None;
        self.next_request()
    }

    fn finish_write(
        &mut self,
        generation: u64,
        percent: u8,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
    ) -> Option<Request> {
        if !self.write_matches(generation, percent) {
            return None;
        }
        if self.write_current(generation, percent) {
            if scope_current {
                let request = self.pending.expect("Matched write retains its request");
                self.accept(observed, error);
                if matches!(self.observation, Observation::Confirmed(_))
                    && self
                        .preview
                        .is_some_and(|edit| edit.revision == request.edit_revision)
                {
                    self.preview = None;
                }
            } else {
                // A newly discovered route change revokes this scope's queued
                // intent, while an already invalidated epoch leaves its successor alone.
                self.disconnect();
                self.read_needed = false;
                self.error = error;
            }
        }
        self.pending = None;
        self.operation_cancel = None;
        self.next_request()
    }

    fn next_request(&mut self) -> Option<Request> {
        if let Some(edit) = self.queued.take() {
            return Some(Request::Write(self.start_write(edit)));
        }
        if self.read_needed {
            return self.begin_read().map(Request::Read);
        }
        None
    }

    fn displayed_value(&self) -> Option<u8> {
        self.preview
            .map(|edit| edit.percent)
            .or(match self.observation {
                Observation::Confirmed(percent) => Some(percent),
                _ => None,
            })
    }

    fn cancellation_guard(&self, generation: u64, percent: Option<u8>) -> Option<Arc<AtomicBool>> {
        let current = match percent {
            Some(percent) => self.pending.is_some_and(|request| {
                request.generation == generation
                    && request.percent == percent
                    && request.epoch == self.epoch
            }),
            None => self.read_pending.is_some_and(|request| {
                request.generation == generation && request.epoch == self.epoch
            }),
        };
        // Preview precedence does not retire the read's serial slot or cancel
        // the newer write waiting for this same device.
        current.then(|| self.operation_cancel.clone()).flatten()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(canceled) = &self.operation_cancel {
            canceled.store(true, Ordering::Release);
        }
    }
}
impl EventEmitter<KeyboardBrightnessRequested> for KeyboardProductWorkspace {}
impl EventEmitter<KeyboardBrightnessReadRequested> for KeyboardProductWorkspace {}
impl KeyboardProductWorkspace {
    pub(super) fn preview_brightness(&mut self, percent: u8, cx: &mut Context<Self>) {
        self.brightness.preview_edit(percent);
        cx.notify();
    }

    /// Synchronize only a confirmed reading or an explicit user edit. The
    /// getter never changes the persisted source profile or guesses its switch.
    pub fn sync_brightness_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(percent) = self.brightness.displayed_value() else {
            return;
        };
        if let Some(slider) = self.sliders.get("/brightness/value")
            && slider.read(cx).value().start() != f32::from(percent)
        {
            slider.update(cx, |slider, cx| {
                slider.set_value(f32::from(percent), window, cx);
            });
        }
    }

    pub(super) fn brightness_slider(
        &self,
        slider: &Entity<SliderState>,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        if self
            .brightness
            .displayed_value()
            .is_some_and(|percent| slider.read(cx).value().start() == f32::from(percent))
        {
            return Slider::new(slider).disabled(disabled).into_any_element();
        }
        // Preserve the locked Slider's horizontal geometry without exposing a
        // local/default thumb or accessibility value as a device observation.
        div()
            .id("keyboard-brightness-unobserved")
            .aria_label(self.brightness_runtime_text())
            .flex()
            .items_center()
            .h_6()
            .w_full()
            .flex_shrink_0()
            .child(
                div()
                    .w_full()
                    .h_1p5()
                    .bg(cx.theme().tokens.slider_bar.opacity(0.2))
                    .rounded_full_style(cx),
            )
            .into_any_element()
    }

    fn emit_brightness_request(&self, request: Option<Request>, cx: &mut Context<Self>) {
        match request {
            Some(Request::Read(request)) => cx.emit(request),
            Some(Request::Write(request)) => cx.emit(request),
            None => {}
        }
    }
    pub(super) fn request_brightness(&mut self, cx: &mut Context<Self>) {
        if self.lighting_controls_disabled() {
            return;
        }
        if razer_device::keyboard_settings::capability(self.spec.product_id).is_none() {
            return;
        }
        let Some(value) = self
            .draft
            .pointer("/brightness/value")
            .and_then(serde_json::Value::as_u64)
        else {
            return;
        };
        let enabled = self
            .draft
            .pointer("/brightness/isEnabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let Ok(percent) = u8::try_from(if enabled { value } else { 0 }) else {
            return;
        };
        if percent > 100 {
            return;
        }
        if let Some(request) = self.brightness.begin_write(percent) {
            cx.emit(request);
        }
    }
    pub fn brightness_request_matches(&self, generation: u64, percent: u8) -> bool {
        self.brightness.write_matches(generation, percent)
    }
    pub fn brightness_request_current(&self, generation: u64, percent: u8) -> bool {
        self.brightness.write_current(generation, percent)
    }
    pub fn cancel_brightness_connection(&mut self) {
        self.brightness.disconnect();
    }
    pub fn brightness_cancellation_guard(
        &self,
        generation: u64,
        percent: Option<u8>,
    ) -> Option<Arc<AtomicBool>> {
        self.brightness.cancellation_guard(generation, percent)
    }
    pub fn finish_brightness(
        &mut self,
        generation: u64,
        percent: u8,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.brightness_request_matches(generation, percent) {
            return;
        }
        let next =
            self.brightness
                .finish_write(generation, percent, observed, error, scope_current);
        self.emit_brightness_request(next, cx);
        cx.notify();
    }
    pub(super) fn invalidate_brightness(&mut self) {
        self.brightness.invalidate();
    }
    pub fn set_brightness_read_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.brightness.set_active(active);
        if active && self.brightness.read_needed {
            self.request_brightness_read(cx);
        }
        cx.notify();
    }
    pub(super) fn request_brightness_read(&mut self, cx: &mut Context<Self>) {
        if razer_device::keyboard_settings::capability(self.spec.product_id).is_none() {
            return;
        }
        if let Some(request) = self.brightness.begin_read() {
            cx.emit(request);
        }
    }
    pub fn brightness_read_matches(&self, generation: u64) -> bool {
        self.brightness.read_matches(generation)
    }
    pub fn brightness_read_current(&self, generation: u64) -> bool {
        self.brightness.read_current(generation)
    }
    pub fn finish_brightness_read(
        &mut self,
        generation: u64,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.brightness_read_matches(generation) {
            return;
        }
        let next = self
            .brightness
            .finish_read(generation, observed, error, scope_current);
        self.emit_brightness_request(next, cx);
        cx.notify();
    }
    pub(in super::super) fn brightness_runtime_text(&self) -> String {
        let mut text = if self.brightness.pending.is_some() {
            "正在设置设备亮度…".into()
        } else if self.brightness.read_pending.is_some() {
            "正在读取设备亮度…".into()
        } else {
            match self.brightness.observation {
                Observation::Unobserved => "设备亮度尚未读取".into(),
                Observation::Disconnected => "键盘连接已断开，当前亮度未确认".into(),
                Observation::Failed => "设备亮度读取失败".into(),
                Observation::Unknown(raw) => format!("设备返回未知亮度：{raw}"),
                Observation::Confirmed(percent) => format!("设备亮度：{percent}%"),
            }
        };
        if let Some(edit) = self.brightness.preview {
            text.push_str(&format!("；待确认亮度：{}%", edit.percent));
        }
        if let Some(error) = &self.brightness.error {
            text.push_str(&format!("；{error}"));
        }
        text
    }
}

#[cfg(test)]
#[path = "keyboard_brightness_lifecycle_tests.rs"]
mod lifecycle_tests;
