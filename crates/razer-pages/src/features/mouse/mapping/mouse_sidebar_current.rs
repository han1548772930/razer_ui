//! Actual ordinary ButtonPanel consumers, independently audited for all 76 mice.
//! The page's local mapping projection supplies rows; no device write is inferred.
use super::*;
use gpui_kit::base::{Button as BaseButton, ElementExt as _};
use razer_widgets::theme::{MouseCustomizeColors as Colors, MouseSidebarColors as PanelColors};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
    time::Duration,
};

#[path = "mouse_sidebar_index_current.rs"]
mod index;
#[path = "mouse_sidebar_secondary_current.rs"]
mod secondary;

#[derive(Deserialize)]
struct Filter {
    content: String,
}
#[derive(Deserialize)]
struct Labels {
    title: String,
    message1: String,
    message2: String,
    save: String,
    dont_save: String,
}
#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    babel_162: bool,
    key_toggle_render: bool,
    localized_counter: bool,
    split_min_utf16: usize,
    split_slash: bool,
    sticky_min_utf16: usize,
    filters: Vec<Filter>,
    labels: Labels,
}
fn policy(pid: u32) -> Option<&'static Policy> {
    static DATA: OnceLock<Vec<Policy>> = OnceLock::new();
    DATA.get_or_init(|| read_mouse_sidebar_current_catalog())
        .iter()
        .find(|p| p.product_id == pid)
}
#[derive(Deserialize)]
struct Assets {
    product_id: u32,
    close_asset: String,
    expand_asset: String,
}
fn assets(pid: u32) -> &'static Assets {
    static DATA: OnceLock<Vec<Assets>> = OnceLock::new();
    DATA.get_or_init(|| read_mouse_sidebar_assets_current_catalog())
        .iter()
        .find(|p| p.product_id == pid)
        .expect("audited mouse sidebar assets")
}

#[derive(Deserialize)]
struct RowGeometry {
    product_id: u32,
    padding: f32,
    height: Option<f32>,
    min_height: Option<f32>,
    binding_line_height: f32,
    binding_max_height: Option<f32>,
}
fn row_geometry(pid: u32) -> &'static RowGeometry {
    static DATA: OnceLock<Vec<RowGeometry>> = OnceLock::new();
    DATA.get_or_init(|| read_mouse_sidebar_geometry_current_catalog())
        .iter()
        .find(|geometry| geometry.product_id == pid)
        .expect("actual sidebar row geometry")
}

#[derive(Clone)]
enum Pending {
    Toggle,
    Row(String),
    CanvasRow(String),
    Hypershift,
    Page(String),
    Close,
}
#[derive(Default)]
pub(super) struct State {
    open: bool,
    selected_type: usize,
    filter_open: bool,
    from_panel: bool,
    pending: Option<Pending>,
    panel_scroll: ScrollHandle,
    body_scroll: ScrollHandle,
    filter_anchor: Rc<Cell<Bounds<Pixels>>>,
    filter_bounds: Rc<Cell<Bounds<Pixels>>>,
    tooltip: Option<String>,
    tooltip_anchor: Rc<Cell<Bounds<Pixels>>>,
    index_sticky: Rc<RefCell<BTreeSet<String>>>,
    index_hover: RefCell<Option<String>>,
    index_bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    index_tip_bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    secondary: secondary::State,
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
fn text(value: &Value) -> String {
    match value {
        Value::String(v) => v.clone(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

/// This follows updateButtons then render, preserving strict false vs absence.
fn rows<'a>(
    buttons: &'a [Value],
    key_property: Option<&str>,
    hyper: bool,
    filter: usize,
    policy: &Policy,
) -> Vec<&'a Value> {
    indexed_rows(buttons, key_property, hyper, filter, policy)
        .into_iter()
        .map(|(_, button)| button)
        .collect()
}
fn indexed_rows<'a>(
    buttons: &'a [Value],
    key_property: Option<&str>,
    hyper: bool,
    filter: usize,
    policy: &Policy,
) -> Vec<(usize, &'a Value)> {
    let mut result = buttons
        .iter()
        .filter(|b| {
            let copy = if policy.babel_162 {
                key_property == Some("multi") && hyper
                    || key_property == Some("func") && !hyper
                    || key_property.is_none() && !hyper
            } else {
                key_property == Some("func") && !hyper || key_property.is_none() && !hyper
            };
            let keep = if copy {
                true
            } else if !policy.babel_162 && key_property == Some("multi") && !hyper {
                !truthy(&b["disableHypershiftMapping"]) || b["isKeyToggle"] == false
            } else if !policy.babel_162 && key_property == Some("multi") && hyper {
                !truthy(&b["HID"]) || b["isKeyToggle"] != false
            } else {
                !truthy(&b["HID"]) || !truthy(&b["disableHypershiftMapping"])
            };
            keep && (filter != 1 || truthy(&b["isMapped"]))
        })
        .collect::<Vec<_>>();
    if result
        .iter()
        .all(|b| b["counter"].as_f64().is_some_and(|n| n.fract() == 0.))
    {
        result.sort_by(|a, b| {
            a["counter"]
                .as_f64()
                .unwrap()
                .partial_cmp(&b["counter"].as_f64().unwrap())
                .expect("finite JSON numbers")
        });
    }
    result
        .into_iter()
        .enumerate()
        .filter(|(_, b)| {
            !truthy(&b["disabled"])
                && b.get("isSidePanelList").is_none_or(truthy)
                && (!policy.key_toggle_render
                    || b.get("isKeyToggle").is_none()
                    || truthy(&b["isKeyToggle"]) == hyper)
                && b.get("counter").is_some()
                && text(&b["counter"]) != "mediaVolume"
        })
        .collect()
}

impl MouseProductWorkspace {
    pub(super) fn request_current_canvas_mapping(
        &mut self,
        input: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_current_sidebar(Pending::CanvasRow(input), window, cx);
    }

    pub(super) fn request_current_hypershift(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_current_sidebar(Pending::Hypershift, window, cx);
    }

    pub(super) fn current_mapping_confirmation_pending(&self) -> bool {
        self.sidebar_current_state.pending.is_some()
    }
    pub(super) fn current_mouse_mapping_panel_open(&self) -> bool {
        self.sidebar_current_state.open
    }
    pub(super) fn current_mouse_mapping_from_panel(&self) -> bool {
        self.sidebar_current_state.from_panel
    }
    pub(super) fn current_mouse_mapping_body_scroll(&self) -> ScrollHandle {
        self.sidebar_current_state.body_scroll.clone()
    }
    pub(super) fn request_current_mapping_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_current_sidebar(Pending::Close, window, cx);
    }
    /// MapSensitivity's configure link installs changeSelectedTab as the
    /// parent's SaveAlert continuation; Don't Save cancels that continuation.
    pub(super) fn request_current_mapping_page(
        &mut self,
        page: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id == 190 {
            self.mapping_190_request(
                customize_190::source_mapping::Next::Tab(page.into()),
                window,
                cx,
            );
            return;
        }
        self.request_current_sidebar(Pending::Page(page.into()), window, cx);
    }
    pub(super) fn current_sidebar_expand_asset(&self) -> SharedString {
        assets(self.spec.product_id).expand_asset.clone().into()
    }
    pub(super) fn current_sidebar_filter_rows<'a>(
        &self,
        buttons: &'a [Value],
        filter: usize,
    ) -> Vec<&'a Value> {
        let Some(policy) = policy(self.spec.product_id) else {
            return Vec::new();
        };
        let property = self.mouse_customize_observation["customizeBladeReducer"]["keyProperty"]
            .as_str()
            .filter(|s| !s.is_empty());
        rows(buttons, property, self.hypershift, filter, policy)
    }
    pub(super) fn current_sidebar_counter(
        &self,
        input: String,
        button: &Value,
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let policy = policy(self.spec.product_id).expect("audited ordinary sidebar counter");
        let mut label = text(&button["counter"]);
        if policy.localized_counter && truthy(&button["localizedName"]) {
            label = t(&label).to_string();
        }
        index::Index::new(self, input, label, !enabled, policy, cx).into_any_element()
    }
    pub(super) fn current_sidebar_toggle(&self, cx: &Context<Self>) -> AnyElement {
        BaseButton::new("mouse-current-sidebar-toggle")
            .selected(self.sidebar_current_state.open)
            .accessibility_label("button panel")
            .w(surface::css(38.))
            .h(surface::css(27.))
            .p_0()
            .border_1()
            .rounded(surface::css(14.))
            .border_color(if self.sidebar_current_state.open {
                Colors::selected()
            } else {
                Colors::line()
            })
            .bg(Colors::selected_surface())
            .hover(|s| s.border_color(Colors::selected()))
            .child(
                img(if self.sidebar_current_state.open {
                    "synapse/drawer-active.svg"
                } else {
                    "synapse/drawer.svg"
                })
                .size(surface::css(18.)),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.request_current_sidebar(Pending::Toggle, window, cx)
            }))
            .into_any_element()
    }

    fn request_current_sidebar(
        &mut self,
        pending: Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(&pending, Pending::Row(input) | Pending::CanvasRow(input) if self.mapping_input.as_ref() == Some(input))
        {
            return;
        }
        if self.current_mouse_mapping_changed() {
            self.sidebar_current_state.pending = Some(pending);
        } else {
            self.apply_current_sidebar(pending, window, cx);
        }
        cx.notify();
    }
    fn apply_current_sidebar(
        &mut self,
        pending: Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pending {
            Pending::Close => self.discard_current_mouse_mapping(cx),
            Pending::Page(page) => self.set_page(&page, window, cx),
            Pending::Toggle => {
                if self.sidebar_current_state.open && self.sidebar_current_state.from_panel {
                    self.set_current_customize_input(None, false);
                    self.mapping_assignment = None;
                }
                self.sidebar_current_state.open = !self.sidebar_current_state.open;
                self.sidebar_current_state.from_panel = false;
                self.sidebar_current_state.filter_open = false;
                self.sidebar_current_state.tooltip = None;
            }
            Pending::Row(input) => {
                self.sidebar_current_state.from_panel = true;
                self.open_current_mouse_mapping(input, window, cx);
            }
            Pending::CanvasRow(input) => {
                self.sidebar_current_state.from_panel = false;
                self.open_current_mouse_mapping(input, window, cx);
            }
            Pending::Hypershift => {
                // PID 70 lT.updateHypeShift runs after SaveAlert's save
                // continuation, and closes the old layer's mapping editor.
                self.hypershift = !self.hypershift;
                self.discard_current_mouse_mapping(cx);
                self.mapping_assignment = None;
                self.customize_hover = None;
                self.sidebar_current_state.from_panel = false;
            }
        }
    }

    pub(super) fn current_sidebar_surface(
        &self,
        buttons: &[Value],
        unavailable: bool,
        content: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(policy) = policy(self.spec.product_id) else {
            return content;
        };
        let state = &self.sidebar_current_state;
        let width = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-current-sidebar-width", cx.entity_id())),
            if state.open { 230. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let height = (window.viewport_size().height - window.rem_size() * (218. / 16.))
            .max(window.rem_size() * (380. / 16.));
        let rotation = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-sidebar-dropdown-rotation", cx.entity_id())),
            if state.filter_open { 180_f32 } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let filter_height = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-sidebar-dropdown-height", cx.entity_id())),
            if state.filter_open { 180_f32 } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        )
        .min(policy.filters.len() as f32 * 25. + 2.);
        let confirm_opacity = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-sidebar-confirmation-opacity", cx.entity_id())),
            if state.pending.is_some() { 1_f32 } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(100))
                .easing(gpui_kit::base::motion::Easing::Linear),
            window,
            cx,
        );
        // This consumer reads customizeBladeReducer.keyProperty, not DeviceInfo.
        // Absent/empty reducer produces null in the original connect expression.
        let property = self.mouse_customize_observation["customizeBladeReducer"]["keyProperty"]
            .as_str()
            .filter(|s| !s.is_empty());
        let panel_rows = indexed_rows(
            buttons,
            property,
            self.hypershift,
            state.selected_type,
            policy,
        );
        // The actual React row key is its position in state.buttonList, before
        // render's null-row gates. A filtered list can reuse that component for
        // a different input; only a removed numeric position unmounts it.
        let mounted = panel_rows
            .iter()
            .map(|(index, _)| format!("ordinary-{index}"))
            .collect::<BTreeSet<_>>();
        state
            .index_sticky
            .borrow_mut()
            .retain(|key| !key.starts_with("ordinary-") || mounted.contains(key));
        state
            .index_bounds
            .borrow_mut()
            .retain(|key, _| !key.starts_with("ordinary-") || mounted.contains(key));
        state
            .index_tip_bounds
            .borrow_mut()
            .retain(|key, _| !key.starts_with("ordinary-") || mounted.contains(key));
        if state
            .index_hover
            .borrow()
            .as_ref()
            .is_some_and(|key| key.starts_with("ordinary-") && !mounted.contains(key))
        {
            *state.index_hover.borrow_mut() = None;
        }
        state
            .secondary
            .retain(&panel_rows.iter().map(|(index, _)| *index).collect());
        let mut list = v_flex()
            .id("mouse-current-sidebar-list")
            .w_full()
            .flex_1()
            .min_h_0()
            .max_h(height - window.rem_size() * (145. / 16.))
            .overflow_y_scroll()
            .track_scroll(&state.panel_scroll);
        for (index, button) in panel_rows {
            if let Some(row) = self.current_sidebar_row(button, index, policy, unavailable, cx) {
                list = list.child(row);
            }
        }
        div()
            .w_full()
            .relative()
            .on_prepaint({
                let root = self.mapping_menu_current_state.root.clone();
                move |bounds, _, _| root.set(bounds)
            })
            .child(
                h_flex()
                    .relative()
                    .w_full()
                    .h(height)
                    .items_stretch()
                    .child(
                        div()
                            .relative()
                            .w(surface::css(width))
                            .h_full()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .on_prepaint({
                                let drawer = self.mapping_menu_current_state.drawer.clone();
                                move |bounds, _, _| drawer.set(bounds)
                            })
                            .child(
                                v_flex()
                                    .absolute()
                                    .left(surface::css(width - 230.))
                                    .top_0()
                                    .w(surface::css(230.))
                                    .h_full()
                                    .bg(PanelColors::panel())
                                    .child(
                                        h_flex()
                                            .relative()
                                            .h(surface::css(67.))
                                            .w_full()
                                            .items_center()
                                            .justify_center()
                                            .border_b_1()
                                            .border_color(Colors::line())
                                            .child(
                                                self.current_sidebar_filter(policy, rotation, cx),
                                            )
                                            .child(
                                                BaseButton::new("mouse-current-sidebar-close")
                                                    .absolute()
                                                    .right(surface::css(8.))
                                                    .size(surface::css(26.))
                                                    .p_0()
                                                    .border_1()
                                                    .border_color(Colors::transparent())
                                                    .hover(|s| s.border_color(Colors::line()))
                                                    .child(
                                                        img("synapse/drawer-close.svg")
                                                            .size(surface::css(20.)),
                                                    )
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.request_current_sidebar(
                                                                Pending::Toggle,
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    )),
                                            ),
                                    )
                                    .child(list),
                            ),
                    )
                    .child(
                        div()
                            .id("mouse-current-sidebar-body")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_scroll()
                            .track_scroll(&state.body_scroll)
                            .child(content.when(state.open, |c| c.w(surface::css(1000.)))),
                    ),
            )
            .children(state.open.then(|| {
                deferred(self.current_sidebar_filter_options(policy, filter_height, cx))
                    .with_priority(151)
            }))
            .children(state.tooltip.as_ref().filter(|_| state.open).map(|label| {
                deferred(Layer::new(
                    LayerKind::Binding(state.tooltip_anchor.clone()),
                    tip()
                        .max_w(surface::css(358.))
                        .child(label.clone())
                        .into_any_element(),
                ))
                .with_priority(999)
            }))
            .children(state.pending.as_ref().map(|_| {
                deferred(self.current_sidebar_confirmation(policy, confirm_opacity, cx))
                    .with_priority(1100)
            }))
    }

    fn current_sidebar_row(
        &self,
        button: &Value,
        index: usize,
        policy: &'static Policy,
        unavailable: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let input = button["inputID"].as_str()?.to_owned();
        let geometry = row_geometry(self.spec.product_id);
        let enabled = !unavailable && truthy(&button["isEnabled"]);
        let remapped = truthy(&button["isMapped"]);
        let active =
            truthy(&button["isFocused"]) || self.mapping_input.as_deref() == Some(input.as_str());
        let nano = button["assignment"] == "NANOLEAF";
        let mut counter = text(&button["counter"]);
        if policy.localized_counter && truthy(&button["localizedName"]) {
            counter = t(&counter).to_string();
        }
        let assignment = if nano {
            t("NANOLEAF_PANEL").to_string()
        } else {
            let a = text(&button["assignment"]);
            let value = assignment_key(&a, policy.babel_162);
            t(&value).to_string()
        };
        let assignment = if policy.babel_162 {
            capitalize(&assignment)
        } else {
            assignment.to_uppercase()
        };
        let label = t(&text(if remapped {
            &button["assignmentValue"]
        } else {
            &button["defaultValue"]
        }))
        .to_string();
        let foreground = if !enabled {
            PanelColors::muted()
        } else if remapped {
            if self.hypershift {
                Colors::hypershift()
            } else {
                Colors::selected()
            }
        } else {
            Colors::text()
        };
        let overflow = Rc::new(Cell::new(false));
        let measure = overflow.clone();
        let anchor = Rc::new(Cell::new(Bounds::default()));
        let measured = anchor.clone();
        let label_measure = label.clone();
        let label_hover = label.clone();
        Some(
            BaseButton::new(SharedString::from(format!(
                "mouse-current-sidebar-row-{index}"
            )))
            .w_full()
            .when_some(geometry.height, |row, height| row.h(surface::css(height)))
            .when_some(geometry.min_height, |row, height| {
                row.min_h(surface::css(height))
            })
            .p(surface::css(geometry.padding))
            .justify_start()
            .disabled(!enabled)
            .bg(if active {
                Colors::selected_surface()
            } else {
                Colors::transparent()
            })
            .hover(|s| s.bg(Colors::hover()))
            .active(|s| s.bg(Colors::selected_surface()))
            .on_prepaint(move |bounds, _, _| measured.set(bounds))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .child(
                        v_flex()
                            .w(surface::css(56.))
                            .ml(surface::css(-10.))
                            .flex_shrink_0()
                            .text_center()
                            .text_color(if enabled {
                                Colors::text()
                            } else {
                                PanelColors::muted()
                            })
                            .line_height(surface::css(14.))
                            .children(nano.then(|| div().child(t("PANEL"))))
                            .child(index::Index::new(
                                self,
                                format!("ordinary-{index}"),
                                counter,
                                !enabled,
                                policy,
                                cx,
                            )),
                    )
                    .child(
                        v_flex()
                            .w(surface::css(164.))
                            .flex_shrink_0()
                            .pl(surface::css(5.))
                            .border_l_1()
                            .border_color(PanelColors::muted())
                            .child(
                                div()
                                    .text_size(surface::css(10.))
                                    .text_color(PanelColors::muted())
                                    .line_height(surface::css(12.))
                                    .child(assignment),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!(
                                        "mouse-current-sidebar-value-{input}"
                                    )))
                                    .w_full()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(geometry.binding_line_height))
                                    .when_some(geometry.binding_max_height, |value, height| {
                                        value.max_h(surface::css(height))
                                    })
                                    .text_color(foreground)
                                    .on_prepaint(move |bounds, window, _| {
                                        measure.set(
                                            remapped
                                                && surface::label_width(
                                                    &label_measure,
                                                    14.,
                                                    window,
                                                ) * f32::from(window.rem_size())
                                                    / 16.
                                                    > f32::from(bounds.size.width),
                                        )
                                    })
                                    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                                        this.sidebar_current_state.tooltip =
                                            if *on && overflow.get() {
                                                this.sidebar_current_state.tooltip_anchor =
                                                    anchor.clone();
                                                Some(label_hover.clone())
                                            } else {
                                                None
                                            };
                                        cx.notify();
                                    }))
                                    .child(label),
                            ),
                    )
                    .children(secondary::marker(self, button, index, !enabled, cx)),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.request_current_sidebar(Pending::Row(input.clone()), window, cx)
            }))
            .into_any_element(),
        )
    }

    fn current_sidebar_filter(
        &self,
        policy: &Policy,
        rotation: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let anchor = self.sidebar_current_state.filter_anchor.clone();
        let options = self.sidebar_current_state.filter_bounds.clone();
        BaseButton::new("mouse-current-sidebar-filter")
            .w(surface::css(180.))
            .h(surface::css(26.))
            .mr(surface::css(26.))
            .px(surface::css(4.))
            .py_0()
            .border_1()
            .border_color(Colors::line())
            .bg(Colors::selected_surface())
            .text_color(Colors::text())
            .hover(|s| s.border_color(Colors::selected()))
            .on_prepaint(move |bounds, _, _| anchor.set(bounds))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(div().text_ellipsis().child(t(
                        &policy.filters[self.sidebar_current_state.selected_type].content,
                    )))
                    .child(
                        svg()
                            .path(self.current_sidebar_expand_asset())
                            .text_color(PanelColors::icon())
                            .size(surface::css(10.))
                            .with_transformation(Transformation::rotate(radians(
                                rotation.to_radians(),
                            ))),
                    ),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.sidebar_current_state.filter_open = !this.sidebar_current_state.filter_open;
                cx.notify();
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                    if this.sidebar_current_state.filter_open
                        && !options.get().contains(&event.position)
                    {
                        this.sidebar_current_state.filter_open = false;
                        cx.notify();
                    }
                }),
            )
            .into_any_element()
    }
    fn current_sidebar_filter_options(
        &self,
        policy: &Policy,
        height: f32,
        cx: &Context<Self>,
    ) -> Layer {
        let bounds = self.sidebar_current_state.filter_bounds.clone();
        let options = v_flex()
            .w(surface::css(180.))
            .h(surface::css(height))
            .overflow_hidden()
            .when(height > 0., |view| view.border_1())
            .border_color(Colors::line())
            .bg(PanelColors::tooltip())
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .children(policy.filters.iter().enumerate().map(|(filter, option)| {
                BaseButton::new(SharedString::from(format!(
                    "mouse-current-sidebar-filter-{filter}"
                )))
                .w_full()
                .flex_shrink_0()
                .h(surface::css(25.))
                .px(surface::css(4.))
                .py_0()
                .bg(PanelColors::tooltip())
                .text_color(if filter == self.sidebar_current_state.selected_type {
                    Colors::selected()
                } else {
                    Colors::text()
                })
                .hover(|s| s.bg(PanelColors::option_hover()))
                .justify_start()
                .child(t(&option.content))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.sidebar_current_state.selected_type = filter;
                    this.sidebar_current_state.filter_open = false;
                    this.sidebar_current_state.tooltip = None;
                    cx.notify();
                }))
            }));
        Layer::new(
            LayerKind::Dropdown(self.sidebar_current_state.filter_anchor.clone()),
            options.into_any_element(),
        )
    }
    fn current_sidebar_confirmation(
        &self,
        policy: &Policy,
        opacity: f32,
        cx: &Context<Self>,
    ) -> Layer {
        let button = |id: &'static str, label: SharedString| {
            BaseButton::new(id)
                .min_w(surface::css(100.))
                .h(surface::css(27.))
                .px(surface::css(16.))
                .py_0()
                .border_1()
                .border_color(Colors::line())
                .rounded(surface::css(3.))
                .bg(Colors::selected_surface())
                .text_color(Colors::text())
                .hover(|s| s.border_color(Colors::selected()))
                .child(label)
        };
        let dialog = div()
            .relative()
            .opacity(opacity)
            .w(surface::css(400.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .border_1()
            .border_color(Colors::selected())
            .rounded(surface::css(5.))
            .bg(Colors::selected_surface())
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_center()
            .child(
                BaseButton::new("mouse-sidebar-save-dismiss")
                    .absolute()
                    .right(surface::css(8.))
                    .top(surface::css(8.))
                    .size(surface::css(20.))
                    .p_0()
                    .child(
                        img(SharedString::from(
                            assets(self.spec.product_id).close_asset.clone(),
                        ))
                        .size(surface::css(20.)),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_current_state.pending = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .font_family("RazerF5")
                    .text_color(Colors::selected())
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .mb(surface::css(20.))
                    .child(t(&policy.labels.title).to_uppercase()),
            )
            .child(t(&policy.labels.message1))
            .child(div().h(surface::css(34.)))
            .child(t(&policy.labels.message2))
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .mt(surface::css(20.))
                    .child(
                        button(
                            "mouse-sidebar-dont-save",
                            t(&policy.labels.dont_save).into(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.discard_current_mouse_mapping(cx);
                            this.sidebar_current_state.pending = None;
                        })),
                    )
                    .child(
                        button("mouse-sidebar-save", t(&policy.labels.save).into())
                            .bg(Colors::selected())
                            .text_color(Colors::selected_text())
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.save_current_mouse_mapping_for_navigation(cx)
                                    && let Some(pending) = this.sidebar_current_state.pending.take()
                                {
                                    this.apply_current_sidebar(pending, window, cx);
                                }
                                cx.notify();
                            })),
                    ),
            );
        let mut layer = Layer::new(LayerKind::Confirmation, dialog.into_any_element());
        layer.opacity = opacity;
        layer
    }
}

pub(super) fn capitalize(value: &str) -> String {
    value
        .split_inclusive(char::is_whitespace)
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}
fn assignment_key(value: &str, babel: bool) -> String {
    static SPACE: OnceLock<regex::Regex> = OnceLock::new();
    if !babel && value.contains(' ') {
        SPACE
            .get_or_init(|| regex::Regex::new(r"\s+").expect("source assignment whitespace"))
            .replace_all(value, "_")
            .into_owned()
    } else {
        value.to_owned()
    }
}
fn tip() -> Div {
    div()
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(Colors::line())
        .bg(PanelColors::tooltip())
        .text_color(Colors::text())
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_left()
}

enum LayerKind {
    Index(Rc<Cell<Bounds<Pixels>>>),
    Binding(Rc<Cell<Bounds<Pixels>>>),
    Dropdown(Rc<Cell<Bounds<Pixels>>>),
    Confirmation,
    Secondary(Rc<Cell<Bounds<Pixels>>>),
}
struct Layer {
    kind: LayerKind,
    child: AnyElement,
    backdrop: Option<AnyElement>,
    opacity: f32,
}
impl Layer {
    fn new(kind: LayerKind, child: AnyElement) -> Self {
        Self {
            kind,
            child,
            backdrop: None,
            opacity: 1.,
        }
    }
}
impl IntoElement for Layer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Layer {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let mut children = Vec::new();
        if matches!(self.kind, LayerKind::Confirmation) {
            let opacity = self.opacity;
            let backdrop = self.backdrop.get_or_insert_with(|| {
                div()
                    .absolute()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .bg(PanelColors::backdrop())
                    .opacity(opacity)
                    .occlude()
                    .into_any_element()
            });
            children.push(backdrop.request_layout(window, cx));
        }
        let child = self.child.request_layout(window, cx);
        children.push(child);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                children,
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let unit = window.rem_size() / 16.;
        let measured = window.layout_bounds(*child).size;
        let origin = match &self.kind {
            LayerKind::Index(anchor) => anchor.get().origin + point(unit * 20., unit * 35.),
            LayerKind::Binding(anchor) => {
                let y = anchor.get().top() + unit * 50.;
                point(
                    unit * 61.,
                    if y + measured.height > window.viewport_size().height {
                        y - measured.height - unit * 50.
                    } else {
                        y
                    },
                )
            }
            LayerKind::Dropdown(anchor) => {
                let a = anchor.get();
                let y = if a.bottom() + unit * 52. > window.viewport_size().height {
                    a.top() - measured.height - unit
                } else {
                    a.top() + unit * 28.
                };
                point(a.left(), y)
            }
            LayerKind::Confirmation => point(
                (window.viewport_size().width - measured.width) / 2.,
                window.viewport_size().height / 2. - measured.height,
            ),
            LayerKind::Secondary(position) => position.get().origin,
        };
        if let Some(backdrop) = &mut self.backdrop {
            window.with_element_offset(point(px(0.), px(0.)) - bounds.origin, |window| {
                backdrop.prepaint(window, cx)
            });
        }
        window.with_element_offset(origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(backdrop) = &mut self.backdrop {
            backdrop.paint(window, cx);
        }
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input_names(rows: Vec<&Value>) -> Vec<String> {
        rows.into_iter().map(|row| text(&row["inputID"])).collect()
    }
    #[::core::prelude::v1::test]
    fn modern_multi_layer_keeps_strict_false_gate_and_render_filter() {
        let buttons = vec![
            json!({"inputID":"normal","counter":3,"isKeyToggle":false,"disableHypershiftMapping":true,"HID":"1"}),
            json!({"inputID":"hyper","counter":1,"isKeyToggle":true,"disableHypershiftMapping":true,"HID":"1"}),
            json!({"inputID":"plain","counter":2}),
            json!({"inputID":"hidden","counter":4,"isSidePanelList":false}),
        ];
        let source = policy(70).unwrap();
        assert_eq!(
            input_names(rows(&buttons, Some("multi"), false, 0, source)),
            ["plain", "normal"]
        );
        assert_eq!(
            input_names(rows(&buttons, Some("multi"), true, 0, source)),
            ["hyper", "plain"]
        );
        assert_eq!(
            input_names(rows(&buttons, None, true, 0, source)),
            ["plain"]
        );
    }
    #[::core::prelude::v1::test]
    fn babel_multi_hyper_retains_independent_copy_and_no_toggle_render_gate() {
        let buttons = vec![
            json!({"inputID":"x","counter":2,"HID":"1","disableHypershiftMapping":true,"isKeyToggle":false}),
            json!({"inputID":"y","counter":1,"isMapped":true}),
        ];
        let source = policy(162).unwrap();
        assert_eq!(
            input_names(rows(&buttons, Some("multi"), true, 0, source)),
            ["y", "x"]
        );
        assert_eq!(
            input_names(rows(&buttons, Some("multi"), true, 1, source)),
            ["y"]
        );
        assert!(!source.split_slash);
        assert_eq!(source.sticky_min_utf16, 10);
        assert!(policy(70).unwrap().split_slash);
        assert_eq!(policy(70).unwrap().sticky_min_utf16, 9);
    }
    #[::core::prelude::v1::test]
    fn sorting_only_applies_when_every_counter_is_an_integer() {
        let source = policy(70).unwrap();
        let buttons = vec![
            json!({"inputID":"x","counter":"A/B"}),
            json!({"inputID":"y","counter":1}),
            json!({"inputID":"z","counter":2,"disabled":true}),
        ];
        assert_eq!(
            input_names(rows(&buttons, None, false, 0, source)),
            ["x", "y"]
        );
        assert_eq!(
            assignment_key(" Windows  Shortcut ", false),
            "_Windows_Shortcut_"
        );
        assert_eq!(
            assignment_key(" Windows  Shortcut ", true),
            " Windows  Shortcut "
        );
    }
}

// Generated catalog reader: razer-pages/mouse_sidebar_assets_current_data.
fn read_mouse_sidebar_assets_current_catalog() -> Vec<Assets> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Assets>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Assets> {
        let value = match r.tag()? {
            0 => Assets {
                product_id: r.child(decode_2)?,
                close_asset: r.child(decode_3)?,
                expand_asset: r.child(decode_3)?,
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
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_SIDEBAR_ASSETS_CURRENT_DATA,
        0,
        "b52d95b8accc063ba5b05e12a68322f37d5d9167559b7faa2555ed68760b398b",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_mouse_sidebar_assets_current_data_decodes() {
    let _ = read_mouse_sidebar_assets_current_catalog();
}
// End generated catalog reader: razer-pages/mouse_sidebar_assets_current_data.

// Generated catalog reader: razer-pages/mouse_sidebar_current_data.
fn read_mouse_sidebar_current_catalog() -> Vec<Policy> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Policy>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Policy> {
        let value = match r.tag()? {
            0 => Policy {
                product_id: r.child(decode_2)?,
                babel_162: r.child(decode_3)?,
                key_toggle_render: r.child(decode_3)?,
                localized_counter: r.child(decode_3)?,
                split_min_utf16: r.child(decode_4)?,
                split_slash: r.child(decode_3)?,
                sticky_min_utf16: r.child(decode_4)?,
                filters: r.child(decode_5)?,
                labels: r.child(decode_8)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
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
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Filter>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Filter> {
        let value = match r.tag()? {
            0 => Filter {
                content: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Labels> {
        let value = match r.tag()? {
            0 => Labels {
                title: r.child(decode_7)?,
                message1: r.child(decode_7)?,
                message2: r.child(decode_7)?,
                save: r.child(decode_7)?,
                dont_save: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_SIDEBAR_CURRENT_DATA,
        0,
        "4e9279bdfe0bce66c3e776e3ca827b7c651d27be76aef1b80e0d67283ef98dc6",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_mouse_sidebar_current_data_decodes() {
    let _ = read_mouse_sidebar_current_catalog();
}
// End generated catalog reader: razer-pages/mouse_sidebar_current_data.

// Generated catalog reader: razer-pages/mouse_sidebar_geometry_current_data.
fn read_mouse_sidebar_geometry_current_catalog() -> Vec<RowGeometry> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<RowGeometry>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<RowGeometry> {
        let value = match r.tag()? {
            0 => RowGeometry {
                product_id: r.child(decode_2)?,
                padding: r.child(decode_3)?,
                height: None,
                min_height: r.child(decode_4)?,
                binding_line_height: r.child(decode_3)?,
                binding_max_height: None,
            },
            1 => RowGeometry {
                product_id: r.child(decode_2)?,
                padding: r.child(decode_3)?,
                height: r.child(decode_4)?,
                min_height: None,
                binding_line_height: r.child(decode_3)?,
                binding_max_height: r.child(decode_4)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<f32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_3)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_SIDEBAR_GEOMETRY_CURRENT_DATA,
        0,
        "3fa7415aa4904fff44a3a6084a82d6a6b1fdae69f11806de49aec3c6008b448c",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_mouse_sidebar_geometry_current_data_decodes() {
    let _ = read_mouse_sidebar_geometry_current_catalog();
}
// End generated catalog reader: razer-pages/mouse_sidebar_geometry_current_data.
