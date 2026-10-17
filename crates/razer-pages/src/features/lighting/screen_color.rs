//! Retained screen-picker lifetime shared by the two original color editors.
use gpui_kit::*;
use razer_platform::screen_color::{ScreenColorEvent, ScreenColorPicker};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

const CURSOR: &str = "synapse/screen-color-picker.cur";
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScreenColorUpdateKind {
    Color(u32),
    Done,
}

pub(super) struct ScreenColorUpdate {
    generation: u64,
    pub(super) kind: ScreenColorUpdateKind,
}

#[cfg(test)]
impl ScreenColorUpdate {
    pub(super) fn simulated(capture: &ScreenColorCapture, kind: ScreenColorUpdateKind) -> Self {
        Self {
            generation: capture.generation,
            kind,
        }
    }
}

#[derive(Default)]
pub(super) struct ScreenColorCapture {
    picker: Option<ScreenColorPicker>,
    task: Option<Task<()>>,
    generation: u64,
    request_sequence: u64,
    active: bool,
    failed: bool,
    _window_closed: Option<Subscription>,
}
impl EventEmitter<ScreenColorUpdate> for ScreenColorCapture {}

impl ScreenColorCapture {
    pub(super) fn new(window: &Window, cx: &mut Context<Self>) -> Self {
        let window_id = window.window_handle().window_id();
        let owner = cx.weak_entity();
        let closed = cx.on_window_closed(move |cx, closed| {
            if closed == window_id {
                let _ = owner.update(cx, |capture, _| capture.cancel());
            }
        });
        Self {
            _window_closed: Some(closed),
            request_sequence: REQUEST_SEQUENCE.load(Ordering::Acquire),
            ..Self::default()
        }
    }

    pub(super) fn is_current(&self, update: &ScreenColorUpdate) -> bool {
        self.generation == update.generation
            && self.request_sequence == REQUEST_SEQUENCE.load(Ordering::Acquire)
    }

    pub(super) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active = false;
        self.task = None;
        if let Some(mut picker) = self.picker.take() {
            picker.cancel();
        }
    }

    pub(super) fn start(&mut self, cx: &mut Context<Self>) {
        self.cancel();
        self.request_sequence = REQUEST_SEQUENCE
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        if self.failed {
            if let Err(error) = razer_assets::retry_resources(cx) {
                self.fail(format!("{error:#}"));
                return;
            }
        }
        self.failed = false;
        self.active = true;
        if !self.poll(cx) {
            return;
        }
        let generation = self.generation;
        self.task = Some(cx.spawn(async move |owner, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep_polling = owner.update(cx, |this, cx| {
                    this.generation == generation && this.poll(cx)
                });
                if !matches!(keep_polling, Ok(true)) {
                    break;
                }
            }
        }));
    }

    fn fail(&mut self, error: String) {
        self.failed = true;
        self.active = false;
        self.picker = None;
        // Original pages log failures; diagnostics do not change their draft
        // or synthesize a completion, save, or device acknowledgement.
        eprintln!("Screen color picker: {error}");
    }

    fn poll(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.active {
            return false;
        }
        if self.request_sequence != REQUEST_SEQUENCE.load(Ordering::Acquire) {
            self.cancel();
            return false;
        }
        if self.picker.is_none() {
            // The asset service only checks memory and queues missing bytes.
            // Disk, credentials, HTTP and native screen I/O have worker owners.
            match cx.asset_source().load(CURSOR) {
                Ok(Some(bytes)) => match ScreenColorPicker::start(bytes.into_owned()) {
                    Ok(picker) => self.picker = Some(picker),
                    Err(error) => self.fail(format!("{error:#}")),
                },
                Ok(None) => self.fail(format!("Missing cursor resource {CURSOR}")),
                Err(error) if razer_assets::is_resource_pending(&error) => return true,
                Err(error) => self.fail(format!("{error:#}")),
            }
        }
        let Some(picker) = self.picker.as_ref() else {
            return false;
        };
        let events = match picker.drain() {
            Ok(events) => events,
            Err(error) => {
                self.fail(format!("{error:#}"));
                return false;
            }
        };
        for event in events {
            let kind = match event {
                ScreenColorEvent::Color(color) => ScreenColorUpdateKind::Color(color),
                ScreenColorEvent::Done => {
                    self.active = false;
                    self.picker = None;
                    ScreenColorUpdateKind::Done
                }
                ScreenColorEvent::Failed(error) => {
                    self.fail(error);
                    break;
                }
                ScreenColorEvent::Canceled => {
                    self.active = false;
                    self.picker = None;
                    break;
                }
            };
            cx.emit(ScreenColorUpdate {
                generation: self.generation,
                kind,
            });
            if !self.active {
                break;
            }
        }
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn replacing_or_canceling_a_capture_rejects_queued_previous_events() {
        let mut capture = ScreenColorCapture {
            request_sequence: REQUEST_SEQUENCE.load(Ordering::Acquire),
            ..ScreenColorCapture::default()
        };
        let event = ScreenColorUpdate {
            generation: capture.generation,
            kind: ScreenColorUpdateKind::Color(0x123456),
        };
        assert!(capture.is_current(&event));
        capture.cancel();
        assert!(!capture.is_current(&event));
        let done = ScreenColorUpdate {
            generation: capture.generation,
            kind: ScreenColorUpdateKind::Done,
        };
        capture.cancel();
        assert!(!capture.is_current(&done));
    }
}
