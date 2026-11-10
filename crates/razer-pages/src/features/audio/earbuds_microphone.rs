//! MIC displays hardware mute with independently observed Windows input volume.
use super::*;
use razer_device::earbuds_protocol40::ToggleObservation;
use razer_device::{
    earbuds_events::Notification,
    earbuds_microphone::{Scope, Setting, Snapshot, Update},
};
use razer_widgets::theme::AudioEnhancementColors as C;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
pub enum EarbudsMicrophoneRequested {
    Start {
        scope: Scope,
        active: Arc<AtomicBool>,
    },
    Edit {
        scope_id: Uuid,
        request_id: Uuid,
        setting: Setting,
    },
    HardwareMute {
        scope_id: Uuid,
        muted: u8,
    },
}
impl EventEmitter<EarbudsMicrophoneRequested> for AudioProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    identity: String,
    profile: String,
    connection: u64,
    container: String,
    context_active: bool,
    did_start: bool,
    scope: Option<Scope>,
    active: Option<Arc<AtomicBool>>,
    observed: Option<Snapshot>,
    draft: Option<Setting>,
    preview: Option<u8>,
    pending: Vec<Uuid>,
    loading: bool,
    error: Option<String>,
    focus: Option<FocusHandle>,
    slider: Option<Entity<SliderState>>,
    subscription: Option<Subscription>,
    cursor: Option<Point<Pixels>>,
    over_help: bool,
}
impl State {
    fn cancel(&mut self) {
        if let Some(active) = self.active.take() {
            active.store(false, Ordering::Release);
        }
        self.scope = None;
        self.observed = None;
        self.draft = None;
        self.preview = None;
        self.pending.clear();
        self.loading = false;
        self.error = None;
        self.subscription = None;
        self.slider = None;
        self.cursor = None;
        self.over_help = false;
    }
    fn visible(&self) -> Option<Setting> {
        self.observed
            .as_ref()?
            .setting()
            .map(|setting| self.draft.unwrap_or(setting))
    }
    fn visible_volume(&self) -> Option<u8> {
        let system = self.observed.as_ref()?.system().as_ref().ok()?;
        Some(
            self.draft
                .map(Setting::volume)
                .unwrap_or_else(|| system.setting().volume()),
        )
    }
    fn visible_enabled(&self) -> Option<bool> {
        self.draft
            .map(Setting::enabled)
            .or_else(|| match self.observed.as_ref()?.hardware_mute() {
                ToggleObservation::Observed(muted) => Some(!muted),
                _ => None,
            })
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
#[path = "earbuds_microphone_tests.rs"]
mod tests;

impl AudioProductWorkspace {
    pub(super) fn cancel_earbuds_microphone(&mut self) {
        self.earbuds_microphone.cancel();
    }
    pub(super) fn invalidate_earbuds_microphone(&mut self) {
        self.earbuds_microphone.cancel();
        self.earbuds_microphone.did_start = false;
    }
    pub(super) fn leave_earbuds_microphone_page(&mut self) {
        self.invalidate_earbuds_microphone();
        self.earbuds_microphone.context_active = false;
    }
    pub fn set_earbuds_microphone_context(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        connection: u64,
        cx: &mut Context<Self>,
    ) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(device.product_id) {
            return;
        }
        let identity = device.workspace_identity();
        if self.earbuds_microphone.identity != identity
            || self.earbuds_microphone.connection != connection
            || self.earbuds_microphone.profile != device.active_profile
            || !active
        {
            self.earbuds_microphone.cancel();
            self.earbuds_microphone.did_start = false;
        }
        self.earbuds_microphone.identity = identity;
        self.earbuds_microphone.profile = device.active_profile.clone();
        self.earbuds_microphone.connection = connection;
        self.earbuds_microphone.container = device.device_container_id.clone();
        self.earbuds_microphone.context_active = active;
        if active && !self.earbuds_microphone.did_start {
            self.resume_earbuds_microphone(cx);
        }
        cx.notify();
    }
    pub(super) fn resume_earbuds_microphone(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds_microphone.context_active || self.earbuds_microphone.scope.is_some() {
            return;
        }
        self.earbuds_microphone.did_start = true;
        let scope = match Scope::new(
            self.spec.product_id,
            self.earbuds_microphone.container.clone(),
        ) {
            Ok(scope) => scope,
            Err(error) => {
                self.earbuds_microphone.error = Some(format!("{error:#}"));
                return;
            }
        };
        if self.earbuds_microphone.focus.is_none() {
            self.earbuds_microphone.focus = Some(cx.focus_handle().tab_stop(true));
        }
        let scope_id = scope.id();
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(0.)
        });
        self.earbuds_microphone.subscription =
            Some(cx.subscribe(&slider, move |this, _, event, cx| {
                if this.earbuds_microphone_current(scope_id) {
                    this.earbuds_microphone_slider(event, cx);
                }
            }));
        self.earbuds_microphone.slider = Some(slider);
        let active = Arc::new(AtomicBool::new(true));
        self.earbuds_microphone.active = Some(active.clone());
        self.earbuds_microphone.scope = Some(scope.clone());
        self.earbuds_microphone.loading = true;
        cx.emit(EarbudsMicrophoneRequested::Start { scope, active });
    }
    pub fn earbuds_microphone_current(&self, scope_id: Uuid) -> bool {
        self.earbuds_microphone
            .scope
            .as_ref()
            .is_some_and(|scope| scope.id() == scope_id)
            && self
                .earbuds_microphone
                .active
                .as_ref()
                .is_some_and(|active| active.load(Ordering::Acquire))
    }
    fn edit_earbuds_microphone(
        &mut self,
        enabled: Option<bool>,
        volume: Option<u8>,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds_connection.ready() {
            return;
        }
        let Some(scope) = &self.earbuds_microphone.scope else {
            return;
        };
        if !self.earbuds_microphone_current(scope.id()) {
            return;
        }
        let Some(previous) = self.earbuds_microphone.visible() else {
            return;
        };
        if volume.is_some() && !previous.enabled() {
            return;
        }
        let Ok(setting) = Setting::new(
            enabled.unwrap_or(previous.enabled()),
            volume.unwrap_or(previous.volume()),
        ) else {
            return;
        };
        let request_id = Uuid::new_v4();
        self.earbuds_microphone.draft = Some(setting);
        self.earbuds_microphone.preview = None;
        self.earbuds_microphone.error = None;
        self.earbuds_microphone.pending.push(request_id);
        cx.emit(EarbudsMicrophoneRequested::Edit {
            scope_id: scope.id(),
            request_id,
            setting,
        });
        cx.notify();
    }
    fn earbuds_microphone_slider(&mut self, event: &SliderEvent, cx: &mut Context<Self>) {
        if self.syncing
            || !self.earbuds_connection.ready()
            || !self
                .earbuds_microphone
                .visible()
                .is_some_and(Setting::enabled)
        {
            return;
        }
        match event {
            SliderEvent::Change(value) => {
                self.earbuds_microphone.preview = Some(value.start().clamp(0., 100.).round() as u8);
                cx.notify();
            }
            SliderEvent::Release(value) => self.edit_earbuds_microphone(
                None,
                Some(value.start().clamp(0., 100.).round() as u8),
                cx,
            ),
        }
    }
    pub fn finish_earbuds_microphone(
        &mut self,
        scope_id: Uuid,
        update: Update,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds_microphone_current(scope_id) {
            return;
        }
        if let Some(id) = update.request_id() {
            if !self.earbuds_microphone.pending.contains(&id) {
                return;
            }
            self.earbuds_microphone
                .pending
                .retain(|pending| *pending != id);
        }
        self.earbuds_microphone.loading = false;
        self.earbuds_microphone.error = update.error().map(str::to_owned);
        match update.observation().clone() {
            Ok(snapshot) => {
                let previous = self
                    .earbuds_microphone
                    .observed
                    .as_ref()
                    .and_then(Snapshot::setting);
                let setting = snapshot.setting();
                if self.earbuds_microphone.pending.is_empty() || update.request_id().is_none() {
                    self.earbuds_microphone.draft = None;
                }
                if setting != previous || update.request_id().is_some() {
                    self.earbuds_microphone.preview = None;
                }
                if self.earbuds_microphone.error.is_none() {
                    self.earbuds_microphone.error = snapshot.diagnostic();
                }
                self.earbuds_microphone.observed = Some(snapshot);
                if let (Some(volume), Some(slider)) = (
                    self.earbuds_microphone.visible_volume(),
                    &self.earbuds_microphone.slider,
                ) {
                    self.syncing = true;
                    slider.update(cx, |slider, cx| {
                        slider.set_value(f32::from(volume), window, cx)
                    });
                    self.syncing = false;
                }
            }
            Err(error) => {
                self.earbuds_microphone.observed = None;
                self.earbuds_microphone.draft = None;
                self.earbuds_microphone.preview = None;
                self.earbuds_microphone.error = Some(error);
            }
        }
        cx.notify();
    }
    pub fn fail_earbuds_microphone(
        &mut self,
        scope_id: Uuid,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if self.earbuds_microphone_current(scope_id) {
            self.finish_earbuds_microphone_monitor(error, cx);
        }
    }
    pub fn finish_earbuds_microphone_monitor(&mut self, error: String, cx: &mut Context<Self>) {
        self.earbuds_microphone.cancel();
        self.earbuds_microphone.error = Some(error);
        cx.notify();
    }
    pub fn observe_earbuds_microphone_notification(
        &mut self,
        event: &Notification,
        cx: &mut Context<Self>,
    ) {
        if let Notification::MicrophoneMute { muted } = event {
            if let Some(scope) = &self.earbuds_microphone.scope {
                if self.earbuds_microphone_current(scope.id()) {
                    cx.emit(EarbudsMicrophoneRequested::HardwareMute {
                        scope_id: scope.id(),
                        muted: *muted,
                    });
                }
            }
        }
    }
    pub(super) fn earbuds_microphone_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.earbuds_microphone;
        let setting = state.visible();
        let hardware_enabled = state.visible_enabled();
        let enabled = hardware_enabled == Some(true);
        let ready = self.earbuds_connection.ready() && setting.is_some();
        let value = state
            .visible_volume()
            .map(|volume| state.preview.unwrap_or(volume));
        let mut range = div().id("earbuds-microphone-range").test_support();
        if let (Some(slider), Some(value)) = (&state.slider, value) {
            range = range.child(
                razer_widgets::source_slider::SourceSlider::new(slider, f32::from(value) / 100.)
                    .tip(Some(value.to_string()))
                    .range_labels("0", "100", C::text())
                    .enabled(ready && enabled),
            );
        } else {
            range = range.child(
                div()
                    .id("earbuds-microphone-volume-unobserved")
                    .test_support()
                    .h(surface::css(64.))
                    .flex()
                    .items_center()
                    .child("—"),
            );
        }
        if let Some(focus) = &state.focus {
            let focus = focus.clone();
            range = range
                .track_focus(&focus)
                .capture_any_mouse_down(move |event, window, cx| {
                    if ready && enabled && event.button == MouseButton::Left {
                        focus.focus(window, cx);
                    }
                })
                .on_key_down(|event, _, cx| {
                    if matches!(
                        event.keystroke.key.as_str(),
                        "home" | "end" | "left" | "right" | "up" | "down"
                    ) {
                        cx.stop_propagation();
                    }
                });
        }
        let switch = match hardware_enabled {
            Some(enabled) => surface::SynapseSwitch::new("earbuds-microphone-switch")
                .checked(enabled)
                .disabled(!ready)
                .accessibility_label(t("MICROPHONE_HEADER"))
                .on_change(cx.listener(|this, enabled, _, cx| {
                    this.edit_earbuds_microphone(Some(*enabled), None, cx)
                }))
                .into_any_element(),
            None => div()
                .id("earbuds-microphone-switch-unobserved")
                .test_support()
                .w(surface::css(30.))
                .text_color(C::secondary())
                .child("—")
                .into_any_element(),
        };
        let mut panel = surface::panel_with_title_switch(
            t("MICROPHONE_HEADER"),
            switch,
            div()
                .id("earbuds-microphone-help-owner")
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .size(surface::css(14.))
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.earbuds_microphone.over_help = *hovered;
                    if *hovered {
                        this.earbuds_microphone.cursor = None;
                    }
                    cx.notify();
                }))
                .child(surface::help_control(
                    "earbuds-microphone-help",
                    t("MICROPHONE_TOOLTIP"),
                )),
            cx,
        )
        .id("earbuds-microphone-panel")
        .test_support()
        .relative()
        .when(self.earbuds_connection.disconnected(), |panel| {
            panel.opacity(0.5)
        })
        .child(range)
        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
            if !hovered {
                this.earbuds_microphone.cursor = None;
                cx.notify();
            }
        }))
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
            if this.spec.product_id == 2664 && !this.earbuds_microphone.over_help {
                this.earbuds_microphone.cursor = Some(event.position);
                cx.notify();
            }
        }));
        if self.spec.product_id != 2664
            && hardware_enabled == Some(false)
            && self.earbuds_connection.ready()
        {
            panel = panel.child(
                h_flex()
                    .id("earbuds-microphone-muted-description")
                    .test_support()
                    .items_start()
                    .gap(surface::css(5.))
                    .mt(surface::css(20.))
                    .text_color(C::secondary())
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(
                        razer_assets::resource_svg()
                            .path("synapse/dock-241-icon_info_solid.svg")
                            .size(surface::css(16.))
                            .mt(surface::css(2.))
                            .flex_shrink_0(),
                    )
                    .child(t("MUTED_MIC_TIP_DESCRIPTION")),
            );
        }
        if self.spec.product_id == 2664
            && hardware_enabled == Some(false)
            && self.earbuds_connection.ready()
        {
            if let Some(cursor) = state.cursor {
                panel = panel.child(
                    razer_assets::deferred_resource(
                        super::earbuds_equalizer_view::CursorTip::at_source_corner(
                            cursor,
                            div()
                                .id("earbuds-microphone-muted-tip")
                                .test_support()
                                .max_w(surface::css(300.))
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .border_1()
                                .border_color(C::border())
                                .bg(C::mapping_black())
                                .text_color(C::text())
                                .line_height(surface::css(16.))
                                .child(t("MUTED_MIC_TIP")),
                        ),
                    )
                    .with_priority(100),
                );
            }
        }
        if let Some(error) = state.error.as_deref().or(self.earbuds_connection.error()) {
            panel = panel.child(
                div()
                    .mt(surface::css(10.))
                    .text_color(cx.theme().danger)
                    .child(error.to_owned()),
            );
        } else if state.loading {
            panel = panel.child(div().mt(surface::css(10.)).child(t("LOADING")));
        }
        surface::page_columns()
            .child(surface::page_column(self.earbuds_disconnected_surface(
                super::earbuds_connection::Card::Microphone,
                panel.into_any_element(),
                cx,
            )))
            .into_any_element()
    }
}
