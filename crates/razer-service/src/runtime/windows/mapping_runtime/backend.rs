//! Shared mapping actor: publication, connected driver registrations and capture.
use super::{State, capture, connections::{self, Lease}, device, driver, identity};
use anyhow::{Context, Result, ensure};
use razer_device::{mapping_native_action::device_tables,mapping_native_mode::{ModeState,DeviceInfo}};
use razer_ipc::{ServiceRequest, macro_recording::ActionReply, mapping_control};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet,VecDeque},sync::{Arc,Mutex,mpsc}};
use uuid::Uuid;

pub(super) struct ExecutionContext {
    pub caller:Uuid,
    pub info:identity::DeviceInfo,
    pub route:driver::Route,
    pub cancellation:Arc<super::Cancellation>,
    pub macros:Arc<Mutex<State>>,
    pub modes:Arc<Mutex<ModeState>>,
    pub notifications:mpsc::SyncSender<Notification>,
}
pub(super) struct Notification {pub caller:Uuid,pub input:String,pub output:String,pub tick:u64,pub global:bool}
struct Registration {scope:Uuid,caller:Uuid,target:identity::Target,worker:device::Owner,document:Option<Value>,pending:VecDeque<Value>,failure:Option<String>,revision:Option<u64>,input_registered:bool,modes_removed:bool}
struct Global {scope:Uuid,owner:String,revision:Option<u64>,execution_revision:Option<u64>,engine:Option<Value>,pending:VecDeque<razer_ipc::shortcuts::UnsupportedMapping>,failure:Option<String>}
pub(super) struct Core<E:device::Executor> {
    factory:fn(ExecutionContext)->Result<E>,
    macros:Arc<Mutex<State>>,
    modes:Arc<Mutex<ModeState>>,
    devices:BTreeMap<String,Registration>,
    connections:connections::Registry,
    notification_sender:mpsc::SyncSender<Notification>,
    notifications:mpsc::Receiver<Notification>,
    global:Option<Global>,
    mapping_suppression:BTreeSet<Uuid>,
    shortcut_suppression:BTreeSet<Uuid>,
    mode_subscribers:BTreeSet<Uuid>,
    hotkeys:razer_platform::global_shortcuts::GlobalShortcuts,
    hotkey_scopes:BTreeMap<(u32,u32),BTreeSet<Uuid>>,
    hotkey_events:BTreeMap<Uuid,VecDeque<Value>>,
    hotkey_failures:BTreeMap<Uuid,String>,
    scopes:BTreeSet<Uuid>,
}
pub(super) struct Backend<E:device::Executor> {core:Core<E>,capture:capture::State}
impl<E:device::Executor> Backend<E> {
    pub fn new(factory:fn(ExecutionContext)->Result<E>)->Self {
        let (notification_sender,notifications)=mpsc::sync_channel(512);
        Self {core:Core {factory,macros:Arc::new(Mutex::new(State::new())),modes:Arc::new(Mutex::new(ModeState::default())),
            devices:BTreeMap::new(),connections:connections::Registry::default(),notification_sender,notifications,
            global:None,mapping_suppression:BTreeSet::new(),shortcut_suppression:BTreeSet::new(),mode_subscribers:BTreeSet::new(),
            hotkeys:Default::default(),hotkey_scopes:BTreeMap::new(),hotkey_events:BTreeMap::new(),hotkey_failures:BTreeMap::new(),scopes:BTreeSet::new()},capture:Default::default()}
    }
}
impl<E:device::Executor> Core<E> {
    fn start_device(&mut self,scope:Uuid,product:u32,container:&str,owner:String)->Result<Value> {
        ensure!(!self.devices.contains_key(&owner),"Mapping device owner is already registered");
        let target=identity::Target::observe(product,container,&owner)?;
        let caller=Uuid::new_v4();
        let route=self.connections.device(scope,caller,&target)?;
        let info=target.info.clone();
        let mode_info=DeviceInfo{guid:info.guid.clone(),vendor_id:info.vendor_id.try_into()?,product_id:info.product_id.try_into()?,
            container_id:info.container_id.clone(),edition_id:0,manufacturer_name:String::new(),product_name:String::new(),serial_number:String::new()};
        // DB558/DB630 retain empty optional descriptor strings and edition0
        // when addUsbDevice supplied only physical registration identity.
        self.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.register_device(caller,mode_info)?;
        let macros=self.macros.clone();let modes=self.modes.clone();let worker_modes=modes.clone();
        let notifications=self.notification_sender.clone();let factory=self.factory;
        let result=device::Owner::open(route,caller,worker_modes,move|route,cancellation| factory(ExecutionContext{caller,info,route,cancellation,macros,modes,notifications}));
        let worker=match result {Ok(worker)=>worker,Err(error)=>{
            let release=self.connections.release(&Lease::Device{scope,caller});
            let tick=razer_platform::event_clock::tick()?;
            let modes=self.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.remove_device(caller,tick);
            anyhow::bail!("Device execution initialization: {error:#}; driver cleanup: {release:?}; modes: {modes:?}");
        }};
        self.devices.insert(owner.clone(),Registration{scope,caller,target,worker,document:None,pending:VecDeque::new(),failure:None,revision:None,input_registered:false,modes_removed:false});
        let current=self.devices.get(&owner).unwrap();
        current.worker.mapping(self.mapping_suppression.is_empty())?;
        current.worker.shortcuts(self.shortcut_suppression.is_empty())?;
        Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner}))
    }
    fn registration(&self,scope:Uuid,owner:&str)->Result<&Registration> {
        let row=self.devices.get(owner).context("Mapping device owner absent")?;
        ensure!(row.scope==scope,"Mapping registration belongs to another client");Ok(row)
    }
    fn publish(&mut self,scope:Uuid,owner:&str,document:Value)->Result<Value> {
        let registration=self.registration(scope,owner)?;
        registration.target.revalidate()?;
        ensure!(document.get("productId").and_then(Value::as_u64)==Some(u64::from(registration.target.logical_product.product_id())),"Mapping document belongs to another product");
        ensure!(document.get("_macro_compilation_error").is_none(),"Local macro draft has compilation errors");
        let tables=device_tables(&document,self.global.as_ref().and_then(|global|global.engine.as_ref()))?;
        for(field,key) in [("defaultTurbos","synapseDefaultTurbos"),("synapseTurbos","synapseTurbos"),("macros","synapseMacros"),("phasedMacros","synapsePhasedMacros")] {
            if let Some(value)=document.get(field) {self.publish_storage_value(key,value)?;}
            else if matches!(field,"defaultTurbos"|"synapseTurbos") {anyhow::bail!("Missing compiled {field} list");}
        }
        let row=self.devices.get_mut(owner).unwrap();
        row.worker.publish(tables)?;
        let active=document.get("activeProfile").and_then(Value::as_str).context("Mapping active profile missing")?.to_owned();
        row.document=Some(document);row.target.revalidate()?;
        Ok(json!({"published":true,"owner_guid":owner,"active_profile":active}))
    }
    fn publish_storage_value(&mut self,key:&str,value:&Value)->Result<()> {
        let mut macros=self.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?;
        macros.publish_storage(key,value)
    }
    fn stop_device(&mut self,scope:Uuid,owner:&str)->Result<Value> {
        self.registration(scope,owner)?;
        let row=self.devices.get_mut(owner).unwrap();
        // Keep the registration available for repeated cleanup after a failure.
        row.worker.stop()?;
        self.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?.drain_caller(row.caller)?;
        if !row.modes_removed {
            let tick=razer_platform::event_clock::tick()?;
            self.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.remove_device(row.caller,tick)?;
            row.modes_removed=true;
        }
        self.connections.release(&Lease::Device{scope,caller:row.caller})?;
        self.devices.remove(owner);
        Ok(json!({"removed":true,"owner_guid":owner}))
    }
    fn mapping(&mut self,scope:Uuid,enabled:bool)->Result<ActionReply> {
        if enabled {self.mapping_suppression.remove(&scope);} else {self.mapping_suppression.insert(scope);}
        let effective=self.mapping_suppression.is_empty();let mut errors=Vec::new();
        for row in self.devices.values_mut() {
            if let Err(error)=row.worker.mapping(effective) {errors.push(format!("{}: {error:#}",row.target.info.guid));}
            if !effective {if let Err(error)=self.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?.drain_caller(row.caller){errors.push(format!("{error:#}"));}}
        }
        Ok(ActionReply::new(errors.is_empty(),errors.join("; ")))
    }
    fn shortcuts(&mut self,scope:Uuid,enabled:bool)->Result<ActionReply> {
        if enabled {self.shortcut_suppression.remove(&scope);} else {self.shortcut_suppression.insert(scope);}
        let effective=self.shortcut_suppression.is_empty();let mut errors=Vec::new();
        if let Err(error)=self.hotkeys.enable(effective){errors.push(format!("{error:#}"));}
        for row in self.devices.values_mut(){if let Err(error)=row.worker.shortcuts(effective){errors.push(format!("{error:#}"));}}
        Ok(ActionReply::new(errors.is_empty(),errors.join("; ")))
    }
    fn global_publish(&mut self,scope:Uuid,owner:&str,revision:u64,document:razer_model::shortcuts::CompiledDocument)->Result<Value> {
        let current=self.global.as_mut().context("Global shortcut owner absent")?;
        ensure!(current.scope==scope && current.owner==owner,"Global shortcut owner changed");
        ensure!(current.revision.is_none_or(|old|revision>old),"Stale global shortcut publication");
        document.validate().map_err(anyhow::Error::msg)?;current.revision=Some(revision);
        for(key,values) in [("synapseDefaultTurbos",document.default_turbos()),("synapseTurbos",document.synapse_turbos()),("synapseMacros",document.macros()),("synapsePhasedMacros",document.phased_macros())] {
            self.publish_storage_value(key,&Value::Array(values.to_vec()))?;
        }
        let engine=document.app_engine().clone();
        self.replace_global_engine(Some(engine))?;
        Ok(serde_json::to_value(razer_ipc::shortcuts::PublicationReceipt::new(owner.into(),revision,true))?)
    }
    fn replace_global_engine(&mut self,engine:Option<Value>)->Result<()> {
        let updates=self.devices.iter().filter_map(|(owner,row)|row.document.as_ref().map(|document|(owner,device_tables(document,engine.as_ref())))).map(|(owner,tables)|Ok((owner.clone(),tables?))).collect::<Result<Vec<_>>>()?;
        self.global.as_mut().context("Global shortcut owner absent")?.engine=engine;
        let mut errors=Vec::new();
        for(owner,tables) in updates {if let Err(error)=self.devices[&owner].worker.publish(tables){errors.push(format!("{owner}: {error:#}"));}}
        ensure!(errors.is_empty(),"{}",errors.join("; "));Ok(())
    }
    fn unregister_hotkey(&mut self,scope:Uuid,key:(u32,u32))->Result<()> {
        let scopes=self.hotkey_scopes.get_mut(&key).context("Shortcut is not registered")?;
        ensure!(scopes.contains(&scope),"Shortcut belongs to another client");
        if scopes.len()==1 {self.hotkeys.unregister(key.0,key.1)?;}
        scopes.remove(&scope);
        if scopes.is_empty(){self.hotkey_scopes.remove(&key);}
        Ok(())
    }
    fn poll_hotkeys(&mut self)->Result<()> {
        for event in self.hotkeys.drain()? {
            let shortcut=event.shortcut();
            let Some(scopes)=self.hotkey_scopes.get(&(shortcut.virtual_key,shortcut.modifiers)) else{continue;};
            let value=json!({"eventType":0,"event":json!({"virtualKey":shortcut.virtual_key,"modifiers":shortcut.modifiers,"argument":shortcut.argument}).to_string(),"timeTick":event.time_tick()});
            for scope in scopes {
                let queue=self.hotkey_events.entry(*scope).or_default();
                if queue.len()<512 {queue.push_back(value.clone());}
                else {self.hotkey_failures.insert(*scope,"Shortcut consumer fell behind".into());}
            }
        }
        Ok(())
    }
}
impl<E:device::Executor> capture::Host for Core<E> {
    fn set_mappings(&mut self,scope:Uuid,enabled:bool)->Result<ActionReply>{self.mapping(scope,enabled)}
    fn set_shortcuts(&mut self,scope:Uuid,enabled:bool)->Result<ActionReply>{self.shortcuts(scope,enabled)}
    fn add_target(&mut self,scope:Uuid,target:&capture::Target)->Result<()>{self.connections.capture(scope,target).map(|_|())}
    fn redirect(&mut self,scope:Uuid,target:&capture::Target,kind:razer_device::mapping_native_session::hooks::RedirectKind,enabled:bool)->Result<()> {
        self.connections.redirect(&Lease::Capture{scope,guid:target.guid},kind,enabled)?;
        if enabled && !target.keyboard_only && kind==razer_device::mapping_native_session::hooks::RedirectKind::Keyboard {
            for row in self.devices.values_mut().filter(|row|row.target.info.container_id.eq_ignore_ascii_case(&target.container_id)&&row.target.info.product_id==target.product_id) {
                row.worker.redirect_activated()?;
                self.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?.drain_caller(row.caller)?;
            }
        }
        Ok(())
    }
    fn remove_target(&mut self,scope:Uuid,target:&capture::Target)->Result<()>{self.connections.release(&Lease::Capture{scope,guid:target.guid})}
    fn subscribe_modes(&mut self,scope:Uuid,enabled:bool)->Result<()>{if enabled{self.mode_subscribers.insert(scope);}else{self.mode_subscribers.remove(&scope);}Ok(())}
    fn global_mode(&mut self)->Result<Value>{Ok(self.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.global_mode_json())}
    fn publish_storage(&mut self,_scope:Uuid,key:&str,value:&Value)->Result<()>{self.publish_storage_value(key,value)}
}

impl Registration {
    fn push(&mut self,value:Value) {
        if self.pending.len()<512 {self.pending.push_back(value);} else {self.failure.get_or_insert_with(||"Mapping event queue overflowed".into());}
    }
}
impl<E:device::Executor> crate::mapping::owner::Backend for Backend<E> {
    fn acquire(&mut self,scope:Uuid)->Result<()> {self.core.scopes.insert(scope);Ok(())}
    fn request(&mut self,scope:Uuid,request:ServiceRequest)->Result<Value> {
        if let Some(result)=self.capture.handle(scope,&request,&mut self.core){return result;}
        use ServiceRequest::*;
        match request {
            MappingDeviceStart{product_id,device_container_id,owner_guid}=>self.core.start_device(scope,product_id,&device_container_id,owner_guid),
            MappingDevicePublish{owner_guid,document}=>self.core.publish(scope,&owner_guid,document),
            MappingDeviceStop{owner_guid}=>self.core.stop_device(scope,&owner_guid),
            MappingDeviceEvents{owner_guid}=>{
                self.poll()?;self.core.registration(scope,&owner_guid)?;
                let row=self.core.devices.get_mut(&owner_guid).unwrap();
                if let Some(error)=&row.failure {anyhow::bail!("{error}");}
                Ok(json!({"owner_guid":owner_guid,"events":row.pending.drain(..).collect::<Vec<_>>()}))
            }
            MappingDeviceHypershiftActive{owner_guid}=>{
                self.core.registration(scope,&owner_guid)?.target.revalidate()?;
                let enabled=self.core.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.hypershift();
                Ok(json!({"owner_guid":owner_guid,"isEnabled":enabled}))
            }
            MappingKeyboardGamingIndicator{owner_guid,enabled}=>identity::gaming_indicator(&self.core.registration(scope,&owner_guid)?.target,enabled),
            MappingOtfmInputStart{owner_guid}=>{
                let row=self.core.registration(scope,&owner_guid)?;let caller=row.caller;
                ensure!(!row.input_registered,"OTFM input callback already registered");
                self.core.connections.notifications(&Lease::Device{scope,caller},true)?;
                self.core.devices.get_mut(&owner_guid).unwrap().input_registered=true;
                Ok(json!({"registered":true,"owner_guid":owner_guid}))
            }
            GlobalMode=>capture::Host::global_mode(&mut self.core),
            MacroRecorderStorageWrite{key,value}=>{self.core.publish_storage_value(&key,&value)?;Ok(json!({"key":key,"written":true}))}
            GlobalShortcutStart{owner_guid}=>{
                Uuid::parse_str(&owner_guid)?;ensure!(self.core.global.is_none(),"Global shortcut owner already active");
                self.core.global=Some(Global{scope,owner:owner_guid.clone(),revision:None,execution_revision:None,engine:None,pending:VecDeque::new(),failure:None});
                Ok(serde_json::to_value(razer_ipc::shortcuts::RegistrationReceipt::new(owner_guid,true))?)
            }
            GlobalShortcutPublish{owner_guid,revision,document}=>self.core.global_publish(scope,&owner_guid,revision,document),
            GlobalShortcutPoll{owner_guid}=>{
                self.poll()?;let current=self.core.global.as_mut().context("Global shortcut owner absent")?;
                ensure!(current.scope==scope && current.owner==owner_guid,"Global shortcut owner changed");
                if let Some(error)=&current.failure {anyhow::bail!("{error}");}
                Ok(serde_json::to_value(razer_ipc::shortcuts::EventBatch::new(owner_guid,current.pending.drain(..).collect()))?)
            }
            GlobalShortcutStop{owner_guid}=>{
                let current=self.core.global.as_ref().context("Global shortcut owner absent")?;
                ensure!(current.scope==scope && current.owner==owner_guid,"Global shortcut owner changed");
                self.core.replace_global_engine(None)?;self.core.global=None;
                Ok(serde_json::to_value(razer_ipc::shortcuts::StopReceipt::new(owner_guid,true))?)
            }
            MappingExecution{owner_guid,revision,target}=>{
                if self.core.global.as_ref().is_some_and(|current|current.scope==scope&&current.owner==owner_guid) {
                    let current=self.core.global.as_mut().unwrap();
                    ensure!(current.execution_revision.is_none_or(|old|revision>old),"Stale global execution control");
                    current.execution_revision=Some(revision);
                } else {
                    let current=self.core.registration(scope,&owner_guid)?;
                    ensure!(current.revision.is_none_or(|old|revision>old),"Stale mapping execution control");
                    self.core.devices.get_mut(&owner_guid).unwrap().revision=Some(revision);
                }
                let mut observations=Vec::new();let mut errors=Vec::new();
                let result=self.core.mapping(scope,target.mapping_enabled());
                let operation=if target.mapping_enabled(){mapping_control::Operation::EnableMapping}else{mapping_control::Operation::DisableMapping};
                match result {Ok(reply)=>{if !reply.result(){errors.push(reply.reason().into());}observations.push(mapping_control::Observation::new(operation,reply.result(),reply.reason().into()));},Err(error)=>errors.push(format!("{error:#}"))}
                if let Some(enabled)=target.global_shortcuts_enabled(){
                    let operation=if enabled{mapping_control::Operation::EnableGlobalShortcuts}else{mapping_control::Operation::DisableGlobalShortcuts};
                    match self.core.shortcuts(scope,enabled){Ok(reply)=>{if !reply.result(){errors.push(reply.reason().into());}observations.push(mapping_control::Observation::new(operation,reply.result(),reply.reason().into()));},Err(error)=>errors.push(format!("{error:#}"))}
                }
                ensure!(errors.is_empty(),"{}",errors.join("; "));
                Ok(serde_json::to_value(mapping_control::Receipt::with_observations(owner_guid,revision,target,observations))?)
            }
            EnableGlobalShortcuts{enable}=>{let reply=self.core.shortcuts(scope,enable)?;ensure!(reply.result(),"{}",reply.reason());Ok(json!({"enabled":enable}))}
            RegisterShortcut{vkey_code,modifiers,argument}=>{
                self.core.hotkeys.register(razer_platform::global_shortcuts::Shortcut{virtual_key:vkey_code,modifiers,argument})?;
                self.core.hotkey_scopes.entry((vkey_code,modifiers)).or_default().insert(scope);
                Ok(json!({"registered":true,"vkey_code":vkey_code,"modifiers":modifiers}))
            }
            UnregisterShortcut{vkey_code,modifiers}=>{self.core.unregister_hotkey(scope,(vkey_code,modifiers))?;Ok(json!({"registered":false,"vkey_code":vkey_code,"modifiers":modifiers}))}
            GlobalShortcuts=>Ok(Value::Array(self.core.hotkeys.registered()?.into_iter().filter(|s|self.core.hotkey_scopes.get(&(s.virtual_key,s.modifiers)).is_some_and(|scopes|scopes.contains(&scope))).map(|s|json!({"virtualKey":s.virtual_key,"modifiers":s.modifiers,"argument":s.argument})).collect())),
            ShortcutEvents=>{
                self.core.poll_hotkeys()?;
                if let Some(error)=self.core.hotkey_failures.get(&scope){anyhow::bail!("{error}");}
                Ok(Value::Array(self.core.hotkey_events.entry(scope).or_default().drain(..).collect()))
            }
            Shutdown=>{self.release(scope)?;Ok(json!({"stopped":true}))}
            _=>anyhow::bail!("Request does not belong to the mapping owner"),
        }
    }
    fn poll(&mut self)->Result<()> {
        self.core.poll_hotkeys()?;
        use razer_device::mapping_native_events::{NativeStreamKind,decode_native_stream_packet};
        for input in self.core.connections.inputs() {
            for lease in input.leases {
                match lease {
                    Lease::Device{caller,..}=>if let Some(row)=self.core.devices.values_mut().find(|row|row.caller==caller){
                        match &input.result {Ok(packet)=>if let Err(error)=row.worker.input(packet.clone()){row.failure=Some(format!("{error:#}"));},Err(error)=>{row.failure=Some(error.clone());}}
                    },
                    Lease::Capture{scope,guid}=>match &input.result {
                        Ok(packet)=>match decode_native_stream_packet(&packet.bytes) {
                            Ok(decoded) if decoded.kind==NativeStreamKind::Redirect=>{
                                if matches!(decoded.input,razer_device::mapping_native_events::NativeStreamInput::Mouse(mouse) if mouse.packed_buttons()==0){continue;}
                                if let Some(input)=decoded.input.normalized(false,packet.modifiers).and_then(device::native_lookup){self.capture.input(scope,guid,input.to_native_json(),packet.time_tick);}
                            }
                            Ok(_)=>{},
                            Err(error)=>self.capture.input_failed(scope,guid,error.to_string()),
                        },
                        Err(error)=>self.capture.input_failed(scope,guid,error.clone()),
                    },
                }
            }
        }
        for row in self.core.devices.values_mut() {
            match row.worker.events() {
                Ok(events)=>for event in events {match event {
                    device::Event::Input{input,tick,redirected:false} if row.input_registered=>row.push(json!({"deviceInfo":row.target.info,"deviceEventType":1,"jsonEvent":{"type":"inputnotified","input":input.to_native_string()},"timeTick":tick})),
                    device::Event::Failure(error)=>row.push(json!({"deviceInfo":row.target.info,"deviceEventType":0,"jsonEvent":{"type":"error","error":error},"timeTick":razer_platform::event_clock::tick()?})),
                    _=>{},
                }},
                Err(error)=>row.failure=Some(format!("{error:#}")),
            }
        }
        for event in self.core.notifications.try_iter() {
            if let Some(row)=self.core.devices.values_mut().find(|row|row.caller==event.caller) {
                if event.global {
                    if let Some(global)=self.core.global.as_mut(){
                        if global.pending.len()<512 {global.pending.push_back(razer_ipc::shortcuts::UnsupportedMapping::new(serde_json::to_string(&row.target.info)?,event.input,event.output,event.tick));}
                        else {global.failure=Some("Global shortcut event queue overflowed".into());}
                    }
                } else {row.push(json!({"deviceInfo":row.target.info,"deviceEventType":2,"jsonEvent":{"type":"unsupportedmapping","input":event.input,"output":event.output},"timeTick":event.tick}));}
            }
        }
        let events=self.core.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.take_events();
        for event in events {
            let value=json!({"source":"devicemodechanged","eventType":event.kind(),"event":event.to_native_json().to_string(),"timeTick":event.time_tick()});
            self.capture.mode(value.clone());
            for row in self.core.devices.values_mut(){
                row.push(value.clone());
                if matches!(event,razer_device::mapping_native_mode::Event::Hypershift{enabled:false,..}) {row.worker.hypershift_released()?;}
            }
        }
        let completions=self.core.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?.observed_completions();
        for (id,caller,result) in completions {
            let errors=match result {
                Ok(finished)=>finished.completion.execution_error.into_iter().chain(finished.completion.operation_errors).chain(finished.completion.cleanup_errors).collect::<Vec<_>>(),
                Err(error)=>vec![format!("{error:#}")],
            };
            if !errors.is_empty() {
                if let Some(row)=self.core.devices.values_mut().find(|row|Some(row.caller)==caller) {
                    row.push(json!({"deviceInfo":row.target.info,"deviceEventType":0,"jsonEvent":{"type":"error","error":errors.join("; "),"execution":id},"timeTick":razer_platform::event_clock::tick()?}));
                } else {eprintln!("Mapping execution {id}: {}",errors.join("; "));}
            }
        }
        Ok(())
    }
    fn release(&mut self,scope:Uuid)->Result<()> {
        self.capture.release(scope,&mut self.core)?;
        let owners=self.core.devices.iter().filter_map(|(owner,row)|(row.scope==scope).then_some(owner.clone())).collect::<Vec<_>>();
        let mut errors=Vec::new();
        for owner in owners {if let Err(error)=self.core.stop_device(scope,&owner){errors.push(format!("{error:#}"));}}
        if self.core.global.as_ref().is_some_and(|global|global.scope==scope){
            if let Err(error)=self.core.replace_global_engine(None){errors.push(format!("{error:#}"));}else{self.core.global=None;}
        }
        for lease in self.core.connections.leases_for(scope){if let Err(error)=self.core.connections.release(&lease){errors.push(format!("{error:#}"));}}
        let keys=self.core.hotkey_scopes.iter().filter_map(|(key,scopes)|scopes.contains(&scope).then_some(*key)).collect::<Vec<_>>();
        for key in keys {if let Err(error)=self.core.unregister_hotkey(scope,key){errors.push(format!("{error:#}"));}}
        if self.core.mapping_suppression.contains(&scope) {
            match self.core.mapping(scope,true) {Ok(reply) if !reply.result()=>errors.push(reply.reason().into()),Err(error)=>errors.push(format!("{error:#}")),_=>{}}
        }
        if self.core.shortcut_suppression.contains(&scope) {
            match self.core.shortcuts(scope,true) {Ok(reply) if !reply.result()=>errors.push(reply.reason().into()),Err(error)=>errors.push(format!("{error:#}")),_=>{}}
        }
        ensure!(errors.is_empty(),"{}",errors.join("; "));
        if self.core.scopes.len()==1 && self.core.scopes.contains(&scope) {
            self.shutdown()?;
            *self.core.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?=State::new();
        }
        self.core.hotkey_events.remove(&scope);
        self.core.hotkey_failures.remove(&scope);
        self.core.scopes.remove(&scope);
        Ok(())
    }
    fn shutdown(&mut self)->Result<()> {
        let mut errors=Vec::new();
        for row in self.core.devices.values_mut(){if let Err(error)=row.worker.stop(){errors.push(format!("{error:#}"));}}
        let result=self.core.macros.lock().map_err(|_|anyhow::anyhow!("Mapping macro owner poisoned"))?.shutdown();
        errors.extend(result.cleanup_errors);
        for (_,result) in result.executions {match result{Ok(finished)=>errors.extend(finished.completion.cleanup_errors),Err(error)=>errors.push(format!("{error:#}"))}}
        if let Err(error)=self.core.connections.shutdown(){errors.push(format!("{error:#}"));}
        if let Err(error)=self.core.hotkeys.shutdown(){errors.push(format!("{error:#}"));}
        ensure!(errors.is_empty(),"{}",errors.join("; "));Ok(())
    }
}
