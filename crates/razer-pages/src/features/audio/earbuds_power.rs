//! Power owns a device-global draft, saved preference and actual HID observation.
use super::*;
use razer_device::earbuds_system_volume::Scope;
use razer_device::{
    earbuds_auto_pause::{Operation, Report},
    earbuds_power::{MINUTE_CHOICES, Setting},
};
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as C};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct EarbudsPowerRequested {
    id: Uuid,
    scope: Scope,
    operation: Operation,
    active: Arc<AtomicBool>,
    profile: Option<String>,
}
impl EarbudsPowerRequested {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }
    pub fn setting(&self) -> Option<Setting> {
        match self.operation {
            Operation::SetPowerSaving { setting } => Some(setting),
            _ => None,
        }
    }
    pub fn needs_persistence(&self) -> bool {
        self.setting().is_some() && self.profile.is_none()
    }
}
impl EventEmitter<EarbudsPowerRequested> for AudioProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    identity: String,
    connection: u64,
    container: String,
    context_active: bool,
    did_start: bool,
    scope: Option<Scope>,
    active: Option<Arc<AtomicBool>>,
    persisted: Option<Setting>,
    draft: Option<Setting>,
    report: Option<razer_device::earbuds_power::Report>,
    pending: Vec<EarbudsPowerRequested>,
    profile_source: Option<(String, Setting)>,
    restored_profile: Option<(String, Setting)>,
    restore_error: Option<String>,
    loading: bool,
    error: Option<String>,
    storage_error: Option<String>,
}
impl State {
    pub(super) fn cancel(&mut self) {
        if let Some(active) = self.active.take() {
            active.store(false, Ordering::Release);
        }
        self.scope = None;
        self.draft = None;
        self.report = None;
        for request in &self.pending {
            request.active.store(false, Ordering::Release);
        }
        self.pending.clear();
        self.restored_profile = None;
        self.loading = false;
        self.error = None;
    }
    fn visible(&self) -> Option<Setting> {
        self.report
            .as_ref()?
            .observation()
            .setting()
            .map(|value| self.draft.unwrap_or(value))
    }
    fn visible_mode(&self) -> Option<bool> {
        self.draft.map(Setting::enabled).or_else(|| {
            match self.report.as_ref()?.observation().mode() {
                razer_device::earbuds_protocol40::ToggleObservation::Observed(enabled) => {
                    Some(*enabled)
                }
                _ => None,
            }
        })
    }
    fn visible_seconds(&self) -> Option<u16> {
        self.draft.map(Setting::seconds).or_else(|| {
            match self.report.as_ref()?.observation().time() {
                razer_device::earbuds_power::TimeObservation::Observed(seconds) => Some(*seconds),
                _ => None,
            }
        })
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
#[path = "earbuds_power_tests.rs"]
mod tests;

impl AudioProductWorkspace {
    pub(super) fn leave_earbuds_power(&mut self) {
        self.earbuds_power.cancel();
        self.earbuds_power.context_active = false;
        self.earbuds_power.did_start = false;
    }
    pub fn set_earbuds_power_context(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        connection: u64,
        cx: &mut Context<Self>,
    ) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(device.product_id) {
            return;
        }
        self.earbuds_connection.sync(device, active, connection);
        let identity = device.workspace_identity();
        if self.earbuds_power.identity != identity
            || self.earbuds_power.connection != connection
            || !active
        {
            self.earbuds_power.cancel();
            self.earbuds_power.did_start = false;
        }
        self.earbuds_power.identity = identity;
        self.earbuds_power.connection = connection;
        self.earbuds_power.container = device.device_container_id.clone();
        self.earbuds_power.context_active = active;
        self.load_earbuds_power_profile(device, cx);
        match razer_storage::earbuds_profiles::read_power(device) {
            Ok(setting) => {
                self.earbuds_power.persisted = setting;
                self.earbuds_power.storage_error = None;
            }
            Err(error) => self.earbuds_power.storage_error = Some(format!("{error:#}")),
        }
        if active && !self.earbuds_power.did_start {
            self.resume_earbuds_power(cx);
        }
        self.restore_explicit_earbuds_power_profile(cx);
        cx.notify();
    }
    fn load_earbuds_power_profile(
        &mut self,
        device: &razer_model::model::Device,
        cx: &mut Context<Self>,
    ) {
        let source = match razer_storage::earbuds_profiles::read_power_profile(device) {
            Ok(source) => {
                self.earbuds_power.restore_error = None;
                source
            }
            Err(error) => {
                self.earbuds_power.restore_error = Some(format!("{error:#}"));
                None
            }
        };
        if self.earbuds_power.profile_source == source {
            return;
        }
        self.earbuds_power.profile_source = source;
        self.earbuds_power.restored_profile = None;
        let mut canceled = false;
        self.earbuds_power.pending.retain(|request| {
            if request.profile.is_some() {
                request.active.store(false, Ordering::Release);
                canceled = true;
                false
            } else {
                true
            }
        });
        if canceled {
            self.earbuds_power.report = None;
            self.earbuds_power.draft = None;
            self.earbuds_power.loading = true;
            self.emit_earbuds_power(Operation::ReadPowerSaving, cx);
        }
    }
    fn restore_explicit_earbuds_power_profile(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds_connection.ready()
            || self
                .earbuds_power
                .report
                .as_ref()
                .and_then(|report| report.observation().setting())
                .is_none()
        {
            return;
        }
        let Some(source) = self.earbuds_power.profile_source.clone() else {
            return;
        };
        if self.earbuds_power.restored_profile.as_ref() == Some(&source)
            || self.earbuds_power.restore_error.is_some()
        {
            return;
        }
        self.earbuds_power.restored_profile = Some(source.clone());
        self.earbuds_power.draft = Some(source.1);
        self.emit_earbuds_power_from(
            Operation::SetPowerSaving { setting: source.1 },
            Some(source.0),
            cx,
        );
    }
    pub(super) fn resume_earbuds_power(&mut self, cx: &mut Context<Self>) {
        if self.earbuds_power.context_active && self.earbuds_power.scope.is_none() {
            self.earbuds_power.did_start = true;
            match Scope::new(self.spec.product_id, self.earbuds_power.container.clone()) {
                Ok(scope) => {
                    self.earbuds_power.active = Some(Arc::new(AtomicBool::new(true)));
                    self.earbuds_power.scope = Some(scope);
                    self.earbuds_power.loading = true;
                    self.emit_earbuds_power(Operation::ReadPowerSaving, cx);
                }
                Err(error) => self.earbuds_power.error = Some(format!("{error:#}")),
            }
        }
    }
    pub fn finish_earbuds_power_monitor(&mut self, error: String, cx: &mut Context<Self>) {
        self.earbuds_power.cancel();
        self.earbuds_power.error = Some(error);
        cx.notify();
    }
    fn emit_earbuds_power(&mut self, operation: Operation, cx: &mut Context<Self>) {
        self.emit_earbuds_power_from(operation, None, cx);
    }
    fn emit_earbuds_power_from(
        &mut self,
        operation: Operation,
        profile: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (Some(scope), Some(active)) = (&self.earbuds_power.scope, &self.earbuds_power.active)
        else {
            return;
        };
        if !active.load(Ordering::Acquire) {
            return;
        }
        let request = EarbudsPowerRequested {
            id: Uuid::new_v4(),
            scope: scope.clone(),
            operation,
            active: Arc::new(AtomicBool::new(true)),
            profile,
        };
        self.earbuds_power.pending.push(request.clone());
        cx.emit(request);
    }
    pub fn earbuds_power_request_current(&self, request: &EarbudsPowerRequested) -> bool {
        self.earbuds_power
            .scope
            .as_ref()
            .is_some_and(|scope| scope.id() == request.scope.id())
            && request.active.load(Ordering::Acquire)
            && self
                .earbuds_power
                .active
                .as_ref()
                .is_some_and(|active| active.load(Ordering::Acquire))
            && request.profile.as_ref().is_none_or(|profile| {
                self.earbuds_power
                    .profile_source
                    .as_ref()
                    .is_some_and(|(current, setting)| {
                        current == profile && Some(*setting) == request.setting()
                    })
            })
            && self
                .earbuds_power
                .pending
                .iter()
                .any(|pending| pending.id == request.id)
    }
    pub fn mark_earbuds_power_persisted(&mut self, request: &EarbudsPowerRequested) -> bool {
        if !request.needs_persistence() || !self.earbuds_power_request_current(request) {
            return false;
        }
        let Some(setting) = request.setting() else {
            return false;
        };
        self.earbuds_power.persisted = Some(setting);
        self.earbuds_power.storage_error = None;
        true
    }
    pub fn finish_earbuds_power(
        &mut self,
        request: &EarbudsPowerRequested,
        result: Result<Report, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds_power_request_current(request) {
            return;
        }
        self.earbuds_power
            .pending
            .retain(|pending| pending.id != request.id);
        self.earbuds_power.loading = false;
        let result = result.and_then(|report| {
            if report.product_id != self.spec.product_id || report.operation != request.operation {
                return Err("Earbud power reply belongs to another request".into());
            }
            report
                .power
                .map(|power| (power, report.write))
                .ok_or_else(|| "Earbud power reply omitted the observation".into())
        });
        match result {
            Ok((report, outcome)) => {
                self.earbuds_power.error = match outcome {
                    razer_device::earbuds_protocol40::WriteOutcome::Failed(error) => Some(error),
                    _ => report.observation().diagnostic(),
                };
                if request.setting().is_none() && self.earbuds_power.pending.is_empty() {
                    self.earbuds_power.draft = report.observation().setting();
                } else if self.earbuds_power.pending.is_empty()
                    && self.earbuds_power.error.is_none()
                    && self.earbuds_power.draft == report.observation().setting()
                {
                    self.earbuds_power.draft = None;
                }
                self.earbuds_power.report = Some(report);
            }
            Err(error) => {
                self.earbuds_power.error = Some(error);
                if request.setting().is_none() {
                    self.earbuds_power.report = None;
                    self.earbuds_power.draft = None;
                }
            }
        }
        self.restore_explicit_earbuds_power_profile(cx);
        cx.notify();
    }
    fn earbuds_power_ready(&self) -> bool {
        self.earbuds_power
            .active
            .as_ref()
            .is_some_and(|guard| guard.load(Ordering::Acquire))
            && self.earbuds_power.visible().is_some()
            && self.earbuds_power.storage_error.is_none()
            && self.earbuds_connection.ready()
    }
    fn edit_earbuds_power(
        &mut self,
        enabled: Option<bool>,
        minutes: Option<u16>,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds_power_ready() {
            return;
        }
        let Some(mut setting) = self.earbuds_power.visible() else {
            return;
        };
        if let Some(enabled) = enabled {
            setting = setting.with_enabled(enabled);
        }
        if let Some(minutes) = minutes {
            if !setting.enabled() {
                return;
            }
            let Ok(next) = setting.with_minutes(minutes) else {
                return;
            };
            setting = next;
        }
        self.earbuds_power.draft = Some(setting);
        self.earbuds_power.error = None;
        self.emit_earbuds_power(Operation::SetPowerSaving { setting }, cx);
        cx.notify();
    }
    pub(super) fn earbuds_power_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let mode = self.earbuds_power.visible_mode();
        let seconds = self.earbuds_power.visible_seconds();
        let enabled = mode == Some(true);
        let ready = self.earbuds_power_ready();
        let disconnected = self.earbuds_connection.disconnected();
        let tips = format!(
            "{}\n\n{}",
            t("AUDIO_POWER_SAVING_TOOLTIP"),
            t("AUDIO_POWER_SAVING_TOOLTIP_ADDITIONAL")
        );
        let choices = gpui_kit::base::RadioGroup::new("earbuds-power-minutes")
            .axis(Axis::Horizontal)
            .flex()
            .flex_row()
            .items_center()
            .gap(surface::css(10.))
            .mt(surface::css(20.))
            .children(MINUTE_CHOICES.map(|minutes| {
                let selected = seconds == Some(minutes * 60);
                Button::new(("earbuds-power-minutes", minutes as u64))
                    .selected(selected)
                    .role(Role::RadioButton)
                    .aria_selected(selected)
                    .aria_toggled(if selected {
                        gpui_kit::accesskit::Toggled::True
                    } else {
                        gpui_kit::accesskit::Toggled::False
                    })
                    .disabled(!ready || !enabled)
                    .h(surface::css(27.))
                    .min_w(surface::css(90.))
                    .px(surface::css(30.))
                    .py(surface::css(7.))
                    .border_1()
                    .border_color(if selected { C::selected() } else { C::border() })
                    .rounded(surface::css(3.))
                    .bg(if selected { C::tab_active() } else { C::background() })
                    .text_color(C::mapping_white())
                    .text_size(surface::css(12.))
                    .line_height(surface::css(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(ready && enabled, |button| {
                        button.hover(|style| style.border_color(C::selected()))
                    })
                    .child(minutes.to_string())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit_earbuds_power(None, Some(minutes), cx)
                    }))
            }));
        let switch = match mode {
            Some(enabled) => surface::SynapseSwitch::new("earbuds-power-switch")
                .checked(enabled)
                .disabled(!ready)
                .accessibility_label(t("POWER_SAVING_HEADER"))
                .on_change(cx.listener(|this, enabled, _, cx| {
                    this.edit_earbuds_power(Some(*enabled), None, cx)
                }))
                .into_any_element(),
            None => div()
                .id("earbuds-power-switch-unobserved")
                .test_support()
                .w(surface::css(30.))
                .text_color(C::secondary())
                .child("—")
                .into_any_element(),
        };
        let mut panel = surface::panel_with_title_switch(
            t("POWER_SAVING_HEADER"),
            switch,
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(surface::help_control("earbuds-power-help", tips)),
            cx,
        )
        .id("earbuds-power-panel")
        .test_support()
        .relative()
        .when(disconnected, |panel| panel.opacity(0.5))
        .child(
            v_flex()
                .when(!enabled, |body| body.opacity(0.3))
                .child(t("AUDIO_POWER_SAVING_DESC"))
                .child(choices),
        );
        if let Some(error) = self
            .earbuds_power
            .storage_error
            .as_deref()
            .or(self.earbuds_power.error.as_deref())
            .or(self.earbuds_power.restore_error.as_deref())
            .or(self.earbuds_connection.error())
        {
            panel = panel.child(
                div()
                    .mt(surface::css(10.))
                    .text_color(cx.theme().danger)
                    .child(error.to_owned()),
            );
        } else if self.earbuds_power.loading {
            panel = panel.child(div().mt(surface::css(10.)).child(t("LOADING")));
        }
        surface::page_columns()
            .child(surface::page_column(self.earbuds_disconnected_surface(
                super::earbuds_connection::Card::Power,
                panel.into_any_element(),
                cx,
            )))
            .into_any_element()
    }
}
