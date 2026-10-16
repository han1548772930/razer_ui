//! Explicit keyboard linked-game observations forwarded to the mounted owner.
use super::*;

impl ProductWorkspace {
    pub fn observe_keyboard_runtime_profile(
        &mut self,
        guid: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_runtime_profile(guid, window, cx)
            }),
            _ => false,
        }
    }
    pub fn select_keyboard_runtime_profile(
        &mut self,
        guid: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.select_keyboard_runtime_profile(guid, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_keyboard_linked_game_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_keyboard_linked_game_active(active, cx)
            });
        }
    }
}
