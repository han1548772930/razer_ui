//! The retained Enhancement switch owns draft, persistence and device results.
use super::*;
use razer_device::earbuds_do_not_disturb::Observation as DndObservation;
use razer_device::earbuds_protocol40::ToggleObservation;
use razer_device::{
    earbuds_auto_pause::{FirmwareObservation, Operation, Report},
    earbuds_controls::{EarbudsControls, EarbudsTask},
    earbuds_protocol40::{AutoPauseObservation, WriteOutcome},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestSource {
    Observation,
    UserEdit,
    ProfileRestore,
    HardwareNotification,
}

#[derive(Clone, Copy)]
pub(super) struct Mutation {
    pub(super) operation: Operation,
    source: RequestSource,
}

#[derive(Clone)]
pub struct EarbudsAutoPauseRequested {
    id: Uuid,
    profile_id: String,
    initialize_profile: bool,
    operation: Operation,
    active: Arc<AtomicBool>,
    monitor: Arc<AtomicBool>,
    source: RequestSource,
}

#[cfg(test)]
#[path = "earbuds_auto_pause_tests.rs"]
mod tests;
impl EarbudsAutoPauseRequested {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn initializes_profile(&self) -> bool {
        self.initialize_profile
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }
    pub fn monitor_guard(&self) -> Arc<AtomicBool> {
        self.monitor.clone()
    }
    pub fn is_profile_restore(&self) -> bool {
        self.source == RequestSource::ProfileRestore
    }
    pub fn is_hardware_notification(&self) -> bool {
        self.source == RequestSource::HardwareNotification
    }
    pub fn needs_persistence(&self) -> bool {
        if matches!(self.operation, Operation::SwitchInputSource | Operation::ActivationCode) { return false; }
        matches!(
            self.source,
            RequestSource::UserEdit | RequestSource::HardwareNotification
        ) || (self.is_profile_restore() && self.initialize_profile)
    }
}

#[derive(Default)]
pub(super) struct State {
    pub(super) eq_persisted: Option<razer_device::earbuds_equalizer::Setting>,
    pub(super) eq_presets: Vec<razer_device::earbuds_equalizer::Setting>,
    pub(super) eq_defaults: Vec<razer_device::earbuds_equalizer::Setting>,
    pub(super) eq_revision: u64,
    pub(super) eq_graph: super::earbuds_equalizer_graph::State,
    pub(super) eq_ui: super::earbuds_equalizer_view::State,
    pub(super) eq_profile_error: Option<String>,
    pub(super) eq_result: Option<Result<WriteOutcome, String>>,
    pub(super) input_popup: bool,
    pub(super) input_disabled_cursor: Option<Point<Pixels>>,
    pub(super) input_result: Option<Result<WriteOutcome, String>>,
    pub(super) controls: Option<EarbudsControls>,
    identity: String,
    profile_id: String,
    selected_profile_id: String,
    initialize_profile: bool,
    pub(super) active: bool,
    connected: bool,
    pending: Option<EarbudsAutoPauseRequested>,
    queued: Option<Operation>,
    pub(super) edits: VecDeque<Mutation>,
    restore_sequence: VecDeque<Operation>,
    dnd_device: DndObservation,
    dnd_persisted: Option<bool>,
    dnd_draft: bool,
    dnd_error: Option<String>,
    dnd_profile_error: Option<String>,
    dnd_last_write: Option<WriteOutcome>,
    voice_device: ToggleObservation,
    voice_persisted: Option<bool>,
    voice_draft: bool,
    voice_error: Option<String>,
    voice_profile_error: Option<String>,
    voice_last_write: Option<WriteOutcome>,
    error: Option<String>,
    profile_error: Option<String>,
    pub(super) initialized: bool,
    monitor: Option<Arc<AtomicBool>>,
    was_off: bool,
    reconnect_restores: u8,
    reread_after_restore: bool,
    firmware: FirmwareObservation,
    firmware_due: bool,
    pub(super) gesture: super::earbuds_gesture::State,
    pub(super) anc: super::earbuds_anc::State,
}
impl State {
    fn cancel_profile_requests(&mut self) {
        if let Some(request) = &self.pending {
            if request.source != RequestSource::Observation {
                request.active.store(false, Ordering::Release);
            }
        }
        self.queued = None;
        self.restore_sequence.clear();
        self.edits.clear();
        self.reconnect_restores = 0;
        self.reread_after_restore = false;
    }
    fn cancel(&mut self) {
        self.eq_revision = self.eq_revision.wrapping_add(1);
        self.eq_graph = super::earbuds_equalizer_graph::State::default();
        self.eq_ui = super::earbuds_equalizer_view::State::default();
        self.eq_result = None;
        self.input_popup = false;
        self.input_disabled_cursor = None;
        self.input_result = None;
        if let Some(guard) = self.monitor.take() {
            guard.store(false, Ordering::Release);
        }
        if let Some(request) = &self.pending {
            request.active.store(false, Ordering::Release);
        }
        self.queued = None;
        self.restore_sequence.clear();
        self.edits.clear();
        self.dnd_device = DndObservation::Unobserved;
        self.voice_device = ToggleObservation::Unobserved;
        self.firmware = FirmwareObservation::Unobserved;
        self.firmware_due = false;
        self.gesture.clear_observations();
        self.gesture.popup = None;
        self.anc.device = razer_device::earbuds_anc::Observation::Unobserved;
        self.initialized = false;
        self.was_off = false;
        self.reconnect_restores = 0;
        self.reread_after_restore = false;
        if let Some(controls) = &mut self.controls {
            controls.disconnect();
        }
    }

    fn schedule_restore(&mut self, auto_pause: Option<bool>) {
        if !self.restore_sequence.is_empty() {
            return;
        }
        if self.anc.profile_error.is_none() {
            if let Some(controls) = self
                .controls
                .as_ref()
                .filter(|control| control.anc_runtime.is_some())
            {
                let setting = self
                    .edits
                    .iter()
                    .rev()
                    .find_map(|edit| match edit.operation {
                        Operation::Anc { setting, .. } => Some(setting),
                        _ => None,
                    })
                    .or(controls.anc_persisted)
                    .unwrap_or(razer_device::earbuds_protocol40::ANC_PROFILE_DEFAULT);
                self.restore_sequence.push_back(Operation::Anc { setting });
            }
        }
        if self.eq_profile_error.is_none() {
            if let Some(setting) = self.edits.iter().rev().find_map(|edit| match edit.operation {
                Operation::Equalizer { setting } => Some(setting), _ => None,
            }).or(self.eq_persisted) {
                self.restore_sequence.push_back(Operation::Equalizer { setting });
            }
        }
        let dnd = self
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .rev()
            .find_map(|operation| match operation {
                Operation::DoNotDisturb { enabled } => Some(*enabled),
                _ => None,
            })
            .or(self.dnd_persisted)
            .or(self.initialize_profile.then_some(false));
        let auto_pause = self
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .rev()
            .find_map(|operation| match operation {
                Operation::Write { enabled } => Some(*enabled),
                _ => None,
            })
            .or(auto_pause);
        if let Some(enabled) = dnd {
            self.restore_sequence
                .push_back(Operation::DoNotDisturb { enabled });
        }
        let gesture = self
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .rev()
            .find_map(|operation| match operation {
                Operation::Gesture { enabled } => Some(*enabled),
                _ => None,
            })
            .or(self.gesture.persisted);
        if self.gesture.profile_error.is_none() {
            self.restore_sequence
                .push_back(Operation::RestoreGesture { enabled: gesture });
        }
        let tap = self
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .rev()
            .find_map(|operation| match operation {
                Operation::TapSpeed { speed } => Some(*speed),
                _ => None,
            })
            .or(self.gesture.tap_persisted);
        if self.gesture.tap_profile_error.is_none() {
            self.restore_sequence
                .push_back(Operation::RestoreTapSpeed { speed: tap });
        }
        let voice = self
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .rev()
            .find_map(|operation| match operation {
                Operation::VoicePrompt { enabled } => Some(*enabled),
                _ => None,
            })
            .or(self.voice_persisted);
        if self.voice_profile_error.is_none() {
            self.restore_sequence
                .push_back(Operation::RestoreVoicePrompt { enabled: voice });
        }
        self.restore_sequence.push_back(Operation::Restore {
            enabled: auto_pause,
        });
    }

    pub(super) fn queue_edit(&mut self, operation: Operation) {
        self.queue_mutation(operation, RequestSource::UserEdit);
    }
    pub(super) fn queue_notification(&mut self, operation: Operation) {
        self.queue_mutation(operation, RequestSource::HardwareNotification);
    }
    fn queue_mutation(&mut self, operation: Operation, source: RequestSource) {
        self.edits.retain(|queued| {
            queued.source != source
                || std::mem::discriminant(&queued.operation) != std::mem::discriminant(&operation)
        });
        for restore in &mut self.restore_sequence {
            match (&operation, restore) {
                (Operation::Equalizer { setting }, Operation::Equalizer { setting: saved }) => *saved = *setting,
                (Operation::Anc { setting, .. }, Operation::Anc { setting: saved, .. }) => {
                    *saved = *setting
                }
                (
                    Operation::DoNotDisturb { enabled },
                    Operation::DoNotDisturb { enabled: saved },
                ) => *saved = *enabled,
                (Operation::Write { enabled }, Operation::Restore { enabled: saved }) => {
                    *saved = Some(*enabled)
                }
                (
                    Operation::VoicePrompt { enabled },
                    Operation::RestoreVoicePrompt { enabled: saved },
                ) => *saved = Some(*enabled),
                (Operation::Gesture { enabled }, Operation::RestoreGesture { enabled: saved }) => {
                    *saved = Some(*enabled)
                }
                (Operation::TapSpeed { speed }, Operation::RestoreTapSpeed { speed: saved }) => {
                    *saved = Some(*speed)
                }
                _ => {}
            }
        }
        self.edits.push_back(Mutation { operation, source });
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn stored_field_changed<T: PartialEq>(
    current: anyhow::Result<Option<T>>,
    previous: Option<T>,
    previous_error: Option<&String>,
) -> bool {
    match current {
        Ok(value) => previous_error.is_some() || value != previous,
        Err(error) => previous_error != Some(&format!("{error:#}")),
    }
}
impl EventEmitter<EarbudsAutoPauseRequested> for AudioProductWorkspace {}

impl AudioProductWorkspace {
    pub(super) fn earbuds_enhancement_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (left, right) = if self.spec.product_id == 2657 {
            (
                vec![
                    self.earbuds_anc_panel(window, cx),
                    self.earbuds_auto_pause_panel(cx),
                    self.earbuds_gesture_panel(cx),
                ],
                vec![
                    self.earbuds_do_not_disturb_panel(cx),
                    self.earbuds_voice_prompt_panel(cx),
                ],
            )
        } else {
            (
                vec![
                    self.earbuds_do_not_disturb_panel(cx),
                    self.earbuds_auto_pause_panel(cx),
                ],
                vec![
                    self.earbuds_gesture_panel(cx),
                    self.earbuds_voice_prompt_panel(cx),
                ],
            )
        };
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .id("earbuds-enhancement-left")
                    .test_support()
                    .children(left),
            ))
            .child(surface::page_column(
                v_flex()
                    .id("earbuds-enhancement-right")
                    .test_support()
                    .children(right),
            ))
            .into_any_element()
    }

    pub(super) fn earbuds_snapshot(&self, snapshot: &mut Value) {
        let Some(controls) = &self.earbuds.controls else {
            return;
        };
        if let Some(setting) = self.earbuds.eq_persisted {
            let mut source = Some(snapshot.clone());
            if razer_storage::earbuds_profiles::merge_equalizer_source(&mut source, setting).is_ok() {
                *snapshot = source.expect("validated source snapshot");
            }
        }
        if let Some(device) = snapshot.get_mut("device").and_then(Value::as_object_mut) {
            device.remove("autoPause");
            device.remove("audioPrompts");
        }
        if let Some(profile) = snapshot.get_mut("profile").and_then(Value::as_object_mut) {
            if self.spec.product_id == 2657 {
                if let Some(setting) = controls.anc_persisted {
                    let field = profile
                        .entry("noiseCancellation")
                        .or_insert_with(|| json!({}));
                    if let Some(field) = field.as_object_mut() {
                        field.insert("isEnabled".into(), Value::Bool(setting.enabled));
                        field.insert("status".into(), Value::from(setting.mode.ui_status()));
                        field.insert("level".into(), Value::from(setting.level as u8));
                    }
                } else if self.earbuds.anc.profile_error.is_none() {
                    profile.remove("noiseCancellation");
                }
            }
            if let Some(enabled) = self.earbuds.gesture.persisted {
                let field = profile
                    .entry("audioGestureControl")
                    .or_insert_with(|| json!({}));
                if let Some(field) = field.as_object_mut() {
                    field.insert("isEnabled".into(), Value::Bool(enabled));
                }
            } else if self.earbuds.gesture.profile_error.is_none() {
                profile.remove("audioGestureControl");
            }
            if let Some(speed) = self.earbuds.gesture.tap_persisted {
                let field = profile.entry("audioTapSpeed").or_insert_with(|| json!({}));
                if let Some(field) = field.as_object_mut() {
                    field.insert("tapSpeed".into(), Value::String(speed.key().into()));
                }
            } else if self.earbuds.gesture.tap_profile_error.is_none() {
                profile.remove("audioTapSpeed");
            }
            if let Some(enabled) = self.earbuds.voice_persisted {
                let field = profile
                    .entry("audioVoicePromptControl")
                    .or_insert_with(|| json!({}));
                if let Some(field) = field.as_object_mut() {
                    field.insert("isEnabled".into(), Value::Bool(enabled));
                }
            } else if self.earbuds.voice_profile_error.is_none() {
                profile.remove("audioVoicePromptControl");
            }
            if let Some(enabled) = self.earbuds.dnd_persisted {
                let field = profile.entry("notDisturb").or_insert_with(|| json!({}));
                if let Some(field) = field.as_object_mut() {
                    field.insert("isEnabled".into(), Value::Bool(enabled));
                }
            } else if self.earbuds.dnd_profile_error.is_none() {
                profile.remove("notDisturb");
            }
            if let Some(enabled) = controls.auto_pause_persisted {
                let field = profile
                    .entry("audioAutoPauseControl")
                    .or_insert_with(|| json!({}));
                if let Some(field) = field.as_object_mut() {
                    field.insert("isEnabled".into(), Value::Bool(enabled));
                }
            }
        }
    }

    pub fn set_earbuds_context(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id) {
            return;
        }
        let connected = matches!(
            device.dashboard.connection_observation,
            Some(
                razer_model::model::DeviceConnectionObservation::UsbPresent
                    | razer_model::model::DeviceConnectionObservation::HidPresent
            )
        );
        let initializes_profile = device.profiles.is_empty() && device.active_profile.is_empty();
        let local = if initializes_profile {
            Ok(razer_device::earbuds_protocol40::AutoPauseProfileField { is_enabled: None })
        } else {
            razer_storage::earbuds_profiles::read_auto_pause(device, &device.active_profile)
        };
        let ready = active && connected && local.is_ok();
        if let Err(error) = &local {
            self.earbuds.error = Some(format!("{error:#}"));
        }
        let changed = self.earbuds.identity != device.workspace_identity()
            || self.earbuds.selected_profile_id != device.active_profile
            || self.earbuds.connected != connected
            || (!initializes_profile && self.earbuds_profile_changed(device));
        let became_active = ready && !self.earbuds.active;
        if self.earbuds.identity != device.workspace_identity()
            || self.earbuds.connected != connected
        {
            self.earbuds_graphic.clear();
            self.earbuds_activation.clear_device();
        }
        if !ready {
            self.earbuds_graphic.leave();
            self.earbuds_activation.cancel();
        }
        if changed {
            let continuing_connection = self.earbuds.identity == device.workspace_identity()
                && self.earbuds.connected == connected
                && self.earbuds.initialized
                && self.earbuds.active
                && ready;
            if continuing_connection {
                self.earbuds.cancel_profile_requests();
            } else {
                self.earbuds.cancel();
            }
            self.earbuds.identity = device.workspace_identity();
            self.earbuds.selected_profile_id = device.active_profile.clone();
            self.earbuds.initialize_profile = initializes_profile;
            self.earbuds.profile_id = if initializes_profile {
                Uuid::new_v4().to_string()
            } else {
                device.active_profile.clone()
            };
            self.earbuds.active = ready;
            self.earbuds.connected = connected;
            let eq = if initializes_profile {
                razer_storage::earbuds_profiles::initial_equalizer(device.product_id).map(Some)
            } else {
                razer_storage::earbuds_profiles::read_equalizer(device, &device.active_profile)
            };
            match eq {
                Ok(setting) => {
                    self.earbuds.eq_persisted = setting;
                    self.earbuds.eq_profile_error = None;
                    self.load_earbuds_equalizer_profile(device, setting);
                }
                Err(error) => {
                    self.earbuds.eq_persisted = None;
                    self.earbuds.eq_presets.clear();
                    self.earbuds.eq_defaults.clear();
                    self.earbuds.eq_profile_error = Some(format!("{error:#}"));
                }
            }
            match if initializes_profile {
                Ok(None)
            } else {
                razer_storage::earbuds_profiles::read_do_not_disturb(device, &device.active_profile)
            } {
                Ok(value) => {
                    self.earbuds.dnd_persisted = value;
                    self.earbuds.dnd_error = None;
                    self.earbuds.dnd_profile_error = None;
                }
                Err(error) => {
                    self.earbuds.dnd_persisted = None;
                    self.earbuds.dnd_profile_error = Some(format!("{error:#}"));
                }
            }
            if continuing_connection {
                if let Some(value) = self.earbuds.dnd_persisted {
                    self.earbuds.dnd_draft = value;
                }
            } else {
                self.earbuds.dnd_draft = false;
                self.earbuds.voice_draft = false;
            }
            self.earbuds
                .gesture
                .load_profile(device, initializes_profile, continuing_connection);
            match if initializes_profile {
                Ok(None)
            } else {
                razer_storage::earbuds_profiles::read_voice_prompt(device, &device.active_profile)
            } {
                Ok(value) => {
                    self.earbuds.voice_persisted = value;
                    if continuing_connection {
                        if let Some(value) = value {
                            self.earbuds.voice_draft = value;
                        }
                    }
                    self.earbuds.voice_error = None;
                    self.earbuds.voice_profile_error = None;
                }
                Err(error) => {
                    self.earbuds.voice_persisted = None;
                    self.earbuds.voice_profile_error = Some(format!("{error:#}"));
                }
            }
            let mut controls = if continuing_connection {
                self.earbuds
                    .controls
                    .take()
                    .expect("live earbud control owner")
            } else {
                EarbudsControls::new(self.spec.product_id).expect("supported earbuds")
            };
            if self.spec.product_id == 2657 {
                match if initializes_profile {
                    Ok(None)
                } else {
                    razer_storage::earbuds_profiles::read_anc(device, &device.active_profile)
                } {
                    Ok(setting) => {
                        controls.anc_persisted = setting;
                        if let Some(setting) = setting {
                            controls.draft.anc = Some(setting);
                        }
                        self.earbuds.anc.profile_error = None;
                    }
                    Err(error) => self.earbuds.anc.profile_error = Some(format!("{error:#}")),
                }
            }
            match local {
                Ok(field) => {
                    self.earbuds.profile_error = None;
                    controls.auto_pause_persisted = field.is_enabled;
                    if continuing_connection {
                        if let Some(value) = field.is_enabled {
                            controls.draft.auto_pause = value;
                        }
                    }
                }
                Err(error) => {
                    self.earbuds.profile_error = Some(format!("{error:#}"));
                    self.earbuds.error = Some(format!("{error:#}"));
                    self.earbuds.active = false;
                }
            }
            if connected && !continuing_connection {
                controls.connect();
            }
            self.earbuds.controls = Some(controls);
            if self.earbuds.active {
                if continuing_connection {
                    self.earbuds.schedule_restore(
                        self.earbuds
                            .controls
                            .as_ref()
                            .and_then(|control| control.auto_pause_persisted),
                    );
                } else {
                    self.earbuds.monitor = Some(Arc::new(AtomicBool::new(true)));
                    self.earbuds.queued = Some(Operation::Read);
                }
            }
        }
        if !changed && self.earbuds.active != ready {
            self.earbuds.eq_revision = self.earbuds.eq_revision.wrapping_add(1);
            self.earbuds.eq_graph = super::earbuds_equalizer_graph::State::default();
            self.earbuds.eq_ui = super::earbuds_equalizer_view::State::default();
            self.earbuds.input_disabled_cursor = None;
            if let Some(guard) = self.earbuds.monitor.take() {
                guard.store(false, Ordering::Release);
            }
            if let Some(request) = &self.earbuds.pending {
                request.active.store(false, Ordering::Release);
            }
            self.earbuds.queued = None;
            self.earbuds.restore_sequence.clear();
            self.earbuds.edits.clear();
            self.earbuds.active = ready;
        }
        if became_active && !changed {
            self.earbuds.monitor = Some(Arc::new(AtomicBool::new(true)));
            self.earbuds.queued = Some(Operation::Read);
        }
        self.start_earbuds_request(cx);
    }

    fn earbuds_profile_changed(&self, device: &razer_model::model::Device) -> bool {
        use razer_storage::earbuds_profiles as profiles;
        let profile = &device.active_profile;
        let controls = self.earbuds.controls.as_ref();
        self.earbuds_equalizer_collection_changed(device) || stored_field_changed(
            profiles::read_equalizer(device, profile),
            self.earbuds.eq_persisted,
            self.earbuds.eq_profile_error.as_ref(),
        ) || stored_field_changed(
            profiles::read_auto_pause(device, profile).map(|field| field.is_enabled),
            controls.and_then(|controls| controls.auto_pause_persisted),
            self.earbuds.profile_error.as_ref(),
        ) || stored_field_changed(
            profiles::read_do_not_disturb(device, profile),
            self.earbuds.dnd_persisted,
            self.earbuds.dnd_profile_error.as_ref(),
        ) || stored_field_changed(
            profiles::read_voice_prompt(device, profile),
            self.earbuds.voice_persisted,
            self.earbuds.voice_profile_error.as_ref(),
        ) || stored_field_changed(
            profiles::read_gesture(device, profile),
            self.earbuds.gesture.persisted,
            self.earbuds.gesture.profile_error.as_ref(),
        ) || stored_field_changed(
            profiles::read_tap_speed(device, profile),
            self.earbuds.gesture.tap_persisted,
            self.earbuds.gesture.tap_profile_error.as_ref(),
        ) || (self.spec.product_id == 2657
            && stored_field_changed(
                profiles::read_anc(device, profile),
                controls.and_then(|controls| controls.anc_persisted),
                self.earbuds.anc.profile_error.as_ref(),
            ))
    }

    pub(super) fn start_earbuds_request(&mut self, cx: &mut Context<Self>) {
        if self.earbuds.pending.is_some() || !self.earbuds.active {
            return;
        }
        let initial_notification = self.earbuds.initialize_profile
            && !self.earbuds.initialized
            && self
                .earbuds
                .edits
                .front()
                .is_some_and(|edit| edit.source == RequestSource::HardwareNotification);
        let mutation = if initial_notification {
            None
        } else {
            self.earbuds.edits.pop_front()
        };
        let Some((operation, source)) = mutation
            .map(|edit| (edit.operation, edit.source))
            .or_else(|| {
                self.earbuds
                    .queued
                    .take()
                    .map(|operation| (operation, RequestSource::Observation))
            })
            .or_else(|| {
                self.earbuds
                    .firmware_due
                    .then_some((Operation::Firmware, RequestSource::Observation))
            })
            .or_else(|| {
                self.earbuds
                    .restore_sequence
                    .pop_front()
                    .map(|operation| (operation, RequestSource::ProfileRestore))
            })
        else {
            return;
        };
        if operation == Operation::Firmware {
            self.earbuds.firmware_due = false;
        }
        if source == RequestSource::ProfileRestore {
            match operation {
                Operation::Anc { setting, .. } => {
                    if let Some(controls) = &mut self.earbuds.controls {
                        controls.draft.anc = Some(setting);
                    }
                }
                Operation::DoNotDisturb { enabled } => self.earbuds.dnd_draft = enabled,
                Operation::RestoreGesture { enabled } => {
                    self.earbuds.gesture.draft = enabled.unwrap_or(true)
                }
                Operation::RestoreTapSpeed { speed } => {
                    self.earbuds.gesture.tap_draft =
                        speed.unwrap_or(razer_device::earbuds_gesture::TapSpeed::Normal)
                }
                Operation::RestoreVoicePrompt { enabled } => {
                    self.earbuds.voice_draft = enabled.unwrap_or(true)
                }
                Operation::Restore { enabled } => {
                    if let Some(controls) = &mut self.earbuds.controls {
                        controls.draft.auto_pause = enabled.unwrap_or(true);
                    }
                }
                _ => {}
            }
        }
        let request = EarbudsAutoPauseRequested {
            id: Uuid::new_v4(),
            profile_id: self.earbuds.profile_id.clone(),
            initialize_profile: self.earbuds.initialize_profile,
            operation,
            source,
            active: Arc::new(AtomicBool::new(true)),
            monitor: self
                .earbuds
                .monitor
                .as_ref()
                .expect("active event owner")
                .clone(),
        };
        self.earbuds.pending = Some(request.clone());
        cx.emit(request);
    }

    pub fn earbuds_request_current(&self, request: &EarbudsAutoPauseRequested) -> bool {
        self.earbuds.active
            && request.active.load(Ordering::Acquire)
            && (request.source == RequestSource::Observation
                || self.earbuds.profile_id == request.profile_id)
            && self
                .earbuds
                .monitor
                .as_ref()
                .is_some_and(|monitor| Arc::ptr_eq(monitor, &request.monitor))
            && self
                .earbuds
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == request.id)
    }

    pub fn mark_earbuds_persisted(&mut self, request: &EarbudsAutoPauseRequested) -> bool {
        if !self.earbuds_request_current(request) || !request.needs_persistence() {
            return false;
        }
        self.earbuds.selected_profile_id = request.profile_id.clone();
        self.earbuds.initialize_profile = false;
        let Some(controls) = &mut self.earbuds.controls else {
            return false;
        };
        if request.initializes_profile() {
            controls.auto_pause_persisted = Some(true);
            self.earbuds.dnd_persisted = Some(false);
            self.earbuds.voice_persisted = Some(true);
            self.earbuds.gesture.persisted = Some(true);
            if self.spec.product_id == 2657 {
                controls.anc_persisted =
                    Some(razer_device::earbuds_protocol40::ANC_PROFILE_DEFAULT);
            }
        }
        let enabled = match request.operation {
            Operation::Equalizer { setting } => {
                let mut source = Some(self.draft.clone());
                if let Err(error) = razer_storage::earbuds_profiles::merge_equalizer_source(&mut source, setting) {
                    self.earbuds.eq_profile_error = Some(format!("{error:#}"));
                    return false;
                }
                if let Some(equalizer) = source.as_ref().and_then(|value| value.pointer("/profile/audioEqualizer")) {
                    self.draft["profile"]["audioEqualizer"] = equalizer.clone();
                }
                self.earbuds.eq_persisted = Some(setting);
                if let Some(preset) = self.earbuds.eq_presets.iter_mut().find(|preset| preset.mode == setting.mode && preset.preset == setting.preset) {
                    *preset = setting;
                }
                return true;
            }
            Operation::Anc { setting, .. } => {
                return controls
                    .acknowledge_persistence(EarbudsTask::SetAnc {
                        generation: controls.generation(),
                        setting,
                    })
                    .is_ok();
            }
            Operation::Write { enabled } => enabled,
            Operation::Restore { enabled } => enabled.unwrap_or(true),
            Operation::Read | Operation::Battery { .. } | Operation::Firmware | Operation::SwitchInputSource | Operation::ActivationCode => return false,
            Operation::DoNotDisturb { enabled } => {
                self.earbuds.dnd_persisted = Some(enabled);
                return true;
            }
            Operation::VoicePrompt { enabled } => {
                self.earbuds.voice_persisted = Some(enabled);
                return true;
            }
            Operation::RestoreVoicePrompt { .. } => return request.initializes_profile(),
            Operation::Gesture { enabled } => {
                self.earbuds.gesture.persisted = Some(enabled);
                return true;
            }
            Operation::TapSpeed { speed } => {
                self.earbuds.gesture.tap_persisted = Some(speed);
                return true;
            }
            Operation::RestoreGesture { .. } | Operation::RestoreTapSpeed { .. } => {
                return request.initializes_profile();
            }
        };
        controls
            .acknowledge_persistence(EarbudsTask::SetAutoPause {
                generation: controls.generation(),
                enabled,
            })
            .is_ok()
    }

    pub fn finish_earbuds_notification_save(
        &mut self,
        request: &EarbudsAutoPauseRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !request.is_hardware_notification()
            || !self
                .earbuds
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == request.id)
        {
            return;
        }
        if self.earbuds_request_current(request) {
            self.earbuds.anc.storage_error = error;
        }
        self.earbuds.pending = None;
        self.start_earbuds_request(cx);
        cx.notify();
    }

    pub fn finish_earbuds_request(
        &mut self,
        request: &EarbudsAutoPauseRequested,
        result: Result<Report, String>,
        cx: &mut Context<Self>,
    ) {
        if request.is_hardware_notification() {
            let error = Some(match result {
                Err(error) => error,
                Ok(_) => "Notification profile save cannot accept a device report".into(),
            });
            self.finish_earbuds_notification_save(request, error, cx);
            return;
        }
        if !self
            .earbuds
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == request.id)
        {
            return;
        }
        let current = self.earbuds_request_current(request);
        let result = result.and_then(|report| {
            if report.product_id != self.spec.product_id || report.operation != request.operation {
                Err("Earbud response belongs to another request".into())
            } else {
                Ok(report)
            }
        });
        self.earbuds.pending = None;
        if request.operation == Operation::ActivationCode {
            if current {
                self.earbuds_activation.code = Some(result.and_then(|report| {
                    report.activation_code.ok_or_else(|| "Activation response omitted the code".into())
                }));
                self.earbuds_activation.loading = false;
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if request.operation == Operation::SwitchInputSource {
            if current {
                self.earbuds.input_result = Some(result.map(|report| report.write));
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if matches!(request.operation, Operation::Equalizer { .. }) {
            if current { self.earbuds.eq_result = Some(result.map(|report| report.write)); }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if matches!(request.operation, Operation::Anc { .. }) {
            if current {
                match result {
                    Ok(report) => self.observe_anc_report(report, request.is_profile_restore()),
                    Err(error) => {
                        self.earbuds.anc.error = Some(error.clone());
                        if let Some(controls) = &mut self.earbuds.controls {
                            controls.last_anc_write =
                                Some(razer_device::earbuds_protocol40::AncWrite::Failed(error));
                        }
                    }
                }
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if matches!(
            request.operation,
            Operation::Gesture { .. }
                | Operation::RestoreGesture { .. }
                | Operation::TapSpeed { .. }
                | Operation::RestoreTapSpeed { .. }
        ) {
            if current {
                match result {
                    Ok(report) => self.observe_gesture_report(&report, request.operation),
                    Err(error) => self.fail_gesture_request(request.operation, error),
                }
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if request.operation == Operation::Firmware {
            if current {
                self.earbuds.firmware = match result {
                    Ok(report) => report.firmware,
                    Err(error) => FirmwareObservation::ReadFailed(error),
                };
                if let Some(controls) = &mut self.earbuds.controls {
                    controls.firmware_version = match &self.earbuds.firmware {
                        FirmwareObservation::Observed(reading) => {
                            Some(reading.version().to_string())
                        }
                        _ => None,
                    };
                }
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if matches!(
            request.operation,
            Operation::VoicePrompt { .. } | Operation::RestoreVoicePrompt { .. }
        ) {
            if current {
                match result {
                    Ok(report) => {
                        if let Some(observed) = report.voice_prompt {
                            if let ToggleObservation::Observed(enabled) = observed {
                                if !self
                                    .earbuds
                                    .edits
                                    .iter()
                                    .map(|edit| &edit.operation)
                                    .any(|op| matches!(op, Operation::VoicePrompt { .. }))
                                {
                                    self.earbuds.voice_draft = match request.operation {
                                        Operation::RestoreVoicePrompt { enabled } => {
                                            enabled.unwrap_or(true)
                                        }
                                        _ => enabled,
                                    };
                                }
                            }
                            self.earbuds.voice_error = observed.diagnostic();
                            self.earbuds.voice_device = observed;
                        }
                        if let WriteOutcome::Failed(error) = &report.write {
                            self.earbuds.voice_error = Some(error.clone());
                        }
                        self.earbuds.voice_last_write = Some(report.write);
                    }
                    Err(error) => {
                        if request.is_profile_restore() {
                            self.earbuds.voice_device =
                                ToggleObservation::ReadFailed(error.clone());
                        }
                        self.earbuds.voice_error = Some(error.clone());
                        self.earbuds.voice_last_write = Some(WriteOutcome::Failed(error));
                    }
                }
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        if matches!(request.operation, Operation::DoNotDisturb { .. }) {
            if current {
                match result {
                    Ok(report) => {
                        let failed = matches!(&report.write, WriteOutcome::Failed(_));
                        if let Some(observed) = report.do_not_disturb {
                            if let DndObservation::Observed(enabled) = observed {
                                if (!failed || request.is_profile_restore())
                                    && !self
                                        .earbuds
                                        .edits
                                        .iter()
                                        .map(|edit| &edit.operation)
                                        .any(|op| matches!(op, Operation::DoNotDisturb { .. }))
                                {
                                    self.earbuds.dnd_draft = if request.is_profile_restore() {
                                        match request.operation {
                                            Operation::DoNotDisturb { enabled } => enabled,
                                            _ => enabled,
                                        }
                                    } else {
                                        enabled
                                    };
                                }
                            }
                            self.earbuds.dnd_device = observed;
                        }
                        self.earbuds.dnd_error = match &report.write {
                            WriteOutcome::Failed(error) => Some(error.clone()),
                            _ => None,
                        };
                        self.earbuds.dnd_last_write = Some(report.write);
                    }
                    Err(error) => {
                        self.earbuds.dnd_device = DndObservation::ReadFailed(error.clone());
                        self.earbuds.dnd_error = Some(error.clone());
                        self.earbuds.dnd_last_write = Some(WriteOutcome::Failed(error));
                    }
                }
            }
            self.start_earbuds_request(cx);
            cx.notify();
            return;
        }
        let mut schedule_restore = None;
        if current {
            match result {
                Ok(report) => {
                    self.earbuds_graphic.observe(&report);
                    self.observe_gesture_report(&report, request.operation);
                    if request.operation == Operation::Read
                        && report.connected == Some(true)
                        && matches!(self.earbuds.firmware, FirmwareObservation::Unobserved)
                    {
                        self.earbuds.firmware_due = true;
                    }
                    if let Some(observed) = &report.voice_prompt {
                        if let ToggleObservation::Observed(enabled) = observed {
                            if !self
                                .earbuds
                                .edits
                                .iter()
                                .map(|edit| &edit.operation)
                                .any(|op| matches!(op, Operation::VoicePrompt { .. }))
                            {
                                self.earbuds.voice_draft = *enabled;
                            }
                        }
                        self.earbuds.voice_error = observed.diagnostic();
                        self.earbuds.voice_device = observed.clone();
                    }
                    if let Some(observed) = &report.do_not_disturb {
                        if let DndObservation::Observed(enabled) = observed {
                            if !self
                                .earbuds
                                .edits
                                .iter()
                                .map(|edit| &edit.operation)
                                .any(|op| matches!(op, Operation::DoNotDisturb { .. }))
                            {
                                self.earbuds.dnd_draft = *enabled;
                            }
                        }
                        self.earbuds.dnd_error = observed.diagnostic();
                        self.earbuds.dnd_device = observed.clone();
                    }
                    self.earbuds.error = match &report.write {
                        WriteOutcome::Failed(error) => Some(error.clone()),
                        _ => report.observation.diagnostic(),
                    };
                    let controls = self
                        .earbuds
                        .controls
                        .as_mut()
                        .expect("earbuds control owner");
                    if let Some(connected) = report.connected {
                        if connected && self.earbuds.was_off {
                            self.earbuds.initialized = false;
                            self.earbuds.reconnect_restores =
                                if self.spec.product_id == 2689 { 1 } else { 2 };
                        }
                        self.earbuds.was_off = !connected;
                        if !connected {
                            self.earbuds.initialized = false;
                            self.earbuds.queued = None;
                            self.earbuds.restore_sequence.clear();
                            self.earbuds.edits.clear();
                            self.earbuds.dnd_device = DndObservation::Unobserved;
                            self.earbuds.voice_device = ToggleObservation::Unobserved;
                            self.earbuds.gesture.clear_observations();
                        }
                    }
                    if report.connected.is_some() {
                        if controls.earbud_disconnected != report.has_disconnected_earbud() {
                            self.earbuds.input_disabled_cursor = None;
                        }
                        controls.earbud_disconnected = report.has_disconnected_earbud();
                        if controls.earbud_disconnected {
                            self.earbuds.edits.clear();
                        }
                    }
                    if !matches!(request.operation, Operation::Battery { .. })
                        || report.connected == Some(false)
                    {
                        controls.auto_pause_device = report.observation.clone();
                    }
                    controls.last_auto_pause_write = Some(report.write.clone());
                    if request.operation == Operation::Read && report.connected == Some(true) {
                        self.earbuds.initialized = true;
                        schedule_restore = Some(controls.auto_pause_persisted);
                    }
                    if let AutoPauseObservation::Observed(enabled) = report.observation {
                        if !self
                            .earbuds
                            .edits
                            .iter()
                            .map(|edit| &edit.operation)
                            .any(|op| matches!(op, Operation::Write { .. }))
                        {
                            controls.draft.auto_pause = if request.is_profile_restore() {
                                match request.operation {
                                    Operation::Restore { enabled } => enabled.unwrap_or(true),
                                    _ => enabled,
                                }
                            } else {
                                enabled
                            };
                        }
                    }
                    if matches!(request.operation, Operation::Restore { .. })
                        && self.earbuds.reconnect_restores > 0
                    {
                        self.earbuds.reconnect_restores -= 1;
                        if self.earbuds.reconnect_restores > 0 && self.earbuds.queued.is_none() {
                            schedule_restore = Some(controls.auto_pause_persisted);
                        } else if self.earbuds.reconnect_restores == 0
                            && self.earbuds.reread_after_restore
                        {
                            self.earbuds.reread_after_restore = false;
                            if self.earbuds.queued.is_none() {
                                self.earbuds.queued = Some(Operation::Battery { levels: None });
                            }
                        }
                    }
                    if matches!(request.operation, Operation::Battery { .. })
                        && self.earbuds.reconnect_restores > 0
                        && self.earbuds.queued.is_none()
                    {
                        if self.earbuds.initialize_profile {
                            self.earbuds.initialized = false;
                            self.earbuds.queued = Some(Operation::Read);
                        } else {
                            self.earbuds.initialized = true;
                            schedule_restore = Some(controls.auto_pause_persisted);
                        }
                    }
                }
                Err(error) => {
                    if matches!(request.operation, Operation::Read | Operation::Battery { .. }) {
                        self.earbuds_graphic.clear();
                    }
                    if let Some(controls) = &mut self.earbuds.controls {
                        match request.operation {
                            Operation::Read | Operation::Battery { .. } => {
                                self.earbuds.initialized = false;
                                controls.auto_pause_device = AutoPauseObservation::Unobserved;
                                self.earbuds.dnd_device = DndObservation::Unobserved;
                                self.earbuds.voice_device = ToggleObservation::Unobserved;
                                self.earbuds.gesture.clear_observations();
                                self.earbuds.anc.device =
                                    razer_device::earbuds_anc::Observation::Unobserved;
                            }
                            _ => {
                                controls.last_auto_pause_write =
                                    Some(WriteOutcome::Failed(error.clone()))
                            }
                        }
                    }
                    self.earbuds.error = Some(error);
                }
            }
        }
        if let Some(enabled) = schedule_restore {
            self.earbuds.schedule_restore(enabled);
        }
        self.start_earbuds_request(cx);
        cx.notify();
    }

    pub fn observe_earbuds_notification(
        &mut self,
        owner: &Arc<AtomicBool>,
        event: razer_device::earbuds_events::Notification,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds.active
            || !owner.load(Ordering::Acquire)
            || !self
                .earbuds
                .monitor
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, owner))
        {
            return;
        }
        use razer_device::earbuds_events::Notification;
        if let Notification::Connection { status, .. } = &event {
            if let Err(error) = razer_device::earbuds_auto_pause::connection_is_online(*status) {
                self.earbuds_graphic.clear();
                self.earbuds_activation.clear_device();
                let error = format!("{error:#}");
                if let Some(request) = &self.earbuds.pending {
                    request.active.store(false, Ordering::Release);
                }
                self.earbuds.queued = None;
                self.earbuds.restore_sequence.clear();
                self.earbuds.edits.clear();
                self.earbuds.initialized = false;
                self.earbuds.dnd_device = DndObservation::Unobserved;
                self.earbuds.voice_device = ToggleObservation::Unobserved;
                self.earbuds.gesture.clear_observations();
                self.earbuds.anc.device = razer_device::earbuds_anc::Observation::Unobserved;
                if let Some(controls) = &mut self.earbuds.controls {
                    controls.auto_pause_device = AutoPauseObservation::Unobserved;
                }
                self.earbuds.error = Some(error);
                cx.notify();
                return;
            }
        }
        match event {
            Notification::Anc { state } => self.observe_anc_notification(state, cx),
            Notification::Connection { status: 0, .. } => {
                self.earbuds_graphic.clear();
                self.earbuds_activation.clear_device();
                self.earbuds.was_off = true;
                self.earbuds.initialized = false;
                self.earbuds.firmware = FirmwareObservation::Unobserved;
                self.earbuds.firmware_due = false;
                if let Some(request) = &self.earbuds.pending {
                    request.active.store(false, Ordering::Release);
                }
                self.earbuds.queued = None;
                self.earbuds.restore_sequence.clear();
                self.earbuds.edits.clear();
                self.earbuds.dnd_device = DndObservation::Unobserved;
                self.earbuds.voice_device = ToggleObservation::Unobserved;
                self.earbuds.gesture.clear_observations();
                self.earbuds.anc.device = razer_device::earbuds_anc::Observation::Unobserved;
                if let Some(controls) = &mut self.earbuds.controls {
                    controls.earbud_disconnected = true;
                    controls.firmware_version = None;
                    controls.auto_pause_device = AutoPauseObservation::Unobserved;
                }
            }
            Notification::Connection { .. } => {
                if self.earbuds.was_off {
                    self.earbuds.was_off = false;
                    self.earbuds.reconnect_restores =
                        if self.spec.product_id == 2689 { 1 } else { 2 };
                    self.earbuds.reread_after_restore = true;
                    self.earbuds.initialized = false;
                    self.earbuds.queued = Some(Operation::Read);
                } else if self.earbuds.queued.is_none() {
                    self.earbuds.queued = Some(if self.earbuds.initialized {
                        Operation::Battery { levels: None }
                    } else {
                        Operation::Read
                    });
                }
                self.start_earbuds_request(cx);
            }
            Notification::Battery { left, right } => {
                // Device confirmation and the current connection gate are read by
                // the service; incoming bytes never fabricate an auto-pause value.
                if self.earbuds.queued.is_none() {
                    self.earbuds.queued = Some(Operation::Battery {
                        levels: Some([left, right]),
                    });
                }
                self.start_earbuds_request(cx);
            }
        }
        cx.notify();
    }

    pub fn finish_earbuds_monitor(
        &mut self,
        owner: &Arc<AtomicBool>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .earbuds
            .monitor
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, owner))
        {
            return;
        }
        if let Some(error) = error {
            self.earbuds.error = Some(error);
            self.earbuds.cancel();
            self.earbuds.active = false;
        }
        cx.notify();
    }

    fn toggle_earbuds_auto_pause(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.active {
            return;
        }
        let Some(controls) = &mut self.earbuds.controls else {
            return;
        };
        if !self.earbuds.initialized {
            return;
        }
        if let Some(EarbudsTask::SetAutoPause { enabled, .. }) = controls.toggle_auto_pause() {
            self.earbuds.queue_edit(Operation::Write { enabled });
            self.start_earbuds_request(cx);
            cx.notify();
        }
    }

    pub(super) fn earbuds_auto_pause_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let control = self.earbuds.controls.as_ref();
        let enabled = self.earbuds.active
            && control
                .is_some_and(|control| !control.earbud_disconnected && self.earbuds.initialized);
        surface::panel_with_title_switch(
            t("AUTO_PAUSE"),
            surface::SynapseSwitch::new("earbuds-auto-pause")
                .checked(control.is_some_and(|control| control.draft.auto_pause))
                .disabled(!enabled)
                .accessibility_label(t("AUTO_PAUSE"))
                .on_change(cx.listener(|this, _, _, cx| this.toggle_earbuds_auto_pause(cx))),
            div(),
            cx,
        )
        .opacity(
            if control.is_some_and(|control| control.earbud_disconnected) {
                0.5
            } else {
                1.
            },
        )
        .child(
            div()
                .text_size(surface::css(14.))
                .child(t("AUTO_PAUSE_DESC")),
        )
        .into_any_element()
    }

    fn toggle_do_not_disturb(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.active
            || self.earbuds.dnd_profile_error.is_some()
            || !self.earbuds.initialized
            || self
                .earbuds
                .controls
                .as_ref()
                .is_none_or(|control| control.earbud_disconnected)
        {
            return;
        }
        self.earbuds.dnd_draft = !self.earbuds.dnd_draft;
        self.earbuds.queue_edit(Operation::DoNotDisturb {
            enabled: self.earbuds.dnd_draft,
        });
        self.start_earbuds_request(cx);
        cx.notify();
    }

    pub(super) fn earbuds_do_not_disturb_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let disconnected = self
            .earbuds
            .controls
            .as_ref()
            .is_some_and(|control| control.earbud_disconnected);
        let enabled = self.earbuds.active
            && !disconnected
            && self.earbuds.dnd_profile_error.is_none()
            && self.earbuds.initialized;
        let description = format!("{}\n\n", t("DO_NOT_DISTURB_DESC"));
        let start = description.len();
        let text = format!("{description}{}", t("DO_NOT_DISTURB_DESC_NOTE_EARBUDS"));
        let end = text.len();
        surface::panel_with_title_switch(
            t("DO_NOT_DISTURB"),
            surface::SynapseSwitch::new("earbuds-do-not-disturb")
                .checked(self.earbuds.dnd_draft)
                .disabled(!enabled)
                .accessibility_label(t("DO_NOT_DISTURB"))
                .on_change(cx.listener(|this, _, _, cx| this.toggle_do_not_disturb(cx))),
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(surface::help_control(
                    "earbuds-do-not-disturb-help",
                    format!(
                        "{}\n\n{}",
                        t("DO_NOT_DISTURB_TIP"),
                        t("DO_NOT_DISTURB_TIP_EARBUDS")
                    ),
                )),
            cx,
        )
        .relative()
        .opacity(if disconnected { 0.5 } else { 1. })
        .child(StyledText::new(text).with_highlights([(
            start..end,
            HighlightStyle {
                color: Some(razer_widgets::theme::AudioEnhancementColors::secondary()),
                ..Default::default()
            },
        )]))
        .into_any_element()
    }

    fn toggle_voice_prompt(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.active
            || self.earbuds.voice_profile_error.is_some()
            || !self.earbuds.initialized
            || self
                .earbuds
                .controls
                .as_ref()
                .is_none_or(|control| control.earbud_disconnected)
        {
            return;
        }
        self.earbuds.voice_draft = !self.earbuds.voice_draft;
        self.earbuds.queue_edit(Operation::VoicePrompt {
            enabled: self.earbuds.voice_draft,
        });
        self.start_earbuds_request(cx);
        cx.notify();
    }

    pub(super) fn earbuds_voice_prompt_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let disconnected = self
            .earbuds
            .controls
            .as_ref()
            .is_some_and(|control| control.earbud_disconnected);
        let enabled = self.earbuds.active
            && !disconnected
            && self.earbuds.voice_profile_error.is_none()
            && self.earbuds.initialized;
        surface::panel_with_title_switch(
            t("AUDIO_PROMPTS"),
            surface::SynapseSwitch::new("earbuds-voice-prompt")
                .checked(self.earbuds.voice_draft)
                .disabled(!enabled)
                .accessibility_label(t("AUDIO_PROMPTS"))
                .on_change(cx.listener(|this, _, _, cx| this.toggle_voice_prompt(cx))),
            div(),
            cx,
        )
        .opacity(if disconnected { 0.5 } else { 1. })
        .child(t("AUDIO_PROMPTS_DESC"))
        .into_any_element()
    }
}
