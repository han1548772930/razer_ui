//! Current Sound product artwork and observed left/right earbud state.
use super::*;
use razer_device::earbuds_auto_pause::Report;

#[derive(Default)]
pub(super) struct State {
    levels: [Option<u8>; 2],
    cursor: Option<Point<Pixels>>,
}

impl State {
    pub(super) fn clear(&mut self) {
        self.levels = [None; 2];
        self.cursor = None;
    }

    pub(super) fn leave(&mut self) {
        self.cursor = None;
    }

    pub(super) fn observe(&mut self, report: &Report) {
        if let Some(connected) = report.connected {
            let next = if connected {
                [report.left_battery, report.right_battery]
            } else {
                [None; 2]
            };
            if next != self.levels {
                self.cursor = None;
            }
            self.levels = next;
        }
    }

    fn level(&self, side: usize) -> Option<i16> {
        self.levels[side].map(|level| if level == 255 { -1 } else { i16::from(level) })
    }
}

impl AudioProductWorkspace {
    pub(super) fn earbuds_product_graphic(&self, cx: &mut Context<Self>) -> AnyElement {
        let graphic = &self.earbuds_graphic;
        let pair = if self.spec.product_id == 2657 {
            [
                "synapse/earbuds-2657-left.svg",
                "synapse/earbuds-2657-right.svg",
            ]
        } else {
            [
                "synapse/earbuds-standard-left.svg",
                "synapse/earbuds-standard-right.svg",
            ]
        };
        let mut ears = h_flex().gap(surface::css(8.)).items_center();
        for (side, alias) in pair.into_iter().enumerate() {
            let dimmed = graphic
                .level(side)
                .is_some_and(|level| !(0..=100).contains(&level));
            let hint = self.earbuds.active && graphic.level(side).is_some_and(|level| level <= 0);
            ears = ears.child(
                div()
                    .id(if side == 0 {
                        "earbuds-graphic-left"
                    } else {
                        "earbuds-graphic-right"
                    })
                    .test_support()
                    .w(surface::css(106.))
                    .h(surface::css(160.))
                    .when(dimmed, |ear| ear.opacity(0.5))
                    .child(img(alias).w_full().h_full())
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if !*hovered {
                            this.earbuds_graphic.cursor = None;
                            cx.notify();
                        }
                    }))
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        this.earbuds_graphic.cursor = hint.then_some(event.position);
                        cx.notify();
                    })),
            );
        }
        div()
            .id("earbuds-product-graphic")
            .test_support()
            .relative()
            .w_full()
            .min_w(surface::css(1024.))
            .max_w(surface::css(1220.))
            .h(surface::css(250.))
            .my(surface::css(10.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(surface::dot_background(cx))
            .child(ears)
            .children(graphic.cursor.map(|cursor| {
                use razer_widgets::theme::AudioEqualizerColors as C;
                razer_assets::deferred_resource(super::earbuds_equalizer_view::CursorTip::new(
                    cursor,
                    div()
                        .id("earbuds-graphic-disconnected-tip")
                        .test_support()
                        .max_w(surface::css(320.))
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .border_1()
                        .border_color(C::border())
                        .bg(C::tip_surface())
                        .text_color(C::text())
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .child(
                            gpui_kit::base::Tooltip::new("earbuds-graphic-disconnected-content")
                                .child(t("DISCONNECTED")),
                        ),
                ))
                .with_priority(999999)
            }))
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "earbuds_graphic_tests.rs"]
mod tests;
