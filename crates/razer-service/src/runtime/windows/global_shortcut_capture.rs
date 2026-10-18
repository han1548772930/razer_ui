//! Both source callbacks publish into one bounded FIFO. Their publication lock
//! provides the order; native timestamps do not define cross-callback ordering.
use anyhow::{Context as _, ensure};
use razer_ipc::global_shortcut_capture::{Batch, Event};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

const CAPACITY: usize = 512;
static SINK: Mutex<Option<Weak<Mutex<Inbox>>>> = Mutex::new(None);
static CALLBACK_FAILED: AtomicBool = AtomicBool::new(false);

pub(super) enum CallbackEvent {
    Input(Value),
    Mode(Value),
}

#[derive(Default)]
struct Inbox {
    events: VecDeque<CallbackEvent>,
    overflow: bool,
}
impl Inbox {
    fn push(&mut self, event: CallbackEvent) {
        if self.events.len() == CAPACITY {
            self.overflow = true;
        } else {
            self.events.push_back(event);
        }
    }
}

pub(super) struct State(Arc<Mutex<Inbox>>);
impl State {
    pub(super) fn new() -> Self {
        Self(Arc::new(Mutex::new(Inbox::default())))
    }
    /// The caller holds the ordinary input callback lock while transferring
    /// already queued input, before installing the source mode callback.
    pub(super) fn activate(&self, pending_input: Vec<Value>) -> anyhow::Result<()> {
        let mut sink = SINK
            .lock()
            .map_err(|_| anyhow::anyhow!("Global capture callback lock poisoned"))?;
        ensure!(
            sink.as_ref().and_then(Weak::upgrade).is_none(),
            "Global capture callbacks already have an owner"
        );
        let mut inbox = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Global capture inbox lock poisoned"))?;
        inbox.events.clear();
        inbox.overflow = false;
        for event in pending_input {
            inbox.push(CallbackEvent::Input(event));
        }
        CALLBACK_FAILED.store(false, Ordering::Release);
        *sink = Some(Arc::downgrade(&self.0));
        Ok(())
    }
    pub(super) fn deactivate(&self) -> anyhow::Result<()> {
        let mut sink = SINK
            .lock()
            .map_err(|_| anyhow::anyhow!("Global capture callback lock poisoned"))?;
        if let Some(current) = sink.as_ref().and_then(Weak::upgrade) {
            ensure!(
                Arc::ptr_eq(&current, &self.0),
                "Global capture callback owner changed"
            );
            *sink = None;
        }
        Ok(())
    }
    pub(super) fn drain(&self) -> anyhow::Result<Vec<CallbackEvent>> {
        ensure!(
            !CALLBACK_FAILED.load(Ordering::Acquire),
            "Global capture callback publication failed"
        );
        let mut inbox = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Global capture inbox lock poisoned"))?;
        ensure!(!inbox.overflow, "Global capture callback buffer overflowed");
        Ok(inbox.events.drain(..).collect())
    }
}

enum Kind {
    Input,
    Mode,
}
fn publish(kind: Kind, value: Value) -> Result<(), Value> {
    let Ok(sink) = SINK.lock() else {
        CALLBACK_FAILED.store(true, Ordering::Release);
        return Ok(());
    };
    let Some(inbox) = sink.as_ref().and_then(Weak::upgrade) else {
        return Err(value);
    };
    let Ok(mut inbox) = inbox.lock() else {
        CALLBACK_FAILED.store(true, Ordering::Release);
        return Ok(());
    };
    inbox.push(match kind {
        Kind::Input => CallbackEvent::Input(value),
        Kind::Mode => CallbackEvent::Mode(value),
    });
    Ok(())
}
pub(super) fn input(value: Value) -> Result<(), Value> {
    publish(Kind::Input, value)
}
pub(super) fn mode(value: Value) -> Result<(), Value> {
    publish(Kind::Mode, value)
}

pub(super) fn decode(
    callbacks: Vec<CallbackEvent>,
    mut owned_input: impl FnMut(&Value) -> anyhow::Result<bool>,
) -> anyhow::Result<Batch> {
    #[derive(Deserialize)]
    struct Mode {
        #[serde(rename = "isEnabled")]
        enabled: bool,
    }
    let mut events = Vec::new();
    for callback in callbacks {
        match callback {
            CallbackEvent::Input(event) => {
                if owned_input(&event)? {
                    events.push(Event::Input(event));
                }
            }
            CallbackEvent::Mode(event) => {
                let payload = event["event"]
                    .as_str()
                    .context("Global capture mode callback has no source event string")?;
                let payload: Mode = serde_json::from_str(payload)?;
                events.push(Event::Mode(payload.enabled));
            }
        }
    }
    Ok(Batch::new(events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    static TESTS: Mutex<()> = Mutex::new(());

    #[test]
    fn callback_publication_order_survives_draining_and_unrelated_source_timestamps() {
        let _tests = TESTS.lock().unwrap();
        let state = State::new();
        state
            .activate(vec![json!({"input":{"key":64},"timeTick":900})])
            .unwrap();
        mode(json!({"event":"{\"isEnabled\":true}","timeTick":800})).unwrap();
        input(json!({"input":{"key":65},"timeTick":1})).unwrap();
        mode(json!({"event":"{\"isEnabled\":false}","timeTick":700})).unwrap();
        let events = decode(state.drain().unwrap(), |_| Ok(true))
            .unwrap()
            .into_events();
        assert_eq!(
            events,
            [
                Event::Input(json!({"input":{"key":64},"timeTick":900})),
                Event::Mode(true),
                Event::Input(json!({"input":{"key":65},"timeTick":1})),
                Event::Mode(false),
            ]
        );
        input(json!({"input":{"key":66},"timeTick":0})).unwrap();
        assert_eq!(
            decode(state.drain().unwrap(), |_| Ok(true))
                .unwrap()
                .into_events(),
            [Event::Input(json!({"input":{"key":66},"timeTick":0}))]
        );
        state.deactivate().unwrap();
    }

    #[test]
    fn unstarted_owners_do_not_steal_callbacks_and_queue_overflow_is_explicit() {
        let _tests = TESTS.lock().unwrap();
        let first = State::new();
        first.activate(vec![]).unwrap();
        let second = State::new();
        assert!(second.activate(vec![]).is_err());
        assert!(second.deactivate().is_err());
        for key in 0..=CAPACITY {
            input(json!({"input":{"key":key}})).unwrap();
        }
        assert!(first.drain().is_err());
        assert!(second.drain().unwrap().is_empty());
        first.deactivate().unwrap();
        assert!(input(json!({})).is_err());
    }

    #[test]
    fn malformed_modes_fail_without_defaulting_or_reordering_other_input() {
        for event in [
            json!({}),
            json!({"event":"{}"}),
            json!({"event":"{\"isEnabled\":\"true\"}"}),
            json!({"event":"not-json"}),
        ] {
            assert!(decode(vec![CallbackEvent::Mode(event)], |_| Ok(true)).is_err());
        }
        let batch = decode(
            vec![
                CallbackEvent::Input(json!({"owned":false})),
                CallbackEvent::Mode(json!({"event":"{\"isEnabled\":false}"})),
                CallbackEvent::Input(json!({"owned":true})),
            ],
            |event| Ok(event["owned"] == true),
        )
        .unwrap();
        assert_eq!(
            batch.into_events(),
            [Event::Mode(false), Event::Input(json!({"owned":true}))]
        );
    }
}
