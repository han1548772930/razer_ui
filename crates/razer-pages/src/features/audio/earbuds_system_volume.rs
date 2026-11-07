//! Source cp/Go/qo Sound card, with a separate observed endpoint and user draft.
use super::*;
use razer_device::earbuds_system_volume::{Edit, Scope, Setting, Snapshot, Update, supports};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
pub enum EarbudsVolumeRequested {
    Start {
        scope: Scope,
        active: Arc<AtomicBool>,
    },
    Edit {
        scope_id: Uuid,
        request_id: Uuid,
        setting: Setting,
    },
}
impl EventEmitter<EarbudsVolumeRequested> for AudioProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    identity: String,
    profile: String,
    connection: u64,
    scope: Option<Scope>,
    active: Option<Arc<AtomicBool>>,
    observed: Option<Snapshot>,
    draft: Option<Setting>,
    preview: Option<u8>,
    pending: Vec<Uuid>,
    error: Option<String>,
    loading: bool,
    focus: Option<FocusHandle>,
    slider: Option<Entity<SliderState>>,
    slider_subscription: Option<Subscription>,
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
        self.error = None;
        self.loading = false;
        self.slider_subscription = None;
        self.slider = None;
    }
    fn visible(&self) -> Option<Setting> {
        self.observed
            .as_ref()
            .map(|snapshot| self.draft.unwrap_or_else(|| snapshot.setting()))
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl AudioProductWorkspace {
    pub(super) fn cancel_earbuds_volume(&mut self) {
        self.earbuds_volume.cancel();
    }
    pub fn set_earbuds_volume_context(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        connection: u64,
        cx: &mut Context<Self>,
    ) {
        if !supports(self.spec.product_id) {
            return;
        }
        let identity = device.workspace_identity();
        let changed = self.earbuds_volume.identity != identity
            || self.earbuds_volume.profile != device.active_profile
            || self.earbuds_volume.connection != connection
            || self
                .earbuds_volume
                .scope
                .as_ref()
                .is_some_and(|scope| scope.container_id() != device.device_container_id);
        if changed || !active {
            self.earbuds_volume.cancel();
        }
        self.earbuds_volume.identity = identity;
        self.earbuds_volume.profile = device.active_profile.clone();
        self.earbuds_volume.connection = connection;
        if active && self.earbuds_volume.scope.is_none() {
            if self.earbuds_volume.focus.is_none() {
                self.earbuds_volume.focus = Some(cx.focus_handle().tab_stop(true));
            }
            match Scope::new(device.product_id, device.device_container_id.clone()) {
                Ok(scope) => {
                    let scope_id = scope.id();
                    let slider = cx.new(|_| {
                        SliderState::new()
                            .min(0.)
                            .max(100.)
                            .step(1.)
                            .default_value(0.)
                    });
                    self.earbuds_volume.slider_subscription =
                        Some(cx.subscribe(&slider, move |this, _, event, cx| {
                            if this.earbuds_volume_current(scope_id) {
                                this.earbuds_volume_slider(event, cx);
                            }
                        }));
                    self.earbuds_volume.slider = Some(slider);
                    let active = Arc::new(AtomicBool::new(true));
                    self.earbuds_volume.scope = Some(scope.clone());
                    self.earbuds_volume.active = Some(active.clone());
                    self.earbuds_volume.loading = true;
                    self.earbuds_volume.error = None;
                    cx.emit(EarbudsVolumeRequested::Start { scope, active });
                }
                Err(error) => self.earbuds_volume.error = Some(format!("{error:#}")),
            }
        }
        cx.notify();
    }
    pub fn earbuds_volume_current(&self, scope_id: Uuid) -> bool {
        self.earbuds_volume
            .scope
            .as_ref()
            .is_some_and(|scope| scope.id() == scope_id)
            && self
                .earbuds_volume
                .active
                .as_ref()
                .is_some_and(|active| active.load(Ordering::Acquire))
    }
    fn edit_earbuds_volume(&mut self, edit: Edit, cx: &mut Context<Self>) {
        let Some(scope) = &self.earbuds_volume.scope else {
            return;
        };
        if !self.earbuds_volume_current(scope.id()) || edit.validate().is_err() {
            return;
        }
        let Some(previous) = self.earbuds_volume.visible() else {
            return;
        };
        let request_id = Uuid::new_v4();
        let Ok(setting) = previous.edit(edit) else {
            return;
        };
        self.earbuds_volume.draft = Some(setting);
        self.earbuds_volume.preview = None;
        self.earbuds_volume.pending.push(request_id);
        self.earbuds_volume.error = None;
        // SET_VOLUME is Storage_NoSave: no AudioProductChanged, snapshot merge,
        // profile save, or persistence event is emitted by this card.
        cx.emit(EarbudsVolumeRequested::Edit {
            scope_id: scope.id(),
            request_id,
            setting,
        });
        cx.notify();
    }
    fn earbuds_volume_slider(&mut self, event: &SliderEvent, cx: &mut Context<Self>) {
        if self.syncing || !self.earbuds_volume.visible().is_some_and(Setting::enabled) {
            return;
        }
        match event {
            SliderEvent::Change(value) => {
                self.earbuds_volume.preview = Some(value.start().clamp(0., 100.).round() as u8);
                cx.notify();
            }
            SliderEvent::Release(value) => self.edit_earbuds_volume(
                Edit::Volume(value.start().clamp(0., 100.).round() as u8),
                cx,
            ),
        }
    }
    pub fn finish_earbuds_volume(
        &mut self,
        scope_id: Uuid,
        update: Update,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds_volume_current(scope_id) {
            return;
        }
        let latest = update
            .request_id()
            .is_some_and(|id| self.earbuds_volume.pending.last() == Some(&id));
        if let Some(id) = update.request_id() {
            self.earbuds_volume.pending.retain(|current| *current != id);
        }
        self.earbuds_volume.loading = false;
        self.earbuds_volume.error = update.error().map(str::to_owned);
        match update.observation().clone() {
            Ok(snapshot) => {
                let previous = self.earbuds_volume.observed.as_ref().map(Snapshot::setting);
                let observed = snapshot.setting();
                // cp.componentDidUpdate synchronizes each field only when its
                // published prop changes. An unchanged failed readback must
                // not silently replace the optimistic edit with another value.
                self.earbuds_volume.draft = match (self.earbuds_volume.draft, previous) {
                    (Some(draft), Some(previous)) => {
                        let setting = Setting::new(
                            if previous.enabled() != observed.enabled() {
                                observed.enabled()
                            } else {
                                draft.enabled()
                            },
                            if previous.volume() != observed.volume() {
                                observed.volume()
                            } else {
                                draft.volume()
                            },
                        )
                        .expect("Observed and draft settings are validated");
                        (setting != observed).then_some(setting)
                    }
                    _ => None,
                };
                if latest || previous != Some(observed) {
                    self.earbuds_volume.preview = None;
                }
                self.earbuds_volume.observed = Some(snapshot);
                let visible = self
                    .earbuds_volume
                    .visible()
                    .expect("received an observed setting");
                if let Some(slider) = &self.earbuds_volume.slider {
                    self.syncing = true;
                    slider.update(cx, |slider, cx| {
                        slider.set_value(f32::from(visible.volume()), window, cx)
                    });
                    self.syncing = false;
                }
            }
            Err(error) => {
                self.earbuds_volume.observed = None;
                self.earbuds_volume.preview = None;
                self.earbuds_volume.error = Some(error);
            }
        }
        cx.notify();
    }
    pub fn fail_earbuds_volume(&mut self, scope_id: Uuid, error: String, cx: &mut Context<Self>) {
        if !self.earbuds_volume_current(scope_id) {
            return;
        }
        self.earbuds_volume.observed = None;
        self.earbuds_volume.preview = None;
        self.earbuds_volume.pending.clear();
        self.earbuds_volume.loading = false;
        self.earbuds_volume.error = Some(error);
        cx.notify();
    }
    pub(super) fn earbuds_volume_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.earbuds_volume;
        let setting = state.visible();
        let ready = state.observed.is_some();
        let enabled = setting.is_some_and(Setting::enabled);
        let value = setting.map(|setting| state.preview.unwrap_or_else(|| setting.volume()));
        let mut range = div().id("earbuds-volume-range");
        if let Some(slider) = &state.slider {
            range = range.child(
                razer_widgets::source_slider::SourceSlider::new(
                    slider,
                    f32::from(value.unwrap_or(0)) / 100.,
                )
                .tip(Some(
                    value.map_or_else(|| "—".into(), |value| value.to_string()),
                ))
                .range_labels("0", "100", cx.theme().foreground)
                .enabled(enabled && ready),
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
        let mut panel = surface::panel_with_title_switch(
            t("VOLUME_HEADER"),
            surface::SynapseSwitch::new("earbuds-volume-switch")
                .checked(enabled)
                .disabled(!ready)
                .accessibility_label(t("VOLUME_HEADER"))
                .on_change(cx.listener(|this, enabled, _, cx| {
                    this.edit_earbuds_volume(Edit::Enabled(*enabled), cx)
                })),
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(surface::help_control(
                    "earbuds-volume-help",
                    t("VOLUME_NOMMO_TOOLTIP"),
                )),
            cx,
        )
        .relative()
        .child(range)
        .child(
            razer_widgets::keyboard_navigation::Button::new("earbuds-open-volume-mixer")
                .group("earbuds-open-volume-mixer")
                .accessibility_label(t("OPEN_WINDOW_VOLUME_MIXER"))
                .h(surface::css(44.))
                .line_height(surface::css(44.))
                .flex()
                .items_center()
                .self_start()
                .text_color(cx.theme().foreground)
                .text_size(surface::css(14.))
                .underline()
                .hover(|style| style.text_color(cx.theme().primary))
                .focus_visible(|style| style.text_color(cx.theme().primary))
                .active(|style| style.opacity(0.7))
                .child(t("OPEN_WINDOW_VOLUME_MIXER"))
                .child(
                    div()
                        .relative()
                        .size(surface::css(20.))
                        .ml(surface::css(4.))
                        .mb(surface::css(2.))
                        .child(img("synapse/earbuds-volume-external.svg").size_full())
                        .child(
                            img("synapse/earbuds-volume-external-active.svg")
                                .absolute()
                                .inset_0()
                                .size_full()
                                .opacity(0.)
                                .group_hover("earbuds-open-volume-mixer", |style| {
                                    style.opacity(1.)
                                }),
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(owner) = &this.source_owner {
                        owner.update(cx, |owner, cx| owner.request_sound_windows(false, cx));
                    }
                })),
        );
        // Observation diagnostics remain separate from the source setting and
        // draft. No default or saved profile value stands in for a failed read.
        if let Some(error) = &state.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        } else if state.loading {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t("LOADING")),
            );
        }
        panel.into_any_element()
    }
}

#[cfg(test)]
#[path = "earbuds_system_volume_tests.rs"]
mod tests;
