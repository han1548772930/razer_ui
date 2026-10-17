//! A window's resource cache covers the Base root and Component overlays.
//! Deferred and portal payloads are wrapped at their content boundary because
//! GPUI does not retain an ancestor image-cache scope for deferred draws.
use gpui_kit::{
    base::{Root, RootPlugin},
    *,
};

pub(crate) fn init(cx: &mut App) {
    Root::register_plugin::<WindowResources>(cx, |_, _| WindowResources);
}

struct WindowResources;
impl RootPlugin for WindowResources {
    fn decorate(
        &self,
        surface: AnyElement,
        _: &Root,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        razer_assets::resource_scope(surface)
    }
}
impl Render for WindowResources {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}
