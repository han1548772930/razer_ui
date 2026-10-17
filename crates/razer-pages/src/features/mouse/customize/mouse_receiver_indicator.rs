//! Current mounted 190/229/239 three-LED receiver widget, not a generic icon.
//! Complete source owners/resources: mouse-receiver-indicator unified query.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::base::{Button as BaseButton, DialogBackdrop, DialogPopup};
use gpui_kit::component::{
    IndexPath,
    select::{SelectEvent, SelectState},
};
use razer_device::mouse_indicator_led::{Configuration, Mode, Quality, Report};
use razer_widgets::theme::MouseReceiverColors as Colors;
use std::{
    cell::Cell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[path = "mouse_receiver_legacy.rs"]
mod legacy;
#[path = "mouse_receiver_indicator_state.rs"]
mod source;

#[derive(Deserialize)]
struct Source {
    product_id: u32,
    image: String,
    guide_icon: String,
    show_guide: String,
    guide_title: String,
    guide_description: String,
}
fn source(product_id: u32) -> Option<&'static Source> {
    static SOURCE: OnceLock<Vec<Source>> = OnceLock::new();
    SOURCE
        .get_or_init(|| read_mouse_receiver_indicator_catalog())
        .iter()
        .find(|source| source.product_id == product_id)
}

#[derive(Clone)]
pub enum MouseReceiverAction {
    Initialize,
    Set { configuration: Value },
    LegacyInitialize,
    LegacySet { status: Value, persist: bool },
    LegacyBindingInfo,
}
#[derive(Clone)]
pub struct MouseReceiverIndicatorRequested {
    scope: MousePollingScope,
    sequence: u64,
    action: MouseReceiverAction,
    valid: Arc<AtomicBool>,
    profile: Value,
}
impl MouseReceiverIndicatorRequested {
    pub fn scope(&self) -> MousePollingScope {
        self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn action(&self) -> &MouseReceiverAction {
        &self.action
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn profile(&self) -> &Value {
        &self.profile
    }
    fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}
#[derive(Clone)]
pub struct MouseReceiverMonitorRequested {
    scope: MousePollingScope,
    valid: Arc<AtomicBool>,
}
impl MouseReceiverMonitorRequested {
    pub fn scope(&self) -> MousePollingScope {
        self.scope
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn matches_scope(&self, scope: MousePollingScope) -> bool {
        self.scope.same_device_connection(scope)
    }
}
impl EventEmitter<MouseReceiverIndicatorRequested> for MouseProductWorkspace {}
impl EventEmitter<MouseReceiverMonitorRequested> for MouseProductWorkspace {}

pub(super) struct State {
    configuration: Value,
    // Current et reducer defaults. They are presentation defaults, not reads.
    quality: Quality,
    battery: f64,
    selectors: Vec<Entity<SelectState<Vec<Choice>>>>,
    syncing: bool,
    sequence: u64,
    pending: Option<MouseReceiverIndicatorRequested>,
    queued: VecDeque<MouseReceiverIndicatorRequested>,
    initialized: Option<MousePollingScope>,
    monitor: Option<MouseReceiverMonitorRequested>,
    diagnostic: Option<Report>,
    guide: bool,
    guide_tab: usize,
    preview: &'static str,
    help: Entity<Help>,
    widget_mounted: bool,
    legacy_status: Value,
    legacy_image_epoch: u64,
    legacy_tip_hovered: bool,
    legacy_binding_requested: bool,
    legacy_binding_requests: Vec<MouseReceiverIndicatorRequested>,
}
impl State {
    pub(super) fn new(cx: &mut Context<MouseProductWorkspace>) -> Self {
        Self {
            configuration: json!(Configuration::default()),
            quality: Quality::Green,
            battery: 0.,
            selectors: Vec::new(),
            syncing: false,
            sequence: 0,
            pending: None,
            queued: VecDeque::new(),
            initialized: None,
            monitor: None,
            diagnostic: None,
            guide: false,
            guide_tab: 0,
            preview: "none",
            help: cx.new(|_| Help::default()),
            widget_mounted: false,
            legacy_status: json!(1),
            legacy_image_epoch: 0,
            legacy_tip_hovered: false,
            legacy_binding_requested: false,
            legacy_binding_requests: Vec::new(),
        }
    }
    fn cancel_requests(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.cancel();
        }
        for queued in self.queued.drain(..) {
            queued.cancel();
        }
        self.initialized = None;
        self.diagnostic = None;
    }
    pub(super) fn cancel(&mut self) {
        self.cancel_requests();
        for request in self.legacy_binding_requests.drain(..) {
            request.cancel();
        }
        if let Some(monitor) = self.monitor.take() {
            monitor.valid.store(false, Ordering::Release);
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn choices() -> Vec<Choice> {
    source::MODES
        .iter()
        .map(|(mode, key)| Choice::localized(u8::from(*mode).to_string(), *key))
        .collect()
}
impl MouseProductWorkspace {
    pub(super) fn prepare_receiver_indicator(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if source(self.spec.product_id).is_none() {
            return;
        }
        for index in 0..3 {
            let selected = source::MODES
                .iter()
                .position(|(value, _)| {
                    Some(*value) == source::mode(&self.receiver_indicator.configuration, index)
                })
                .map(IndexPath::new);
            let select = cx.new(|cx| SelectState::new(choices(), selected, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &select,
                window,
                move |this, _, event, _, cx| {
                    if this.receiver_indicator.syncing {
                        return;
                    }
                    if let SelectEvent::Confirm(Some(value)) = event
                        && let Ok(value) = value.parse::<u8>()
                        && let Ok(mode) = Mode::try_from(value)
                    {
                        this.choose_receiver_indicator(index, mode, cx);
                    }
                },
            ));
            self.receiver_indicator.selectors.push(select);
        }
    }
    pub(super) fn restore_receiver_indicator(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.receiver_indicator.cancel_requests();
        if legacy::source(self.spec.product_id).is_some() {
            self.restore_legacy_receiver(cx);
            return;
        }
        // MW reads the selected local profile and only posts its configuration
        // after the initialization SET resolves. Profile selection itself does
        // not replace this source reducer, even if the following SET fails.
        self.sync_receiver_indicator(cx);
    }
    fn sync_receiver_selectors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.receiver_indicator.syncing = true;
        for (index, select) in self.receiver_indicator.selectors.iter().enumerate() {
            let selected = source::MODES
                .iter()
                .position(|(mode, _)| {
                    Some(*mode) == source::mode(&self.receiver_indicator.configuration, index)
                })
                .map(IndexPath::new);
            select.update(cx, |select, cx| {
                select.set_selected_index(selected, window, cx)
            });
        }
        self.receiver_indicator.syncing = false;
    }
    pub(super) fn receiver_indicator_visible(&self) -> bool {
        if legacy::source(self.spec.product_id).is_some() {
            return self.legacy_receiver_visible();
        }
        source(self.spec.product_id).is_some()
            && self.polling_state.connection() == Some(PollingConnection::Dongle)
            && self.mouse_customize_observation["displayMode"] != "macro"
    }
    pub(super) fn sync_receiver_widget_mount(&mut self, cx: &mut Context<Self>) {
        if self.page != "TAB_CUSTOMIZE" {
            self.receiver_indicator.legacy_binding_requested = false;
        }
        let mounted = self.page == "TAB_CUSTOMIZE" && self.receiver_indicator_visible();
        if self.receiver_indicator.widget_mounted && !mounted {
            // CO removes Ja altogether on these branches. Its local guide and
            // carousel state do not survive that component unmount.
            self.receiver_indicator.guide = false;
            self.receiver_indicator.guide_tab = 0;
            self.receiver_indicator.preview = "none";
            self.receiver_indicator
                .help
                .update(cx, |help, _| *help = Help::default());
            self.receiver_indicator.legacy_image_epoch =
                self.receiver_indicator.legacy_image_epoch.wrapping_add(1);
            self.receiver_indicator.legacy_tip_hovered = false;
        }
        self.receiver_indicator.widget_mounted = mounted;
        self.request_legacy_binding_info(cx);
    }
    pub(super) fn sync_receiver_indicator(&mut self, cx: &mut Context<Self>) {
        self.sync_receiver_widget_mount(cx);
        if legacy::source(self.spec.product_id).is_some() {
            self.sync_legacy_receiver(cx);
            return;
        }
        if source(self.spec.product_id).is_none() {
            return;
        }
        let scope = self.mouse_polling_scope(cx);
        if self
            .receiver_indicator
            .monitor
            .as_ref()
            .is_some_and(|monitor| scope.is_none_or(|scope| !monitor.matches_scope(scope)))
        {
            self.receiver_indicator.cancel();
        }
        // MW feature initialization and event registration belong to the
        // device/profile lifetime, including wired mode and hidden host tabs.
        // Ja's isDongle/displayMode check only controls the rendered widget.
        let Some(scope) = scope else {
            return;
        };
        if self.receiver_indicator.monitor.is_none() {
            let monitor = MouseReceiverMonitorRequested {
                scope,
                valid: Arc::new(AtomicBool::new(true)),
            };
            self.receiver_indicator.monitor = Some(monitor.clone());
            cx.emit(monitor);
        }
        if self.receiver_indicator.initialized == Some(scope) {
            return;
        }
        self.receiver_indicator.initialized = Some(scope);
        self.enqueue_receiver_indicator(MouseReceiverAction::Initialize, scope, cx);
    }
    fn enqueue_receiver_indicator(
        &mut self,
        action: MouseReceiverAction,
        scope: MousePollingScope,
        cx: &mut Context<Self>,
    ) {
        self.receiver_indicator.sequence = self.receiver_indicator.sequence.wrapping_add(1);
        let request = MouseReceiverIndicatorRequested {
            scope,
            sequence: self.receiver_indicator.sequence,
            action,
            profile: self.snapshot(),
            valid: Arc::new(AtomicBool::new(true)),
        };
        if matches!(request.action, MouseReceiverAction::LegacyBindingInfo) {
            // Mounted TopView owns this getter independently of the active
            // profile. A profile reload cancels LED writes, not this read.
            self.receiver_indicator
                .legacy_binding_requests
                .push(request.clone());
            cx.emit(request);
            return;
        }
        if matches!(
            request.action,
            MouseReceiverAction::LegacyInitialize
                | MouseReceiverAction::LegacySet { .. }
                | MouseReceiverAction::LegacyBindingInfo
        ) {
            // MW persists every allowed event before its NORMAL_SKIPPABLE
            // device queue coalesces setters. Keep all event owners alive;
            // coalescing here would silently drop local version increments.
            if self.receiver_indicator.pending.is_none() {
                self.receiver_indicator.pending = Some(request.clone());
            } else {
                self.receiver_indicator.queued.push_back(request.clone());
            }
            cx.emit(request);
            return;
        }
        if self.receiver_indicator.pending.is_none() {
            self.receiver_indicator.pending = Some(request.clone());
            cx.emit(request);
        } else {
            if matches!(request.action, MouseReceiverAction::Set { .. })
                && self
                    .receiver_indicator
                    .queued
                    .back()
                    .is_some_and(|queued| matches!(queued.action, MouseReceiverAction::Set { .. }))
            {
                self.receiver_indicator.queued.pop_back().unwrap().cancel();
            }
            self.receiver_indicator.queued.push_back(request);
        }
    }
    fn choose_receiver_indicator(&mut self, index: usize, mode: Mode, cx: &mut Context<Self>) {
        if !self.active || self.page != "TAB_CUSTOMIZE" || !self.receiver_indicator_visible() {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let Ok(configuration) =
            source::with_mode(&self.receiver_indicator.configuration, index, mode)
        else {
            return;
        };
        self.receiver_indicator.configuration = configuration.clone();
        self.draft["indicatorLedConfiguration"] = configuration.clone();
        cx.emit(MouseProductChanged);
        // The original dispatches even when the selected value is unchanged.
        self.enqueue_receiver_indicator(MouseReceiverAction::Set { configuration }, scope, cx);
        cx.notify();
    }
    pub fn receiver_indicator_request_matches(
        &self,
        request: &MouseReceiverIndicatorRequested,
        cx: &App,
    ) -> bool {
        if matches!(request.action, MouseReceiverAction::LegacyBindingInfo) {
            return request.is_valid()
                && self
                    .mouse_polling_scope(cx)
                    .is_some_and(|scope| scope.same_device_connection(request.scope))
                && self
                    .receiver_indicator
                    .legacy_binding_requests
                    .iter()
                    .any(|pending| pending.sequence == request.sequence);
        }
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && (self
                .receiver_indicator
                .pending
                .as_ref()
                .is_some_and(|pending| pending.sequence == request.sequence)
                || matches!(
                    request.action,
                    MouseReceiverAction::LegacyInitialize
                        | MouseReceiverAction::LegacySet { .. }
                        | MouseReceiverAction::LegacyBindingInfo
                ) && self
                    .receiver_indicator
                    .queued
                    .iter()
                    .any(|queued| queued.sequence == request.sequence))
    }
    pub fn finish_receiver_indicator(
        &mut self,
        request: &MouseReceiverIndicatorRequested,
        report: Option<Report>,
        cx: &mut Context<Self>,
    ) {
        if matches!(request.action, MouseReceiverAction::LegacyBindingInfo) {
            if let Some(index) = self
                .receiver_indicator
                .legacy_binding_requests
                .iter()
                .position(|pending| pending.sequence == request.sequence)
            {
                self.receiver_indicator
                    .legacy_binding_requests
                    .remove(index)
                    .cancel();
            }
            cx.notify();
            return;
        }
        if matches!(
            request.action,
            MouseReceiverAction::LegacyInitialize
                | MouseReceiverAction::LegacySet { .. }
                | MouseReceiverAction::LegacyBindingInfo
        ) {
            if self
                .receiver_indicator
                .pending
                .as_ref()
                .is_some_and(|pending| pending.sequence == request.sequence)
            {
                self.receiver_indicator.pending.take().unwrap().cancel();
                self.receiver_indicator.pending = self.receiver_indicator.queued.pop_front();
            } else if let Some(index) = self
                .receiver_indicator
                .queued
                .iter()
                .position(|queued| queued.sequence == request.sequence)
            {
                self.receiver_indicator
                    .queued
                    .remove(index)
                    .unwrap()
                    .cancel();
            }
            cx.notify();
            return;
        }
        if !self
            .receiver_indicator
            .pending
            .as_ref()
            .is_some_and(|pending| pending.sequence == request.sequence)
        {
            return;
        }
        if self.receiver_indicator_request_matches(request, cx) {
            // Source SET/GET acknowledgement is diagnostic only. Quality GET
            // returns an object rejected by the original string-only feature.
            self.receiver_indicator.diagnostic = report;
        }
        self.receiver_indicator.pending.take().unwrap().cancel();
        while let Some(next) = self.receiver_indicator.queued.pop_front() {
            if next.is_valid() && self.mouse_polling_scope(cx) == Some(next.scope) {
                self.receiver_indicator.pending = Some(next.clone());
                cx.emit(next);
                break;
            }
            next.cancel();
        }
        cx.notify();
    }
    pub fn receiver_monitor_matches(
        &self,
        monitor: &MouseReceiverMonitorRequested,
        cx: &App,
    ) -> bool {
        monitor.is_valid()
            && self
                .mouse_polling_scope(cx)
                .is_some_and(|scope| monitor.matches_scope(scope))
            && self
                .receiver_indicator
                .monitor
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(&current.valid, &monitor.valid))
    }
    pub fn observe_receiver_quality(
        &mut self,
        monitor: &MouseReceiverMonitorRequested,
        quality: Quality,
        cx: &mut Context<Self>,
    ) {
        if self.receiver_monitor_matches(monitor, cx) {
            self.receiver_indicator.quality = quality;
            cx.notify();
        }
    }
    pub fn observe_receiver_initialization(
        &mut self,
        request: &MouseReceiverIndicatorRequested,
        configuration: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.receiver_indicator_request_matches(request, cx)
            && matches!(request.action, MouseReceiverAction::LegacyInitialize)
        {
            self.publish_legacy_receiver_status(configuration, cx);
            return;
        }
        if self.receiver_indicator_request_matches(request, cx)
            && matches!(request.action, MouseReceiverAction::Initialize)
        {
            self.receiver_indicator.configuration = configuration.clone();
            self.draft["indicatorLedConfiguration"] = configuration;
            self.sync_receiver_selectors(window, cx);
            cx.notify();
        }
    }
    pub fn observe_receiver_battery(
        &mut self,
        monitor: &MouseReceiverMonitorRequested,
        battery: f64,
        cx: &mut Context<Self>,
    ) {
        if self.receiver_monitor_matches(monitor, cx) && battery.is_finite() {
            self.receiver_indicator.battery = battery;
            cx.notify();
        }
    }
    pub fn restart_receiver_initialization(
        &mut self,
        monitor: &MouseReceiverMonitorRequested,
        cx: &mut Context<Self>,
    ) {
        if self.receiver_monitor_matches(monitor, cx) {
            self.receiver_indicator.initialized = None;
            self.sync_receiver_indicator(cx);
        }
    }
    pub(super) fn dismiss_receiver_guide(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if self.receiver_indicator.guide {
            self.receiver_indicator.guide = false;
            self.receiver_indicator.preview = "none";
        }
    }
    pub(super) fn receiver_indicator_panel(&self, cx: &Context<Self>) -> AnyElement {
        if legacy::source(self.spec.product_id).is_some() {
            return self.legacy_receiver_panel(cx);
        }
        let Some(spec) = source(self.spec.product_id).filter(|_| self.receiver_indicator_visible())
        else {
            return div().into_any_element();
        };
        let quality = self.receiver_indicator.quality;
        let battery = self.receiver_indicator.battery;
        // Ya selects pollingRate.pollingRateWireless. A Rust readback receipt
        // is not a source reducer action and must not override that UI draft.
        let rate = self.draft["pollingRateWireless"].as_u64().unwrap_or(1000) as u32;
        let stages = &self.draft["dpiStages"]["stages"];
        let active = self.draft["dpiStages"]["active"].as_u64().unwrap_or(1) as usize;
        let dpi = source::active_dpi_color(stages.as_array().map_or(&[], Vec::as_slice), active);
        let colors = source::colors(
            &self.receiver_indicator.configuration,
            quality,
            battery,
            rate,
            dpi,
        );
        surface::panel_with_help(
            t("INDICATOR_LED_V2"),
            self.receiver_indicator.help.clone(),
            cx,
        )
        .id("mouse-receiver-indicator-panel")
        .child(
            v_flex()
                .items_center()
                .gap(surface::css(20.))
                .child(
                    h_flex()
                        .items_center()
                        .child(t("INDICATOR_LED_V2_DESC"))
                        .child(
                            BaseButton::new("mouse-receiver-show-guide")
                                .ml(surface::css(5.))
                                .p_0()
                                .h(surface::css(20.))
                                .min_w(surface::css(99.))
                                .gap(surface::css(4.))
                                .text_color(Colors::accent())
                                .focusable(false)
                                .cursor_pointer()
                                .child(div().underline().child(t(&spec.show_guide)))
                                .child(img(spec.guide_icon.clone()).size(surface::css(20.)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.receiver_indicator.guide = true;
                                    this.receiver_indicator.guide_tab = 0;
                                    this.receiver_indicator.preview = "none";
                                    cx.notify();
                                })),
                        ),
                )
                .child(receiver_image(&spec.image, colors))
                .child(
                    h_flex().gap(surface::css(20.)).children(
                        self.receiver_indicator.selectors.iter().enumerate().map(
                            |(index, select)| {
                                v_flex()
                                    .gap(surface::css(10.))
                                    .child(t(["LED_1", "LED_2", "LED_3"][index]))
                                    .when_some(
                                        source::selector_label(
                                            &self.receiver_indicator.configuration,
                                            index,
                                        ),
                                        |row, label| {
                                            row.child(
                                                razer_widgets::synapse_select::select_pointer(
                                                    select,
                                                )
                                                .items(choices())
                                                .selected_text(t(label))
                                                .accessibility_label(t(
                                                    ["LED_1", "LED_2", "LED_3"][index]
                                                ))
                                                .w(surface::css(160.)),
                                            )
                                        },
                                    )
                            },
                        ),
                    ),
                ),
        )
        .into_any_element()
    }
    pub(super) fn receiver_guide(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.receiver_indicator.guide
            || self.page != "TAB_CUSTOMIZE"
            || !self.receiver_indicator_visible()
        {
            return None;
        }
        let spec = source(self.spec.product_id)?;
        let tab = self.receiver_indicator.guide_tab;
        let labels: Vec<String> = match tab {
            0 => ["POOR", "FAIR", "GOOD", "EXCELLENT"].map(t).into(),
            1 => ["0%", "25%", "50%", "75%", "100%"].map(String::from).into(),
            2 => ["125Hz", "500Hz", "1000Hz", "2000Hz", "4000Hz", "8000Hz"]
                .map(String::from)
                .into(),
            _ => ["STAGE1", "STAGE2", "STAGE3", "STAGE4", "STAGE5"]
                .map(t)
                .into(),
        };
        let colors = source::preview_colors(tab, self.receiver_indicator.preview);
        let card = v_flex()
            .id("mouse-receiver-led-guide")
            .w(surface::css(500.))
            .h(surface::css(428.))
            .border_1()
            .border_color(Colors::accent())
            .rounded(surface::css(3.))
            .bg(Colors::surface())
            .pt(surface::css(30.))
            .px(surface::css(40.))
            .pb(surface::css(20.))
            .gap(surface::css(20.))
            .child(
                div()
                    .text_size(surface::css(18.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Colors::accent())
                    .child(t(&spec.guide_title)),
            )
            .child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(Colors::muted())
                    .child(t(&spec.guide_description)),
            )
            .child(v_flex().gap(surface::css(10.)).children((0..2).map(|row| {
                h_flex()
                    .gap(surface::css(10.))
                    .children((row * 2..row * 2 + 2).map(|index| {
                        BaseButton::new(("mouse-receiver-guide-tab", index))
                            .flex_1()
                            .min_w_0()
                            .h(surface::css(27.))
                            .border_1()
                            .rounded(surface::css(3.))
                            .border_color(if tab == index {
                                Colors::accent()
                            } else {
                                Colors::border()
                            })
                            .bg(if tab == index {
                                Colors::selected()
                            } else {
                                Colors::transparent()
                            })
                            .text_color(Colors::text())
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .cursor_pointer()
                            .pt(surface::css(7.))
                            .pr(surface::css(14.))
                            .pb(surface::css(6.))
                            .pl(surface::css(16.))
                            .selected(tab == index)
                            .child(t(source::GUIDE_TABS[index]).to_uppercase())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.receiver_indicator.guide_tab = index;
                                // xa retains local hover state when only its tab prop changes.
                                cx.notify();
                            }))
                    }))
            })))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap(surface::css(20.))
                    .child(receiver_image(&spec.image, colors))
                    .child(self.receiver_guide_scale(tab, labels, cx)),
            )
            .child(
                h_flex().justify_center().child(
                    BaseButton::new("mouse-receiver-guide-close")
                        .h(surface::css(27.))
                        .min_w(surface::css(90.))
                        .px(surface::css(16.))
                        .rounded(surface::css(3.))
                        .bg(Colors::accent())
                        .text_color(Colors::button_text())
                        .text_size(surface::css(12.))
                        .child(t("CLOSE").to_uppercase())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.dismiss_receiver_guide(window, cx);
                            cx.notify();
                        })),
                ),
            );
        // ja is a fixed div with a click propagation boundary. It has native
        // buttons but no focus move/trap/restore, Escape or Enter-dismiss path.
        // Kit 0.7.3 Dialog unconditionally traps focus, so compose its semantic
        // parts without adding that behavior (190 main.js ja complete owner).
        let viewport = window.viewport_size();
        Some(
            razer_assets::deferred_resource(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("mouse-receiver-guide-overlay")
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(viewport.width)
                        .h(viewport.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .role(Role::Dialog)
                        .occlude()
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            DialogBackdrop::new()
                                .absolute()
                                .inset_0()
                                .bg(Colors::overlay()),
                        )
                        .child(DialogPopup::new().child(card)),
                ),
            )
            .with_priority(999)
            .into_any_element(),
        )
    }

    /// ka's original SVG uses baseline-positioned Roboto text and circular
    /// mouseenter targets. Canvas retains that geometry without turning every
    /// label into a focusable control or widening its hover area to a column.
    fn receiver_guide_scale(
        &self,
        tab: usize,
        labels: Vec<String>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let scale = source::GuideScale { tab };
        let preview = self.receiver_indicator.preview;
        let measured = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let pointer_bounds = measured.clone();
        div()
            .id("mouse-receiver-guide-scale")
            .w(surface::css(scale.width()))
            .h(surface::css(37.))
            .flex_shrink_0()
            .when(preview != "none", |element| element.cursor_pointer())
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    let unit = f32::from(window.rem_size()) / 16.;
                    let local = event.position - pointer_bounds.get().origin;
                    let key = scale.key_at(f32::from(local.x) / unit, f32::from(local.y) / unit);
                    if this.receiver_indicator.preview != key {
                        this.receiver_indicator.preview = key;
                        cx.notify();
                    }
                }),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered && this.receiver_indicator.preview != "none" {
                    this.receiver_indicator.preview = "none";
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |bounds, window, _| {
                        measured.set(bounds);
                        let unit = window.rem_size() / 16.;
                        let font_size = unit
                            * if tab == 0 && razer_i18n::locale() == "ru" {
                                13.
                            } else {
                                14.
                            };
                        let lines: Vec<_> = labels
                            .into_iter()
                            .enumerate()
                            .map(|(ix, label)| {
                                let color = if preview == source::GUIDE_KEYS[tab][ix] {
                                    Colors::text()
                                } else {
                                    Colors::border()
                                };
                                let run = TextRun {
                                    len: label.len(),
                                    font: Font {
                                        family: "Roboto".into(),
                                        weight: FontWeight::NORMAL,
                                        ..Default::default()
                                    },
                                    color,
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                };
                                window.text_system().shape_line(
                                    label.into(),
                                    font_size,
                                    &[run],
                                    None,
                                )
                            })
                            .collect();
                        (unit, lines)
                    },
                    move |bounds, (unit, lines), window, cx| {
                        let at = |x, y| bounds.origin + point(unit * x, unit * y);
                        window.paint_quad(fill(
                            Bounds::new(
                                at(scale.x(0), 4.5),
                                size(unit * (scale.x(lines.len() - 1) - scale.x(0)), unit),
                            ),
                            Colors::border(),
                        ));
                        for (ix, line) in lines.into_iter().enumerate() {
                            let color = if preview == source::GUIDE_KEYS[tab][ix] {
                                Colors::text()
                            } else {
                                Colors::border()
                            };
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        at(scale.x(ix) - 5., 0.),
                                        size(unit * 10., unit * 10.),
                                    ),
                                    color,
                                )
                                .corner_radii(unit * 5.),
                            );
                            let origin = at(scale.x(ix), 30.) - point(line.width / 2., line.ascent);
                            let _ = line.paint(
                                origin,
                                line.ascent + line.descent,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}

fn receiver_image(path: &str, colors: [Option<u32>; 3]) -> AnyElement {
    div()
        .relative()
        .w(surface::css(150.))
        .child(img(path.to_owned()).w(surface::css(150.)))
        .child(
            h_flex()
                .absolute()
                .left(surface::css(53.))
                .top(surface::css(58.))
                .children(colors.into_iter().map(|color| {
                    let color = Colors::led(color);
                    div()
                        .relative()
                        .size(surface::css(14.5))
                        .rounded_full()
                        .bg(color.opacity(0.3))
                        .child(
                            div()
                                .absolute()
                                .left(surface::css(2.71875))
                                .top(surface::css(2.71875))
                                .size(surface::css(9.0625))
                                .rounded_full()
                                .bg(color.opacity(0.5)),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(surface::css(5.4375))
                                .top(surface::css(5.4375))
                                .size(surface::css(3.625))
                                .rounded_full()
                                .bg(color.opacity(0.9)),
                        )
                })),
        )
        .into_any_element()
}

#[derive(Default)]
struct Help {
    section: usize,
    icon_hovered: bool,
    bridge_hovered: bool,
    tip_hovered: bool,
}
impl Render for Help {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::base::motion::{self, Easing, Transition};
        let open = self.icon_hovered || self.bridge_hovered || self.tip_hovered;
        let title = [
            "CONNECTION_QUALITY",
            "BATTERY_LIFE",
            "POLLING_RATE",
            "DPI_STAGES",
        ][self.section];
        let entries: Vec<(String, u32)> = match self.section {
            2 => [8000, 4000, 2000, 1000, 500, 125]
                .into_iter()
                .map(|hz| (format!("{hz} Hz"), source::polling_color(hz).unwrap()))
                .collect(),
            _ => ["STAGE1", "STAGE2", "STAGE3", "STAGE4", "STAGE5"]
                .into_iter()
                .enumerate()
                .map(|(index, key)| (t(key), source::dpi_color(index + 1).unwrap()))
                .collect(),
        };
        let content = if self.section < 2 {
            let labels = if self.section == 0 {
                ["EXCELLENT", "GOOD", "FAIR", "POOR"]
            } else {
                ["100%", "LESS_THAN_75", "LESS_THAN_50", "LESS_THAN_25"]
            };
            h_flex()
                .items_stretch()
                .child(
                    div()
                        .w(surface::css(4.))
                        .h(surface::css(85.))
                        .mx(surface::css(10.))
                        .rounded(surface::css(5.))
                        .overflow_hidden()
                        .child(img(help_gradient()).size_full()),
                )
                .child(
                    v_flex()
                        .justify_between()
                        .children(labels.map(|key| div().child(t(key)))),
                )
                .into_any_element()
        } else {
            v_flex()
                .gap(surface::css(6.))
                .children(entries.into_iter().map(|(label, color)| {
                    h_flex()
                        .items_center()
                        .child(
                            div()
                                .size(surface::css(3.))
                                .mx(surface::css(8.))
                                .rounded_full()
                                .bg(Colors::led(Some(color))),
                        )
                        .child(div().text_color(Colors::led(Some(color))).child(label))
                }))
                .into_any_element()
        };
        let tip = v_flex()
            .id("mouse-receiver-help-popup")
            .min_w(surface::css(180.))
            .max_w(surface::css(300.))
            .py(surface::css(8.))
            .px(surface::css(10.))
            .bg(Colors::tip_surface())
            .border_1()
            .border_color(Colors::tip_border())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(18.))
            .text_color(Colors::text())
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.tip_hovered = *hovered;
                cx.notify();
            }))
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(surface::css(16.))
                            .p(surface::css(8.))
                            .children((0..4).map(|index| {
                                div().size(surface::css(8.)).rounded_full().bg(
                                    if index == self.section {
                                        Colors::accent()
                                    } else {
                                        Colors::inactive_dot()
                                    },
                                )
                            })),
                    )
                    .child(
                        BaseButton::new("mouse-receiver-help-next")
                            .focusable(false)
                            .p_0()
                            .size(surface::css(20.))
                            .accessibility_label("Next")
                            .child(img(help_next_icon()).size_full())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = (this.section + 1) % 4;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().mb(surface::css(10.)).child(t(title).to_uppercase()))
            .child(content);
        let background = motion::transition(
            "mouse-receiver-help-background",
            if open {
                Colors::help_hover()
            } else {
                Colors::help_surface()
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let opacity = motion::transition(
            "mouse-receiver-help-opacity",
            if open { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        // The current source is a CSS hover subtree, not the generic delayed
        // HoverCard: .help is 14x14, .tip left:36%/top:19px and :before is
        // left:5px/bottom:-5px/50x10. Keep its bridge and content interactive.
        div()
            .id("mouse-receiver-help")
            .relative()
            .size(surface::css(14.))
            .rounded_full()
            .bg(background)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.icon_hovered = *hovered;
                cx.notify();
            }))
            .child(img("synapse/automation-tooltip_questionmark.svg").size_full())
            .child(
                div()
                    .id("mouse-receiver-help-bridge")
                    .absolute()
                    .left(surface::css(5.))
                    .top(surface::css(9.))
                    .w(surface::css(50.))
                    .h(surface::css(10.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.bridge_hovered = *hovered;
                        cx.notify();
                    })),
            )
            .when(open, |element| {
                element.child(
                    razer_assets::deferred_resource(
                        div()
                            .absolute()
                            .left(relative(0.36))
                            .top(surface::css(19.))
                            .opacity(opacity)
                            .child(tip),
                    )
                    .with_priority(100),
                )
            })
    }
}

/// Va's complete SVG path, copied from the verified current original owner.
fn help_next_icon() -> Arc<Image> {
    static IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    IMAGE.get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Svg,
        br##"<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg"><path fill-rule="evenodd" clip-rule="evenodd" d="M6 17.2889L6.62222 18L14 10.3556V9.64444L6.62222 2L6 2.62222L13.0222 10L6 17.2889Z" fill="#CCCCCC"/></svg>"##.to_vec(),
    ))).clone()
}

/// Exact source gradient stops; a bitmap-independent SVG preserves all three
/// stops, while the locked GPUI background supports only two stops.
fn help_gradient() -> Arc<Image> {
    static IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    IMAGE.get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Svg,
        br##"<svg width="4" height="85" viewBox="0 0 4 85" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1" gradientUnits="objectBoundingBox" color-interpolation="sRGB"><stop offset="0" stop-color="#00ff00"/><stop offset=".325" stop-color="#e3b617"/><stop offset=".985" stop-color="#ff3d00"/></linearGradient></defs><rect width="4" height="85" fill="url(#g)"/></svg>"##.to_vec(),
    ))).clone()
}

// Generated catalog reader: razer-pages/mouse_receiver_indicator_data.
fn read_mouse_receiver_indicator_catalog() -> Vec<Source> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Source>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                product_id: r.child(decode_2)?,
                image: r.child(decode_3)?,
                guide_icon: r.child(decode_3)?,
                show_guide: r.child(decode_3)?,
                guide_title: r.child(decode_3)?,
                guide_description: r.child(decode_3)?,
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
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_RECEIVER_INDICATOR_DATA,
        0,
        "634acfa81b5de2f1e5ef483e95922d79d020c729e5d38cd430bea1e73e52323d",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_receiver_indicator_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_receiver_indicator_data_decodes() {
        let _ = super::read_mouse_receiver_indicator_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_receiver_indicator_data.
