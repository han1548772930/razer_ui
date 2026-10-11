//! Current PID190 dS/AS state and action projection; no service/device simulation.
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
    SetStageValueX { stages: Vec<Value>, active: usize },
    SelectProfile(String),
    RenameProfile(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatcherObservation {
    Calibration(Value),
    Profiles { profiles: Vec<Value>, selected: String },
    Stages { stages: Vec<Value>, active: usize },
    Devices(Vec<Value>),
    Language(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileItem {
    guid: String,
    name: String,
    disabled: bool,
    dpi: Value,
}
impl ProfileItem {
    pub fn guid(&self) -> &str { &self.guid }
    pub fn name(&self) -> &str { &self.name }
    pub fn disabled(&self) -> bool { self.disabled }
}

#[derive(Debug, Clone)]
pub struct MatcherState {
    calibration: Value,
    profiles: Vec<Value>,
    selected: String,
    stages: Vec<Value>,
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
            calibration: json!({"state":"idle","process":0,"newDpi":0}),
            // No external profile or synthetic calibrated DPI is fabricated.
            // Empty observations produce AS's disabled localized none sentinel.
            profiles: vec![], selected:String::new(), stages:vec![], active:0,
            devices:vec![],language:"en".into(),popup:false,start_listener:false,
            first_load:false,actions_open:false,delete_all_open:false,
            ignore_next_document_click:false,rename:None,
        }
    }
}
impl MatcherState {
    pub fn phase(&self) -> Option<&str> { self.calibration["state"].as_str() }
    pub fn popup(&self) -> bool { self.popup }
    pub fn actions_open(&self) -> bool { self.actions_open }
    pub fn delete_all_open(&self) -> bool { self.delete_all_open }
    pub fn rename(&self) -> Option<&str> { self.rename.as_deref() }
    pub fn selected(&self) -> &str { &self.selected }
    pub fn process(&self) -> f32 { self.calibration["process"].as_f64().unwrap_or(0.) as f32 }
    pub fn current_dpi(&self) -> &Value { &self.calibration["currentDpi"] }
    pub fn new_dpi(&self) -> &Value { &self.calibration["newDpi"] }
    pub fn step_active(&self, step: usize) -> bool {
        matches!((step,self.phase()),(1,Some("ready"|"calibrating"|"completed"))|(2,Some("calibrating"|"completed"))|(3,Some("completed")))
    }
    pub fn mount(&mut self) -> Vec<MatcherAction> { vec![MatcherAction::Command(MatcherCommand::GetState)] }
    pub fn unmount(&mut self) { self.start_listener=false; }
    pub fn observe(&mut self, observation: MatcherObservation) -> Vec<MatcherAction> {
        match observation {
            MatcherObservation::Calibration(value) => {
                let changed=value.get("state")!=self.calibration.get("state");
                self.calibration=value;
                if changed && self.phase()!=Some("ready") {
                    self.popup=true;
                    let first_error=self.phase()==Some("error")&&!self.first_load;
                    // React setState is asynchronous in this handler: the error
                    // comparison sees the previous firstLoad before it becomes 1.
                    self.first_load=true;
                    if first_error { return self.close(); }
                }
            }
            MatcherObservation::Profiles {profiles,selected} => {self.profiles=profiles;self.selected=selected;}
            MatcherObservation::Stages {stages,active} => {self.stages=stages;self.active=active;}
            MatcherObservation::Devices(devices) => self.devices=devices,
            MatcherObservation::Language(language) => self.language=language.to_lowercase(),
        }
        vec![]
    }
    pub fn open(&mut self) { self.popup=true;self.start_listener=true; }
    /// Call only for the browser-equivalent click phase. Source Calibrate,
    /// close/cancel/reset stop propagation and must not be passed here.
    pub fn window_click(&mut self, inside_calibrate: bool) -> Vec<MatcherAction> {
        if self.start_listener&&self.popup&&!inside_calibrate {
            self.start_listener=false;
            vec![MatcherAction::Command(MatcherCommand::Start)]
        } else {vec![]}
    }
    pub fn close(&mut self) -> Vec<MatcherAction> {
        self.popup=false;self.start_listener=false;
        vec![MatcherAction::Command(if self.phase()==Some("calibrating") {MatcherCommand::Cancel}else{MatcherCommand::Reset})]
    }
    pub fn cancel(&mut self) -> Vec<MatcherAction> {
        self.start_listener=true;
        // Literal dS.cancel compares the entire prop to a string, unlike close.
        if self.calibration=="calibrating" {vec![MatcherAction::Command(MatcherCommand::Cancel)]}else{self.close()}
    }
    pub fn reset(&mut self) -> Vec<MatcherAction> {
        self.open();vec![MatcherAction::Command(MatcherCommand::Reset)]
    }
    pub fn apply(&mut self) -> Vec<MatcherAction> {
        if self.phase()!=Some("completed") {return vec![];}
        self.popup=false;
        vec![MatcherAction::Command(MatcherCommand::Confirm)]
    }
    pub fn dataset(&self, none: &str) -> (Vec<ProfileItem>,usize) {
        let mut dataset:Vec<_>=self.profiles.iter().map(|profile| {
            let mut name=localized(&profile["name"],&self.language);
            if let Some(device)=self.devices.iter().find(|device|device.get("guid")==profile.get("guid"))
                && truthy(&device["productName"])
            {
                let product_name=localized(&device["productName"],&self.language);
                if !product_name.is_empty() {name=product_name;}
            }
            ProfileItem {guid:profile["guid"].as_str().unwrap_or("").into(),name,disabled:truthy(&profile["disabled"]),dpi:profile["dpi"].clone()}
        }).collect();
        if self.selected.is_empty()||!self.profiles.iter().any(|profile|profile["guid"]==self.selected) {
            dataset.insert(0,ProfileItem{guid:"none".into(),name:none.into(),disabled:true,dpi:Value::Null});
        }
        let selected=dataset.iter().position(|item|item.guid==self.selected).unwrap_or(0);
        (dataset,selected)
    }
    pub fn choose(&self, guid:&str) -> Vec<MatcherAction> {
        if guid=="none"||guid==self.selected {return vec![];}
        let Some(profile)=self.profiles.iter().find(|profile|profile["guid"]==guid)else{return vec![];};
        // If the real stage observation has not arrived, preserve an explicit
        // missing producer instead of inventing a stage list or successful write.
        if self.active==0||self.active>self.stages.len() {return vec![];}
        let mut stages=self.stages.clone();stages[self.active-1]=profile["dpi"].clone();
        vec![MatcherAction::SetStageValueX{stages,active:self.active},MatcherAction::SelectProfile(guid.into())]
    }
    pub fn toggle_actions(&mut self) {self.actions_open=!self.actions_open;}
    pub fn begin_rename(&mut self,none:&str) -> Option<String> {
        let (dataset,index)=self.dataset(none);let name=&dataset.get(index)?.name;
        if name.is_empty(){return None;}
        self.rename=Some(name.clone());self.actions_open=false;Some(name.clone())
    }
    pub fn commit_rename(&mut self,name:&str) -> Vec<MatcherAction> {
        self.rename=None;
        let name=name.trim();
        if name.is_empty()||name.chars().all(char::is_whitespace)||matches!(name,"none"|"None")
            ||self.profiles.iter().any(|profile|profile["name"]==name)
        {return vec![];}
        vec![MatcherAction::RenameProfile(name.into())]
    }
    pub fn escape_rename(&mut self) -> Vec<MatcherAction> {
        let original=self.rename.clone().unwrap_or_default();self.commit_rename(&original)
    }
    pub fn delete_profile(&mut self) -> Vec<MatcherAction> {
        self.actions_open=false;
        vec![MatcherAction::Command(MatcherCommand::DeleteProfile(self.selected.clone()))]
    }
    pub fn open_delete_all(&mut self) {self.ignore_next_document_click=true;self.delete_all_open=true;self.actions_open=false;}
    pub fn close_delete_all(&mut self) {self.delete_all_open=false;}
    pub fn delete_all(&mut self) -> Vec<MatcherAction> {
        self.delete_all_open=false;vec![MatcherAction::Command(MatcherCommand::DeleteAllProfiles)]
    }
    pub fn document_click(&mut self, inside_actions_or_confirmation:bool) {
        if self.ignore_next_document_click {self.ignore_next_document_click=false;return;}
        if !inside_actions_or_confirmation {self.actions_open=false;self.delete_all_open=false;}
    }
}
fn localized(value:&Value,language:&str)->String {
    if let Some(value)=value.as_str(){return value.into();}
    value.get(language).filter(|value|truthy(value)).or_else(||value.get("en")).and_then(Value::as_str).unwrap_or("").into()
}
fn truthy(value:&Value)->bool {match value{Value::Null=>false,Value::Bool(value)=>*value,Value::Number(value)=>value.as_f64().is_some_and(|value|value!=0.),Value::String(value)=>!value.is_empty(),_=>true}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn start_is_one_click_after_open_and_close_preserves_object_comparison() {
        let mut state=MatcherState::default();state.open();
        assert!(state.window_click(true).is_empty());
        assert_eq!(state.window_click(false),vec![MatcherAction::Command(MatcherCommand::Start)]);
        assert!(state.window_click(false).is_empty());
        state.observe(MatcherObservation::Calibration(json!({"state":"calibrating"})));
        assert_eq!(state.cancel(),vec![MatcherAction::Command(MatcherCommand::Cancel)]);assert!(!state.popup());
        state.observe(MatcherObservation::Calibration(json!("calibrating")));state.open();
        assert_eq!(state.cancel(),vec![MatcherAction::Command(MatcherCommand::Cancel)]);assert!(state.popup());
    }
    #[test]
    fn first_error_closes_but_later_error_renders_and_apply_requires_completion() {
        let mut state=MatcherState::default();
        assert_eq!(state.observe(MatcherObservation::Calibration(json!({"state":"error"}))),vec![MatcherAction::Command(MatcherCommand::Reset)]);
        assert!(!state.popup());state.observe(MatcherObservation::Calibration(json!({"state":"ready"})));
        state.observe(MatcherObservation::Calibration(json!({"state":"calibrating"})));
        state.observe(MatcherObservation::Calibration(json!({"state":"error"})));assert!(state.popup());assert!(state.apply().is_empty());
        state.observe(MatcherObservation::Calibration(json!({"state":"completed"})));
        assert_eq!(state.apply(),vec![MatcherAction::Command(MatcherCommand::Confirm)]);
    }
    #[test]
    fn dataset_select_and_rename_use_real_source_values() {
        let mut state=MatcherState::default();
        state.observe(MatcherObservation::Profiles{profiles:vec![json!({"guid":"p","name":{"en":"Mouse","zh-cn":"鼠标"},"dpi":{"x":900,"y":800}})],selected:String::new()});
        let (items,selected)=state.dataset("None");assert_eq!(selected,0);assert!(items[0].disabled());
        state.observe(MatcherObservation::Stages{stages:vec![json!({"x":400})],active:1});
        assert_eq!(state.choose("p"),vec![MatcherAction::SetStageValueX{stages:vec![json!({"x":900,"y":800})],active:1},MatcherAction::SelectProfile("p".into())]);
        assert!(state.commit_rename(" None ").is_empty());
        assert_eq!(state.commit_rename(" NONE "),vec![MatcherAction::RenameProfile("NONE".into())]);
    }
}
