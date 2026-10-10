//! Current 515 ordinary panel plus statically proven analog page consumers.
use super::*;
use gpui_kit::{base::Button as BaseButton, component::WindowExt as _};
use razer_platform::system;
use razer_widgets::theme::SnapTapColors as Colors;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    column: String,
    analog: bool,
}
fn specification(pid: u32) -> Option<&'static Spec> {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_properties_data.json"))
                .expect("independently traced current keyboard Properties callers")
        })
        .iter()
        .find(|spec| spec.product_id == pid)
}

pub(super) fn supported(pid: u32) -> bool {
    specification(pid).is_some()
}

impl KeyboardProductWorkspace {
    pub(super) fn properties_on_left(&self) -> bool {
        specification(self.spec.product_id).is_some_and(|spec| spec.column == "left")
    }

    pub(super) fn properties_full_width(&self) -> bool {
        specification(self.spec.product_id).is_some_and(|spec| spec.column == "full")
    }

    pub(super) fn keyboard_properties(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let icon = self.properties_icon?;
        let analog = specification(self.spec.product_id)?.analog;
        Some(
            surface::panel_with_control(
                t(if analog {
                    "WINDOWS_ANALOG_PROPERTIES"
                } else {
                    "KEYBOARD_PROPERTIES_HEADER"
                }),
                surface::help_control(
                    "keyboard-system-properties-help",
                    t(if analog {
                        "WINDOW_PROPERTIES_TOOLTIP"
                    } else {
                        "KEYBOARD_PROPERTIES_TOOLTIP"
                    }),
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
                        BaseButton::new("keyboard-system-properties")
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
            .when(analog, |panel| {
                panel.child(
                    h_flex()
                        .mt(surface::css(15.))
                        .items_center()
                        .gap(surface::css(20.))
                        .child(
                            img("synapse/keyboard-game-controller.svg")
                                .size(surface::css(44.))
                                .flex_shrink_0()
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(
                            BaseButton::new("keyboard-game-controller-properties")
                                .accessibility_label(t("OPEN_GAMECONTROLLER_PROPERTIES"))
                                .h(surface::css(44.))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(44.))
                                .text_color(Colors::border())
                                .underline()
                                .cursor_pointer()
                                .hover(|style| style.text_color(Colors::accent()))
                                .active(|style| style.opacity(0.7))
                                .focus_visible(|style| style.text_color(Colors::accent()))
                                .child(t("OPEN_GAMECONTROLLER_PROPERTIES"))
                                .on_click(|_, window, cx| {
                                    if let Err(error) =
                                        system::open(system::Properties::GameController)
                                    {
                                        window.push_notification(
                                            format!("无法打开系统属性：{error}"),
                                            cx,
                                        );
                                    }
                                }),
                        ),
                )
            })
            .into_any_element(),
        )
    }
}
