//! D32B0 emits movement before buttons; CE280 flushes per device on a button
//! or strictly more than 100000 source-clock microseconds. CC8C0/CE4D0 flush
//! pending movement before removing a device or stopping the monitor.
#[path = "input_monitoring/identity.rs"]
mod identity;
#[cfg(test)]
#[path = "input_monitoring/tests.rs"]
mod tests;
use super::NativeRuntime;
use anyhow::{Result, ensure};
use razer_ipc::mouse_input_monitoring::{InputEvent, Poll, Receipt};
use razer_platform::mouse_monitor::{Event, MouseMonitor, Sample};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

#[derive(Serialize)]
struct Row { data:String, #[serde(rename="timeTick")] time_tick:String }
struct Device { identity:identity::Identity, rows:Vec<Row>, last_publication:Option<u64> }
impl Device {
    fn push(&mut self,data:Value,tick:u64,button:bool)->Result<Option<InputEvent>> {
        ensure!(self.rows.len()<16384,"Mouse monitor device batch overflowed");
        self.rows.push(Row{data:data.to_string(),time_tick:tick.to_string()});
        if button || self.last_publication.is_some_and(|previous|tick.wrapping_sub(previous)>100000) {
            self.last_publication=Some(tick); return self.flush();
        }
        self.last_publication.get_or_insert(tick); Ok(None)
    }
    fn flush(&mut self)->Result<Option<InputEvent>> {
        if self.rows.is_empty(){return Ok(None);}
        let event=InputEvent::from_native(&serde_json::to_string(&self.identity)?,1,serde_json::to_string(&self.rows)?)?;
        self.rows.clear(); Ok(Some(event))
    }
}
#[derive(Default)]
pub(super) struct State {
    owner:Option<String>, input:MouseMonitor, devices:BTreeMap<String,Device>,
    pending:VecDeque<InputEvent>, failure:Option<String>,
}
impl State {
    pub(super) fn owner(&self)->Option<String>{self.owner.clone()}
    fn check_owner(&self,owner:&str)->Result<()> {
        ensure!(!owner.is_empty() && self.owner.as_deref()==Some(owner),"Input monitor request belongs to another owner"); Ok(())
    }
    fn publish(&mut self,event:Option<InputEvent>)->Result<()> {
        if let Some(event)=event {ensure!(self.pending.len()<512,"Mouse input monitor publication queue overflowed");self.pending.push_back(event);}
        Ok(())
    }
    fn sample(&mut self,sample:Sample,observe:impl FnOnce(&str)->Result<identity::Identity>)->Result<()> {
        let key=sample.path.to_ascii_lowercase();
        if !self.devices.contains_key(&key) {
            let identity=observe(&sample.path)?;
            self.devices.insert(key.clone(),Device{identity,rows:Vec::new(),last_publication:None});
        }
        if sample.x!=0 || sample.y!=0 {
            let event=self.devices.get_mut(&key).unwrap().push(json!({"type":"mouseMove","x":sample.x,"y":sample.y}),sample.time_tick,false)?;
            self.publish(event)?;
        }
        if sample.button_flags!=0 || sample.button_data!=0 {
            let mut input=razer_device::mapping_native_lookup::Input::new(razer_device::mapping_native_lookup::InputKind::Mouse{id:sample.button_flags,data:sample.button_data});
            input.modifiers=sample.modifiers;
            let event=self.devices.get_mut(&key).unwrap().push(input.to_native_json(),sample.time_tick,true)?;
            self.publish(event)?;
        }
        Ok(())
    }
    fn removed(&mut self,path:&str)->Result<()> {
        let key=path.to_ascii_lowercase();
        if let Some(device)=self.devices.get_mut(&key) {let event=device.flush()?;self.publish(event)?;self.devices.remove(&key);}
        Ok(())
    }
    fn accept(&mut self,events:Vec<Event>)->Result<()> {
        for event in events {match event {
            Event::Input(sample)=>self.sample(sample,identity::observe)?,
            Event::Removed(path)=>self.removed(&path)?,
        }} Ok(())
    }
    fn collect(&mut self)->Result<()> { let events=self.input.events()?; self.accept(events) }
    fn poll(&mut self,owner:&str)->Result<Poll> {
        self.check_owner(owner)?;
        if let Err(error)=self.collect(){self.failure.get_or_insert_with(||format!("{error:#}"));}
        let mut events=Vec::new(); let mut bytes=0;
        while events.len()<128 {
            let Some(event)=self.pending.front() else {break;};
            let encoded=serde_json::to_vec(event)?.len();
            ensure!(encoded<3*1024*1024,"Input monitor event exceeds IPC capacity");
            if bytes+encoded>3*1024*1024{break;}
            bytes+=encoded;events.push(self.pending.pop_front().unwrap());
        }
        Ok(Poll::new(owner.into(),events,self.failure.clone()))
    }
}
impl NativeRuntime {
    pub(super) fn start_mouse_input_monitoring(&mut self,owner:String)->Result<Value> {
        ensure!(!owner.is_empty(),"Input monitor owner is empty");
        ensure!(self.mouse_input_monitoring.owner.is_none(),"Input monitor is already owned");
        self.mouse_input_monitoring.owner=Some(owner.clone());
        self.mouse_input_monitoring.input.start()?;
        Ok(serde_json::to_value(Receipt::new(owner,true))?)
    }
    pub(super) fn poll_mouse_input_monitoring(&mut self,owner:&str)->Result<Value> {
        Ok(serde_json::to_value(self.mouse_input_monitoring.poll(owner)?)?)
    }
    pub(super) fn stop_mouse_input_monitoring(&mut self,owner:&str)->Result<Value> {
        self.mouse_input_monitoring.check_owner(owner)?;
        let stopped=self.mouse_input_monitoring.input.stop();
        let collected=stopped.and_then(|events|self.mouse_input_monitoring.accept(events));
        let paths=self.mouse_input_monitoring.devices.keys().cloned().collect::<Vec<_>>();
        let mut errors=Vec::new();
        if let Err(error)=collected {errors.push(format!("Drain monitor: {error:#}"));}
        for path in paths {if let Err(error)=self.mouse_input_monitoring.removed(&path){errors.push(format!("Flush monitor: {error:#}"));}}
        ensure!(errors.is_empty(),"{}",errors.join("; "));
        self.mouse_input_monitoring=State::default();
        Ok(serde_json::to_value(Receipt::new(owner.into(),false))?)
    }
}
