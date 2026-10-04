//! Product 182 KP geometry, keyed by its physical 1368 groupList inputs.
use super::*;

impl BindingDialog {
    pub(super) fn mouse_diagram(
        &self,
        layout: &MacroInputLayout,
        device: &Device,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let bottom = self.input.as_deref() == Some("DKM_SB_03")
            || self.hovered.as_deref() == Some("DKM_SB_03");
        let overlay = resources::device_image(
            device.product_id,
            device.edition_id,
            device.layout_id,
            resources::DeviceImage::MouseBottom,
        );
        let inputs = layout.inputs();
        let selected = self.input.clone();
        let hovered = self.hovered.clone();
        let colors: Vec<_> = inputs
            .iter()
            .map(|input| {
                if !input.enabled() {
                    Colors::disabled_line()
                } else if selected.as_deref() == Some(input.id())
                    || hovered.as_deref() == Some(input.id())
                {
                    self.layer_color()
                } else {
                    Colors::border()
                }
            })
            .collect();
        let focused: Vec<_> = inputs
            .iter()
            .map(|input| {
                selected.as_deref() == Some(input.id()) || hovered.as_deref() == Some(input.id())
            })
            .collect();
        div()
            .w_full()
            .min_w(css(770.))
            .h(css(340.))
            .relative()
            .child(surface::dot_background(cx))
            .child(
                div()
                    .relative()
                    .mx_auto()
                    .w(css(770.))
                    .h(css(340.))
                    .when_some(layout.image(), |view, path| {
                        view.child(
                            img(SharedString::from(path.to_string()))
                                .absolute()
                                .left(css(235.))
                                .w(css(300.))
                                .h(css(340.))
                                .object_fit(ObjectFit::Contain),
                        )
                    })
                    .when_some(bottom.then_some(overlay).flatten(), |view, path| {
                        view.child(
                            img(path)
                                .absolute()
                                .left(css(272.))
                                .top(css(-77.))
                                .w(css(300.))
                                .h(css(340.)),
                        )
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let sx = bounds.size.width / 310.;
                                let sy = bounds.size.height / 340.;
                                for (index, left, row, x, y) in [
                                    (0, true, 0, 114., 53.),
                                    (2, true, 1, 148., 84.),
                                    (6, true, 2, 86., 140.),
                                    (5, true, 3, 90., 178.),
                                    (
                                        7,
                                        true,
                                        4,
                                        if bottom { 185. } else { 90. },
                                        if bottom { 155. } else { 195. },
                                    ),
                                    (1, false, 0, 181., 53.),
                                    (3, false, 1, 148., 67.),
                                    (4, false, 2, 148., 102.),
                                ] {
                                    let Some(color) = colors.get(index).copied() else {
                                        continue;
                                    };
                                    let pt =
                                        |x, y| point(bounds.left() + sx * x, bounds.top() + sy * y);
                                    let start = if left { 3. } else { 307. };
                                    let middle = if left { 56. } else { 254. };
                                    let line_y = 16. + 50. * row as f32;
                                    let dot = pt(x + 7.5, y + 2.5);
                                    let mut path = PathBuilder::stroke(px(1.));
                                    path.move_to(pt(start, line_y));
                                    path.line_to(pt(middle, line_y));
                                    path.line_to(dot);
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, color);
                                    }
                                    for center in [pt(start, line_y), dot] {
                                        let radius = sx * 2.5;
                                        window.paint_quad(quad(
                                            Bounds::new(
                                                center - point(radius, radius),
                                                size(radius * 2., radius * 2.),
                                            ),
                                            radius,
                                            color,
                                            px(0.),
                                            color,
                                            BorderStyle::Solid,
                                        ));
                                    }
                                    if focused.get(index) == Some(&true) {
                                        let radius = sx * 5.5;
                                        window.paint_quad(quad(
                                            Bounds::new(
                                                dot - point(radius, radius),
                                                size(radius * 2., radius * 2.),
                                            ),
                                            radius,
                                            color.opacity(0.),
                                            px(1.),
                                            color,
                                            BorderStyle::Solid,
                                        ));
                                    }
                                }
                            },
                        )
                        .absolute()
                        .left(css(230.))
                        .top_0()
                        .w(css(310.))
                        .h(css(340.)),
                    )
                    .children(
                        [0, 2, 6, 5, 7]
                            .into_iter()
                            .enumerate()
                            .filter_map(|(row, index)| {
                                inputs.get(index).map(|input| {
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(css(row as f32 * 50.))
                                        .w(css(if index == 7 { 220. } else { 210. }))
                                        .flex()
                                        .justify_end()
                                        .child(self.input_button(input, cx))
                                })
                            }),
                    )
                    .children(
                        [1, 3, 4]
                            .into_iter()
                            .enumerate()
                            .filter_map(|(row, index)| {
                                inputs.get(index).map(|input| {
                                    div()
                                        .absolute()
                                        .right_0()
                                        .top(css(row as f32 * 50.))
                                        .w(css(210.))
                                        .flex()
                                        .justify_start()
                                        .child(self.input_button(input, cx))
                                })
                            }),
                    ),
            )
            .into_any_element()
    }
}
