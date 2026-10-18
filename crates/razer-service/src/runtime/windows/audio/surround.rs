//! Independent routing protocol owner for the installed local Surround service.
use anyhow::{Context as _, Result};
use razer_device::audio_surround::{
    self as surround, Change, Channel, Identity, NativeCall, Outcome, Runtime, Transport, Volume,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "surround/client.rs"]
mod client;
use client::Session;

struct ValidatedTransport<'a> {
    session: &'a mut Session,
    validate: &'a dyn Fn() -> Result<()>,
}
impl Transport for ValidatedTransport<'_> {
    fn call(&mut self, call: &NativeCall) -> Result<bool> {
        self.session.call(call, self.validate)
    }
}

struct Owned {
    identity: Identity,
    session: Session,
    runtime: Runtime,
}
impl Owned {
    fn synchronize(&mut self) -> Result<client::Snapshot> {
        let snapshot = self.session.snapshot()?;
        self.runtime.observe_front_endpoint(surround::presentation::Controls::from_service(&snapshot).front().cloned());
        let volume = |channel| -> Option<Volume> {
            if !snapshot.connected {
                return None;
            }
            let id = surround::routing::endpoint_for(
                &snapshot.observations,
                snapshot.front_endpoint.as_deref(),
                channel,
            )
            .ok()?;
            Volume::from_native(snapshot.observations.volumes.get(id)?.volume).ok()
        };
        self.runtime
            .replace_observed_volumes(volume(Channel::Front), volume(Channel::Rear));
        Ok(snapshot)
    }
}

#[derive(Default)]
pub(crate) struct State {
    sessions: BTreeMap<String, Owned>,
}
impl State {
    fn key(identity: &Identity) -> String {
        identity.container_id().to_ascii_lowercase()
    }
    pub(crate) fn observe(
        &mut self,
        identity: &Identity,
        validate: impl Fn() -> Result<()>,
    ) -> Result<Value> {
        validate()?;
        let owned = self
            .sessions
            .get_mut(&Self::key(identity))
            .filter(|owned| owned.identity == *identity)
            .context("Surround owner is not open for this generation")?;
        let snapshot = owned.synchronize()?;
        validate()?;
        Ok(serde_json::to_value(
            surround::observed::ObservationReply::new(identity.clone(), snapshot),
        )?)
    }
    pub(crate) fn open(
        &mut self,
        identity: &Identity,
        settings: &surround::Settings,
        validate: impl Fn() -> Result<()>,
    ) -> Result<Value> {
        validate()?;
        let key = Self::key(identity);
        if let Some(owned) = self.sessions.get_mut(&key) {
            if owned.identity == *identity {
                let snapshot = owned.synchronize()?;
                return Ok(
                    json!({"identity":identity,"initialized":snapshot.connected,"version":snapshot.version,
                    "retained":true,"service":snapshot,"runtime":owned.runtime,"setting_scope":"surround"}),
                );
            }
        }
        if let Some(mut previous) = self.sessions.remove(&key) {
            previous.session.close()?;
        }
        let session = Session::open(identity)?;
        validate()?;
        let mut owned = Owned {
            identity: identity.clone(),
            session,
            runtime: Runtime::with_settings(settings.clone()),
        };
        let snapshot = owned.synchronize()?;
        let result = json!({"identity":identity,"initialized":snapshot.connected,"version":snapshot.version,
            "retained":false,"service":snapshot,"runtime":owned.runtime,"setting_scope":"surround"});
        self.sessions.insert(key, owned);
        Ok(result)
    }
    pub(crate) fn change(
        &mut self,
        identity: &Identity,
        change: &Change,
        validate: impl Fn() -> Result<()>,
    ) -> Result<Value> {
        let owned = self
            .sessions
            .get_mut(&Self::key(identity))
            .filter(|owned| owned.identity == *identity)
            .context("Surround owner is not open for this generation")?;
        let mut outcomes: Vec<Outcome> = Vec::new();
        let mut queue = vec![change.clone()];
        while let Some(next) = queue.pop() {
            let mut transport = ValidatedTransport {
                session: &mut owned.session,
                validate: &validate,
            };
            let outcome = owned.runtime.execute(&next, &mut transport, &validate);
            queue.extend(outcome.follow_ups().iter().rev().cloned());
            let stop = !outcome.is_completed();
            outcomes.push(outcome);
            if stop {
                break;
            }
        }
        let snapshot = owned.synchronize()?;
        Ok(
            json!({"identity":identity,"requested":change,"outcomes":outcomes,"runtime":owned.runtime,
            "service":snapshot,"setting_scope":"surround"}),
        )
    }
    pub(crate) fn close(&mut self, identity: &Identity) -> Result<Value> {
        let key = Self::key(identity);
        let current = self
            .sessions
            .get(&key)
            .is_some_and(|owned| owned.identity == *identity);
        let mut cleanup = None;
        let mut snapshot = None;
        if let Some(mut owned) = current.then(|| self.sessions.remove(&key)).flatten() {
            let result = owned.session.close();
            cleanup = Some(result.as_ref().err().map(|e| format!("{e:#}")));
            snapshot = Some(owned.session.snapshot()?);
        }
        Ok(
            json!({"identity":identity,"closed":current,"cleanup_failure":cleanup.flatten(),
            "service":snapshot,"setting_scope":"surround"}),
        )
    }
}
