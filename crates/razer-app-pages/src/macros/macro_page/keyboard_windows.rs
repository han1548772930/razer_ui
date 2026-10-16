//! Macro key semantics stay in this feature; the focused HWND capture is shared.
use super::keyboard::{RawKey, accepts};
use gpui_kit::Window;

pub struct Capture(razer_widgets::keyboard_capture::Capture);
impl Capture {
    pub fn start(window: &Window) -> Option<Self> {
        razer_widgets::keyboard_capture::Capture::start_filtered(window, accepts).map(Self)
    }
    pub fn start_stream(window: &Window) -> Option<Self> {
        razer_widgets::keyboard_capture::Capture::start_stream(window).map(Self)
    }
    pub fn take(&mut self) -> Option<RawKey> {
        self.0.take()
    }
    pub fn take_all(&mut self) -> Option<Vec<RawKey>> {
        self.0.take_all()
    }
}
