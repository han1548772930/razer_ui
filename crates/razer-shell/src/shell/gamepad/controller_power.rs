//! The retained controller session owns the source's display/idle brightness
//! handlers independently of whichever product page is currently mounted.
use super::*;
use razer_device::controller_lighting::power_policy::{Brightness,Event,Settings};
use razer_platform::power_idle;
use serde::Deserialize;
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use std::time::Duration;

#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct Configuration {
    pub(super) profile_id:String,
    pub(super) settings:Settings,
    pub(super) brightness:Brightness,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct SourceProfile {switch_off_lighting:Settings,brightness:Brightness}
#[derive(Deserialize)]
struct SourceSettings {profile:SourceProfile}
impl Configuration {
    fn read(device:&Device)->anyhow::Result<Self> {
        let profile=device.active_profile_obj().ok_or_else(||anyhow::anyhow!("Controller active profile is missing"))?;
        let source:SourceSettings=serde_json::from_value(profile.source_settings.clone().ok_or_else(||anyhow::anyhow!("Controller source profile is missing"))?)?;
        source.profile.switch_off_lighting.validate()?;
        source.profile.brightness.validate()?;
        Ok(Self {profile_id:profile.id.clone(),settings:source.profile.switch_off_lighting,brightness:source.profile.brightness})
    }
}
#[derive(Default)]
pub(super) struct State {
    pub(super) task:Option<Task<()>>,
    pub(super) stop:Arc<AtomicBool>,
}
impl AppShell {
    pub(super) fn refresh_controller_power_configuration(&mut self,cx:&mut Context<Self>) {
        for (identity,session) in &self.controller_lighting_owners {
            let Some(workspace)=self.devices.iter().find(|workspace|workspace.read(cx).identity(cx)==*identity) else {continue;};
            let saved=workspace.read(cx).saved_snapshot(cx);
            match Configuration::read(&saved).and_then(|configuration|session.owner().configure_power(configuration)) {
                Ok(())=>{},
                Err(error)=>self.status=format!("手柄熄灯策略未读取：{error:#}"),
            }
        }
    }
    pub(super) fn start_controller_power_monitor(&mut self,cx:&mut Context<Self>) {
        if self.controller_power.task.is_some()||self.close_requested {return;}
        self.refresh_controller_power_configuration(cx);
        let stop=Arc::new(AtomicBool::new(false));
        self.controller_power.stop=stop.clone();
        self.controller_lighting_workers+=1;
        self.controller_power.task=Some(cx.spawn(async move |this,cx| {
            let started=cx.background_spawn(async {power_idle::Monitor::start()}).await;
            let result=match started {
                Err(error)=>Err(error),
                Ok((monitor,events))=>{
                    while !stop.load(Ordering::Acquire) {
                        cx.background_executor().timer(Duration::from_millis(200)).await;
                        let current=this.update(cx,|this,cx| {
                            if this.close_requested||!Arc::ptr_eq(&this.controller_power.stop,&stop)||this.controller_lighting_owners.is_empty() {return None;}
                            this.refresh_controller_power_configuration(cx);
                            let owners=this.controller_lighting_owners.iter().filter_map(|(identity,session)| {
                                let workspace=this.devices.iter().find(|workspace|workspace.read(cx).identity(cx)==*identity)?;
                                let owner=session.owner();
                                this.device_observations.iter().any(|route|route.matches(workspace.read(cx).device(cx))&&owner.matches_route(route)).then_some(owner)
                            }).collect::<Vec<_>>();
                            Some(owners)
                        });
                        let Ok(Some(owners))=current else {break;};
                        let mut observations=Vec::new();
                        while let Ok(event)=events.try_recv() {observations.push(event);}
                        if observations.is_empty() {continue;}
                        let mut failure=None;
                        let mut actions=Vec::new();
                        for event in observations {
                            match event {
                                power_idle::Event::DisplayOff=>actions.push(Event::DisplayOff),
                                power_idle::Event::DisplayOn=>actions.push(Event::DisplayOn),
                                power_idle::Event::DisplayDimmed=>actions.push(Event::DisplayDimmed),
                                power_idle::Event::Idle {milliseconds}=>actions.push(Event::Idle {milliseconds}),
                                power_idle::Event::Shutdown=>actions.push(Event::Shutdown),
                                power_idle::Event::Failed(error)=>failure=Some(error),
                            }
                        }
                        let selected=stop.clone();
                        let failures=cx.background_spawn(async move {
                            let mut failures=Vec::new();
                            for event in actions {
                                if selected.load(Ordering::Acquire) {break;}
                                for owner in &owners {
                                    if let Err(error)=owner.apply_power_event(event) {failures.push(format!("{error:#}"));}
                                }
                            }
                            failures
                        }).await;
                        let _=this.update(cx,|this,cx| {
                            if !Arc::ptr_eq(&this.controller_power.stop,&stop) {return;}
                            if let Some(error)=failure {this.status=format!("显示器／闲置状态未读取：{error}");cx.notify();}
                            if !failures.is_empty() {this.status=format!("手柄运行时亮度写入未确认：{}",failures.join("；"));cx.notify();}
                        });
                    }
                    cx.background_spawn(async move {monitor.stop()}).await
                }
            };
            let _=this.update(cx,|this,cx| {
                this.controller_lighting_workers=this.controller_lighting_workers.saturating_sub(1);
                if Arc::ptr_eq(&this.controller_power.stop,&stop) {this.controller_power.task=None;}
                if let Err(error)=result {this.status=format!("显示器／闲置观察停止：{error}");}
                if this.close_requested&&this.save_task.is_none() {this.continue_save_queue(true,cx);}
                cx.notify();
            });
        }));
    }
}
