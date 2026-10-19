//! Source task locks shared by device submission callers. Ordinary synchronous
//! tasks wait for an exclusive operation; calibration requests fail immediately
//! when its physical route is already engaged.
use crate::ServiceRequest;
use anyhow::{Context as _, ensure};
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scope {
    product_id: u32,
    endpoint: Endpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Endpoint {
    Collection(Vec<u8>),
    Container(String),
}

impl Scope {
    pub fn collection(product_id: u32, node: &razer_device::backend::HidNode) -> Self {
        let path = if cfg!(windows) {
            node.path.to_ascii_lowercase()
        } else {
            node.path.clone()
        };
        Self {
            product_id,
            endpoint: Endpoint::Collection(path),
        }
    }

    pub fn container(product_id: u32, container: &str) -> anyhow::Result<Self> {
        ensure!(!container.is_empty(), "Device task scope has no container");
        Ok(Self {
            product_id,
            endpoint: Endpoint::Container(container.to_ascii_lowercase()),
        })
    }

    pub fn target(target: &razer_device::device_reads::DeviceReadTarget) -> anyhow::Result<Self> {
        Self::container(target.product_id, &target.device_container_id)
    }
}

#[derive(Default)]
struct Access {
    shared: usize,
    exclusive: bool,
}

#[derive(Clone, Default)]
pub struct Registry(Arc<(Mutex<BTreeMap<Scope, Access>>, Condvar)>);

impl Registry {
    pub fn global() -> Self {
        static GLOBAL: OnceLock<Registry> = OnceLock::new();
        GLOBAL.get_or_init(Self::default).clone()
    }

    pub fn try_exclusive(&self, scope: Scope) -> anyhow::Result<Lease> {
        let mut scopes = self
            .0
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Device task registry poisoned"))?;
        let access = scopes.entry(scope.clone()).or_default();
        ensure!(
            !access.exclusive && access.shared == 0,
            "EXTERNAL_SERVICE_ENGAGING: device task scope is occupied"
        );
        access.exclusive = true;
        Ok(Lease {
            registry: self.clone(),
            scope,
            exclusive: true,
        })
    }

    pub fn shared(
        &self,
        scope: Scope,
        mut canceled: impl FnMut() -> bool,
    ) -> anyhow::Result<Lease> {
        let mut scopes = self
            .0
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Device task registry poisoned"))?;
        loop {
            ensure!(!canceled(), "Device task scope canceled while waiting");
            let access = scopes.entry(scope.clone()).or_default();
            if !access.exclusive {
                access.shared = access
                    .shared
                    .checked_add(1)
                    .context("Device task lease count overflow")?;
                return Ok(Lease {
                    registry: self.clone(),
                    scope,
                    exclusive: false,
                });
            }
            (scopes, _) = self
                .0
                .1
                .wait_timeout(scopes, Duration::from_millis(50))
                .map_err(|_| anyhow::anyhow!("Device task registry poisoned while waiting"))?;
        }
    }
}

pub struct Lease {
    registry: Registry,
    scope: Scope,
    exclusive: bool,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut scopes) = self.registry.0.0.lock() {
            if let Some(access) = scopes.get_mut(&self.scope) {
                if self.exclusive {
                    access.exclusive = false;
                } else {
                    access.shared -= 1;
                }
                if !access.exclusive && access.shared == 0 {
                    scopes.remove(&self.scope);
                }
            }
            self.registry.0.1.notify_all();
        }
    }
}

impl ServiceRequest {
    /// Synchronous requests finish their device chain before returning. Retained
    /// operations acquire one lease around their entire start/poll/cleanup path.
    pub(crate) fn synchronous_device_task(&self) -> anyhow::Result<Option<Scope>> {
        if let Self::KeyboardProfileInventoryRead { target }
            | Self::KeyboardOnboardProfilesRead { target, .. } = self {
            use crate::keyboard_profile_inventory::Target;
            target.validate()?;
            if let Self::KeyboardOnboardProfilesRead { options, .. } = self {
                crate::keyboard_onboard_profiles::validate_target(target)?;
                options.validate()?;
            }
            return Ok(Some(match target {
                Target::Collection {
                    node, product_id, ..
                } => Scope::collection(*product_id, node),
                Target::WindowsContainer { target } => Scope::target(target)?,
            }));
        }
        if let Self::BleMouseRotation {
            target,
            change: Some(_),
        } = self
        {
            anyhow::ensure!(
                razer_device::mouse_rotation::ble::accepts(
                    target.product_id,
                    target.physical_product_id
                ),
                "Rotation BLE mutation scope has another physical factory"
            );
            return Ok(Some(Scope::container(
                target.product_id,
                &target.device_container_id,
            )?));
        }
        if let Self::HyperpollingReceiver {
            target,
            command:
                razer_device::hyperpolling_receiver::Command::SetIndicator { .. }
                | razer_device::hyperpolling_receiver::Command::CancelScan,
            ..
        } = self
        {
            use razer_device::hyperpolling_receiver::Target;
            return Ok(Some(match target {
                Target::Collection(node) => Scope::collection(179, node),
                Target::WindowsContainer { container, .. } => Scope::container(179, container)?,
            }));
        }
        let collection = match self {
            Self::HidNodeMouseBrightness {
                node,
                product_id,
                submission,
                ..
            } if submission.is_write() => Some((*product_id, node)),
            Self::HidNodeWrite {
                node, product_id, ..
            }
            | Self::HidNodeDpiStagesWrite {
                node, product_id, ..
            }
            | Self::HidNodeMouseReceiverDpiStages {
                node,
                product_id,
                draft: Some(_),
                ..
            }
            | Self::HidNodeMouseReceiverPolling {
                node, product_id, ..
            }
            | Self::HidNodeMouseReceiverIdle {
                node, product_id, ..
            }
            | Self::HidNodeMouseCurrentDpi {
                node,
                product_id,
                change: Some(_),
                ..
            }
            | Self::HidNodeMouseCurrentPolling {
                node, product_id, ..
            } => Some((*product_id, node)),
            Self::HidNodeMouseDynamic {
                node,
                product_id,
                change: Some(_),
                ..
            }
            | Self::HidNodeMouseRotation {
                node,
                product_id,
                change: Some(_),
                ..
            } => Some((*product_id, node)),
            Self::HidNodeMouseCurrentPower {
                node,
                product_id,
                change,
                ..
            } if !matches!(
                change,
                razer_device::mouse_current_power::Change::ReadPowerSaving
            ) =>
            {
                Some((*product_id, node))
            }
            _ => None,
        };
        if let Some((product_id, node)) = collection {
            return Ok(Some(Scope::collection(product_id, node)));
        }
        let target = match self {
            Self::DeviceMouseBrightness { target, submission } if submission.is_write() => {
                Some(target)
            }
            Self::DeviceWrite { target, .. }
            | Self::DeviceDpiStagesWrite { target, .. }
            | Self::DeviceMouseCurrentDpi {
                target,
                change: Some(_),
            }
            | Self::DeviceMouseCurrentPolling { target, .. } => Some(target),
            Self::DeviceMouseDynamic {
                target,
                change: Some(_),
            }
            | Self::DeviceMouseRotation {
                target,
                change: Some(_),
            } => Some(target),
            Self::DeviceMouseCurrentPower { target, change }
                if !matches!(
                    change,
                    razer_device::mouse_current_power::Change::ReadPowerSaving
                ) =>
            {
                Some(target)
            }
            _ => None,
        };
        target.map(Scope::target).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_rotation_ble_ipc_uses_logical_container_scope_only_for_setter() {
        let container = "b63bfcb4-c1ee-4b2b-83f3-90fc06229af0";
        let value = serde_json::json!({"operation":"ble_mouse_rotation","target":{
            "product_id":226,"physical_product_id":228,"device_container_id":container,
            "hid_path":"hid-ble-rotation","device_instance_id":"HID-INSTANCE",
            "parent_instance_id":"BTHLEDevice-INSTANCE","mac_address":177789161760246u64},
            "change":{"enabled":true,"angle_degrees":-44}});
        let request: ServiceRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            request.synchronous_device_task().unwrap(),
            Some(Scope::container(226, container).unwrap())
        );
        let mut read = value.clone();
        read["change"] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<ServiceRequest>(read)
                .unwrap()
                .synchronous_device_task()
                .unwrap()
                .is_none()
        );
        let mut other = value;
        other["target"]["physical_product_id"] = serde_json::json!(227);
        assert!(
            serde_json::from_value::<ServiceRequest>(other)
                .unwrap()
                .synchronous_device_task()
                .is_err()
        );
    }

    #[test]
    fn mouse_rotation_ipc_keeps_wire_identity_and_only_setters_take_mutation_scope() {
        let bytes = br#"{"operation":"hid_node_mouse_rotation","node":{"path":[255,0,34,92,128],"vendor_id":5426,"product_id":241,"usage_page":65280,"usage":1,"interface_number":0,"release_number":4660,"serial_number":null,"manufacturer":null,"product":null},"product_id":226,"peer_product_id":227,"change":{"enabled":true,"angle_degrees":-44}}"#;
        let request: ServiceRequest = serde_json::from_slice(bytes).unwrap();
        assert_eq!(serde_json::to_vec(&request).unwrap(), bytes);
        let ServiceRequest::HidNodeMouseRotation { ref node, .. } = request else {
            panic!("rotation wire kind changed")
        };
        assert_eq!(
            request.synchronous_device_task().unwrap(),
            Some(Scope::collection(226, node))
        );
        let mut read: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        read["change"] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<ServiceRequest>(read)
                .unwrap()
                .synchronous_device_task()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn current_power_ipc_preserves_fixed_wire_bytes_and_collection_task_identity() {
        let bytes = br#"{"operation":"hid_node_mouse_current_power","node":{"path":[255,0,34,92,128],"vendor_id":5426,"product_id":241,"usage_page":65280,"usage":1,"interface_number":0,"release_number":4660,"serial_number":null,"manufacturer":null,"product":null},"product_id":226,"peer_product_id":227,"change":{"field":"power_saving","value":2}}"#;
        let request: ServiceRequest = serde_json::from_slice(bytes).unwrap();
        let ServiceRequest::HidNodeMouseCurrentPower {
            node,
            product_id,
            peer_product_id,
            change,
        } = &request
        else {
            panic!("Power operation changed");
        };
        assert_eq!((*product_id, *peer_product_id), (226, Some(227)));
        assert_eq!(node.path, [255, 0, 34, 92, 128]);
        assert_eq!(
            *change,
            razer_device::mouse_current_power::Change::PowerSaving(2)
        );
        assert_eq!(serde_json::to_vec(&request).unwrap().as_slice(), bytes);
        assert_eq!(
            request.synchronous_device_task().unwrap(),
            Some(Scope::collection(226, node))
        );
    }

    #[test]
    fn current_power_ipc_reads_legacy_messages_without_inventing_a_peer() {
        let bytes = br#"{"operation":"hid_node_mouse_current_power","node":{"path":[255,0,34,92,128],"vendor_id":5426,"product_id":241,"usage_page":65280,"usage":1,"interface_number":0,"release_number":4660,"serial_number":null,"manufacturer":null,"product":null},"product_id":226,"change":{"field":"read_power_saving"}}"#;
        let request: ServiceRequest = serde_json::from_slice(bytes).unwrap();
        assert!(request.synchronous_device_task().unwrap().is_none());
        assert!(matches!(
            request,
            ServiceRequest::HidNodeMouseCurrentPower {
                product_id: 226,
                peer_product_id: None,
                change: razer_device::mouse_current_power::Change::ReadPowerSaving,
                ..
            }
        ));
        let mut wrong_type: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        wrong_type["peer_product_id"] = serde_json::json!("227");
        assert!(serde_json::from_value::<ServiceRequest>(wrong_type).is_err());
    }

    #[test]
    fn device_task_exclusive_conflicts_with_all_shared_owners_and_preserves_other_routes() {
        let registry = Registry::default();
        let current = Scope::container(190, "CURRENT-CONTAINER").unwrap();
        let shared = registry.shared(current.clone(), || false).unwrap();
        let second = registry.shared(current.clone(), || false).unwrap();
        assert!(registry.try_exclusive(current.clone()).is_err());
        drop(shared);
        assert!(registry.try_exclusive(current.clone()).is_err());
        drop(second);
        let exclusive = registry.try_exclusive(current.clone()).unwrap();
        assert!(
            registry
                .try_exclusive(Scope::container(190, "current-container").unwrap())
                .is_err()
        );
        assert!(registry.shared(current.clone(), || true).is_err());
        let other = registry
            .try_exclusive(Scope::container(190, "other-container").unwrap())
            .unwrap();
        drop((exclusive, other));
        assert!(registry.try_exclusive(current).is_ok());
    }

    #[test]
    fn device_task_shared_wait_is_cancellable_before_any_setter_is_submitted() {
        let registry = Registry::default();
        let scope = Scope::container(190, "device").unwrap();
        let exclusive = registry.try_exclusive(scope.clone()).unwrap();
        let error = registry.shared(scope.clone(), || true).err().unwrap();
        assert!(error.to_string().contains("canceled"));
        drop(exclusive);
        assert!(registry.shared(scope, || false).is_ok());
    }

    #[test]
    fn device_task_classification_guards_setters_without_locking_unrelated_getters() {
        let target = razer_device::device_reads::DeviceReadTarget {
            product_id: 190,
            physical_product_id: 190,
            peer_product_id: None,
            device_container_id: "CONTAINER".into(),
            path: "device-path".into(),
        };
        let read = ServiceRequest::DeviceMouseCurrentDpi {
            target: target.clone(),
            change: None,
        };
        assert!(read.synchronous_device_task().unwrap().is_none());
        let change = razer_device::mouse_current_dpi::DpiChange {
            stages: None,
            active: Some(razer_device::mouse_current_dpi::DpiPair { x: 800, y: 800 }),
            profile_guid: None,
            initialize: false,
        };
        let write = ServiceRequest::DeviceMouseCurrentDpi {
            target,
            change: Some(change),
        };
        assert_eq!(
            write.synchronous_device_task().unwrap(),
            Some(Scope::container(190, "container").unwrap())
        );
        assert!(
            ServiceRequest::Shutdown
                .synchronous_device_task()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn current_polling_ipc_preserves_slot_scope_and_temporary_semantics() {
        let target = razer_device::device_reads::DeviceReadTarget {
            product_id: 190,
            physical_product_id: 191,
            peer_product_id: None,
            device_container_id: "CONTAINER".into(),
            path: "device-path".into(),
        };
        let request = ServiceRequest::DeviceMouseCurrentPolling {
            target,
            change: Some(
                razer_device::mouse_current_polling::Change::new(8000)
                    .with_profile_guid("active-guid".into()),
            ),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"operation":"device_mouse_current_polling", "target":{
            "product_id":190,"physical_product_id":191,"peer_product_id":null,"device_container_id":"CONTAINER","path":"device-path"
        },"change":{"hz":8000,"profile_guid":"active-guid"}})
        );
        assert_eq!(
            request.synchronous_device_task().unwrap(),
            Some(Scope::container(190, "container").unwrap())
        );
        let temporary: ServiceRequest = serde_json::from_value(serde_json::json!({"operation":"device_mouse_current_polling", "target":json["target"], "change":{"hz":500,"profile_guid":null}})).unwrap();
        let ServiceRequest::DeviceMouseCurrentPolling { change, .. } = temporary else {
            panic!("wrong request")
        };
        let change = change.unwrap();
        assert_eq!(change.hz(), 500);
        assert_eq!(change.profile_guid(), None);
        let query: ServiceRequest = serde_json::from_value(serde_json::json!({"operation":"device_mouse_current_polling", "target":json["target"], "change":null})).unwrap();
        assert_eq!(
            query.synchronous_device_task().unwrap(),
            Some(Scope::container(190, "container").unwrap())
        );
        assert!(matches!(
            query,
            ServiceRequest::DeviceMouseCurrentPolling { change: None, .. }
        ));
    }

    #[test]
    fn device_task_waiting_shared_operation_resumes_only_after_exclusive_owner_drops() {
        let registry = Registry::default();
        let scope = Scope::container(190, "current").unwrap();
        let exclusive = registry.try_exclusive(scope.clone()).unwrap();
        let (waiting, observed) = std::sync::mpsc::sync_channel(1);
        let (acquired, received) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut waiting = Some(waiting);
            let _shared = registry
                .shared(scope, || {
                    if let Some(waiting) = waiting.take() {
                        waiting.send(()).unwrap();
                    }
                    false
                })
                .unwrap();
            acquired.send(()).unwrap();
        });
        observed.recv().unwrap();
        assert!(received.try_recv().is_err());
        drop(exclusive);
        received.recv().unwrap();
        worker.join().unwrap();
    }
}
