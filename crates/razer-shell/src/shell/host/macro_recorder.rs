//! Current Macro software producer -> host WindowStorage -> MW An/q consumer.
//! Source owners and original bytes: host-current/local-audit.json macro_recorder.
use super::*;
use razer_storage::{
    host::{HostStorageCall, HostStorageView, HostStoreKind},
    macro_recorder::{RecorderLock, STORAGE_KEY, software_exclusive_value},
};
use serde_json::{Value, json};

const OBSERVER_VIEW: u64 = 0xffff_0600;
const SOFTWARE_VIEW: u64 = 0xffff_0601;

#[derive(Default)]
pub(super) struct State {
    lock: RecorderLock,
    software_mounted: bool,
}

impl AppShell {
    fn recorder_storage_call(&mut self, sender_id: u64, action: &str, value: Option<Value>) -> anyhow::Result<Value> {
        let mut payload=json!({"key":STORAGE_KEY});
        if let Some(value)=value {payload["value"]=value;}
        self.host_storage.call(HostStorageCall { store:HostStoreKind::Window,sender_id,
            action:action.into(),payload,target_url_array:vec![] }).map(|reply|reply.value)
    }

    pub(super) fn install_macro_recorder_storage(&mut self, cx: &mut Context<Self>) {
        self.host_storage.register_view(HostStorageView { id:OBSERVER_VIEW,url:"razer-local:macro-recorder-observer".into(),
            remote:false,destroyed:false,crashed:false,disposed:false,tracked_url:true });
        if let Err(error)=self.recorder_storage_call(OBSERVER_VIEW,"registerWindowStorageEvent",None) {
            eprintln!("[macro storage] subscribe: {error:#}");
        }
        let aggregate=self.recorder_storage_call(OBSERVER_VIEW,"getWindowStorageItem",None).ok();
        self.macro_recorder.lock.initialize(aggregate.as_ref().and_then(Value::as_str));
        self.subscriptions.push(cx.on_app_quit(|this,_| {
            this.host_storage.close_view(SOFTWARE_VIEW);
            this.host_storage.close_view(OBSERVER_VIEW);
            async {}
        }));
    }

    pub(super) fn initialize_macro_recorder_page(&mut self) {
        if self.macro_recorder.software_mounted {return;}
        self.host_storage.register_view(HostStorageView { id:SOFTWARE_VIEW,url:"razer-local:macro".into(),
            remote:false,destroyed:false,crashed:false,disposed:false,tracked_url:true });
        self.macro_recorder.software_mounted=true;
        // Er copies the FIRST existing owner's exact value. With no first row,
        // JS sends undefined; the host leaves the item absent in that case.
        let result=(|| {
            let aggregate=self.recorder_storage_call(SOFTWARE_VIEW,"getWindowStorageItem",None)?;
            let rows:Value=serde_json::from_str(aggregate.as_str().unwrap_or("[]"))?;
            if let Some(value)=rows.as_array().and_then(|rows|rows.first()).and_then(|row|row.get("value")) {
                self.recorder_storage_call(SOFTWARE_VIEW,"setWindowStorageItem",Some(value.clone()))?;
            }
            Ok::<_,anyhow::Error>(())
        })();
        if let Err(error)=result {eprintln!("[macro storage] startup copy: {error:#}");}
    }

    pub(super) fn publish_macro_recorder_storage(&mut self, change: macro_page::MacroRecorderStorageChanged, window: &mut Window, cx: &mut Context<Self>) {
        let (action,value)=match change {
            macro_page::MacroRecorderStorageChanged::SoftwareExclusive=>("setWindowStorageItem",Some(Value::String(software_exclusive_value()))),
            macro_page::MacroRecorderStorageChanged::Remove=>("removeWindowStorageItem",None),
        };
        if let Err(error)=self.recorder_storage_call(SOFTWARE_VIEW,action,value) {
            eprintln!("[macro storage] {action}: {error:#}");
        }
        self.sync_receiver_macro_access(cx);
        self.initialize_mouse182_profiles(cx);
        self.sync_keyboard_latest_restore(window,cx);
    }

    pub(super) fn refresh_macro_recorder_storage(&mut self) {
        let events=match self.host_storage.drain_events(OBSERVER_VIEW) {
            Ok(events)=>events,
            Err(error)=>{eprintln!("[macro storage] event owner: {error:#}");return;}
        };
        for event in events.into_iter().filter(|event|event.channel=="windowStorageEvent"&&event.payload["key"]==STORAGE_KEY) {
            let aggregate=self.recorder_storage_call(OBSERVER_VIEW,"getWindowStorageItem",None).ok();
            match self.macro_recorder.lock.event(&event.payload,aggregate.as_ref().and_then(Value::as_str)) {
                Ok(Some(notification))=>eprintln!("[macro storage source notification] {notification:?}"),
                Ok(None)=>{},
                Err(error)=>eprintln!("[macro storage] source consumer: {error:#}"),
            }
        }
    }

    pub(super) fn receiver_macro_exclusive(&self, _: &App) -> bool {
        self.macro_recorder.lock.is_exclusive()
    }
}
