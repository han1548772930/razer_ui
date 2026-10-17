//! Current shared OBM header prompt: PID 70 `Fo`, current main UTF-16 4410765.
//! Visibility is reducer/device observation owned by the product workspace.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use razer_widgets::surface;
use razer_widgets::theme::KeyboardRgbEffectColors as Colors;

pub(super) fn render(window: &Window) -> AnyElement {
    let viewport = window.viewport_size();
    let unit = window.rem_size() / 16.;
    razer_assets::deferred_resource(
        anchored()
            .anchor(Anchor::TopLeft)
            .position(point(px(0.), px(0.)))
            .child(
                div()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(rgba(0x00000080))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        anchored()
                            .anchor(Anchor::BottomCenter)
                            .position(point(viewport.width / 2., viewport.height / 2.))
                            .child(
                                v_flex()
                                    .w(surface::css(400.))
                                    .px(surface::css(30.))
                                    .py(surface::css(20.))
                                    .border_1()
                                    .border_color(Colors::warning())
                                    .rounded(surface::css(5.))
                                    .bg(Colors::background())
                                    .shadow(vec![BoxShadow {
                                        color: rgba(0x00000033).into(),
                                        offset: point(px(0.), unit * 6.),
                                        blur_radius: unit * 10.,
                                        spread_radius: px(0.),
                                        inset: false,
                                    }])
                                    .text_color(Colors::text())
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(16.8))
                                    .text_center()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .justify_center()
                                            .mb(surface::css(20.))
                                            .text_color(Colors::warning())
                                            .text_size(surface::css(16.))
                                            .line_height(surface::css(16.8))
                                            .child(
                                                img("synapse/keyboard-lighting-warning.svg")
                                                    .size(surface::css(25.))
                                                    .mr(surface::css(10.)),
                                            )
                                            .child(
                                                razer_i18n::t("EXIT_ADJUSTMENT_MODE")
                                                    .to_uppercase(),
                                            ),
                                    )
                                    .child(razer_i18n::t("EXIT_ADJUSTMENT_MODE_DES")),
                            ),
                    ),
            ),
    )
    .with_priority(108)
    .into_any_element()
}
