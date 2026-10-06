//! Current 70 cI/EI/XT and 226 Ms/ls/es, with independently verified schemas.
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
    fn dragged() -> Hsla {
        rgba(0x44d62c33).into()
    }
    fn drag_overlay() -> Hsla {
        rgba(0x44c62d33).into()
    }
}

#[derive(Clone)]
struct StageDrag {
    owner: EntityId,
    from: usize,
    ordinal: Option<usize>,
    stages: Value,
    active: usize,
    generation: u64,
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
            .child(
                div()
                    .relative()
                    .size(surface::css(30.))
                    .ml(surface::css(20.))
                    .mr(surface::css(10.))
                    .rounded_full()
                    .bg(Palette::ordinal())
                    .text_color(Palette::text())
                    .children(self.ordinal.map(|ordinal| {
                        div()
                            .size_full()
                            .child(
                                img(SharedString::from(format!("synapse/stage-{ordinal}.svg")))
                                    .absolute()
                                    .top(surface::css(3.))
                                    .left(surface::css(10.5))
                                    .w(surface::css(9.))
                                    .h(surface::css(7.)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(surface::css(10.))
                                    .left(surface::css(11.5))
                                    .child(ordinal.to_string()),
                            )
                    })),
            )
            .child(format!(
                "{} {}",
                t("MOVE_STAGE"),
                self.ordinal
                    .map_or_else(String::new, |ordinal| ordinal.to_string())
            ))
    }
}

impl MouseProductWorkspace {
    fn dpi_row_visible(&self, index: usize) -> bool {
        self.draft
            .pointer(&format!(
                "{}/{index}/{}",
                self.spec.stages_path(),
                self.spec.visible_key()
            ))
            .and_then(Value::as_bool)
            == Some(true)
    }
    fn dpi_select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.dpi_editing_enabled() {
            return;
        }
        if !matches!(self.spec.product_id, 70 | 226)
            || !self.dpi_row_visible(index)
            || (!self.boolean(self.spec.stage_enable_path())
                && self.number(self.spec.active_path()) as usize != index + 1)
        {
            return;
        }
        self.write(self.spec.active_path(), json!(index + 1), cx);
    }
    fn dpi_toggle_xy(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dpi_editing_enabled() {
            return;
        }
        let base = format!("{}/{index}", self.spec.stages_path());
        if self.draft.pointer(&base).is_none() {
            return;
        }
        let independent = !self.boolean(&format!("{base}/{}", self.spec.independent_key()));
        let x = self.number(&format!("{base}/{}", self.spec.dpi_axis(0)));
        let y_path = format!("{base}/{}", self.spec.dpi_axis(1));
        let y = self.number(&y_path);
        set_pointer(
            &mut self.draft,
            &format!("{base}/{}", self.spec.independent_key()),
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
    fn dpi_toggle_visibility(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dpi_editing_enabled() {
            return;
        }
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
        let visible_key = self.spec.visible_key();
        let visible = stages[index][visible_key] == true;
        if visible
            && stages
                .iter()
                .filter(|stage| stage[visible_key] == true)
                .count()
                <= 2
        {
            return;
        }
        stages[index][visible_key] = (!visible).into();
        let active = self.number(self.spec.active_path()) as usize;
        if visible && active == index + 1 {
            let next = stages
                .iter()
                .enumerate()
                .find(|(slot, stage)| *slot > index && stage[visible_key] == true)
                .or_else(|| {
                    stages
                        .iter()
                        .enumerate()
                        .find(|(_, stage)| stage[visible_key] == true)
                });
            if let Some((next, _)) = next {
                set_pointer(&mut self.draft, self.spec.active_path(), json!(next + 1));
            }
        }
        self.write(path, Value::Array(stages), cx);
        self.dismiss_editors(window, cx);
    }
    fn dpi_drop_row(
        &mut self,
        drag: &StageDrag,
        to: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.owner != cx.entity_id()
            || !self.dpi_editing_enabled()
            || drag.generation != self.draft_generation
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
        let visible_key = self.spec.visible_key();
        if stages[active][visible_key] != true {
            active = stages
                .iter()
                .enumerate()
                .find(|(index, stage)| *index > active && stage[visible_key] == true)
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
        self.dismiss_editors(window, cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    pub(super) fn dpi_rows(&self, cx: &Context<Self>) -> AnyElement {
        let stages = self
            .draft
            .pointer(self.spec.stages_path())
            .and_then(Value::as_array);
        let Some(stages) = stages else {
            return div().into_any_element();
        };
        let enabled = self.boolean(self.spec.stage_enable_path());
        let selected = (self.number(self.spec.active_path()) as usize).saturating_sub(1);
        let dragging = self.dpi_dragged_row.is_some();
        let owner = cx.entity_id();
        let product_id = self.spec.product_id;
        let editing_enabled = self.dpi_editing_enabled();
        let mut rows = div()
            .flex()
            .flex_col()
            .opacity(if editing_enabled { 1. } else { 0.3 })
            .when(product_id == 226, |rows| rows.mx(surface::css(-20.)));
        let mut ordinal = 0;
        for (index, stage) in stages.iter().enumerate() {
            let drag_owner = cx.entity().downgrade();
            if !enabled && index != selected {
                continue;
            }
            let visible = stage[self.spec.visible_key()] == true;
            if visible {
                ordinal += 1;
            }
            let current = selected == index;
            let independent = stage[self.spec.independent_key()] == true;
            let group = SharedString::from(format!("dpi-{product_id}-row-{index}"));
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
                                disabled: dragging || !editing_enabled,
                                min: self.spec.min_dpi,
                                max: self.spec.max_dpi,
                                typed: self.dpi_numbers.get(&path).is_some_and(|state| state.typed),
                            })
                            .child(if let Some(grid) = self.dpi_grids.get(&path) {
                                super::dpi_grid::Grid::new(
                                    grid,
                                    !dragging && visible && editing_enabled,
                                    independent.then_some(if axis == 0 { "X" } else { "Y" }),
                                )
                                .into_any_element()
                            } else {
                                Slider::new(slider)
                                    .disabled(dragging || !visible)
                                    .w(surface::css(250.))
                                    .h(surface::css(20.))
                                    .ml(surface::css(10.))
                                    .into_any_element()
                            }),
                    );
                }
            }
            let badge = BaseButton::new((ElementId::from(group.clone()), "ordinal"))
                .disabled(!editing_enabled)
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
            let xy_group = SharedString::from(format!("dpi-{product_id}-xy-{index}"));
            let xy_label = t(if independent {
                "DISABLE_XY"
            } else {
                "ENABLE_XY"
            });
            let xy = BaseButton::new((ElementId::from(group.clone()), "xy"))
                .disabled(!editing_enabled)
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
                .relative()
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
                .when(self.dpi_dragged_row == Some(index), |row| {
                    row.bg(Palette::dragged())
                        .hover(|style| style.bg(Palette::dragged()))
                        .active(|style| style.bg(Palette::dragged()))
                })
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
                                gpui_kit::component::switch::Switch::new((
                                    ElementId::from(group.clone()),
                                    "visible",
                                ))
                                .checked(visible)
                                .disabled(
                                    !editing_enabled
                                        || visible
                                            && stages
                                                .iter()
                                                .filter(|s| s[self.spec.visible_key()] == true)
                                                .count()
                                                <= 2,
                                )
                                .ml(surface::css(15.))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.dpi_toggle_visibility(index, window, cx)
                                    },
                                )),
                            ),
                    )
                    .child(
                        BaseButton::new((ElementId::from(group.clone()), "drag"))
                            .disabled(!editing_enabled)
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
                                    ordinal: if product_id == 226 {
                                        // ls's drag caption reads capital Active,
                                        // not the visible field used by normal rows.
                                        (stage["Active"] == true).then(|| {
                                            stages[..=index]
                                                .iter()
                                                .filter(|stage| stage["Active"] == true)
                                                .count()
                                        })
                                    } else {
                                        Some(ordinal)
                                    },
                                    stages: Value::Array(stages.clone()),
                                    active: selected + 1,
                                    generation: self.draft_generation,
                                },
                                move |drag, _, _, cx| {
                                    let _ = drag_owner.update(cx, |owner, cx| {
                                        owner.dpi_dragged_row = Some(drag.from);
                                        cx.notify();
                                    });
                                    let release_owner = drag_owner.clone();
                                    cx.new(|cx| {
                                        cx.on_release(move |_, cx| {
                                            let _ = release_owner.update(cx, |owner, cx| {
                                                owner.dpi_dragged_row = None;
                                                cx.notify();
                                            });
                                        })
                                        .detach();
                                        drag.clone()
                                    })
                                },
                            ),
                    )
                })
                .when(self.dpi_dragged_row == Some(index), |row| {
                    row.child(div().absolute().size_full().bg(Palette::drag_overlay()))
                })
                .drag_over::<StageDrag>(move |style, drag, _, _| {
                    if drag.owner != owner || drag.from == index || !enabled || !editing_enabled {
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
        surface::panel_with_control(
            t("SENSITIVITY_HEADER"),
            surface::help_control("dpi-stages-help", t("SENSITIVITY_TOOLTIP")),
            cx,
        )
        .when(product_id == 226, |panel| {
            panel.child(
                div()
                    .text_size(surface::css(14.))
                    .text_color(Palette::text())
                    .child(t("SENSITIVITY_DESC")),
            )
        })
        .child(
            div()
                .mt(surface::css(25.))
                .h(surface::css(27.))
                .flex()
                .items_center()
                .text_size(surface::css(14.))
                .child(t("SENSITIVITY_STAGES"))
                .child(
                    div().ml(surface::css(10.)).mt(surface::css(-1.)).child(
                        surface::SynapseSwitch::new("dpi-stages-enabled")
                            .disabled(!editing_enabled)
                            .accessibility_label(t("SENSITIVITY_STAGES"))
                            .checked(enabled)
                            .on_change(cx.listener(|this, value, window, cx| {
                                if !this.dpi_editing_enabled() {
                                    return;
                                }
                                this.write(this.spec.stage_enable_path(), json!(*value), cx);
                                this.dismiss_editors(window, cx);
                            })),
                    ),
                ),
        )
        .child(
            div()
                .relative()
                .h(surface::css(20.))
                .opacity(if editing_enabled { 1. } else { 0.3 })
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
