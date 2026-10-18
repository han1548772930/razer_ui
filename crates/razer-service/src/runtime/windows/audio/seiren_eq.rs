use super::*;
use razer_device::seiren_eq::{self as eq, Change};
#[path = "seiren_session.rs"]
mod audio_session;
#[path = "seiren_driver.rs"]
mod driver;
#[path = "seiren_endpoint.rs"]
mod endpoint;
use eq::{NativeOperation, NativeReply, NativeTransport};
use std::collections::BTreeMap;

struct OwnedSession {
    target: Value,
    instance: Value,
    session: audio_session::Session,
    initialization: NativeReply,
}
#[derive(Default)]
pub(crate) struct State {
    sessions: BTreeMap<String, OwnedSession>,
}
impl State {
    fn key(target: &DeviceReadTarget) -> String {
        format!("{}:{}", target.product_id, target.device_container_id)
    }
    fn verified(target: &DeviceReadTarget) -> anyhow::Result<Value> {
        ensure!(
            target.product_id == target.physical_product_id && target.peer_product_id.is_none(),
            "Marie owner requires its direct product"
        );
        let cap =
            eq::capability(target.product_id).context("Marie current product source is absent")?;
        current_target(target, cap)
    }
    fn current(&self, target: &DeviceReadTarget) -> anyhow::Result<&OwnedSession> {
        let owned = self
            .sessions
            .get(&Self::key(target))
            .context("Marie native owner is not initialized")?;
        ensure!(
            owned.target == serde_json::to_value(target)?,
            "Marie native owner route changed"
        );
        ensure!(
            Self::verified(target)?["device_instance_id"] == owned.instance,
            "Marie native owner device instance changed"
        );
        Ok(owned)
    }
    pub(crate) fn open(&mut self, target: &DeviceReadTarget) -> anyhow::Result<Value> {
        let key = Self::key(target);
        if self.sessions.contains_key(&key) {
            let owned = self.current(target)?;
            return Ok(
                json!({"target":target,"initialized":owned.initialization,"version":owned.session.version(),"retained":true}),
            );
        }
        let before = Self::verified(target)?;
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        let session = audio_session::Session::open(target.product_id, &target.device_container_id)?;
        let initialization = session.call(&NativeOperation::DeviceInitialize)?;
        let version = session.version().map(str::to_owned);
        self.sessions.insert(
            key,
            OwnedSession {
                target: serde_json::to_value(target)?,
                instance: before["device_instance_id"].clone(),
                session,
                initialization: initialization.clone(),
            },
        );
        Ok(json!({"target":target,"initialized":initialization,"version":version,"retained":false}))
    }
    pub(crate) fn request(
        &mut self,
        target: &DeviceReadTarget,
        change: &Change,
    ) -> anyhow::Result<Value> {
        if !change.requires_native() {
            return request(target, change);
        }
        if !self.sessions.contains_key(&Self::key(target)) {
            return request(target, change);
        }
        let started = Instant::now();
        let owned = self.current(target)?;
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        let report =
            eq::execute_native_in_session(target.product_id, &owned.session, change, || {
                ensure!(
                    started.elapsed() < Duration::from_secs(20),
                    "Marie retained operation exceeded deadline"
                );
                self.current(target).map(|_| ())
            })?;
        self.current(target)?;
        Ok(
            json!({"target":target,"source_class":eq::capability(target.product_id).unwrap().source_class,"report":report,"setting_scope":"seiren_eq_profile","elapsed_ms":started.elapsed().as_millis() as u64}),
        )
    }
    pub(crate) fn call(
        &self,
        target: &DeviceReadTarget,
        operation: &NativeOperation,
    ) -> anyhow::Result<Value> {
        // Lifecycle is owned by Open/Close. Typed source operations keep the
        // driver worker from becoming an arbitrary command dispatcher.
        ensure!(
            !matches!(
                operation,
                NativeOperation::DeviceInitialize | NativeOperation::DeviceTerminate
            ),
            "Marie lifecycle has a separate owner"
        );
        match operation {
            NativeOperation::SetMasterSampleRate { rate } => ensure!(
                matches!(rate, 48_000 | 96_000),
                "Marie sample rate differs from source"
            ),
            NativeOperation::SetDeviceVolume { level, .. }
            | NativeOperation::SetMixLevel { level, .. } => {
                ensure!(*level <= 100, "Marie native level exceeds source range")
            }
            NativeOperation::SetMicMonitorLevel { level } => ensure!(
                (0..=100).contains(level),
                "Marie monitor level exceeds source range"
            ),
            _ => {}
        }
        let owned = self.current(target)?;
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        let reply = owned.session.call(operation)?;
        self.current(target)?;
        Ok(json!({"target":target,"operation":operation,"reply":reply}))
    }
    pub(crate) fn events(&self, target: &DeviceReadTarget) -> anyhow::Result<Value> {
        let batch = self.current(target)?.session.drain()?;
        self.current(target)?;
        Ok(json!({"target":target,"events":batch.events,"diagnostics":batch.diagnostics}))
    }
    pub(crate) fn close(&mut self, target: &DeviceReadTarget) -> anyhow::Result<Value> {
        let key = Self::key(target);
        if let Some(owned) = self.sessions.get(&key) {
            ensure!(
                owned.target == serde_json::to_value(target)?,
                "Marie cleanup owner differs"
            );
        }
        let owned = self.sessions.remove(&key);
        // Removal does not query the now-disconnected device. Drop also
        // terminates a partially initialized driver session.
        let reply = owned
            .as_ref()
            .map(|owned| owned.session.call(&NativeOperation::DeviceTerminate))
            .transpose();
        drop(owned);
        Ok(json!({"target":target,"terminated":reply?}))
    }
}
pub(super) fn request(target: &DeviceReadTarget, change: &Change) -> anyhow::Result<Value> {
    let cap = eq::capability(target.product_id).context("Seiren EQ transport source is absent")?;
    ensure!(
        target.physical_product_id == target.product_id && target.peer_product_id.is_none(),
        "Seiren EQ target is not its direct product"
    );
    eq::validate_change(target.product_id, change)?;
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Seiren EQ task deadline exceeded"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Seiren EQ device instance changed"
        );
        Ok(())
    };
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        validate()?;
        let bytes = before["feature_report_bytes"]
            .as_u64()
            .context("Seiren EQ Feature Report size is absent")? as usize;
        ensure!(
            bytes == cap.report_bytes,
            "Seiren EQ Feature Report size differs"
        );
        if change.requires_native() {
            let device =
                audio_session::Session::open(target.product_id, &target.device_container_id)?;
            eq::execute_native(target.product_id, &device, change, &validate)?
        } else {
            let device = hid_transport::open(&target.path, bytes)?;
            eq::execute(
                target.product_id,
                &device,
                cap,
                change,
                || transaction(target, cap),
                &validate,
            )?
        }
    };
    validate()?;
    Ok(
        json!({"target":target,"source_class":cap.source_class,"report":report,"setting_scope":"seiren_eq_profile",
        "elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}
