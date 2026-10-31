//! Camera identity belongs to a physical connection, not the selected page.
use super::*;
use razer_device::{camera_serial::CameraSerialProgress, camera_submission::CameraScope};
use razer_model::model::{CameraSerialObservation, DeviceConnectionObservation};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub struct CameraIdentityRequested {
    owner: EntityId,
    generation: u64,
    real_product_id: u32,
    scope: CameraScope,
    cancelled: Arc<AtomicBool>,
}

impl CameraIdentityRequested {
    pub fn owner(&self) -> EntityId {
        self.owner
    }

    pub fn scope(&self) -> &CameraScope {
        &self.scope
    }

    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn same_connection(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.generation == other.generation
            && self.real_product_id == other.real_product_id
            && self.scope == other.scope
            && Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }

    fn matches_device(&self, device: &Device, generation: u64) -> bool {
        !self.is_cancelled()
            && self.generation == generation
            && self.real_product_id == device.real_product_id
            && self.scope.product_id == device.product_id
            && self.scope.container_id == device.device_container_id
    }
}

#[derive(Default)]
pub(super) struct Owner {
    request: Option<CameraIdentityRequested>,
    attempted: bool,
}

impl Owner {
    pub(super) fn invalidate(&mut self) {
        if let Some(request) = self.request.take() {
            request.cancelled.store(true, Ordering::Release);
        }
        self.attempted = false;
    }

    pub(super) fn attempted(&self) -> bool {
        self.attempted
            && self
                .request
                .as_ref()
                .is_some_and(|request| !request.is_cancelled())
    }

    fn ensure_connection(
        &mut self,
        owner: EntityId,
        generation: u64,
        device: &Device,
        exiting: bool,
    ) -> Option<CameraIdentityRequested> {
        if exiting
            || !matches!(device.product_id, 3594..=3596)
            || !matches!(
                device.dashboard.connection_observation,
                Some(
                    DeviceConnectionObservation::UsbPresent
                        | DeviceConnectionObservation::HidPresent
                )
            )
            || device
                .serial_number
                .to_ascii_uppercase()
                .starts_with("PREVIEW-")
            || device
                .serial_number
                .to_ascii_uppercase()
                .starts_with("DEMO-")
        {
            self.invalidate();
            return None;
        }
        if self.request.as_ref().is_some_and(|request| {
            request.owner == owner && request.matches_device(device, generation)
        }) {
            return None;
        }
        self.invalidate();
        let request = CameraIdentityRequested {
            owner,
            generation,
            real_product_id: device.real_product_id,
            scope: CameraScope {
                product_id: device.product_id,
                container_id: device.device_container_id.clone(),
            },
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        self.request = Some(request.clone());
        Some(request)
    }

    fn complete(&mut self, request: &CameraIdentityRequested) -> bool {
        if request.is_cancelled()
            || !self
                .request
                .as_ref()
                .is_some_and(|current| current.same_connection(request))
        {
            return false;
        }
        self.attempted = true;
        true
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.invalidate();
    }
}

impl SourceProductWorkspace {
    pub(super) fn ensure_camera_identity(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.camera_identity.ensure_connection(
            cx.entity_id(),
            self.mapping_connection_generation,
            &self.device,
            self.exiting,
        ) else {
            return;
        };
        self.publish_camera_serial(CameraSerialObservation::Reading, cx);
        cx.emit(WorkspaceEvent::CameraIdentityRequested(request));
    }

    pub(in crate::features) fn camera_identity_request(&self) -> Option<&CameraIdentityRequested> {
        self.camera_identity.request.as_ref().filter(|request| {
            !self.exiting
                && request.matches_device(&self.device, self.mapping_connection_generation)
        })
    }

    pub(in crate::features) fn accepts_camera_identity(
        &self,
        request: &CameraIdentityRequested,
    ) -> bool {
        self.camera_identity_request()
            .is_some_and(|current| current.same_connection(request))
    }

    pub(in crate::features) fn observe_camera_identity(
        &mut self,
        request: &CameraIdentityRequested,
        result: Result<CameraSerialProgress, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_camera_identity(request)
            || result
                .as_ref()
                .is_ok_and(|progress| progress.product_id() != self.device.product_id)
        {
            return false;
        }
        let observation = match result {
            Ok(progress) => CameraSerialObservation::Read(progress),
            Err(error) => CameraSerialObservation::Failed(error),
        };
        if !self.camera_identity.complete(request) {
            return false;
        }
        self.publish_camera_serial(observation, cx);
        // setupDevice awaits the identity producer before camera settings.
        // A failed read is retained as failure and does not supply defaults.
        self.sync_camera_activity(cx);
        true
    }

    pub(super) fn publish_camera_serial(
        &mut self,
        observation: CameraSerialObservation,
        cx: &mut Context<Self>,
    ) {
        self.device.observe_camera_serial(observation.clone());
        self.saved.observe_camera_serial(observation);
        let device = self.device.clone();
        self.help
            .update(cx, |help, cx| help.set_device(&device, cx));
        cx.notify();
    }
}

#[cfg(test)]
#[path = "camera_identity_tests.rs"]
mod tests;
