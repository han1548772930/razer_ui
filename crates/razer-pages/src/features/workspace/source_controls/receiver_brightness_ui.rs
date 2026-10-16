//! Source brightness change/toggle intents and serialized active-device requests.
//! Draft persistence and requested-value completion stay separate from readback.
use super::SourceControls;
use gpui_kit::*;

#[derive(Clone, Copy)]
pub struct ReceiverBrightnessRequested {
    generation: u64,
    percent: u8,
    enabled: bool,
    value: u8,
    epoch: u64,
}
impl ReceiverBrightnessRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn percent(&self) -> u8 {
        self.percent
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn value(&self) -> u8 {
        self.value
    }
}
#[derive(Clone, Copy)]
pub struct ReceiverBrightnessReadRequested {
    generation: u64,
    epoch: u64,
    edit_revision: u64,
}
impl ReceiverBrightnessReadRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}
#[derive(Default)]
pub(super) struct State {
    generation: u64,
    epoch: u64,
    pending: Option<ReceiverBrightnessRequested>,
    read_pending: Option<ReceiverBrightnessReadRequested>,
    edit_revision: u64,
    active: bool,
    read_needed: bool,
    error: Option<String>,
    queued: Option<u8>,
    observed: Option<u8>,
    // The original TaskRunner updates curValue/value with its requested value
    // after the setter or same-percent skip. This local completion state is
    // separate from the getter's actual observation and Chromium memory storage.
    submitted_requested: Option<u8>,
    preview: Option<u8>,
}
impl State {
    fn invalidate(&mut self) {
        // A UI reset cannot stop a worker which may already be transmitting.
        // Keep its serialization slot until its actual completion arrives.
        self.epoch = self.epoch.wrapping_add(1);
        self.queued = None;
        self.observed = None;
        self.submitted_requested = None;
        self.preview = None;
        self.error = None;
        self.read_needed = self.active;
    }
}
impl EventEmitter<ReceiverBrightnessRequested> for SourceControls {}
impl EventEmitter<ReceiverBrightnessReadRequested> for SourceControls {}
impl SourceControls {
    pub(super) fn preview_brightness(&mut self, percent: Option<u8>, cx: &mut Context<Self>) {
        if percent.is_some() {
            self.brightness.edit_revision = self.brightness.edit_revision.wrapping_add(1);
        }
        self.brightness.preview = percent;
        cx.notify();
    }
    pub(super) fn request_brightness(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.spec.product_id, 164 | 241) {
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
        // The original skippable setting task keeps the newest desired setting.
        // Avoid multiple service workers racing one hardware setting.
        self.brightness.edit_revision = self.brightness.edit_revision.wrapping_add(1);
        self.brightness.observed = None;
        self.brightness.error = None;
        self.brightness.read_needed = false;
        if self.brightness.pending.is_some() || self.brightness.read_pending.is_some() {
            self.brightness.queued = Some(percent);
            return;
        }
        self.brightness.generation = self.brightness.generation.wrapping_add(1);
        let request = ReceiverBrightnessRequested {
            generation: self.brightness.generation,
            percent,
            enabled,
            value: value as u8,
            epoch: self.brightness.epoch,
        };
        self.brightness.pending = Some(request);
        cx.emit(request);
    }
    pub fn brightness_request_matches(&self, generation: u64, percent: u8) -> bool {
        self.brightness
            .pending
            .is_some_and(|r| r.generation == generation && r.percent == percent)
    }
    pub fn brightness_request_current(&self, generation: u64, percent: u8) -> bool {
        self.brightness_request_matches(generation, percent)
            && self
                .brightness
                .pending
                .is_some_and(|r| r.epoch == self.brightness.epoch)
    }
    pub fn cancel_brightness_connection(&mut self) {
        self.brightness.invalidate();
    }
    pub fn finish_brightness(
        &mut self,
        generation: u64,
        percent: u8,
        submitted_requested: Option<u8>,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.brightness_request_matches(generation, percent) {
            return;
        }
        let mut current = self.brightness_request_current(generation, percent);
        if !scope_current && current {
            // A route change not yet observed by the view cancels the old
            // queued intent. Previously invalidated epochs can retain only
            // an explicitly requested intent from their newer scope.
            self.brightness.invalidate();
            current = false;
        }
        self.brightness.pending = None;
        if current {
            if let Some(value) = submitted_requested {
                self.brightness.submitted_requested = Some(value);
            }
            self.brightness.observed = observed;
            self.brightness.error = error;
        }
        if self.brightness.queued.take().is_some() {
            self.request_brightness(cx);
        } else if self.brightness.read_needed {
            self.request_brightness_read(cx);
        }
        cx.notify();
    }
    pub(super) fn invalidate_brightness(&mut self) {
        self.brightness.invalidate();
    }
    pub fn set_brightness_read_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.brightness.active != active {
            self.brightness.active = active;
            self.brightness.invalidate();
        }
        if active && self.brightness.read_needed {
            self.request_brightness_read(cx);
        }
        cx.notify();
    }
    pub(super) fn request_brightness_read(&mut self, cx: &mut Context<Self>) {
        if !self.brightness.active
            || self.brightness.pending.is_some()
            || self.brightness.read_pending.is_some()
            || !matches!(self.spec.product_id, 164 | 241)
        {
            return;
        }
        self.brightness.generation = self.brightness.generation.wrapping_add(1);
        let request = ReceiverBrightnessReadRequested {
            generation: self.brightness.generation,
            epoch: self.brightness.epoch,
            edit_revision: self.brightness.edit_revision,
        };
        self.brightness.read_pending = Some(request);
        self.brightness.read_needed = false;
        self.brightness.error = None;
        cx.emit(request);
    }
    pub fn brightness_read_matches(&self, generation: u64) -> bool {
        self.brightness
            .read_pending
            .is_some_and(|request| request.generation == generation)
    }
    pub fn brightness_read_current(&self, generation: u64) -> bool {
        self.brightness.read_pending.is_some_and(|request| {
            request.generation == generation
                && request.epoch == self.brightness.epoch
                && request.edit_revision == self.brightness.edit_revision
        }) && self.brightness.active
    }
    pub fn brightness_read_scope_current(&self, generation: u64) -> bool {
        self.brightness.active
            && self.brightness.read_pending.is_some_and(|request| {
                request.generation == generation && request.epoch == self.brightness.epoch
            })
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
        if !scope_current
            && self
                .brightness
                .read_pending
                .is_some_and(|r| r.epoch == self.brightness.epoch)
        {
            self.brightness.invalidate();
            self.brightness.read_needed = false;
            self.brightness.error = error.clone();
        }
        if self.brightness_read_current(generation) && scope_current {
            self.brightness.observed = observed;
            self.brightness.error = error;
        }
        self.brightness.read_pending = None;
        if self.brightness.queued.take().is_some() {
            self.request_brightness(cx);
        } else if self.brightness.read_needed {
            self.request_brightness_read(cx);
        }
        cx.notify();
    }
    pub(super) fn brightness_runtime_text(&self) -> String {
        if self.brightness.pending.is_some() {
            "正在设置设备亮度…".into()
        } else if self.brightness.read_pending.is_some() {
            "正在读取设备亮度…".into()
        } else if let Some(error) = &self.brightness.error {
            self.brightness.observed.map_or_else(
                || format!("设备亮度未确认：{error}"),
                |value| format!("设备实际亮度：{value}%；提交未确认：{error}"),
            )
        } else if let Some(value) = self.brightness.submitted_requested {
            format!("设备亮度原链提交完成：{value}%；本地配置单独保存")
        } else if let Some(value) = self.brightness.observed {
            format!("设备亮度：{value}%；本地配置单独保存")
        } else {
            "设备亮度尚未读取；当前滑条为本地配置".into()
        }
    }
}
