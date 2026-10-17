//! Local Chroma storage/host boundary for the mounted Kitsune page. Absence
//! of an installation/profile observation never becomes a successful result.
use super::{GamepadProductWorkspace, GamepadProductChanged};
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{cell::{Cell, RefCell}, collections::BTreeMap, sync::{Arc, atomic::{AtomicBool,Ordering}}};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ControllerChromaProfile { id:String, name:String }
impl ControllerChromaProfile {
    pub fn new(id:impl Into<String>,name:impl Into<String>) -> anyhow::Result<Self> {
        let profile=Self {id:id.into(),name:name.into()};
        anyhow::ensure!(!profile.id.is_empty(),"Chroma profile identity is empty");
        Ok(profile)
    }
    pub fn id(&self)->&str { &self.id }
    pub fn name(&self)->&str { &self.name }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ControllerSwitchOffLighting { display_off:bool, idle_enabled:bool, idle_minutes:u8 }
impl ControllerSwitchOffLighting {
    pub fn new(display_off:bool,idle_enabled:bool,idle_minutes:u8)->anyhow::Result<Self> {
        anyhow::ensure!((1..=15).contains(&idle_minutes),"Idle lighting minutes exceed the source control range");
        Ok(Self {display_off,idle_enabled,idle_minutes})
    }
    pub fn display_off(self)->bool {self.display_off}
    pub fn idle_enabled(self)->bool {self.idle_enabled}
    pub fn idle_minutes(self)->u8 {self.idle_minutes}
}

#[derive(Clone, Debug)]
pub enum ControllerChromaCommand {
    Observe,
    Toggle { enabled:bool },
    SelectProfile { id:String },
    /// Exact current ON_SYNC_QUICK_EFFECT setting envelope, at the service boundary.
    SyncQuick { source_effect:Value },
    SwitchOffLighting(ControllerSwitchOffLighting),
    CustomColors(razer_state::CustomColorSlots),
    LaunchStudio,
    OpenSettings,
    ManageApps,
}

#[derive(Clone, Debug, Default)]
pub struct ControllerChromaObservation {
    fully_installed:Option<bool>,
    module_installed:Option<bool>,
    profiles:Option<Vec<ControllerChromaProfile>>,
    selected_profile:Option<String>,
    enabled:Option<bool>,
    connected_products:Option<Vec<u32>>,
    visualizer_enabled:Option<bool>,
    app_name:Option<String>,
    app_icon:Option<String>,
    dynamic_lighting:Option<bool>,
    power_saving_quick_effect:Option<bool>,
    on_battery:Option<bool>,
}
impl ControllerChromaObservation {
    pub fn new()->Self { Self::default() }
    pub fn with_fully_installed(mut self,installed:bool)->Self {self.fully_installed=Some(installed);self}
    pub fn with_module_installed(mut self,installed:bool)->Self {self.module_installed=Some(installed);self}
    pub fn with_installation(mut self,fully_installed:bool,module_installed:bool)->Self {self.fully_installed=Some(fully_installed);self.module_installed=Some(module_installed);self}
    pub fn with_profiles(mut self,profiles:Vec<ControllerChromaProfile>,selected:Option<String>)->anyhow::Result<Self> {
        let mut ids=std::collections::BTreeSet::new();
        anyhow::ensure!(profiles.iter().all(|p|!p.id.is_empty()&&ids.insert(&p.id)),"Invalid or duplicate observed Chroma profiles");
        self.profiles=Some(profiles);self.selected_profile=selected;Ok(self)
    }
    pub fn with_enabled(mut self,enabled:bool)->Self {self.enabled=Some(enabled);self}
    pub fn with_connected_products(mut self,products:Vec<u32>)->Self {self.connected_products=Some(products);self}
    pub fn with_visualizer(mut self,enabled:bool,app_name:Option<String>,app_icon:Option<String>)->Self {self.visualizer_enabled=Some(enabled);self.app_name=app_name;self.app_icon=app_icon;self}
    pub fn with_dynamic_lighting(mut self,enabled:bool)->Self {self.dynamic_lighting=Some(enabled);self}
    pub fn with_power_state(mut self,power_saving_quick_effect:bool,on_battery:bool)->Self {self.power_saving_quick_effect=Some(power_saving_quick_effect);self.on_battery=Some(on_battery);self}
    pub fn fully_installed(&self)->Option<bool> {self.fully_installed}
    pub fn module_installed(&self)->Option<bool> {self.module_installed}
    pub fn profiles(&self)->Option<&[ControllerChromaProfile]> {self.profiles.as_deref()}
    pub fn selected_profile(&self)->Option<&str> {self.selected_profile.as_deref()}
    pub(super) fn select_profile(&mut self,id:&str)->bool {
        if !self.profiles.as_ref().is_some_and(|profiles|profiles.iter().any(|profile|profile.id==id)) {return false;}
        self.selected_profile=Some(id.to_owned());true
    }
    pub fn enabled(&self)->Option<bool> {self.enabled}
    pub fn connected_products(&self)->Option<&[u32]> {self.connected_products.as_deref()}
    pub fn visualizer_enabled(&self)->Option<bool> {self.visualizer_enabled}
    pub fn app_name(&self)->Option<&str> {self.app_name.as_deref()}
    pub fn app_icon(&self)->Option<&str> {self.app_icon.as_deref()}
    pub fn dynamic_lighting(&self)->Option<bool> {self.dynamic_lighting}
    pub(super) fn quick_disabled(&self)->bool {self.visualizer_enabled==Some(true)||(self.power_saving_quick_effect==Some(true)&&self.on_battery==Some(true))}
    fn merge(&mut self,next:Self) {
        if next.fully_installed.is_some(){self.fully_installed=next.fully_installed;}
        if next.module_installed.is_some(){self.module_installed=next.module_installed;}
        if next.profiles.is_some(){self.profiles=next.profiles;self.selected_profile=next.selected_profile;}
        if next.enabled.is_some(){self.enabled=next.enabled;}
        if next.connected_products.is_some(){self.connected_products=next.connected_products;}
        if next.visualizer_enabled.is_some(){self.visualizer_enabled=next.visualizer_enabled;self.app_name=next.app_name;self.app_icon=next.app_icon;}
        if next.dynamic_lighting.is_some(){self.dynamic_lighting=next.dynamic_lighting;}
        if next.power_saving_quick_effect.is_some(){self.power_saving_quick_effect=next.power_saving_quick_effect;}
        if next.on_battery.is_some(){self.on_battery=next.on_battery;}
    }
}

#[derive(Clone)]
pub struct ControllerChromaRequested {request_id:u64,command:ControllerChromaCommand,guard:Arc<AtomicBool>}
impl ControllerChromaRequested {
    pub fn request_id(&self)->u64 {self.request_id}
    pub fn command(&self)->&ControllerChromaCommand {&self.command}
    pub fn guard(&self)->Arc<AtomicBool> {Arc::clone(&self.guard)}
    pub fn is_valid(&self)->bool {self.guard.load(Ordering::Acquire)}
}
impl EventEmitter<ControllerChromaRequested> for GamepadProductWorkspace {}

pub(super) struct ChromaRuntime {
    guard:Arc<AtomicBool>,next:Cell<u64>,last_mutation:Cell<u64>,pending:RefCell<BTreeMap<u64,ControllerChromaCommand>>,
    pub(super) observation:ControllerChromaObservation,
    pub(super) advanced:bool,
    pub(super) sync_started:Option<std::time::Instant>,
    pub(super) sync_generation:u64,
    pub(super) last_error:Option<String>,
}
impl Default for ChromaRuntime {
    fn default()->Self {Self {guard:Arc::new(AtomicBool::new(true)),next:Cell::new(0),last_mutation:Cell::new(0),pending:RefCell::default(),observation:Default::default(),advanced:false,sync_started:None,sync_generation:0,last_error:None}}
}
impl ChromaRuntime {
    pub(super) fn invalidate(&mut self) {self.guard.store(false,Ordering::Release);self.guard=Arc::new(AtomicBool::new(true));self.pending.borrow_mut().clear();self.observation=Default::default();self.advanced=false;self.sync_started=None;self.last_error=None;}
    fn accepts(&self,request:&ControllerChromaRequested)->bool {request.is_valid()&&Arc::ptr_eq(&self.guard,&request.guard)&&self.pending.borrow().contains_key(&request.request_id)}
}
impl Drop for ChromaRuntime {fn drop(&mut self){self.guard.store(false,Ordering::Release);}}

impl GamepadProductWorkspace {
    pub fn accepts_controller_chroma(&self,request:&ControllerChromaRequested)->bool {self.chroma_runtime.accepts(request)}
    pub fn refresh_controller_chroma(&self,cx:&mut Context<Self>) {if self.spec.product_id==4115 {self.request_controller_chroma(ControllerChromaCommand::Observe,cx);}}
    pub(super) fn request_controller_chroma(&self,command:ControllerChromaCommand,cx:&mut Context<Self>) {
        let request_id=self.chroma_runtime.next.get().wrapping_add(1);self.chroma_runtime.next.set(request_id);
        if !matches!(command,ControllerChromaCommand::Observe) {self.chroma_runtime.last_mutation.set(request_id);}
        self.chroma_runtime.pending.borrow_mut().insert(request_id,command.clone());
        cx.emit(ControllerChromaRequested {request_id,command,guard:Arc::clone(&self.chroma_runtime.guard)});
    }
    pub fn complete_controller_chroma(&mut self,request:&ControllerChromaRequested,result:Result<Option<ControllerChromaObservation>,String>,window:&mut Window,cx:&mut Context<Self>)->bool {
        if !self.chroma_runtime.accepts(request){return false;}
        self.chroma_runtime.pending.borrow_mut().remove(&request.request_id);
        match result {
            Ok(observation)=>{self.chroma_runtime.last_error=None;if request.request_id>=self.chroma_runtime.last_mutation.get() {if let Some(observation)=observation {self.observe_controller_chroma(observation,window,cx);}}},
            Err(error)=>{eprintln!("[controller-chroma] {error}");self.chroma_runtime.last_error=Some(error);},
        }
        cx.notify();true
    }
    pub fn observe_controller_chroma(&mut self,observation:ControllerChromaObservation,window:&mut Window,cx:&mut Context<Self>) {
        if let Some(enabled)=observation.enabled {self.chroma_runtime.advanced=enabled;}
        self.chroma_runtime.observation.merge(observation);
        self.sync_quick_lighting_controls(window,cx);
        cx.notify();
    }
    pub(super) fn controller_switch_off_changed(&mut self,cx:&mut Context<Self>) {
        #[derive(Deserialize)]
        #[serde(rename_all="camelCase")]
        struct SourceSwitch {is_display_on:bool,is_idle_enabled:bool,idle_minutes:u8}
        let settings=(||->anyhow::Result<_>{let s:SourceSwitch=serde_json::from_value(self.draft["profile"]["switchOffLighting"].clone())?;ControllerSwitchOffLighting::new(s.is_display_on,s.is_idle_enabled,s.idle_minutes)})();
        match settings {Ok(settings)=>self.request_controller_chroma(ControllerChromaCommand::SwitchOffLighting(settings),cx),Err(error)=>eprintln!("[controller-chroma] {error:#}")}
    }
    pub(super) fn controller_advanced_changed(&mut self,advanced:bool,window:&mut Window,cx:&mut Context<Self>) {
        self.dismiss_quick_lighting_controls(cx);
        self.chroma_runtime.advanced=advanced;
        self.request_controller_chroma(ControllerChromaCommand::Toggle {enabled:advanced},cx);
        if !advanced {self.submit_quick_lighting(cx);}
        self.sync_quick_lighting_controls(window,cx);
        cx.emit(GamepadProductChanged);cx.notify();
    }
}
