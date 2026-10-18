//! Source-declared DPI rows with independently verified schema and presentation capabilities.
use super::*;
use gpui_kit::base::Button as BaseButton;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct HoverState {
    row: Option<usize>,
    xy: Option<usize>,
}

impl HoverState {
    fn row_is_hovered(&self, index: usize) -> bool {
        self.row == Some(index)
    }

    fn xy_is_hovered(&self, index: usize) -> bool {
        self.xy == Some(index)
    }

    fn set_row(&mut self, index: usize, entered: bool) -> bool {
        let previous = *self;
        if entered && self.row != Some(index) {
            self.row = Some(index);
            self.xy = None;
        } else if !entered && self.row == Some(index) {
            self.clear();
        }
        *self != previous
    }

    fn set_xy(&mut self, index: usize, entered: bool) -> bool {
        let previous = *self;
        if entered && self.row == Some(index) {
            self.xy = Some(index);
        } else if !entered && self.xy == Some(index) {
            self.xy = None;
        }
        *self != previous
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Deserialize)]
pub(super) struct RowsSpec {
    product_id: u32,
    stages_path: String,
    active_path: String,
    enabled_path: String,
    x_key: String,
    y_key: String,
    independent_key: String,
    visible_key: String,
    minimum_visible_stages: usize,
    rows_margin_x: f32,
    description_key: Option<String>,
    drag_ordinal_key: Option<String>,
    #[serde(default = "row_height")]
    row_height: f32,
    #[serde(default = "xy_row_height")]
    xy_row_height: f32,
    #[serde(default = "control_margin_bottom")]
    control_margin_bottom: f32,
    #[serde(default = "header_left")]
    header_left: [f32; 3],
    #[serde(default)]
    two_way_tab: bool,
    #[serde(default)]
    no_header: bool,
    #[serde(default)]
    single_min: Option<u32>,
    #[serde(default)]
    single_max: Option<u32>,
    #[serde(default = "slider_requires_visible")]
    slider_requires_visible: bool,
}
fn row_height() -> f32 {
    68.
}
fn xy_row_height() -> f32 {
    114.
}
fn control_margin_bottom() -> f32 {
    10.
}
fn header_left() -> [f32; 3] {
    [50., 110., 375.]
}
fn slider_requires_visible() -> bool {
    true
}

pub(super) fn source_spec(product_id: u32) -> Option<&'static RowsSpec> {
    static SPECS: OnceLock<Vec<RowsSpec>> = OnceLock::new();
    SPECS
        .get_or_init(|| read_mouse_dpi_rows_catalog())
        .iter()
        .find(|spec| spec.product_id == product_id)
}

impl RowsSpec {
    pub(super) fn single_limits(&self) -> Option<(u32, u32)> {
        if self.two_way_tab {
            Some((self.single_min?, self.single_max?))
        } else {
            None
        }
    }
    pub(super) fn slider_requires_visible(&self) -> bool {
        self.slider_requires_visible
    }
    pub(super) fn stages_path(&self) -> &str {
        &self.stages_path
    }
    pub(super) fn active_path(&self) -> &str {
        &self.active_path
    }
    pub(super) fn enabled_path(&self) -> &str {
        &self.enabled_path
    }
    pub(super) fn x_key(&self) -> &str {
        &self.x_key
    }
    pub(super) fn y_key(&self) -> &str {
        &self.y_key
    }
    pub(super) fn independent_key(&self) -> &str {
        &self.independent_key
    }
    pub(super) fn visible_key(&self) -> &str {
        &self.visible_key
    }
    pub(super) fn has_number(&self, path: &str) -> bool {
        let Some((slot, axis)) = path
            .strip_prefix(self.stages_path())
            .and_then(|path| path.strip_prefix('/'))
            .and_then(|path| path.split_once('/'))
        else {
            return false;
        };
        slot.parse::<usize>().is_ok() && (axis == self.x_key || axis == self.y_key)
    }
    fn drag_ordinal(&self, stages: &[Value], index: usize) -> Option<usize> {
        let key = self.drag_ordinal_key.as_deref()?;
        (stages.get(index)?[key] == true).then(|| {
            stages[..=index]
                .iter()
                .filter(|stage| stage[key] == true)
                .count()
        })
    }
}

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
    fn dpi_row_hover_changed(
        &mut self,
        index: usize,
        entered: bool,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        if generation == self.draft_generation
            && self.active
            && self.page == "TAB_PERFORMANCE"
            && self.dpi_dragged_row.is_none()
            && self.dpi_hover.set_row(index, entered)
        {
            cx.notify();
        }
    }

    fn dpi_xy_hover_changed(
        &mut self,
        index: usize,
        entered: bool,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        if generation == self.draft_generation
            && self.active
            && self.page == "TAB_PERFORMANCE"
            && self.dpi_dragged_row.is_none()
            && self.dpi_hover.set_xy(index, entered)
        {
            cx.notify();
        }
    }

    pub(super) fn prepare_dpi_single_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((minimum, maximum)) =
            source_spec(self.spec.product_id).and_then(RowsSpec::single_limits)
        else {
            return;
        };
        let count = self
            .draft
            .pointer(self.spec.stages_path())
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        for slot in 0..count {
            let path = format!(
                "{}/{slot}/{}",
                self.spec.stages_path(),
                self.spec.dpi_axis(0)
            );
            let value = self.number(&path).clamp(minimum as f32, maximum as f32);
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(minimum as f32)
                    .max(maximum as f32)
                    .step(self.spec.dpi_step as f32)
                    .default_value(value)
            });
            self.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing {
                        return;
                    }
                    if !this.dpi_editing_enabled()
                        || this.boolean(this.spec.stage_enable_path())
                        || this.number(this.spec.active_path()) as usize != slot + 1
                    {
                        this.sync_dpi_single_controls(window, cx);
                        return;
                    }
                    // The source 70130 range retains pointer previews in its own
                    // state; changeValue reaches the sensitivity parent on release.
                    match event {
                        SliderEvent::Change(_) => cx.notify(),
                        SliderEvent::Release(value) => {
                            this.write_number(
                                &path,
                                value.start().clamp(minimum as f32, maximum as f32),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ));
            self.dpi_single_sliders.insert(slot, slider);
        }
    }
    pub(super) fn sync_dpi_single_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((minimum, maximum)) =
            source_spec(self.spec.product_id).and_then(RowsSpec::single_limits)
        else {
            return;
        };
        for (slot, slider) in &self.dpi_single_sliders {
            let path = format!(
                "{}/{slot}/{}",
                self.spec.stages_path(),
                self.spec.dpi_axis(0)
            );
            let value = self.number(&path).clamp(minimum as f32, maximum as f32);
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
    }
    fn set_dpi_stage_mode(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dpi_editing_enabled() {
            return;
        }
        let previous = self.boolean(self.spec.stage_enable_path());
        self.write(self.spec.stage_enable_path(), json!(enabled), cx);
        if previous
            && !enabled
            && let Some((minimum, maximum)) =
                source_spec(self.spec.product_id).and_then(RowsSpec::single_limits)
            && let Some(stages) = self
                .draft
                .pointer(self.spec.stages_path())
                .and_then(Value::as_array)
        {
            let mut stages = stages.clone();
            for stage in &mut stages {
                for axis in 0..2 {
                    let key = self.spec.dpi_axis(axis);
                    if let Some(value) = stage[key].as_u64() {
                        stage[key] = json!(value.clamp(u64::from(minimum), u64::from(maximum)));
                    }
                }
            }
            set_pointer(
                &mut self.draft,
                self.spec.stages_path(),
                Value::Array(stages),
            );
            self.syncing = true;
            for (path, slider) in &self.sliders {
                if path.starts_with(self.spec.stages_path()) {
                    let value = self.number(path);
                    slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                    if let Some(input) = self.inputs.get(path) {
                        input.update(cx, |input, cx| {
                            input.set_value((value as u32).to_string(), window, cx)
                        });
                    }
                }
            }
            self.syncing = false;
            self.submit_current_dpi(current_dpi::Edit::Stages, cx);
            cx.emit(MouseProductChanged);
        }
        self.dismiss_editors(window, cx);
        self.sync_dpi_single_controls(window, cx);
        cx.notify();
    }
    fn dpi_stage_mode_control(
        &self,
        row_spec: &RowsSpec,
        enabled: bool,
        editing_enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        if !self.current_dpi_profile_known() {
            return div()
                .mt(surface::css(25.))
                .h(surface::css(27.))
                .flex()
                .items_center()
                .text_size(surface::css(14.))
                .opacity(0.3)
                .child(t("SENSITIVITY_STAGES"))
                .child(div().ml(surface::css(10.)).child("—"))
                .into_any_element();
        }
        if row_spec.two_way_tab {
            return div()
                .h(surface::css(72.))
                .pt(surface::css(12.))
                .child(
                    BaseButton::new("dpi-stage-mode")
                        .role(Role::Switch)
                        .selected(enabled)
                        .disabled(!editing_enabled)
                        .accessibility_label(t("SENSITIVITY_STAGES"))
                        .h(surface::css(36.))
                        .p(surface::css(5.))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .rounded(surface::css(18.))
                        .bg(Palette::dark())
                        .hover(|style| style.border_color(Palette::green()))
                        .child(h_flex().children([false, true].into_iter().map(|staged| {
                            let selected = staged == enabled;
                            div()
                                .h(surface::css(26.))
                                .px(surface::css(10.))
                                .py(surface::css(5.))
                                .rounded(surface::css(13.))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .bg(if selected {
                                    Palette::green()
                                } else {
                                    Palette::dark()
                                })
                                .text_color(if selected {
                                    Palette::dark()
                                } else {
                                    Palette::text()
                                })
                                .child(t(if staged { "ADVANCED" } else { "BASIC" }))
                        })))
                        // Source TwoWay routes either half, including the selected
                        // half, to toggleStages; it does not select by target half.
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_dpi_stage_mode(
                                !this.boolean(this.spec.stage_enable_path()),
                                window,
                                cx,
                            );
                        })),
                )
                .into_any_element();
        }
        div()
            .mt(surface::css(25.))
            .h(surface::css(27.))
            .flex()
            .items_center()
            .text_size(surface::css(14.))
            .child(
                div()
                    .opacity(if editing_enabled { 1. } else { 0.3 })
                    .child(t("SENSITIVITY_STAGES")),
            )
            .child(
                div().ml(surface::css(10.)).mt(surface::css(-1.)).child(
                    surface::SynapseSwitch::new("dpi-stages-enabled")
                        .disabled(!editing_enabled)
                        .accessibility_label(t("SENSITIVITY_STAGES"))
                        .checked(enabled)
                        .on_change(cx.listener(|this, value, window, cx| {
                            this.set_dpi_stage_mode(*value, window, cx);
                        })),
                ),
            )
            .into_any_element()
    }
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
        if source_spec(self.spec.product_id).is_none()
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
        self.submit_legacy_dpi(!independent && x != y, cx);
        self.submit_current_dpi(
            current_dpi::Edit::AxisMode {
                reset_y: !independent && x != y,
            },
            cx,
        );
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    fn dpi_toggle_visibility(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row_spec) = source_spec(self.spec.product_id) else {
            return;
        };
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
                <= row_spec.minimum_visible_stages
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
        self.move_current_dpi_row(drag.from, to);
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
        self.submit_legacy_dpi(true, cx);
        self.submit_current_dpi(current_dpi::Edit::Stages, cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    pub(super) fn dpi_rows(&self, cx: &Context<Self>) -> AnyElement {
        let Some(row_spec) = source_spec(self.spec.product_id) else {
            return div().into_any_element();
        };
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
        let generation = self.draft_generation;
        let mut rows = div()
            .flex()
            .flex_col()
            .opacity(if editing_enabled { 1. } else { 0.3 })
            .mx(surface::css(row_spec.rows_margin_x));
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
            let observed = self.current_dpi_row_available(index);
            let independent = self.spec.support_xy && stage[self.spec.independent_key()] == true;
            let group = SharedString::from(format!("dpi-{product_id}-row-{index}"));
            // Resolve the source's `.stage:hover` display before layout so
            // prepaint and paint receive the same participating children.
            let row_hovered = !dragging && self.dpi_hover.row_is_hovered(index);
            let xy_hovered = row_hovered && self.dpi_hover.xy_is_hovered(index);
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
                            .mb(surface::css(row_spec.control_margin_bottom))
                            .child(if observed { super::dpi_number::DpiNumber {
                                path: path.clone(),
                                group: group.clone(),
                                input: input.clone(),
                                owner: cx.entity().downgrade(),
                                disabled: dragging || !editing_enabled,
                                min: self.spec.min_dpi,
                                max: self.spec.max_dpi,
                                typed: self.dpi_numbers.get(&path).is_some_and(|state| state.typed),
                            }.into_any_element() } else { self.current_dpi_unknown_number(cx) })
                            .child(if !observed {
                                self.current_dpi_unknown_track(cx)
                            } else if let Some(grid) = self.dpi_grids.get(&path) {
                                super::dpi_grid::Grid::new(
                                    grid,
                                    !dragging && visible && editing_enabled,
                                    independent.then_some(if axis == 0 { "X" } else { "Y" }),
                                )
                                .into_any_element()
                            } else {
                                div()
                                    .w(surface::css(250.))
                                    .h(surface::css(20.))
                                    .ml(surface::css(10.))
                                    .child(
                                        super::mapping_sensitivity_slider_current::SensitivitySlider::new(
                                            slider,
                                            self.spec.min_dpi,
                                            self.spec.max_dpi,
                                            independent.then_some(if axis == 0 { "X" } else { "Y" }),
                                            false,
                                        )
                                        .stage_row()
                                        .enabled(!dragging && editing_enabled
                                            && (!row_spec.slider_requires_visible || visible)),
                                    )
                                    .into_any_element()
                            }),
                    );
                }
            }
            let badge = BaseButton::new((ElementId::from(group.clone()), "ordinal"))
                .disabled(!editing_enabled || !observed)
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
                .text_color(if current { Palette::dark() } else { Palette::text() })
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
            let xy_label = t(if independent { "DISABLE_XY" } else { "ENABLE_XY" });
            let xy = BaseButton::new((ElementId::from(group.clone()), "xy"))
                .disabled(!editing_enabled || !observed)
                .accessibility_label(xy_label.clone())
                .relative()
                .flex()
                .when(!row_hovered, |view| view.hidden())
                .p_0()
                .size(surface::css(32.))
                .ml(surface::css(10.))
                .child(img(asset).size_full())
                .child(
                    div()
                        .absolute()
                        .left(surface::css(20.))
                        .top(surface::css(35.))
                        .block()
                        .when(!xy_hovered, |view| view.hidden())
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
                )
                .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                    this.dpi_xy_hover_changed(index, *entered, generation, cx);
                }));
            let row = div()
                .id(SharedString::from(format!(
                    "dpi-{product_id}-row-{index}-generation-{generation}"
                )))
                .group(group.clone())
                .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                    this.dpi_row_hover_changed(index, *entered, generation, cx);
                }))
                .relative()
                .flex()
                .items_center()
                .h(surface::css(if independent {
                    row_spec.xy_row_height
                } else {
                    row_spec.row_height
                }))
                .mb(surface::css(if index + 1 == stages.len() || !enabled {
                    0.
                } else {
                    4.
                }))
                .rounded(surface::css(3.))
                // GPUI accepts one hover refinement per element; a dragged row
                // keeps its drag background for hover and press.
                .map(|row| {
                    if self.dpi_dragged_row == Some(index) {
                        row.bg(Palette::dragged())
                            .hover(|style| style.bg(Palette::dragged()))
                            .active(|style| style.bg(Palette::dragged()))
                    } else {
                        row.hover(|style| style.bg(Palette::ordinal()))
                            .active(|style| style.bg(Palette::pressed()))
                    }
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .when(!visible, |view| view.opacity(0.3))
                        .child(badge)
                        .child(controls)
                        .children(self.spec.support_xy.then_some(xy)),
                )
                .when(enabled, |view| {
                    view.child(
                        div().flex().when(!row_hovered, |view| view.hidden()).child(
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
                                            <= row_spec.minimum_visible_stages,
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
                            .flex()
                            .when(!row_hovered, |view| view.hidden())
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
                                    ordinal: row_spec.drag_ordinal(stages, index),
                                    stages: Value::Array(stages.clone()),
                                    active: selected + 1,
                                    generation: self.draft_generation,
                                },
                                move |drag, _, _, cx| {
                                    let _ = drag_owner.update(cx, |owner, cx| {
                                        if owner.dpi_editing_enabled()
                                            && owner.draft_generation == drag.generation
                                        {
                                            owner.dpi_dragged_row = Some(drag.from);
                                            owner.dpi_hover.clear();
                                            cx.notify();
                                        }
                                    });
                                    let release_owner = drag_owner.clone();
                                    let generation = drag.generation;
                                    let from = drag.from;
                                    cx.new(|cx| {
                                        cx.on_release(move |_, cx| {
                                            let _ = release_owner.update(cx, |owner, cx| {
                                                if owner.draft_generation == generation
                                                    && owner.dpi_dragged_row == Some(from)
                                                {
                                                    owner.draft_generation =
                                                        owner.draft_generation.wrapping_add(1);
                                                    owner.dpi_dragged_row = None;
                                                    owner.dpi_hover.clear();
                                                    cx.notify();
                                                }
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
        let panel = if self.spec.product_id == 70 {
            // Current cI passes disabled to hr; `.widget.disabled>div`
            // dims the title, description and labels as well as the rows.
            v_flex()
                .w_full()
                .flex_shrink_0()
                .my(surface::css(10.))
                .py(surface::css(30.))
                .px(surface::css(40.))
                .bg(Palette::dark())
                .rounded(surface::css(5.))
                .text_size(surface::css(14.))
                .child(
                    h_flex()
                        .justify_between()
                        .opacity(if editing_enabled { 1. } else { 0.3 })
                        .child(
                            div()
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .text_color(Palette::green())
                                .mb(surface::css(20.))
                                .child(t("SENSITIVITY_HEADER").to_uppercase()),
                        )
                        .child(surface::help_control(
                            "dpi-stages-help",
                            t("SENSITIVITY_TOOLTIP"),
                        )),
                )
        } else {
            surface::panel_with_help(
                t("SENSITIVITY_HEADER"),
                surface::help_control("dpi-stages-help", t("SENSITIVITY_TOOLTIP")),
                cx,
            )
        };
        panel
            .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
            .children(self.current_dpi_status(cx))
            .when_some(row_spec.description_key.as_deref(), |panel, key| {
                panel.child(
                    div()
                        .text_size(surface::css(14.))
                        .text_color(Palette::text())
                        .when(self.spec.product_id == 70, |description| {
                            description
                                .mb(surface::css(10.))
                                .opacity(if editing_enabled { 1. } else { 0.3 })
                        })
                        .child(t(key)),
                )
            })
            .child(self.dpi_stage_mode_control(row_spec, enabled, editing_enabled, cx))
            .child(
                if row_spec.two_way_tab && !enabled && !self.current_dpi_row_available(selected) {
                    h_flex()
                        .items_center()
                        .child(self.current_dpi_unknown_number(cx))
                        .child(self.current_dpi_unknown_track(cx))
                        .into_any_element()
                } else if row_spec.two_way_tab && !enabled {
                    self.dpi_single_sliders
                        .get(&selected)
                        .map(|slider| {
                            let (minimum, maximum) =
                                row_spec.single_limits().expect("source BASIC DPI limits");
                            super::mapping_sensitivity_slider_current::SensitivitySlider::new(
                                slider, minimum, maximum, None, false,
                            )
                            .single_dpi()
                            .enabled(editing_enabled)
                            .into_any_element()
                        })
                        .unwrap_or_else(|| div().into_any_element())
                } else {
                    v_flex()
                        .children((!row_spec.no_header).then(|| {
                            div()
                                .relative()
                                .h(surface::css(20.))
                                .opacity(if editing_enabled { 1. } else { 0.3 })
                                .children(
                                    [
                                        (row_spec.header_left[0], "DPI".to_owned()),
                                        (row_spec.header_left[1], self.spec.min_dpi.to_string()),
                                        (row_spec.header_left[2], self.spec.max_dpi.to_string()),
                                    ]
                                    .into_iter()
                                    .map(|(left, label)| {
                                        div()
                                            .absolute()
                                            .left(surface::css(left))
                                            .bottom_0()
                                            .child(label)
                                    }),
                                )
                        }))
                        .child(rows)
                        .into_any_element()
                },
            )
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "mouse_dpi_rows_tests.rs"]
mod hover_tests;

// Generated catalog reader: razer-pages/mouse_dpi_rows_data.
fn read_mouse_dpi_rows_catalog() -> Vec<RowsSpec> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<RowsSpec>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<RowsSpec> {
        let value = match r.tag()? {
            0 => RowsSpec {
                product_id: r.child(decode_2)?,
                stages_path: r.child(decode_3)?,
                active_path: r.child(decode_3)?,
                enabled_path: r.child(decode_3)?,
                x_key: r.child(decode_3)?,
                y_key: r.child(decode_3)?,
                independent_key: r.child(decode_3)?,
                visible_key: r.child(decode_3)?,
                minimum_visible_stages: r.child(decode_4)?,
                rows_margin_x: r.child(decode_5)?,
                description_key: r.child(decode_6)?,
                drag_ordinal_key: None,
                row_height: r.child(decode_5)?,
                xy_row_height: r.child(decode_5)?,
                control_margin_bottom: r.child(decode_5)?,
                header_left: r.child(decode_7)?,
                two_way_tab: r.child(decode_8)?,
                no_header: r.child(decode_8)?,
                single_min: None,
                single_max: None,
                slider_requires_visible: r.child(decode_8)?,
            },
            1 => RowsSpec {
                product_id: r.child(decode_2)?,
                stages_path: r.child(decode_3)?,
                active_path: r.child(decode_3)?,
                enabled_path: r.child(decode_3)?,
                x_key: r.child(decode_3)?,
                y_key: r.child(decode_3)?,
                independent_key: r.child(decode_3)?,
                visible_key: r.child(decode_3)?,
                minimum_visible_stages: r.child(decode_4)?,
                rows_margin_x: r.child(decode_5)?,
                description_key: r.child(decode_6)?,
                drag_ordinal_key: r.child(decode_6)?,
                row_height: r.child(decode_5)?,
                xy_row_height: r.child(decode_5)?,
                control_margin_bottom: r.child(decode_5)?,
                header_left: r.child(decode_7)?,
                two_way_tab: r.child(decode_8)?,
                no_header: r.child(decode_8)?,
                single_min: None,
                single_max: None,
                slider_requires_visible: r.child(decode_8)?,
            },
            2 => RowsSpec {
                product_id: r.child(decode_2)?,
                stages_path: r.child(decode_3)?,
                active_path: r.child(decode_3)?,
                enabled_path: r.child(decode_3)?,
                x_key: r.child(decode_3)?,
                y_key: r.child(decode_3)?,
                independent_key: r.child(decode_3)?,
                visible_key: r.child(decode_3)?,
                minimum_visible_stages: r.child(decode_4)?,
                rows_margin_x: r.child(decode_5)?,
                description_key: r.child(decode_6)?,
                drag_ordinal_key: r.child(decode_6)?,
                row_height: r.child(decode_5)?,
                xy_row_height: r.child(decode_5)?,
                control_margin_bottom: r.child(decode_5)?,
                header_left: r.child(decode_7)?,
                two_way_tab: r.child(decode_8)?,
                no_header: r.child(decode_8)?,
                single_min: r.child(decode_9)?,
                single_max: r.child(decode_9)?,
                slider_requires_visible: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_3)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 3]> {
        let value = match r.tag()? {
            0 => r.array::<_, 3>(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_2)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_DPI_ROWS_DATA,
        0,
        "12ba7ce01951d53090ea04a43270da34d77ebd64a6ce3e5278c75fc406ac24ab",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_dpi_rows_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_dpi_rows_data_decodes() {
        let _ = super::read_mouse_dpi_rows_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_dpi_rows_data.
