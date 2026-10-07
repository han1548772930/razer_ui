//! Current 226 ordinary Performance wi -> ei/Qs -> qs/Ks.
use super::*;
use crate::backend::system;
use gpui_kit::{base::Button as BaseButton, component::WindowExt as _};

impl MouseProductWorkspace {
    pub(super) fn mouse_properties(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel_with_control(
            t("MOUSE_PROPERTIES_HEADER"),
            surface::help_control("mouse-226-properties-help", t("MOUSE_PROPERTIES_TOOLTIP")),
            cx,
        )
        .child(
            h_flex()
                .items_center()
                .child(
                    img(self.properties_icon.expect("226 Windows icon"))
                        .size(surface::css(44.))
                        .mr(surface::css(20.))
                        .flex_shrink_0(),
                )
                .child(
                    BaseButton::new("mouse-226-properties")
                        .accessibility_label(t("MOUSE_PROPERTIES_DESC"))
                        .h(surface::css(44.))
                        .p_0()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(44.))
                        .text_color(rgb(0xcccccc))
                        .underline()
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(0x44d62c)))
                        .active(|style| style.opacity(0.7))
                        .child(t("MOUSE_PROPERTIES_DESC"))
                        .on_click(|_, window, cx| {
                            // Existing local OS command. Never call/acknowledge the
                            // original vendor DLL operation or alter the local profile.
                            if let Err(error) = system::open(system::Properties::Mouse) {
                                window.push_notification(format!("无法打开系统属性：{error}"), cx);
                            }
                        }),
                ),
        )
        .into_any_element()
    }
}
