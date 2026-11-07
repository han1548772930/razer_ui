//! The source Gesture modal is an informational legend with click dismissal.
use super::*;
use gpui_kit::base::Dialog;

pub(in crate::features::audio_products) struct Popup {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    scroll: ScrollHandle,
}

impl AudioProductWorkspace {
    pub(in crate::features::audio_products) fn open_earbuds_gestures(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.gesture_connected() || self.earbuds.gesture.popup.is_some() {
            return;
        }
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.earbuds.gesture.popup = Some(Popup {
            focus,
            return_focus,
            scroll: ScrollHandle::new(),
        });
        cx.notify();
    }

    pub(in crate::features::audio_products) fn close_earbuds_gestures(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(popup) = self.earbuds.gesture.popup.take() {
            if let Some(focus) = popup.return_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    pub(in crate::features::audio_products) fn earbuds_gesture_modal(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let popup = self.earbuds.gesture.popup.as_ref()?;
        let scale = window.rem_size() / 16.;
        let available_height = (window.viewport_size().height - scale * 110.).max(px(0.));
        let short = window.viewport_size().height <= scale * 720.;
        let anc = self.spec.product_id == 2657;
        let rows = [
            ("single", "SINGLE_TAP_TIP", "PLAY_PAUSE", "PLAY_PAUSE", true),
            (
                "double",
                "DOUBLE_TAP_TIP",
                "GESTURE_ACTION_PREVIOUS_TRACK",
                "GESTURE_ACTION_NEXT_TRACK",
                false,
            ),
            (
                "triple",
                "TRIPLE_TAP_TIP",
                "GESTURE_ACTION_SWITCH_AUDIO_SOURCE",
                "GESTURE_ACTION_SWITCH_AUDIO_SOURCE",
                true,
            ),
            (
                "hold",
                "GESTURE_LONG_PRESS_LABEL",
                if anc {
                    "GESTURE_ACTION_ANC_TOGGLE"
                } else {
                    "GESTURE_ACTION_MUTE_MIC"
                },
                "GESTURE_ACTION_MUTE_MIC",
                !anc,
            ),
            (
                "double-hold",
                "GESTURE_DOUBLE_TAP_HOLD_LABEL",
                "GESTURE_ACTION_DECREASE_VOLUME",
                "GESTURE_ACTION_INCREASE_VOLUME",
                false,
            ),
        ];
        let table = div()
            .id("earbuds-gesture-table")
            .test_support()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&popup.scroll)
            .children(rows.into_iter().map(|(id, name, left, right, merged)| {
                h_flex()
                    .id((ElementId::from("earbuds-gesture-row"), id))
                    .test_support()
                    .items_stretch()
                    .gap(surface::css(4.))
                    .mb(surface::css(4.))
                    .child(
                        div()
                            .w(surface::css(218.))
                            .p(surface::css(20.))
                            .rounded(surface::css(5.))
                            .bg(Colors::gesture_cell())
                            .child(
                                v_flex()
                                    .gap(surface::css(8.))
                                    .child(
                                        div()
                                            .text_size(surface::css(14.))
                                            .line_height(surface::css(14.))
                                            .child(t(name).to_uppercase()),
                                    )
                                    .child(gesture_icon(id)),
                            ),
                    )
                    .child(
                        h_flex()
                            .w(surface::css(362.))
                            .p(surface::css(20.))
                            .rounded(surface::css(5.))
                            .bg(Colors::gesture_cell())
                            .text_color(Colors::secondary())
                            .justify_center()
                            .text_center()
                            .child(div().w_full().child(t(left)))
                            .when(!merged, |cell| cell.child(div().w_full().child(t(right)))),
                    )
            }));
        let panel = v_flex()
            .id("earbuds-gesture-modal")
            .test_support()
            .occlude()
            .relative()
            .w(surface::css(640.))
            .max_w_full()
            .max_h(available_height)
            .p(surface::css(30.))
            .pb(surface::css(if short { 15. } else { 30. }))
            .bg(Colors::background())
            .border_1()
            .border_color(Colors::selected())
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::text())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                Button::new("earbuds-gesture-close")
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.))
                    .size(surface::css(14.))
                    .cursor_pointer()
                    .accessibility_label(t("CLOSE"))
                    .child(img("synapse/audio-surround-tutorial-close.svg").size_full())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.close_earbuds_gestures(window, cx)),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .mb(surface::css(20.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(16.))
                    .text_color(Colors::selected())
                    .child(t("GESTURES_TITLE_24_GHZ").to_uppercase()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .gap(surface::css(20.))
                    .pb(surface::css(1.))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .child(div().w(surface::css(218.)))
                            .children(
                                [
                                    ("LEFT", "synapse/earbuds-gesture-left.svg"),
                                    ("RIGHT", "synapse/earbuds-gesture-right.svg"),
                                ]
                                .into_iter()
                                .map(|(label, path)| {
                                    v_flex()
                                        .w(surface::css(181.))
                                        .items_center()
                                        .gap(surface::css(8.))
                                        .child(img(path).size(surface::css(80.)))
                                        .child(div().line_height(surface::css(14.)).child(t(label)))
                                }),
                            ),
                    )
                    .child(table),
            );
        // The mounted source has no Escape/Enter handlers; dismissal is its
        // backdrop click or close button. Base retains modal focus containment.
        Some(
            Dialog::new(cx)
                .focus_handle(popup.focus.clone())
                .close_on_escape(false)
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_close(
                    cx.listener(|this, _, window, cx| this.close_earbuds_gestures(window, cx)),
                )
                .backdrop(div().absolute().inset_0().bg(Colors::gesture_backdrop()))
                .popup(razer_assets::resource_scope(
                    div()
                        .id("earbuds-gesture-backdrop")
                        .test_support()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .pt(surface::css(90.))
                        .px(surface::css(20.))
                        .pb(surface::css(20.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_earbuds_gestures(window, cx)
                        }))
                        .child(panel),
                ))
                .into_any_element(),
        )
    }
}

fn gesture_dot() -> Div {
    div()
        .flex_shrink_0()
        .size(surface::css(12.))
        .rounded_full()
        .bg(Colors::selected())
}
fn hold_icon() -> AnyElement {
    h_flex()
        .gap(surface::css(4.))
        .child(
            div()
                .relative()
                .w(surface::css(28.))
                .h(surface::css(12.))
                .child(gesture_dot())
                .child(
                    div()
                        .absolute()
                        .left(surface::css(8.))
                        .top(surface::css(4.))
                        .w(surface::css(20.))
                        .h(surface::css(4.))
                        .rounded(surface::css(2.))
                        .bg(Colors::selected()),
                ),
        )
        .child(
            div()
                .text_size(surface::css(12.))
                .text_color(Colors::selected())
                .child(t("GESTURE_2_SECONDS")),
        )
        .into_any_element()
}
fn gesture_icon(id: &str) -> AnyElement {
    let taps = h_flex().gap(surface::css(20.));
    match id {
        "single" => img("synapse/earbuds-single-tap.svg")
            .size(surface::css(12.))
            .into_any_element(),
        "double" => taps
            .child(gesture_dot())
            .child(gesture_dot())
            .into_any_element(),
        "triple" => taps
            .children((0..3).map(|_| img("synapse/earbuds-single-tap.svg").size(surface::css(12.))))
            .into_any_element(),
        "hold" => hold_icon(),
        "double-hold" => taps
            .child(gesture_dot())
            .child(hold_icon())
            .into_any_element(),
        _ => unreachable!("fixed source gesture rows"),
    }
}
