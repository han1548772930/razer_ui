//! Current host 4.0.827 Key/Memory/WindowStorage semantics. These original stores
//! are process-local Maps, separate from this project's persisted draft files.
//! Sources and wire adaptations: docs/re/host-storage-current.md.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostStoreKind {
    Key,
    Memory,
    Window,
}

/// Metadata supplied by the owning host, never device identity or a guessed URL.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostStorageView {
    pub id: u64,
    pub url: String,
    pub remote: bool,
    pub destroyed: bool,
    pub crashed: bool,
    pub disposed: bool,
    /// Original globalNodeVar.rzWindowVersion.urlInfoMap.has(url).
    pub tracked_url: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostStorageCall {
    pub store: HostStoreKind,
    pub sender_id: u64,
    pub action: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub target_url_array: Vec<String>,
}

/// JSON transport distinguishes JS undefined from JSON null. A Map snapshot
/// uses insertion-ordered entry arrays; aggregate getters retain original JSON strings.
#[derive(Debug, Serialize, Deserialize)]
pub struct HostStorageReply {
    pub defined: bool,
    pub value: Value,
}
impl HostStorageReply {
    fn value(value: Value) -> Self {
        Self {
            defined: true,
            value,
        }
    }
    fn undefined() -> Self {
        Self {
            defined: false,
            value: Value::Null,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostStorageEvent {
    pub channel: String,
    pub payload: Value,
}

#[derive(Default)]
struct KeySlot {
    key: String,
    value: Option<Value>,
    subscribers: Vec<String>,
}
#[derive(Default)]
struct UrlStore {
    entries: Vec<(String, Vec<(String, Value)>)>,
    subscribers: Vec<String>,
}
#[derive(Default)]
pub struct HostStorage {
    keys: Vec<KeySlot>,
    memory: UrlStore,
    window: UrlStore,
    views: Vec<(HostStorageView, Vec<HostStorageEvent>)>,
}

impl HostStorage {
    pub fn register_view(&mut self, view: HostStorageView) {
        if let Some((current, _)) = self.views.iter_mut().find(|(v, _)| v.id == view.id) {
            *current = view;
        } else {
            self.views.push((view, Vec::new()));
        }
    }
    pub fn close_view(&mut self, id: u64) {
        // Original URL subscriptions survive destruction; only live recipients
        // are filtered. Do not silently unregister another view with the same URL.
        self.views.retain(|(view, _)| view.id != id);
    }
    pub fn drain_events(&mut self, id: u64) -> anyhow::Result<Vec<HostStorageEvent>> {
        let (_, queue) = self
            .views
            .iter_mut()
            .find(|(v, _)| v.id == id)
            .ok_or_else(|| anyhow::anyhow!("Host storage event recipient is not registered"))?;
        Ok(std::mem::take(queue))
    }
    pub fn call(&mut self, call: HostStorageCall) -> anyhow::Result<HostStorageReply> {
        let sender = self
            .views
            .iter()
            .find(|(v, _)| v.id == call.sender_id)
            .map(|(v, _)| v.clone())
            .ok_or_else(|| anyhow::anyhow!("Host storage sender is not registered"))?;
        anyhow::ensure!(
            !sender.destroyed && !sender.crashed && !sender.disposed,
            "Host storage sender has been destroyed or disposed"
        );
        match call.store {
            HostStoreKind::Key => self.key_call(&sender, &call),
            HostStoreKind::Memory | HostStoreKind::Window => self.url_call(&sender, &call),
        }
    }
    fn key_call(
        &mut self,
        sender: &HostStorageView,
        call: &HostStorageCall,
    ) -> anyhow::Result<HostStorageReply> {
        let ok = || HostStorageReply::value(json!({"result":true,"reason":""}));
        let failure =
            |reason: &str| HostStorageReply::value(json!({"result":false,"reason":reason}));
        if call.action == "getKeys" {
            return Ok(HostStorageReply::value(json!(
                self.keys
                    .iter()
                    .filter(|s| s.value.is_some())
                    .map(|s| &s.key)
                    .collect::<Vec<_>>()
            )));
        }
        if !matches!(
            call.action.as_str(),
            "setItem" | "getItem" | "removeItem" | "registerEvent" | "unregisterEvent"
        ) {
            return Ok(HostStorageReply::undefined());
        }
        let Some(key) = call.payload.get("key").filter(|v| !v.is_null()) else {
            return Ok(failure("missing arg.payload key"));
        };
        let key = key.as_str().ok_or_else(|| {
            anyhow::anyhow!("Non-string JS Map keys are not implemented on the JSON storage wire")
        })?;
        let index = self.keys.iter().position(|s| s.key == key);
        if call.action == "getItem" {
            return Ok(index
                .and_then(|i| self.keys[i].value.clone())
                .map(HostStorageReply::value)
                .unwrap_or_else(HostStorageReply::undefined));
        }
        match call.action.as_str() {
            "setItem" => {
                let Some(value) = call.payload.get("value").filter(|v| !v.is_null()) else {
                    return Ok(failure("missing arg.payload value"));
                };
                let i = index.unwrap_or_else(|| {
                    self.keys.push(KeySlot {
                        key: key.into(),
                        ..Default::default()
                    });
                    self.keys.len() - 1
                });
                self.keys[i].value = Some(value.clone());
                let subscribers = self.keys[i].subscribers.clone();
                self.emit(
                    sender,
                    "keyStorageEvent",
                    json!({"key":key,"value":value,"srcURL":sender.url,"keyState":"changed"}),
                    Some(&subscribers),
                    false,
                    &[],
                );
                Ok(ok())
            }
            "removeItem" => {
                let Some(i) = index.filter(|i| self.keys[*i].value.is_some()) else {
                    return Ok(failure(""));
                };
                self.keys[i].value = None;
                let subscribers = self.keys[i].subscribers.clone();
                if subscribers.is_empty() {
                    self.keys.remove(i);
                }
                self.emit(
                    sender,
                    "keyStorageEvent",
                    json!({"key":key,"srcURL":sender.url,"keyState":"removed"}),
                    Some(&subscribers),
                    false,
                    &[],
                );
                Ok(ok())
            }
            "registerEvent" => {
                let i = index.unwrap_or_else(|| {
                    self.keys.push(KeySlot {
                        key: key.into(),
                        ..Default::default()
                    });
                    self.keys.len() - 1
                });
                if !self.keys[i].subscribers.contains(&sender.url) {
                    self.keys[i].subscribers.push(sender.url.clone());
                }
                Ok(ok())
            }
            "unregisterEvent" => {
                let Some(i) = index else {
                    return Ok(failure("key does not exist"));
                };
                self.keys[i].subscribers.retain(|url| url != &sender.url);
                if self.keys[i].value.is_none() && self.keys[i].subscribers.is_empty() {
                    self.keys.remove(i);
                }
                Ok(ok())
            }
            _ => unreachable!(),
        }
    }
    fn url_call(
        &mut self,
        sender: &HostStorageView,
        call: &HostStorageCall,
    ) -> anyhow::Result<HostStorageReply> {
        let memory = matches!(call.store, HostStoreKind::Memory);
        let noun = if memory { "Memory" } else { "Window" };
        let action = call.action.as_str();
        let store = if memory {
            &mut self.memory
        } else {
            &mut self.window
        };
        if action == format!("register{noun}StorageEvent") {
            if !store.subscribers.contains(&sender.url) {
                store.subscribers.push(sender.url.clone());
            }
            return Ok(HostStorageReply::value(json!(1)));
        }
        if action == format!("unRegister{noun}StorageEvent") {
            store.subscribers.retain(|url| url != &sender.url);
            return Ok(HostStorageReply::value(json!(1)));
        }
        if action == format!("reset{noun}Storage") {
            store.entries.clear();
            return Ok(HostStorageReply::undefined());
        }
        if action == format!("clear{noun}Storage") {
            if memory && sender.url.is_empty() {
                return Ok(HostStorageReply::value(json!(true)));
            }
            let old = store.entries.len();
            store.entries.retain(|(url, _)| url != &sender.url);
            return Ok(HostStorageReply::value(json!(old != store.entries.len())));
        }
        if action == format!("get{noun}Storage") {
            return Ok(HostStorageReply::value(json!(store.entries)));
        }
        if action == format!("get{noun}StorageKeys") {
            let mut keys = Vec::new();
            for (_, entries) in &store.entries {
                for (key, _) in entries {
                    if !keys.contains(key) {
                        keys.push(key.clone());
                    }
                }
            }
            return Ok(HostStorageReply::value(json!(keys)));
        }
        let set = action == format!("set{noun}StorageItem");
        let no_event = memory && action == "setMemoryStorageItemNoEvent";
        let get = action == format!("get{noun}StorageItem");
        let remove = action == format!("remove{noun}StorageItem");
        if !(set || no_event || get || remove) {
            return Ok(HostStorageReply::undefined());
        }
        let Some(key) = call.payload.get("key") else {
            return Ok(if remove {
                HostStorageReply::value(json!(false))
            } else {
                HostStorageReply::undefined()
            });
        };
        let key = key.as_str().ok_or_else(|| {
            anyhow::anyhow!("Non-string JS Map keys are not implemented on the JSON storage wire")
        })?;
        if get {
            let rows: Vec<_> = store
                .entries
                .iter()
                .filter_map(|(url, entries)| {
                    entries
                        .iter()
                        .find(|(k, _)| k == key)
                        .filter(|(_, v)| js_truthy(v))
                        .map(|(_, value)| json!({"value":value,"windowName":url}))
                })
                .collect();
            return Ok(HostStorageReply::value(Value::String(
                serde_json::to_string(&rows)?,
            )));
        }
        let url = if set {
            call.payload
                .get("url")
                .filter(|v| !v.is_null())
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| anyhow::anyhow!("Non-string storage URL is unsupported"))
                })
                .transpose()?
                .unwrap_or_else(|| sender.url.clone())
        } else {
            sender.url.clone()
        };
        let mut old = None;
        let state;
        let result;
        if set || no_event {
            let Some(value) = call.payload.get("value") else {
                return Ok(HostStorageReply::undefined());
            };
            let i = store
                .entries
                .iter()
                .position(|(u, _)| u == &url)
                .unwrap_or_else(|| {
                    store.entries.push((url.clone(), Vec::new()));
                    store.entries.len() - 1
                });
            if let Some((_, current)) = store.entries[i].1.iter_mut().find(|(k, _)| k == key) {
                old = Some(std::mem::replace(current, value.clone()));
                state = "changed";
            } else {
                store.entries[i].1.push((key.into(), value.clone()));
                state = "created";
            }
            result = if no_event {
                HostStorageReply::value(json!(true))
            } else {
                old.clone()
                    .map(HostStorageReply::value)
                    .unwrap_or_else(HostStorageReply::undefined)
            };
        } else {
            if let Some((_, entries)) = store.entries.iter_mut().find(|(u, _)| u == &url) {
                if let Some(i) = entries.iter().position(|(k, _)| k == key) {
                    old = Some(entries.remove(i).1);
                }
            }
            result = HostStorageReply::value(json!(old.is_some()));
            state = "removed";
        }
        if no_event || (remove && old.is_none()) {
            return Ok(result);
        }
        let subscribers = store.subscribers.clone();
        let mut payload = json!({"key":key,"keyState":state,"srcURL":sender.url});
        if let Some(old) = old {
            payload["oldValue"] = old;
        }
        if let Some(new) = call.payload.get("value") {
            payload["newValue"] = new.clone();
        }
        self.emit(
            sender,
            if memory {
                "memoryStorageEvent"
            } else {
                "windowStorageEvent"
            },
            payload,
            Some(&subscribers),
            true,
            if memory && set {
                &call.target_url_array
            } else {
                &[]
            },
        );
        Ok(result)
    }
    fn emit(
        &mut self,
        sender: &HostStorageView,
        channel: &str,
        payload: Value,
        subscribers: Option<&[String]>,
        untracked_allowed: bool,
        targets: &[String],
    ) {
        for (view, queue) in &mut self.views {
            if view.id == sender.id
                || view.remote
                || view.destroyed
                || view.crashed
                || view.disposed
            {
                continue;
            }
            if !subscribers.is_some_and(|s| s.contains(&view.url))
                && !(untracked_allowed && !view.tracked_url)
            {
                continue;
            }
            let count = if targets.is_empty() {
                1
            } else {
                targets
                    .iter()
                    .filter(|t| view.url.contains(t.as_str()))
                    .count()
            };
            for _ in 0..count {
                queue.push(HostStorageEvent {
                    channel: channel.into(),
                    payload: payload.clone(),
                });
            }
        }
    }
}

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.0),
        Value::String(v) => !v.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
