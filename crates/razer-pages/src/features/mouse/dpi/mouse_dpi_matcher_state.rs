//! Current PID190 dS/AS state and action projection; no service/device simulation.
use razer_device::dpi_matcher::{Calibration, Phase};
use razer_device::dpi_matcher_storage::{Library, MatcherProfile, ProfileName, ProfileUpdate};
use razer_device::mouse_current_dpi::SourceStage;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatcherCommand {
    GetState,
    Start,
    Confirm,
    Cancel,
    Reset,
    DeleteProfile(String),
    DeleteAllProfiles,
}
impl MatcherCommand {
    /// The exact inner payload sent to `DPI_MATCHER_UI_COMMAND`.
    pub fn payload(&self) -> Value {
        match self {
            Self::GetState => json!({"type":"GET_STATE"}),
            Self::Start => json!({"type":"START"}),
            Self::Confirm => json!({"type":"CONFIRM"}),
            Self::Cancel => json!({"type":"CANCEL"}),
            Self::Reset => json!({"type":"RESET"}),
            Self::DeleteProfile(guid) => json!({"type":"DELETE_PROFILE","payload":guid}),
            Self::DeleteAllProfiles => json!({"type":"DELETE_ALL_PROFILES"}),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatcherAction {
    Command(MatcherCommand),
    /// AS assigns the entire observed profile.dpi to E[active-1]. The parent
    /// must call the current X-stage action producer, then SelectProfile.
    SetStageValueX {
        stages: Vec<SourceStage>,
        active: usize,
    },
    SelectProfile(String),
    RenameProfile(String),
    /// The observed library was normalized by the source reducer and must be
    /// persisted once, independently of the device calibration request.
    NormalizeProfiles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatcherReducerError {
    NonStringProfileName,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatcherObservation {
    Calibration(Calibration),
    Profiles {
        library: Library,
        selected: String,
    },
    Stages {
        stages: Vec<SourceStage>,
        active: usize,
    },
    Devices(Vec<Value>),
    Language(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileItem {
    guid: String,
    name: String,
    disabled: bool,
}
impl ProfileItem {
    pub fn value_id(&self) -> &String {
        &self.guid
    }
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn disabled(&self) -> bool {
        self.disabled
    }
}

#[derive(Debug, Clone)]
pub struct MatcherState {
    calibration: Calibration,
    library: Library,
    selected: String,
    stages: Vec<SourceStage>,
    active: usize,
    devices: Vec<Value>,
    language: String,
    popup: bool,
    start_listener: bool,
    first_load: bool,
    actions_open: bool,
    delete_all_open: bool,
    ignore_next_document_click: bool,
    rename: Option<String>,
}
impl Default for MatcherState {
    fn default() -> Self {
        Self {
            calibration: Calibration::default(),
            // Current rt is a local UI placeholder with a fresh v4 UUID and
            // null DPI. It is not an observed device or a calibrated profile.
            library: Library::new(vec![MatcherProfile::new(
                uuid::Uuid::new_v4().to_string(),
                ProfileName::Text("None".into()),
                Some(SourceStage::null()),
            )]),
            selected: String::new(),
            stages: vec![],
            active: 0,
            devices: vec![],
            language: "en".into(),
            popup: false,
            start_listener: false,
            first_load: false,
            actions_open: false,
            delete_all_open: false,
            ignore_next_document_click: false,
            rename: None,
        }
    }
}
impl MatcherState {
    pub fn phase(&self) -> Phase {
        self.calibration.phase()
    }
    pub fn popup(&self) -> bool {
        self.popup
    }
    pub fn actions_open(&self) -> bool {
        self.actions_open
    }
    pub fn delete_all_open(&self) -> bool {
        self.delete_all_open
    }
    pub fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }
    pub fn selected(&self) -> &str {
        &self.selected
    }
    pub fn library(&self) -> &Library {
        &self.library
    }
    pub fn process(&self) -> f32 {
        self.calibration.process() as f32
    }
    pub fn current_dpi(&self) -> Option<f64> {
        self.calibration.current_dpi()
    }
    pub fn new_dpi(&self) -> f64 {
        self.calibration.new_dpi()
    }
    pub fn step_active(&self, step: usize) -> bool {
        matches!(
            (step, self.phase()),
            (1, Phase::Ready | Phase::Calibrating | Phase::Completed)
                | (2, Phase::Calibrating | Phase::Completed)
                | (3, Phase::Completed)
        )
    }
    pub fn mount(&mut self) -> Vec<MatcherAction> {
        vec![MatcherAction::Command(MatcherCommand::GetState)]
    }
    pub fn unmount(&mut self) {
        self.start_listener = false;
        // AS/dS instance state is destroyed, while Redux observations survive.
        self.first_load = false;
        self.actions_open = false;
        self.delete_all_open = false;
        self.ignore_next_document_click = false;
        self.rename = None;
    }
    /// Source reducers optimistically update UI state before posting middleware
    /// messages. This is a local draft, never a device/storage acknowledgement.
    pub fn apply_local_action(
        &mut self,
        action: &MatcherAction,
    ) -> Result<(), MatcherReducerError> {
        match action {
            MatcherAction::SetStageValueX { stages, active } => {
                self.stages = stages.clone();
                self.active = *active;
            }
            MatcherAction::SelectProfile(guid) => {
                let profiles = self
                    .library
                    .profiles()
                    .iter()
                    .filter(|profile| !profile.name().is_english_none())
                    .cloned()
                    .collect();
                self.library = self.library.with_profiles(profiles);
                self.selected = guid.clone();
            }
            MatcherAction::RenameProfile(name) => {
                let mut profiles = self.library.profiles().to_vec();
                for profile in &mut profiles {
                    if profile.guid() == self.selected {
                        profile.set_name(name.clone());
                    }
                }
                if profiles.len() > 1 {
                    if profiles
                        .iter()
                        .any(|profile| profile.name().english_or_first_property_or_empty().is_err())
                    {
                        return Err(MatcherReducerError::NonStringProfileName);
                    }
                    profiles.sort_by_cached_key(|profile| {
                        profile
                            .name()
                            .english_or_first_property_or_empty()
                            .expect("profile names checked above")
                            .to_uppercase()
                            .encode_utf16()
                            .collect::<Vec<_>>()
                    });
                }
                self.library = self.library.with_profiles(profiles);
            }
            MatcherAction::NormalizeProfiles | MatcherAction::Command(_) => {}
        }
        Ok(())
    }
    pub fn profile_payload(&self) -> Value {
        self.profile_update().file_payload()
    }
    pub fn profile_update(&self) -> ProfileUpdate {
        ProfileUpdate::new(self.library.clone(), Some(self.selected.clone()))
    }
    pub fn observe(&mut self, observation: MatcherObservation) -> Vec<MatcherAction> {
        let changed = match &observation {
            MatcherObservation::Calibration(value) => value.phase() != self.calibration.phase(),
            _ => false,
        };
        let mut actions = self.observe_unmounted(observation);
        if changed && self.phase() != Phase::Ready {
            self.popup = true;
            let first_error = self.phase() == Phase::Error && !self.first_load;
            // React setState is asynchronous in this handler: the error
            // comparison sees the previous firstLoad before it becomes 1.
            self.first_load = true;
            if first_error {
                actions.extend(self.close());
            }
        }
        actions
    }
    /// Redux receives library updates while dS/AS are absent. Its normalization
    /// still requests persistence; component popup/reset reactions stay unmounted.
    pub fn observe_unmounted(&mut self, observation: MatcherObservation) -> Vec<MatcherAction> {
        match observation {
            MatcherObservation::Calibration(value) => {
                self.calibration = value;
            }
            MatcherObservation::Profiles { library, selected } => {
                let normalized = library.with_deduplicated_names();
                let changed = normalized
                    .profiles()
                    .iter()
                    .zip(library.profiles())
                    .any(|(normalized, original)| normalized.name() != original.name());
                self.library = normalized;
                self.selected = selected;
                if changed {
                    return vec![MatcherAction::NormalizeProfiles];
                }
            }
            MatcherObservation::Stages { stages, active } => {
                self.stages = stages;
                self.active = active;
            }
            MatcherObservation::Devices(devices) => self.devices = devices,
            MatcherObservation::Language(language) => self.language = language.to_lowercase(),
        }
        vec![]
    }
    pub fn open(&mut self) {
        self.popup = true;
        self.start_listener = true;
    }
    /// Call only for the browser-equivalent click phase. Source Calibrate,
    /// close/cancel/reset stop propagation and must not be passed here.
    pub fn window_click(&mut self, inside_calibrate: bool) -> Vec<MatcherAction> {
        if self.start_listener && self.popup && !inside_calibrate {
            self.start_listener = false;
            vec![MatcherAction::Command(MatcherCommand::Start)]
        } else {
            vec![]
        }
    }
    pub fn close(&mut self) -> Vec<MatcherAction> {
        self.popup = false;
        self.start_listener = false;
        vec![MatcherAction::Command(
            if self.phase() == Phase::Calibrating {
                MatcherCommand::Cancel
            } else {
                MatcherCommand::Reset
            },
        )]
    }
    pub fn cancel(&mut self) -> Vec<MatcherAction> {
        self.start_listener = true;
        // Every middleware producer sends an object. dS.cancel compares that
        // entire prop to a string, so its object path always calls close().
        self.close()
    }
    pub fn reset(&mut self) -> Vec<MatcherAction> {
        self.open();
        vec![MatcherAction::Command(MatcherCommand::Reset)]
    }
    pub fn apply(&mut self) -> Vec<MatcherAction> {
        if self.phase() != Phase::Completed {
            return vec![];
        }
        self.popup = false;
        vec![MatcherAction::Command(MatcherCommand::Confirm)]
    }
    pub fn dataset(&self, none: &str) -> (Vec<ProfileItem>, usize) {
        let mut dataset: Vec<_> = self
            .library
            .profiles()
            .iter()
            .map(|profile| {
                let mut name = profile.name().localized(&self.language);
                if name.is_empty()
                    && let Some(device) = self.devices.iter().find(|device| {
                        device.get("guid").and_then(Value::as_str) == Some(profile.guid())
                    })
                    && truthy(&device["productName"])
                {
                    // AS reads productName[lang] || productName.en || name;
                    // unlike profile.name, a plain string is not an override.
                    if let Some(product_name) = device["productName"]
                        .get(&self.language)
                        .filter(|value| truthy(value))
                        .or_else(|| {
                            device["productName"]
                                .get("en")
                                .filter(|value| truthy(value))
                        })
                        .and_then(Value::as_str)
                    {
                        name = product_name.into();
                    }
                }
                ProfileItem {
                    guid: profile.guid().into(),
                    name,
                    disabled: profile.is_disabled(),
                }
            })
            .collect();
        if self.selected.is_empty()
            || !self
                .library
                .profiles()
                .iter()
                .any(|profile| profile.guid() == self.selected)
        {
            dataset.insert(
                0,
                ProfileItem {
                    guid: "none".into(),
                    name: none.into(),
                    disabled: true,
                },
            );
        }
        let selected = dataset
            .iter()
            .position(|item| item.guid == self.selected)
            .unwrap_or(0);
        (dataset, selected)
    }
    pub fn choose(&self, guid: &str) -> Vec<MatcherAction> {
        if guid == "none" || guid == self.selected {
            return vec![];
        }
        let Some(profile) = self
            .library
            .profiles()
            .iter()
            .find(|profile| profile.guid() == guid)
        else {
            return vec![];
        };
        // If the real stage observation has not arrived, preserve an explicit
        // missing producer instead of inventing a stage list or successful write.
        if self.active == 0 || self.active > self.stages.len() {
            return vec![];
        }
        let mut stages = self.stages.clone();
        let Some(dpi) = profile.dpi() else {
            return vec![];
        };
        stages[self.active - 1] = dpi.clone();
        let stages = stages
            .iter()
            .map(|row| {
                SourceStage::from_source_file(&row.file_value())
                    .expect("typed source stage JSON projection")
            })
            .collect();
        vec![
            MatcherAction::SetStageValueX {
                stages,
                active: self.active,
            },
            MatcherAction::SelectProfile(guid.into()),
        ]
    }
    pub fn toggle_actions(&mut self) {
        self.actions_open = !self.actions_open;
    }
    pub fn begin_rename(&mut self, none: &str) -> Option<String> {
        let (dataset, index) = self.dataset(none);
        let name = &dataset.get(index)?.name;
        if name.is_empty() {
            return None;
        }
        self.rename = Some(name.clone());
        self.actions_open = false;
        Some(name.clone())
    }
    pub fn commit_rename(&mut self, name: &str) -> Vec<MatcherAction> {
        self.rename = None;
        let name = razer_device::source_string::trim(name);
        if name.is_empty()
            || matches!(name, "none" | "None")
            || self
                .library
                .profiles()
                .iter()
                .any(|profile| profile.name().text() == Some(name))
        {
            return vec![];
        }
        vec![MatcherAction::RenameProfile(name.into())]
    }
    pub fn escape_rename(&mut self) -> Vec<MatcherAction> {
        let original = self.rename.clone().unwrap_or_default();
        self.commit_rename(&original)
    }
    pub fn delete_profile(&mut self) -> Vec<MatcherAction> {
        self.actions_open = false;
        vec![MatcherAction::Command(MatcherCommand::DeleteProfile(
            self.selected.clone(),
        ))]
    }
    pub fn open_delete_all(&mut self) {
        self.ignore_next_document_click = true;
        self.delete_all_open = true;
        self.actions_open = false;
    }
    pub fn close_delete_all(&mut self) {
        self.delete_all_open = false;
    }
    pub fn delete_all(&mut self) -> Vec<MatcherAction> {
        self.delete_all_open = false;
        vec![MatcherAction::Command(MatcherCommand::DeleteAllProfiles)]
    }
    pub fn document_click(&mut self, inside_actions_or_confirmation: bool) {
        if self.ignore_next_document_click {
            self.ignore_next_document_click = false;
            return;
        }
        if !inside_actions_or_confirmation {
            self.actions_open = false;
            self.delete_all_open = false;
        }
    }
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("mouse_dpi_matcher_state_tests.rs");

    fn observation(phase: Phase) -> MatcherObservation {
        MatcherObservation::Calibration(Calibration::new(phase, 0., 800., 0.))
    }

    #[::core::prelude::v1::test]
    fn matcher_start_is_one_click_after_open_and_cancel_uses_the_object_path() {
        let mut state = MatcherState::default();
        state.open();
        assert!(state.window_click(true).is_empty());
        assert_eq!(
            state.window_click(false),
            vec![MatcherAction::Command(MatcherCommand::Start)]
        );
        assert!(state.window_click(false).is_empty());
        state.observe(observation(Phase::Calibrating));
        assert_eq!(
            state.cancel(),
            vec![MatcherAction::Command(MatcherCommand::Cancel)]
        );
        assert!(!state.popup());
        assert!(state.window_click(false).is_empty());
    }
    #[::core::prelude::v1::test]
    fn matcher_first_error_closes_but_later_error_renders_and_apply_requires_completion() {
        let mut state = MatcherState::default();
        assert_eq!(
            state.observe(observation(Phase::Error)),
            vec![MatcherAction::Command(MatcherCommand::Reset)]
        );
        assert!(!state.popup());
        state.observe(observation(Phase::Ready));
        state.observe(observation(Phase::Calibrating));
        state.observe(observation(Phase::Error));
        assert!(state.popup());
        assert!(state.apply().is_empty());
        state.observe(observation(Phase::Completed));
        assert_eq!(
            state.apply(),
            vec![MatcherAction::Command(MatcherCommand::Confirm)]
        );
    }
    #[::core::prelude::v1::test]
    fn matcher_dataset_select_and_rename_use_real_source_values() {
        let mut state = MatcherState::default();
        state.observe(MatcherObservation::Profiles {
            library: library(vec![
                json!({"guid":"p","name":{"en":"Mouse","zh-cn":"鼠标"},"dpi":{"x":900,"y":800}}),
            ]),
            selected: String::new(),
        });
        let (items, selected) = state.dataset("None");
        assert_eq!(selected, 0);
        assert!(items[0].disabled());
        state.observe(MatcherObservation::Stages {
            stages: stages(vec![json!({"x":400})]),
            active: 1,
        });
        assert_eq!(
            state.choose("p"),
            vec![
                MatcherAction::SetStageValueX {
                    stages: stages(vec![json!({"x":900,"y":800})]),
                    active: 1
                },
                MatcherAction::SelectProfile("p".into())
            ]
        );
        assert!(state.commit_rename(" None ").is_empty());
        assert_eq!(
            state.commit_rename(" NONE "),
            vec![MatcherAction::RenameProfile("NONE".into())]
        );
    }
}
