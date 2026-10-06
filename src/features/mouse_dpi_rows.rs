//! Product 70 cI/EI/XT: current source receipts in mouse-70-dpi-current-evidence.json.
use super::*;
use gpui_kit::{base::Button as BaseButton, prelude::FluentBuilder as _};

struct Palette;
impl Palette {
    fn green() -> Hsla {
        rgb(0x44d62c).into()
    }
    fn ordinal() -> Hsla {
        rgb(0x222222).into()
    }
    fn dark() -> Hsla {
        rgb(0x111111).into()
    }
    fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    fn pressed() -> Hsla {
        rgb(0x1e1e1e).into()
    }
}

#[derive(Clone)]
struct StageDrag {
    owner: EntityId,
    from: usize,
    ordinal: usize,
    stages: Value,
    active: usize,
}
impl Render for StageDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .w(surface::css(250.))
            .h(surface::css(50.))
            .pl(rems(1.))
            .rounded(surface::css(5.))
            .bg(Palette::green())
            .text_size(surface::css(14.))
            .font_weight(FontWeight::BOLD)
            .text_color(Palette::dark())
            .child(format!("{} {}", t("MOVE_STAGE"), self.ordinal))
    }
}

impl MouseProductWorkspace {
    fn dpi_row_visible(&self, index: usize) -> bool {
        self.draft
            .pointer(&format!("{}/{index}/Active", self.spec.stages_path()))
            .and_then(Value::as_bool)
            == Some(true)
    }
    fn dpi_select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.spec.product_id != 70 || !self.dpi_row_visible(index) {
            return;
        }
        self.write(self.spec.active_path(), json!(index + 1), cx);
    }
    fn dpi_toggle_xy(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let base = format!("{}/{index}", self.spec.stages_path());
        if self.draft.pointer(&base).is_none() {
            return;
        }
        let independent = !self.boolean(&format!("{base}/Independent"));
        let x = self.number(&format!("{base}/X"));
        let y_path = format!("{base}/Y");
        let y = self.number(&y_path);
        set_pointer(
            &mut self.draft,
            &format!("{base}/Independent"),
            json!(independent),
        );
        // cI.toggleY -> II/$xU only changes independent. The separate TI/Tme
        // dispatch selects this row (even when hidden) only if Y needs resetting.
        if !independent && x != y {
            set_pointer(&mut self.draft, &y_path, json!(x as i64));
            set_pointer(&mut self.draft, self.spec.active_path(), json!(index + 1));
            self.syncing = true;
            if let Some(input) = self.inputs.get(&y_path) {
                input.update(cx, |input, cx| {
                    input.set_value((x as i64).to_string(), window, cx)
                });
            }
            if let Some(slider) = self.sliders.get(&y_path) {
                slider.update(cx, |slider, cx| slider.set_value(x, window, cx));
            }
            self.syncing = false;
        }
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    fn dpi_toggle_visibility(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.boolean(self.spec.stage_enable_path()) {
            return;
        }
        let path = self.spec.stages_path();
        let Some(stages) = self.draft.pointer(path).and_then(Value::as_array) else {
            return;
        };
        if index >= stages.len() {
            return;
        }
        let mut stages = stages.clone();
        let visible = stages[index]["Active"] == true;
        if visible
            && stages
                .iter()
                .filter(|stage| stage["Active"] == true)
                .count()
                <= 2
        {
            return;
        }
        stages[index]["Active"] = (!visible).into();
        let active = self.number(self.spec.active_path()) as usize;
        if visible && active == index + 1 {
            let next = stages
                .iter()
                .enumerate()
                .find(|(slot, stage)| *slot > index && stage["Active"] == true)
                .or_else(|| {
                    stages
                        .iter()
                        .enumerate()
                        .find(|(_, stage)| stage["Active"] == true)
                });
            if let Some((next, _)) = next {
                set_pointer(&mut self.draft, self.spec.active_path(), json!(next + 1));
            }
        }
        self.write(path, Value::Array(stages), cx);
    }
    fn dpi_drop_row(
        &mut self,
        drag: &StageDrag,
        to: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.owner != cx.entity_id()
            || !self.boolean(self.spec.stage_enable_path())
            || self.draft.pointer(self.spec.stages_path()) != Some(&drag.stages)
            || self.number(self.spec.active_path()) as usize != drag.active
        {
            return;
        }
        let Some(stages) = drag.stages.as_array() else {
            return;
        };
        if drag.from >= stages.len()
            || to >= stages.len()
            || drag.from == to
            || drag.active == 0
            || drag.active > stages.len()
        {
            return;
        }
        let mut stages = stages.clone();
        let moved = stages.remove(drag.from);
        stages.insert(to, moved);
        let selected = drag.active.saturating_sub(1);
        let mut active = if selected == drag.from {
            to
        } else if drag.from < selected && selected <= to {
            selected - 1
        } else if to <= selected && selected < drag.from {
            selected + 1
        } else {
            selected
        };
        if stages[active]["Active"] != true {
            active = stages
                .iter()
                .enumerate()
                .find(|(index, stage)| *index > active && stage["Active"] == true)
                .map_or(0, |(index, _)| index);
        }
        set_pointer(
            &mut self.draft,
            self.spec.stages_path(),
            Value::Array(stages),
        );
        set_pointer(&mut self.draft, self.spec.active_path(), json!(active + 1));
        self.syncing = true;
        for (path, input) in &self.inputs {
            if path.starts_with(self.spec.stages_path()) {
                let value = self.number(path);
                input.update(cx, |input, cx| {
                    input.set_value((value as u32).to_string(), window, cx)
                });
                if let Some(slider) = self.sliders.get(path) {
                    slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                }
            }
        }
        self.syncing = false;
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    pub(super) fn dpi_rows_70(&self, cx: &Context<Self>) -> AnyElement {
        let stages = self
            .draft
            .pointer(self.spec.stages_path())
            .and_then(Value::as_array);
        let Some(stages) = stages else {
            return div().into_any_element();
        };
        let enabled = self.boolean(self.spec.stage_enable_path());
        let selected = (self.number(self.spec.active_path()) as usize).saturating_sub(1);
        let dragging = cx.has_active_drag();
        let owner = cx.entity_id();
        let mut rows = div().flex().flex_col();
        let mut ordinal = 0;
        for (index, stage) in stages.iter().enumerate() {
            if !enabled && index != selected {
                continue;
            }
            let visible = stage["Active"] == true;
            if visible {
                ordinal += 1;
            }
            let current = selected == index;
            let independent = stage["Independent"] == true;
            let group = SharedString::from(format!("dpi-70-row-{index}"));
            let mut controls = div().flex().flex_col();
            for axis in 0..if independent { 2 } else { 1 } {
                let path = format!(
                    "{}/{index}/{}",
                    self.spec.stages_path(),
                    self.spec.dpi_axis(axis)
                );
                if let (Some(input), Some(slider)) =
                    (self.inputs.get(&path), self.sliders.get(&path))
                {
                    controls = controls.child(
                        div()
                            .flex()
                            .items_center()
                            .h(surface::css(27.))
                            .mb(surface::css(10.))
                            .child(super::dpi_number::DpiNumber {
                                path: path.clone(),
                                group: group.clone(),
                                input: input.clone(),
                                owner: cx.entity().downgrade(),
                                disabled: dragging,
                                min: self.spec.min_dpi,
                                max: self.spec.max_dpi,
                                typed: self.dpi_numbers.get(&path).is_some_and(|state| state.typed),
                            })
                            .child(
                                Slider::new(slider)
                                    .disabled(dragging || !visible)
                                    .w(surface::css(250.))
                                    .h(surface::css(20.))
                                    .ml(surface::css(10.)),
                            ),
                    );
                }
            }
            let badge = BaseButton::new(("dpi-70-ordinal", index))
                .accessibility_label(format!("{} {}", t("STAGE"), ordinal))
                .relative()
                .size(surface::css(30.))
                .ml(surface::css(20.))
                .rounded_full()
                .p_0()
                .bg(if current {
                    Palette::green()
                } else {
                    Palette::ordinal()
                })
                .text_color(if current {
                    Palette::dark()
                } else {
                    Palette::text()
                })
                .when(visible, |view| {
                    view.child(
                        img(SharedString::from(format!("synapse/stage-{ordinal}.svg")))
                            .absolute()
                            .top(surface::css(3.))
                            .left(surface::css(11.))
                            .w(surface::css(8.))
                            .h(surface::css(6.)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(surface::css(10.))
                            .child(ordinal.to_string()),
                    )
                })
                .on_click(cx.listener(move |this, _, _, cx| this.dpi_select_row(index, cx)));
            let asset = if !visible {
                "synapse/sensitivity-xy-disabled.svg"
            } else if independent {
                "synapse/sensitivity-xy-active.svg"
            } else {
                "synapse/sensitivity-xy.svg"
            };
            let xy_group = SharedString::from(format!("dpi-70-xy-{index}"));
            let xy_label = t(if independent {
                "DISABLE_XY"
            } else {
                "ENABLE_XY"
            });
            let xy = BaseButton::new(("dpi-70-xy", index))
                .accessibility_label(xy_label.clone())
                .group(xy_group.clone())
                .relative()
                .hidden()
                .group_hover(group.clone(), |style| style.flex())
                .p_0()
                .size(surface::css(32.))
                .ml(surface::css(10.))
                .child(img(asset).size_full())
                .child(
                    div()
                        .absolute()
                        .left(surface::css(20.))
                        .top(surface::css(35.))
                        .hidden()
                        .group_hover(xy_group, |style| style.block())
                        .bg(rgb(0x000000))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .text_color(Palette::text())
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .whitespace_nowrap()
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .child(xy_label),
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.dpi_toggle_xy(index, window, cx)),
                );
            let row = div()
                .id(group.clone())
                .group(group.clone())
                .flex()
                .items_center()
                .h(surface::css(if independent { 114. } else { 68. }))
                .mb(surface::css(if index + 1 == stages.len() || !enabled {
                    0.
                } else {
                    4.
                }))
                .rounded(surface::css(3.))
                .hover(|style| style.bg(Palette::ordinal()))
                .active(|style| style.bg(Palette::pressed()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .when(!visible, |view| view.opacity(0.3))
                        .child(badge)
                        .child(controls)
                        .child(xy),
                )
                .when(enabled, |view| {
                    view.child(
                        div()
                            .hidden()
                            .group_hover(group.clone(), |style| style.flex())
                            .child(
                                gpui_kit::component::switch::Switch::new(("dpi-70-visible", index))
                                    .checked(visible)
                                    .disabled(
                                        visible
                                            && stages
                                                .iter()
                                                .filter(|s| s["Active"] == true)
                                                .count()
                                                <= 2,
                                    )
                                    .ml(surface::css(15.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.dpi_toggle_visibility(index, cx)
                                    })),
                            ),
                    )
                    .child(
                        BaseButton::new(("dpi-70-drag", index))
                            .hidden()
                            .group_hover(group.clone(), |style| style.flex())
                            .when(!visible, |view| view.opacity(0.3))
                            .accessibility_label(t("STAGE"))
                            .w(surface::css(10.))
                            .h_full()
                            .ml(surface::css(15.))
                            .p_0()
                            .child(img("synapse/dpi-draggable.svg").w(surface::css(10.)))
                            .on_drag(
                                StageDrag {
                                    owner: cx.entity_id(),
                                    from: index,
                                    ordinal,
                                    stages: Value::Array(stages.clone()),
                                    active: selected + 1,
                                },
                                |drag, _, _, cx| cx.new(|_| drag.clone()),
                            ),
                    )
                })
                .drag_over::<StageDrag>(move |style, drag, _, _| {
                    if drag.owner != owner || drag.from == index || !enabled {
                        return style;
                    }
                    let style = if drag.from > index {
                        style.border_t_2()
                    } else {
                        style.border_b_2()
                    };
                    style.border_color(Palette::green())
                })
                .on_drop(cx.listener(move |this, drag: &StageDrag, window, cx| {
                    this.dpi_drop_row(drag, index, window, cx)
                }));
            rows = rows.child(row);
        }
        surface::panel(t("SENSITIVITY_HEADER"), cx)
            .child(self.toggle(self.spec.stage_enable_path(), t("SENSITIVITY_STAGES"), cx))
            .child(
                div()
                    .relative()
                    .h(surface::css(20.))
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(50.))
                            .bottom_0()
                            .child("DPI"),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(110.))
                            .bottom_0()
                            .child(self.spec.min_dpi.to_string()),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(375.))
                            .bottom_0()
                            .child(self.spec.max_dpi.to_string()),
                    ),
            )
            .child(rows)
            .into_any_element()
    }
}
