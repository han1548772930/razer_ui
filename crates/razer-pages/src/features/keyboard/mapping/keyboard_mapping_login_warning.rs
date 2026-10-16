//! Current Customize Windows-login warning and global local-storage preference.
use super::*;

#[derive(Default)]
pub(super) struct State {
    shown: bool,
    hidden: bool,
}
impl State {
    pub(super) fn unmount(&mut self) {
        self.shown = false;
    }
}
/// Source `windowsLoginWarningHide=true`; this is a local global preference.
pub struct KeyboardMappingLoginWarningHidden;
impl EventEmitter<KeyboardMappingLoginWarningHidden> for KeyboardProductWorkspace {}
/// Source Customize componentDidMount requests the global local preference.
pub struct KeyboardMappingLoginWarningReadRequested;
impl EventEmitter<KeyboardMappingLoginWarningReadRequested> for KeyboardProductWorkspace {}

impl KeyboardProductWorkspace {
    pub fn observe_mapping_login_warning_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        self.mapping_login_warning.hidden = hidden;
        cx.notify();
    }
    pub(super) fn show_mapping_login_warning(&mut self) {
        #[derive(Deserialize)]
        struct Source {
            product_id: u32,
            confirmed: bool,
        }
        static DATA: OnceLock<Vec<Source>> = OnceLock::new();
        if DATA
            .get_or_init(|| crate::data::keyboard_mapping_login_warning_data!())
            .iter()
            .any(|row| row.product_id == self.spec.product_id && row.confirmed)
        {
            self.mapping_login_warning.shown = true;
        }
    }
    pub(super) fn mapping_login_warning_element(&self, cx: &Context<Self>) -> Option<AnyElement> {
        (self.mapping_login_warning.shown
            && !self.mapping_login_warning.hidden
            && matches!(
                self.spec.config["DeviceInfo"]["category"].as_str(),
                Some("SYSTEM" | "KEYBOARD")
            ))
        .then(|| {
            deferred(Warning { owner: cx.entity() })
                .with_priority(106)
                .into_any_element()
        })
    }
}

#[derive(IntoElement)]
struct Warning {
    owner: Entity<KeyboardProductWorkspace>,
}
impl RenderOnce for Warning {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let size = window.viewport_size();
        let top = window.rem_size() / 16. * 42.;
        let owner = self.owner;
        div()
            .child(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .position(point(px(0.), top))
                    .child(
                        div()
                            .id("keyboard-mapping-login-backdrop")
                            .w(size.width)
                            .h((size.height - top).max(px(0.)))
                            .occlude()
                            .bg(rgba(0x00000080))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                            .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
                    ),
            )
            .child(
                anchored()
                    .anchor(Anchor::BottomCenter)
                    .position(point(size.width / 2., size.height / 2.))
                    .child(
                        v_flex()
                            .id("keyboard-mapping-login-warning")
                            .w(surface::css(400.))
                            .py(surface::css(20.))
                            .px(surface::css(30.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0xfd8611))
                            .rounded(surface::css(5.))
                            .font_family("Roboto")
                            .text_color(rgb(0xcccccc))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_center()
                                    .mb(surface::css(20.))
                                    .text_size(surface::css(16.))
                                    .line_height(surface::css(16.8))
                                    .text_color(rgb(0xfd8611))
                                    .child(
                                        img("synapse/keyboard-mapping-login-warning.svg")
                                            .size(surface::css(25.))
                                            .mr(surface::css(10.)),
                                    )
                                    .child(t("TEXT_WINDOW_LOGIN_WARNING").to_uppercase()),
                            )
                            .child(
                                div()
                                    .text_center()
                                    .line_height(surface::css(16.8))
                                    .child(t("TEXT_WINDOW_LOGIN_WARNING_NOTE")),
                            )
                            .child(
                                h_flex().justify_center().mt(surface::css(17.)).child(
                                    gpui_kit::base::Button::new("keyboard-mapping-login-ok")
                                        .accessibility_label(t("OK"))
                                        .w(surface::css(90.))
                                        .h(surface::css(27.))
                                        .rounded(surface::css(3.))
                                        .bg(rgb(0x707070))
                                        .text_size(surface::css(12.))
                                        .text_color(rgb(0xcccccc))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(t("OK").to_uppercase())
                                        .on_click(move |_, _, cx| {
                                            owner.update(cx, |this, cx| {
                                                this.mapping_login_warning.shown = false;
                                                this.mapping_login_warning.hidden = true;
                                                cx.emit(KeyboardMappingLoginWarningHidden);
                                                cx.notify();
                                            });
                                        }),
                                ),
                            ),
                    ),
            )
    }
}
