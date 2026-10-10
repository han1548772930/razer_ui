//! Current 190/192/196/222/226/229/239 dynamic-sensitivity editor.
//! Local drafts and requests are distinct from device observations.
use super::super::Choice;
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog};
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::prelude::FluentBuilder as _;
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[path = "mouse_dynamic_curve.rs"]
mod curve;
use curve::{Curve, Point as CurvePoint};

const LOCAL: &str = "_dynamicSensitivityLocalV1";
const LABELS: [&str; 4] = ["CLASSIC", "NATURAL", "JUMP", "CUSTOM"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MouseDynamicScope {
    owner: EntityId,
    profile_epoch: u64,
    connection_epoch: u64,
}
/// Only publish values actually received from the current source transport.
pub enum MouseDynamicObservation {
    DynamicSensitivity(Value),
    Loading(bool),
    MouseSpeed(f64),
    FirmwareVersion(Option<String>),
    /// Invalidates previous connection requests and producer callbacks.
    ConnectionChanged,
}
/// The adapter supplies currentTimerTick when available; the source fallback is
/// Date.now(). Emitting this event is not an acknowledged hardware write.
#[derive(Clone)]
pub struct MouseDynamicRequested {
    request_id: u64,
    scope: MouseDynamicScope,
    product_id: u32,
    command: &'static str,
    payload: Value,
    timer_tick: Option<u64>,
}
impl MouseDynamicRequested {
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn scope(&self) -> MouseDynamicScope {
        self.scope
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn command(&self) -> &'static str {
        self.command
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    pub fn timer_tick(&self) -> Option<u64> {
        self.timer_tick
    }
}
pub enum MouseDynamicCompletion {
    /// Actual refreshed dynamicSensitivity from the source response chain.
    Observed(Value),
    Failed(String),
    Unsupported(String),
    Cancelled,
}
pub struct MouseDynamicTutorialChanged {
    visible: bool,
}
impl MouseDynamicTutorialChanged {
    pub fn visible(&self) -> bool {
        self.visible
    }
}
impl EventEmitter<MouseDynamicTutorialChanged> for MouseProductWorkspace {}
impl EventEmitter<MouseDynamicRequested> for MouseProductWorkspace {}

pub(super) struct State {
    owner: EntityId,
    profile_epoch: u64,
    connection_epoch: u64,
    observed: Option<Value>,
    pub(super) firmware: Option<String>,
    local: bool,
    loading: bool,
    points: Vec<CurvePoint>,
    drag: Option<usize>,
    hover: Option<(usize, Point<Pixels>)>,
    gap_x: f64,
    gap_y: f64,
    mouse_speed: f64,
    speed_task: Option<Task<()>>,
    template: Entity<SelectState<Vec<Choice>>>,
    zoom_hover: Option<&'static str>,
    zoom_tip: Option<(&'static str, bool)>,
    firmware_hover: Option<Point<Pixels>>,
    tutorial_visible: bool,
    next_request: u64,
    pending: BTreeMap<u64, &'static str>,
    latest_mutation: Option<u64>,
    last_failure: Option<String>,
}
impl State {
    pub(super) fn new(window: &mut Window, cx: &mut Context<MouseProductWorkspace>) -> Self {
        let template = cx.new(|cx| SelectState::new(choices(), None, window, cx));
        template.update(cx, |state, cx| {
            state.set_selected_value(&"0".to_owned(), window, cx)
        });
        Self {
            owner: cx.entity_id(),
            profile_epoch: 0,
            connection_epoch: 0,
            observed: None,
            firmware: None,
            local: false,
            loading: false,
            points: curve::CLASSIC.to_vec(),
            drag: None,
            hover: None,
            gap_x: 15.,
            gap_y: 0.25,
            mouse_speed: 0.,
            speed_task: None,
            template,
            zoom_hover: None,
            zoom_tip: None,
            firmware_hover: None,
            tutorial_visible: true,
            next_request: 0,
            pending: BTreeMap::new(),
            latest_mutation: None,
            last_failure: None,
        }
    }
    fn scope(&self) -> MouseDynamicScope {
        MouseDynamicScope {
            owner: self.owner,
            profile_epoch: self.profile_epoch,
            connection_epoch: self.connection_epoch,
        }
    }
}
fn choices() -> Vec<Choice> {
    LABELS
        .iter()
        .enumerate()
        .map(|(index, key)| Choice::new(index.to_string(), t(key)))
        .collect()
}

impl MouseProductWorkspace {
    pub(super) fn prepare_dynamic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.spec.dynamic {
            return;
        }
        let template = self.dynamic_state.template.clone();
        self.subscriptions.push(cx.subscribe_in(
            &template,
            window,
            |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                let Ok(template) = value.parse::<u32>() else {
                    return;
                };
                if !this.dynamic_editable() || this.dynamic_mode() != 3 || template > 3 {
                    return;
                }
                this.finish_dynamic_drag(window, cx);
                if let Some(points) = curve::preset(template) {
                    this.dynamic_state.points = points.to_vec();
                }
                this.commit_dynamic_points(template, window, cx);
            },
        ));
    }
    pub fn mouse_dynamic_scope(&self) -> Option<MouseDynamicScope> {
        self.spec.dynamic.then(|| self.dynamic_state.scope())
    }
    pub fn mouse_dynamic_request_is_current(&self, request: &MouseDynamicRequested) -> bool {
        self.mouse_dynamic_scope() == Some(request.scope)
            && self.spec.product_id == request.product_id
            && self.dynamic_state.pending.get(&request.request_id) == Some(&request.command)
    }
    pub fn complete_mouse_dynamic(
        &mut self,
        request: &MouseDynamicRequested,
        completion: MouseDynamicCompletion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.mouse_dynamic_request_is_current(request) {
            return;
        }
        self.dynamic_state.pending.remove(&request.request_id);
        let latest = self.dynamic_state.latest_mutation == Some(request.request_id);
        match completion {
            MouseDynamicCompletion::Observed(value) if latest && valid_state(&value) => {
                self.dynamic_state.observed = Some(value);
                self.dynamic_state.local = false;
                self.dynamic_state.loading = false;
                self.dynamic_state.last_failure = None;
                self.sync_dynamic_points(window, cx);
            }
            MouseDynamicCompletion::Failed(reason)
            | MouseDynamicCompletion::Unsupported(reason) => {
                if latest {
                    self.dynamic_state.loading = false;
                }
                self.dynamic_state.last_failure = Some(reason);
            }
            MouseDynamicCompletion::Cancelled => {
                if latest {
                    self.dynamic_state.loading = false;
                }
            }
            _ => {}
        }
        cx.notify();
    }
    /// Transport cancellation only. The source editor has no Cancel button.
    pub fn cancel_mouse_dynamic(&mut self, scope: MouseDynamicScope, cx: &mut Context<Self>) {
        if self.mouse_dynamic_scope() != Some(scope) {
            return;
        }
        self.dynamic_state.pending.clear();
        self.dynamic_state.latest_mutation = None;
        self.dynamic_state.loading = false;
        cx.notify();
    }
    pub fn mouse_dynamic_failure(&self) -> Option<&str> {
        self.dynamic_state.last_failure.as_deref()
    }
    pub fn dynamic_tutorial_visible(&self) -> bool {
        self.has_dynamic_tutorial() && self.dynamic_state.tutorial_visible
    }
    pub fn has_dynamic_tutorial(&self) -> bool {
        self.spec.dynamic
            && self.spec.product_id != 226
            && (self.spec.product_id != 196 || self.advanced_enabled())
    }
    /// Source localStorage["isShowSensitivityTutorial"]: undefined/nonboolean
    /// shows the tutorial; false remains false. This is global local UI state.
    pub fn restore_dynamic_tutorial_preference(
        &mut self,
        visible: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        self.dynamic_state.tutorial_visible = visible.unwrap_or(true);
        cx.notify();
    }
    pub fn toggle_dynamic_tutorial(&mut self, cx: &mut Context<Self>) {
        if !self.has_dynamic_tutorial() {
            return;
        }
        self.dynamic_state.tutorial_visible = !self.dynamic_state.tutorial_visible;
        cx.emit(MouseDynamicTutorialChanged {
            visible: self.dynamic_state.tutorial_visible,
        });
        cx.notify();
    }
    pub fn close_dynamic_tutorial(&mut self, cx: &mut Context<Self>) {
        self.dynamic_state.tutorial_visible = false;
        cx.emit(MouseDynamicTutorialChanged { visible: false });
        cx.notify();
    }
    pub fn invalidate_dynamic_connection(&mut self, cx: &mut Context<Self>) {
        self.dynamic_state.connection_epoch = self.dynamic_state.connection_epoch.wrapping_add(1);
        self.dynamic_state.observed = None;
        self.dynamic_state.firmware = None;
        self.dynamic_state.drag = None;
        self.dynamic_state.hover = None;
        self.dynamic_state.firmware_hover = None;
        self.dynamic_state.pending.clear();
        self.dynamic_state.latest_mutation = None;
        self.dynamic_state.mouse_speed = 0.;
        self.dynamic_state.speed_task = None;
        self.dynamic_state.loading = false;
        self.dynamic_state.last_failure = None;
        self.dynamic_state.points = if let Some(points) = curve::preset(self.dynamic_mode()) {
            points.to_vec()
        } else {
            serde_json::from_value(self.dynamic_value()["points"].clone()).unwrap_or_default()
        };
        cx.notify();
    }
    pub fn observe_mouse_dynamic(
        &mut self,
        scope: MouseDynamicScope,
        observation: MouseDynamicObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_dynamic_scope() != Some(scope) {
            return;
        }
        match observation {
            MouseDynamicObservation::DynamicSensitivity(value) => {
                if !valid_state(&value) {
                    return;
                }
                let changing_branch = value["mode"] != self.dynamic_value()["mode"]
                    || value["state"] != self.dynamic_value()["state"];
                if changing_branch {
                    self.finish_dynamic_drag(window, cx);
                }
                let drag = self.dynamic_state.drag;
                self.dynamic_state.observed = Some(value);
                self.dynamic_state.local = false;
                self.sync_dynamic_points(window, cx);
                if !changing_branch {
                    self.dynamic_state.drag =
                        drag.filter(|index| *index < self.dynamic_state.points.len());
                }
            }
            MouseDynamicObservation::Loading(value) => self.dynamic_state.loading = value,
            MouseDynamicObservation::FirmwareVersion(value) => {
                self.finish_dynamic_drag(window, cx);
                self.dynamic_state.firmware = value;
                self.dynamic_state.gap_x = 15.;
                self.dynamic_state.gap_y = 0.25;
                self.dynamic_ui_command(cx);
            }
            MouseDynamicObservation::ConnectionChanged => {
                self.invalidate_dynamic_connection(cx);
                self.sync_dynamic_points(window, cx);
            }
            MouseDynamicObservation::MouseSpeed(speed) => {
                if !speed.is_finite() || speed < 0. {
                    return;
                }
                self.dynamic_state.mouse_speed = speed;
                self.dynamic_state.speed_task = Some(cx.spawn_in(window, async move |view, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(150))
                        .await;
                    let _ = view.update(cx, |this, cx| {
                        if this.mouse_dynamic_scope() == Some(scope) {
                            this.dynamic_state.mouse_speed = 0.;
                            cx.notify();
                        }
                    });
                }));
            }
        }
        cx.notify();
    }
    fn dynamic_value(&self) -> &Value {
        if !self.dynamic_state.local {
            if let Some(value) = &self.dynamic_state.observed {
                return value;
            }
        }
        &self.draft["dynamicSensitivity"]
    }
    fn dynamic_mode(&self) -> u32 {
        self.dynamic_value()["mode"].as_u64().unwrap_or(0) as u32
    }
    fn dynamic_on(&self) -> bool {
        self.dynamic_value()["state"].as_u64() == Some(1)
    }
    fn dynamic_editable(&self) -> bool {
        self.active
            && self.page == "ADVANCED"
            && self.dynamic_on()
            && self.advanced_enabled()
            && !self.dynamic_state.loading
    }
    fn dynamic_request(
        &mut self,
        command: &'static str,
        payload: Value,
        timer_tick: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        self.dynamic_state.next_request = self.dynamic_state.next_request.wrapping_add(1);
        let request_id = self.dynamic_state.next_request;
        self.dynamic_state.pending.insert(request_id, command);
        if command == "ON_SET_DYNAMIC_SENSITIVITY" {
            self.dynamic_state.latest_mutation = Some(request_id);
        }
        cx.emit(MouseDynamicRequested {
            request_id,
            scope: self.dynamic_state.scope(),
            product_id: self.spec.product_id,
            command,
            payload,
            timer_tick,
        });
    }
    pub(super) fn dynamic_ui_command(&mut self, cx: &mut Context<Self>) {
        if !self.spec.dynamic || !self.advanced_enabled() {
            return;
        }
        self.dynamic_request(
            "DYNAMIC_SENSITIVITY_UI_COMMAND",
            json!({"type":if self.dynamic_on() { "START" } else { "STOP" }}),
            None,
            cx,
        );
    }
    fn patch_dynamic(&mut self, patch: Value, window: &mut Window, cx: &mut Context<Self>) {
        let mut value = self.dynamic_value().clone();
        let Some(target) = value.as_object_mut() else {
            return;
        };
        let Some(patch) = patch.as_object() else {
            return;
        };
        target.extend(
            patch
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        self.draft["dynamicSensitivity"] = value.clone();
        self.draft[LOCAL] = json!(true);
        self.dynamic_state.local = true;
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|value| value.as_millis() as u64);
        self.dynamic_request(
            "ON_SET_DYNAMIC_SENSITIVITY",
            json!({"dynamicSensitivity":value}),
            tick,
            cx,
        );
        self.sync_dynamic_template(window, cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }
    fn sync_dynamic_template(&self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.dynamic_value()["templateId"]
            .as_u64()
            .unwrap_or(0)
            .to_string();
        self.dynamic_state.template.update(cx, |state, cx| {
            state.set_selected_value(&selected, window, cx)
        });
    }
    fn sync_dynamic_points(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dynamic_state.drag = None;
        self.dynamic_state.hover = None;
        self.dynamic_state.points = if let Some(points) = curve::preset(self.dynamic_mode()) {
            points.to_vec()
        } else {
            serde_json::from_value(self.dynamic_value()["points"].clone()).unwrap_or_default()
        };
        self.sync_dynamic_template(window, cx);
    }
    pub(super) fn restore_dynamic(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dynamic_state.profile_epoch = self.dynamic_state.profile_epoch.wrapping_add(1);
        self.dynamic_state.observed = None;
        self.dynamic_state.local = saved.is_some_and(|saved| {
            saved.get("dynamicSensitivity").is_some()
                && saved
                    .get(LOCAL)
                    .is_none_or(|value| value.as_bool() == Some(true))
        });
        self.dynamic_state.loading = false;
        self.dynamic_state.pending.clear();
        self.dynamic_state.latest_mutation = None;
        self.dynamic_state.last_failure = None;
        self.dynamic_state.gap_x = 15.;
        self.dynamic_state.gap_y = 0.25;
        self.dynamic_state.mouse_speed = 0.;
        self.dynamic_state.speed_task = None;
        self.sync_dynamic_points(window, cx);
    }
    pub(super) fn dismiss_dynamic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_dynamic_drag(window, cx);
        self.dynamic_state.hover = None;
        self.dynamic_state.zoom_hover = None;
        self.dynamic_state.zoom_tip = None;
        self.dynamic_state.gap_x = 15.;
        self.dynamic_state.gap_y = 0.25;
        self.dynamic_state.mouse_speed = 0.;
        self.dynamic_state.speed_task = None;
    }
    fn commit_dynamic_points(
        &mut self,
        template: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dynamic_mode() != 3 {
            return;
        }
        let Some(table) = curve::acceleration_table(&self.dynamic_state.points) else {
            self.dynamic_state.last_failure =
                Some("Dynamic sensitivity curve has invalid or overlapping coordinates".into());
            cx.notify();
            return;
        };
        let points = self.dynamic_state.points.clone();
        self.patch_dynamic(json!({"mode":3,"templateId":template,"points":points,"customSensorAccelerations":table}), window, cx);
        // Original points effect calls setShowLoading(true). Only the real
        // producer may clear it; a local draft is not a service acknowledgement.
        self.dynamic_state.loading = true;
    }
    fn select_dynamic_mode(&mut self, mode: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dynamic_editable() || mode > 3 || mode == self.dynamic_mode() {
            return;
        }
        self.finish_dynamic_drag(window, cx);
        self.patch_dynamic(json!({"mode":mode}), window, cx);
        if mode == 3 {
            let template = self.dynamic_value()["templateId"].as_u64().unwrap_or(0) as u32;
            self.dynamic_state.points = if let Some(points) = curve::preset(template) {
                points.to_vec()
            } else {
                serde_json::from_value(self.dynamic_value()["points"].clone()).unwrap_or_default()
            };
            self.commit_dynamic_points(template, window, cx);
        } else {
            self.dynamic_state.points = curve::preset(mode).unwrap().to_vec();
        }
    }
    fn finish_dynamic_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dynamic_state.drag.take().is_some() {
            self.commit_dynamic_points(3, window, cx);
        }
    }
    fn dynamic_grid(&self) -> Grid {
        if self.dynamic_mode() == 3 {
            Grid {
                gap_x: self.dynamic_state.gap_x,
                gap_y: self.dynamic_state.gap_y,
                start_y: 0.,
            }
        } else {
            Grid {
                gap_x: 10.,
                gap_y: 0.1,
                start_y: 1.,
            }
        }
    }
    fn dynamic_hit(
        &self,
        position: Point<Pixels>,
        area: Bounds<Pixels>,
        unit: f32,
    ) -> Option<usize> {
        let pointer = [
            f32::from(position.x - area.origin.x) / unit,
            f32::from(position.y - area.origin.y) / unit,
        ];
        self.dynamic_state.points.iter().position(|point| {
            let point = self.dynamic_grid().pixel(*point);
            (pointer[0] - point[0]).powi(2) + (pointer[1] - point[1]).powi(2) <= 16.
        })
    }
    fn move_dynamic_drag(
        &mut self,
        position: Point<Pixels>,
        area: Bounds<Pixels>,
        unit: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.dynamic_state.drag else {
            return;
        };
        let grid = self.dynamic_grid();
        let point = grid.domain([
            f32::from(position.x - area.origin.x) / unit,
            f32::from(position.y - area.origin.y) / unit,
        ]);
        curve::drag(
            &mut self.dynamic_state.points,
            index,
            point,
            grid.gap_x * 7.,
            grid.start_y + grid.gap_y * 6.,
        );
        cx.notify();
    }
    fn zoom_dynamic(&mut self, axis: &str, direction: f64, cx: &mut Context<Self>) {
        if axis != "y" {
            self.dynamic_state.gap_x = (self.dynamic_state.gap_x + direction * 5.).clamp(5., 40.);
        }
        if axis != "x" {
            self.dynamic_state.gap_y =
                (self.dynamic_state.gap_y + direction * 0.25).clamp(0.25, 1.);
        }
        cx.notify();
    }
    pub(super) fn dynamic_panel(&self, cx: &Context<Self>) -> AnyElement {
        let editable = self.dynamic_editable();
        let on = self.dynamic_on();
        let content_opacity =
            (if on { 1. } else { 0.3 }) * (if self.dynamic_state.loading { 0.3 } else { 1. });
        let mut panel = surface::panel_with_title_switch(
            t("DYNAMIC_SENSITIVITY"),
            surface::SynapseSwitch::new("mouse-dynamic-enabled")
                .accessibility_label(t("DYNAMIC_SENSITIVITY"))
                .checked(on)
                .disabled(!self.advanced_enabled())
                .on_change(cx.listener(|this, value: &bool, window, cx| {
                    if !this.advanced_enabled() {
                        return;
                    }
                    this.finish_dynamic_drag(window, cx);
                    this.patch_dynamic(json!({"state":if *value {1} else {0}}), window, cx);
                    this.dynamic_ui_command(cx);
                })),
            surface::help_control("mouse-dynamic-help", t("DYNAMIC_SENSITIVITY_TOOLTIP")),
            cx,
        )
        .max_w(surface::css(1200.))
        .ml(surface::css(10.))
        .p(surface::css(40.))
        .opacity(if self.advanced_enabled() { 1. } else { 0.3 })
        .child(
            div()
                .opacity(if self.spec.product_id == 226 {
                    1.
                } else {
                    content_opacity
                })
                .child(t("DYNAMIC_SENSITIVITY_DESC")),
        )
        .child(
            h_flex()
                .gap(surface::css(10.))
                .pt(surface::css(16.))
                .pb(surface::css(6.))
                .opacity(content_opacity)
                .children(LABELS.iter().enumerate().map(|(mode, key)| {
                    let selected = self.dynamic_mode() == mode as u32;
                    BaseButton::new(SharedString::from(format!("mouse-dynamic-{mode}")))
                        .disabled(!editable)
                        .h(surface::css(27.))
                        .flex_1()
                        .w(surface::css(268.))
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(rgb(if selected { 0x44d62c } else { 0x5d5d5d }))
                        .bg(rgb(if selected { 0x292929 } else { 0x111111 }))
                        .flex()
                        .justify_center()
                        .items_center()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .child(t(key).to_uppercase())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_dynamic_mode(mode as u32, window, cx)
                        }))
                })),
        )
        .child(self.dynamic_chart(cx));
        if self.dynamic_state.loading {
            panel = panel.child(
                div()
                    .absolute()
                    .top(surface::css(38.))
                    .left(surface::css(310.))
                    .child(loading_spinner()),
            );
        }
        let panel = panel.id("mouse-dynamic-panel").on_hover(cx.listener(
            |this, hover: &bool, window, cx| {
                this.dynamic_state.firmware_hover = if *hover && !this.advanced_enabled() {
                    Some(window.mouse_position())
                } else {
                    None
                };
                cx.notify();
            },
        ));
        let mut root = div().child(panel);
        if let Some(position) = self.dynamic_state.firmware_hover {
            root = root.child(
                deferred(
                    anchored().position(position).child(
                        div()
                            .w(surface::css(300.))
                            .px(surface::css(10.))
                            .py(surface::css(8.))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .bg(rgb(0x000000))
                            .text_color(rgb(0xcccccc))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .child(t(if self.spec.product_id == 229 {
                                "FW_UPDATE_MASSAGE"
                            } else {
                                "FW_UPDATE_MESSAGE"
                            }))
                            .with_animation(
                                "mouse-dynamic-firmware-tip",
                                Animation::new(Duration::from_millis(300)),
                                |tip, delta| tip.opacity(delta),
                            ),
                    ),
                )
                .with_priority(99),
            );
        }
        root.into_any_element()
    }
    pub fn dynamic_tutorial_icon(&self, cx: &Context<Self>) -> AnyElement {
        if !self.has_dynamic_tutorial() {
            return div().into_any_element();
        }
        DynamicTutorialIcon {
            active: self.dynamic_state.tutorial_visible,
            owner: cx.entity(),
        }
        .into_any_element()
    }
    pub fn dynamic_tutorial_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.dynamic_tutorial_visible() {
            return div().into_any_element();
        }
        let unit = window.rem_size() / 16.;
        let panel = div()
            .absolute()
            .occlude()
            .left((window.viewport_size().width - unit * 802.) / 2.)
            .top(surface::css(134.))
            .w(surface::css(802.))
            .h(surface::css(422.))
            .border_1()
            .border_color(rgb(0x44d62c))
            .rounded(surface::css(5.))
            .bg(rgb(0x111111))
            .flex()
            .child(
                div()
                    .w(surface::css(320.))
                    .h(surface::css(420.))
                    .flex_shrink_0()
                    .child(
                        img(tutorial_media())
                            .id("mouse-dynamic-tutorial-media")
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    ),
            )
            .child(
                v_flex()
                    .w(surface::css(480.))
                    .h(surface::css(420.))
                    .flex_shrink_0()
                    .pt(surface::css(50.))
                    .pb(surface::css(50.))
                    .pl(surface::css(50.))
                    .pr(surface::css(30.))
                    .text_size(surface::css(14.))
                    .child(
                        div()
                            .text_size(surface::css(20.))
                            .child(t("DYNAMIC_SENSITIVITY")),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x30961f))
                            .font_weight(FontWeight::LIGHT)
                            .child(t("DYNAMIC_SENSITIVITY_TUTORIAL_SUBTITLE").to_uppercase()),
                    )
                    .child(
                        div()
                            .my(surface::css(10.))
                            .child(t("DYNAMIC_SENSITIVITY_TUTORIAL_CAPTION")),
                    )
                    .child(
                        v_flex()
                            .id("mouse-dynamic-tutorial-items")
                            .mt(surface::css(20.))
                            .pl(surface::css(20.))
                            .gap(surface::css(10.))
                            .h(surface::css(220.))
                            .overflow_y_scroll()
                            .child(tutorial_item("CLASSIC"))
                            .child(tutorial_item("NATURAL"))
                            .child(tutorial_item("JUMP"))
                            .child(tutorial_item("CUSTOM")),
                    ),
            )
            .child(
                BaseButton::new("mouse-dynamic-tutorial-close")
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .size(surface::css(25.))
                    .bg(cx.theme().transparent)
                    .accessibility_label(t("CLOSE"))
                    .child(img(close_icon()).size(surface::css(25.)))
                    .on_click(cx.listener(|this, _, _, cx| this.close_dynamic_tutorial(cx))),
            );
        Dialog::new(cx)
            .close_on_escape(false)
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .h(relative(1.5))
                    .bg(rgba(0x00000080)),
            )
            .popup(div().absolute().inset_0().child(panel))
            .into_any_element()
    }
    fn dynamic_chart(&self, cx: &Context<Self>) -> AnyElement {
        let grid = self.dynamic_grid();
        let custom = self.dynamic_mode() == 3;
        let enabled = self.dynamic_editable();
        let points = self.dynamic_state.points.clone();
        let firmware = self.advanced_enabled();
        let speed = self.dynamic_state.mouse_speed;
        let area = Rc::new(Cell::new(Bounds::default()));
        let measured = area.clone();
        let drag_area = area.clone();
        let hover_area = area.clone();
        let owner = cx.entity().downgrade();
        let canvas = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                measured.set(bounds);
                let unit = f32::from(window.rem_size()) / 16.;
                paint_chart(bounds, unit, grid, &points, custom, firmware, speed, window);
                let Some(entity) = owner.upgrade() else {
                    return;
                };
                // Source wheel listener is document-wide, even when presets are
                // displayed. Keep transient custom zoom until component unmount.
                let wheel_owner = entity.clone();
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, _, cx| {
                    if phase.capture() && event.modifiers.control {
                        let delta = event.delta.pixel_delta(px(1.)).y;
                        if delta != px(0.) {
                            wheel_owner.update(cx, |this, cx| {
                                this.zoom_dynamic("both", if delta > px(0.) { -1. } else { 1. }, cx)
                            });
                        }
                    }
                });
                if entity.read(cx).dynamic_state.drag.is_none() {
                    return;
                }
                let motion = entity.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase.capture() {
                        motion.update(cx, |this, cx| {
                            this.move_dynamic_drag(event.position, bounds, unit, cx)
                        });
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase.capture() && event.button == MouseButton::Left {
                        entity.update(cx, |this, cx| this.finish_dynamic_drag(window, cx));
                    }
                });
            },
        )
        .w(surface::css(1100.))
        .h(surface::css(300.));
        let mut chart = div()
            .id("mouse-dynamic-chart")
            .relative()
            .w(surface::css(1100.))
            .h(surface::css(300.))
            .mt(surface::css(10.))
            .mb(surface::css(20.))
            .child(canvas)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if this.dynamic_mode() != 3 || !this.dynamic_editable() {
                        return;
                    }
                    let unit = f32::from(window.rem_size()) / 16.;
                    this.dynamic_state.drag =
                        this.dynamic_hit(event.position, drag_area.get(), unit);
                    if this.dynamic_state.drag.is_some() {
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    let hover = if this.dynamic_mode() == 3 && this.dynamic_editable() {
                        this.dynamic_hit(
                            event.position,
                            hover_area.get(),
                            f32::from(window.rem_size()) / 16.,
                        )
                        .map(|index| (index, event.position))
                    } else {
                        None
                    };
                    if this.dynamic_state.hover != hover {
                        this.dynamic_state.hover = hover;
                        cx.notify();
                    }
                }),
            )
            .on_hover(cx.listener(|this, hover: &bool, _, cx| {
                if !*hover {
                    this.dynamic_state.hover = None;
                    cx.notify();
                }
            }));
        chart = chart
            .child(
                h_flex()
                    .absolute()
                    .left(surface::css(70.))
                    .bottom(surface::css(-10.))
                    .w(surface::css(1022.))
                    .h(surface::css(20.))
                    .justify_between()
                    .text_size(surface::css(10.))
                    .text_color(rgb(0x666666))
                    .children(
                        (0..8).map(|index| div().child(format!("{}", index as f64 * grid.gap_x))),
                    ),
            )
            .child(
                v_flex()
                    .absolute()
                    .left(surface::css(38.))
                    .top(surface::css(13.))
                    .w(surface::css(20.))
                    .h(surface::css(267.))
                    .justify_between()
                    .text_size(surface::css(10.))
                    .text_color(rgb(0x666666))
                    .children((0..7).rev().filter(|index| index % 2 == 0).map(|index| {
                        div().child(format!("{:.1}", grid.start_y + index as f64 * grid.gap_y))
                    })),
            );
        let mut root = div()
            .relative()
            .w(surface::css(1100.))
            .opacity(
                (if self.dynamic_on() { 1. } else { 0.09 })
                    * (if self.dynamic_state.loading { 0.3 } else { 1. }),
            )
            .child(
                img(ratio_label(self.spec.product_id))
                    .absolute()
                    .left_0()
                    .top(surface::css(145.))
                    .w(surface::css(20.))
                    .h(surface::css(130.)),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_end()
                    .gap(surface::css(10.))
                    .mt(surface::css(10.))
                    .h(surface::css(27.))
                    .opacity(if custom { 1. } else { 0. })
                    .child(t("DYNAMIC_SENSITIVITY_SELECT_TEMPLATE").to_uppercase())
                    .child(
                        surface::select(&self.dynamic_state.template)
                            .items(choices())
                            .accessibility_label(t("DYNAMIC_SENSITIVITY_SELECT_TEMPLATE"))
                            .disabled(!custom || !enabled)
                            .w(surface::css(200.)),
                    ),
            )
            .child(chart)
            .child(
                div()
                    .absolute()
                    .w_full()
                    .text_center()
                    .child(t("DYNAMIC_SENSITIVITY_INPUT_SPEED")),
            );
        if custom {
            root = root
                .child(
                    self.dynamic_zoom("y", enabled, cx)
                        .absolute()
                        .left_0()
                        .top_0(),
                )
                .child(
                    self.dynamic_zoom("x", enabled, cx)
                        .absolute()
                        .right_0()
                        .bottom(surface::css(-30.)),
                );
        }
        if let Some((index, position)) = self.dynamic_state.hover {
            let curve_point = self.dynamic_state.points[index];
            // Deferred window overlay preserves fixed client coordinates and
            // escapes the page scroll mask, as the original tooltip does.
            root = root.child(
                deferred(
                    anchored()
                        .position(point(position.x - px(55.), position.y - px(45.)))
                        .child(
                            div()
                                .w(surface::css(110.))
                                .text_center()
                                .p(surface::css(5.))
                                .rounded(surface::css(4.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .bg(rgb(0x222222))
                                .text_color(rgb(0xcccccc))
                                .text_size(surface::css(14.))
                                .child(format!("({:.0}, {:.2})", curve_point.x, curve_point.y)),
                        ),
                )
                .with_priority(10),
            );
        }
        root.into_any_element()
    }
    fn dynamic_zoom(&self, axis: &'static str, enabled: bool, cx: &Context<Self>) -> Stateful<Div> {
        let hovered = self.dynamic_state.zoom_hover == Some(axis);
        h_flex()
            .id(SharedString::from(format!("mouse-dynamic-zoom-{axis}")))
            .on_hover(cx.listener(move |this, hover: &bool, _, cx| {
                this.dynamic_state.zoom_hover = if *hover { Some(axis) } else { None };
                cx.notify();
            }))
            .children(
                [(false, 1.), (true, -1.)]
                    .into_iter()
                    .map(|(plus, direction)| {
                        let key = if plus {
                            "DYNAMIC_SENSITIVITY_RATIO_ZOOM_IN"
                        } else {
                            "DYNAMIC_SENSITIVITY_RATIO_ZOOM_OUT"
                        };
                        let shortcut = if plus {
                            "DYNAMIC_SENSITIVITY_RATIO_ZOOM_IN_SHORTCUT"
                        } else {
                            "DYNAMIC_SENSITIVITY_RATIO_ZOOM_OUT_SHORTCUT"
                        };
                        let button = BaseButton::new(SharedString::from(format!(
                            "mouse-dynamic-zoom-{axis}-{}",
                            if plus { "in" } else { "out" }
                        )))
                        .disabled(!enabled)
                        .size(surface::css(30.))
                        .border_1()
                        .border_color(rgb(if hovered { 0x44d62c } else { 0x5d5d5d }))
                        .when(plus, |button| button.border_l_0())
                        .when(!plus, |button| button.border_r_0())
                        .bg(rgb(0x222222))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(zoom_image(plus)).size(surface::css(24.)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.dynamic_editable() {
                                this.zoom_dynamic(axis, direction, cx);
                            }
                        }));
                        div()
                            .id(SharedString::from(format!(
                                "mouse-dynamic-zoom-tip-{axis}-{plus}"
                            )))
                            .relative()
                            .on_hover(cx.listener(move |this, hover: &bool, _, cx| {
                                this.dynamic_state.zoom_tip =
                                    if *hover { Some((axis, plus)) } else { None };
                                cx.notify();
                            }))
                            .child(button)
                            .children(
                                (enabled && self.dynamic_state.zoom_tip == Some((axis, plus)))
                                    .then(|| {
                                        let tip = div()
                                            .absolute()
                                            .bottom_full()
                                            .mb(surface::css(5.))
                                            .w(surface::css(300.))
                                            .flex()
                                            .when(axis == "x", |tip| tip.right_0().justify_end())
                                            .when(axis == "y", |tip| tip.left_0().justify_start())
                                            .child(
                                                div()
                                                    .px(surface::css(10.))
                                                    .py(surface::css(8.))
                                                    .border_1()
                                                    .border_color(rgb(0x5d5d5d))
                                                    .bg(rgb(0x000000))
                                                    .text_size(surface::css(14.))
                                                    .line_height(surface::css(16.))
                                                    .child(t(key))
                                                    .child(" ")
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0x44d62c))
                                                            .child(t(shortcut)),
                                                    ),
                                            );
                                        deferred(tip.with_animation(
                                            SharedString::from(format!(
                                                "mouse-dynamic-zoom-tip-motion-{axis}-{plus}"
                                            )),
                                            Animation::new(Duration::from_millis(100)),
                                            |tip, delta| tip.opacity(delta),
                                        ))
                                        .with_priority(10)
                                    }),
                            )
                    }),
            )
    }
}
#[derive(IntoElement)]
struct DynamicTutorialIcon {
    active: bool,
    owner: Entity<MouseProductWorkspace>,
}
impl RenderOnce for DynamicTutorialIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::base::motion::{self, Easing, Transition};
        let hovered = window.use_keyed_state(
            ("mouse-dynamic-tutorial-hover", self.owner.entity_id()),
            cx,
            |_, _| false,
        );
        let opacity = motion::transition(
            (
                ElementId::from(("mouse-dynamic-tutorial", self.owner.entity_id())),
                "opacity",
            ),
            if self.active || *hovered.read(cx) {
                1_f32
            } else {
                0.8
            },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        div()
            .w(surface::css(24.))
            .h(surface::css(24.))
            .mr(surface::css(10.))
            .rounded(surface::css(5.))
            .flex()
            .items_center()
            .justify_center()
            .child(
                BaseButton::new("mouse-dynamic-tutorial-toggle")
                    .role(Role::Tab)
                    .size(surface::css(25.))
                    .p_0()
                    .bg(cx.theme().transparent)
                    .accessibility_label(t("TUTORIAL"))
                    .selected(self.active)
                    .border_color(if self.active {
                        rgb(0x44d62c).into()
                    } else {
                        cx.theme().transparent
                    })
                    .when(self.active, |button| button.border_1())
                    .opacity(opacity)
                    .child(img(tutorial_icon()).size(surface::css(25.)))
                    .on_hover(window.listener_for(&hovered, |state, value: &bool, _, cx| {
                        *state = *value;
                        cx.notify();
                    }))
                    .on_click(move |_, _, cx| {
                        self.owner
                            .update(cx, |owner, cx| owner.toggle_dynamic_tutorial(cx));
                    }),
            )
    }
}

fn valid_state(value: &Value) -> bool {
    value["state"].as_u64().is_some_and(|v| v <= 1)
        && value["mode"].as_u64().is_some_and(|v| v <= 3)
        && value["templateId"].as_u64().is_some_and(|v| v <= 3)
        && value["customSensorAccelerations"].as_array().is_some()
        && value["points"].as_array().is_some_and(|v| {
            v.is_empty()
                || serde_json::from_value::<Vec<CurvePoint>>(value["points"].clone())
                    .ok()
                    .is_some_and(|points| Curve::new(&points).is_some())
        })
}

#[derive(Clone, Copy)]
struct Grid {
    gap_x: f64,
    gap_y: f64,
    start_y: f64,
}
impl Grid {
    fn pixel(self, point: CurvePoint) -> [f32; 2] {
        [
            (70. + point.x / (7. * self.gap_x) * 1010.) as f32,
            (280. - (point.y - self.start_y) / (6. * self.gap_y) * 260.) as f32,
        ]
    }
    fn domain(self, pixel: [f32; 2]) -> CurvePoint {
        CurvePoint {
            x: (pixel[0] as f64 - 70.) / 1010. * 7. * self.gap_x,
            y: (280. - pixel[1] as f64) / 260. * 6. * self.gap_y + self.start_y,
        }
    }
}
fn paint_chart(
    area: Bounds<Pixels>,
    unit: f32,
    grid: Grid,
    points: &[CurvePoint],
    custom: bool,
    firmware: bool,
    speed: f64,
    window: &mut Window,
) {
    let position = |p: [f32; 2]| area.origin + point(px(p[0] * unit), px(p[1] * unit));
    let line = |a, b, color, window: &mut Window| {
        let mut path = PathBuilder::stroke(px(unit));
        path.move_to(position(a));
        path.line_to(position(b));
        if let Ok(path) = path.build() {
            window.paint_path(path, rgb(color));
        }
    };
    let pixel_points: Vec<_> = points
        .iter()
        .map(|p| {
            let p = grid.pixel(*p);
            CurvePoint {
                x: p[0] as f64,
                y: p[1] as f64,
            }
        })
        .collect();
    let color = rgb(if custom { 0xe4790f } else { 0x44d62c });
    let curve = Curve::new(&pixel_points);
    if let Some(curve) = &curve {
        let end = pixel_points.last().unwrap().x.ceil();
        let samples = |last: f64| {
            (70..=last.floor() as i32)
                .map(|x| [x as f32, curve.interpolate(x as f64) as f32])
                .collect::<Vec<_>>()
        };
        // Fill comes from the separate observed mouseSpeed canvas, not from a
        // synthetic animated preview. The original resets it after 150ms.
        if speed > 0. {
            let last = grid.pixel(CurvePoint { x: speed, y: 0. })[0] as f64;
            let p = samples(last.min(end));
            if let Some(path) = smooth_path(&p, true, unit, area) {
                window.paint_path(path, color.opacity(0.3));
            }
        }
    }
    for index in 0..7 {
        let y = grid.pixel(CurvePoint {
            x: 0.,
            y: grid.start_y + index as f64 * grid.gap_y,
        })[1];
        line([58., y], [1092., y], 0x333333, window);
    }
    for index in 0..8 {
        let x = grid.pixel(CurvePoint {
            x: index as f64 * grid.gap_x,
            y: 0.,
        })[0];
        line([x, 13.], [x, 287.], 0x333333, window);
    }
    line([58., 280.], [1092., 280.], 0x5d5d5d, window);
    line([70., 13.], [70., 287.], 0x5d5d5d, window);
    if let Some(curve) = curve {
        let end = pixel_points.last().unwrap().x.ceil();
        let samples = |last: f64| {
            (70..=last.floor() as i32)
                .map(|x| [x as f32, curve.interpolate(x as f64) as f32])
                .collect::<Vec<_>>()
        };
        let clip = Bounds::new(
            position([66., 16.]),
            size(px(1018. * unit), px(264. * unit)),
        );
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            if firmware {
                if let Some(path) = smooth_path(&samples(end), false, unit, area) {
                    window.paint_path(path, color);
                }
            }
            if custom {
                for p in &pixel_points {
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                position([p.x as f32 - 4., p.y as f32 - 4.]),
                                size(px(8. * unit), px(8. * unit)),
                            ),
                            color,
                        )
                        .corner_radii(px(4. * unit)),
                    );
                }
            }
        });
    }
}
/// spinner.ef2d0235.svg's SMIL ring: dash 10% -> 50% -> 10%,
/// rotation 0 -> 180 -> 720 degrees, two seconds, linear, repeating.
fn loading_spinner() -> impl IntoElement {
    div()
        .w(surface::css(26.))
        .h(surface::css(20.))
        .with_animation(
            "mouse-dynamic-loading",
            Animation::new(Duration::from_secs(2)).repeat(),
            |element, delta| {
                element.child(
                    canvas(
                        |_, _, _| (),
                        move |area, _, window, _| {
                            let unit = f32::from(window.rem_size()) / 16.;
                            let center = area.origin + point(px(13. * unit), px(10. * unit));
                            let angle = if delta <= 0.5 {
                                delta * 360.
                            } else {
                                180. + (delta - 0.5) * 1080.
                            };
                            let fraction = if delta <= 0.5 {
                                0.1 + delta * 0.8
                            } else {
                                0.5 - (delta - 0.5) * 0.8
                            };
                            for (start, length, color) in [
                                (0., std::f32::consts::TAU, rgb(0x44d62c).opacity(0.3)),
                                (
                                    angle.to_radians(),
                                    std::f32::consts::TAU * fraction,
                                    rgb(0x44d62c).into(),
                                ),
                            ] {
                                let mut path = PathBuilder::stroke(px(2.6 * unit));
                                for index in 0..=100 {
                                    let angle = start + length * index as f32 / 100.;
                                    let p = center
                                        + point(
                                            px(angle.cos() * 7.8 * unit),
                                            px(angle.sin() * 7.8 * unit),
                                        );
                                    if index == 0 {
                                        path.move_to(p);
                                    } else {
                                        path.line_to(p);
                                    }
                                }
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, color);
                                }
                            }
                        },
                    )
                    .size_full(),
                )
            },
        )
}
fn smooth_path(
    points: &[[f32; 2]],
    filled: bool,
    unit: f32,
    area: Bounds<Pixels>,
) -> Option<Path<Pixels>> {
    let first = points.first()?;
    let last = points.last()?;
    let position = |p: [f32; 2]| area.origin + point(px(p[0] * unit), px(p[1] * unit));
    let mut path = if filled {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(px(unit))
    };
    path.move_to(position(*first));
    for index in 1..points.len().saturating_sub(1) {
        let p = points[index];
        let next = points[index + 1];
        path.curve_to(
            position([(p[0] + next[0]) / 2., (p[1] + next[1]) / 2.]),
            position(p),
        );
    }
    path.line_to(position(*last));
    if filled {
        path.line_to(position([last[0], 280.]));
        path.line_to(position([70., 280.]));
        path.close();
    }
    path.build().ok()
}
fn zoom_image(plus: bool) -> Arc<Image> {
    static PLUS: OnceLock<Arc<Image>> = OnceLock::new();
    static MINUS: OnceLock<Arc<Image>> = OnceLock::new();
    (if plus { &PLUS } else { &MINUS })
        .get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                if plus {
                    include_bytes!("../../../../assets/synapse/mouse-dynamic-plus.svg").to_vec()
                } else {
                    include_bytes!("../../../../assets/synapse/mouse-dynamic-minus.svg").to_vec()
                },
            ))
        })
        .clone()
}
fn tutorial_icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            include_bytes!("../../../../assets/synapse/mouse-dynamic-tutorial.svg").to_vec(),
        ))
    })
    .clone()
}
fn tutorial_media() -> Arc<Image> {
    static MEDIA: OnceLock<Arc<Image>> = OnceLock::new();
    MEDIA
        .get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Webp,
                include_bytes!("../../../../assets/synapse/mouse-dynamic-tutorial.webp").to_vec(),
            ))
        })
        .clone()
}
fn close_icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            include_bytes!("../../../../assets/synapse/mouse-dynamic-close.svg").to_vec(),
        ))
    })
    .clone()
}
fn tutorial_item(mode: &'static str) -> impl IntoElement {
    let escape = |text: String| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let title = escape(t(&format!("DYNAMIC_SENSITIVITY_TUTORIAL_{mode}_TITLE")));
    let description = escape(t(&format!("DYNAMIC_SENSITIVITY_TUTORIAL_{mode}_DESC")));
    h_flex()
        .items_start()
        .child(div().w(surface::css(12.)).ml(surface::css(-12.)).child("•"))
        .child(
            gpui_kit::base::TextView::html(
                SharedString::from(format!("mouse-dynamic-tutorial-text-{mode}")),
                format!("<strong>{title}.</strong> {description}"),
            )
            .selectable(false)
            .scrollable(false),
        )
}
fn ratio_label(pid: u32) -> Arc<Image> {
    let text = t("DYNAMIC_SENSITIVITY_RATIO");
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let upright = pid != 226
        && pid != 229
        && ["ja", "zh-CN", "zh-TW", "kr"].contains(&razer_i18n::locale().as_str());
    let content = if upright {
        format!(
            "<text x='10' y='0' font-family='Roboto' font-size='14' fill='#ccc' text-anchor='middle' writing-mode='tb'>{escaped}</text>"
        )
    } else {
        format!(
            "<text x='65' y='15' font-family='Roboto' font-size='14' fill='#ccc' text-anchor='middle' transform='translate(0 130) rotate(-90)'>{escaped}</text>"
        )
    };
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='20' height='130'>{content}</svg>")
            .into_bytes(),
    ))
}
