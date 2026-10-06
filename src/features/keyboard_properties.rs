//! Current 515 $l right column → MA, with no analog/windowBigIcon props.
use super::*;
use crate::{backend::system, ui::theme::SnapTapColors as Colors};
use gpui_kit::{base::Button as BaseButton, component::WindowExt as _};

impl KeyboardProductWorkspace {
    pub(super) fn keyboard_properties(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let icon = self.properties_icon?;
        Some(
            surface::panel_with_control(
                t("KEYBOARD_PROPERTIES_HEADER"),
                surface::help_control(
                    "keyboard-515-properties-help",
                    t("KEYBOARD_PROPERTIES_TOOLTIP"),
                ),
                cx,
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(surface::css(20.))
                    .child(
                        img(icon)
                            .size(surface::css(44.))
                            .flex_shrink_0()
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        BaseButton::new("keyboard-515-properties")
                            .accessibility_label(t("OPEN_KEYBOARD_PROPERTIES"))
                            .h(surface::css(44.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(44.))
                            .text_color(Colors::border())
                            .underline()
                            .cursor_pointer()
                            .hover(|s| s.text_color(Colors::accent()))
                            .active(|s| s.opacity(0.7))
                            .focus_visible(|s| s.text_color(Colors::accent()))
                            .child(t("OPEN_KEYBOARD_PROPERTIES"))
                            .on_click(|_, window, cx| {
                                // Existing local Windows command; the vendor's void DLL
                                // operation is deliberately not invoked or acknowledged.
                                if let Err(error) = system::open(system::Properties::Keyboard) {
                                    window.push_notification(
                                        format!("无法打开系统属性：{error}"),
                                        cx,
                                    );
                                }
                            }),
                    ),
            )
            .into_any_element(),
        )
    }
}
