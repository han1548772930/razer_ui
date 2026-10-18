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
    /// These requests complete their setter/ACK chain before returning. Retained
    /// operations acquire one lease around their entire start/poll/cleanup path.
    pub(crate) fn synchronous_device_task(&self) -> anyhow::Result<Option<Scope>> {
        if let Self::HyperpollingReceiver {
            target,
            command: razer_device::hyperpolling_receiver::Command::SetIndicator { .. },
        } = self
        {
            use razer_device::hyperpolling_receiver::Target;
            return Ok(Some(match target {
                Target::Collection(node) => Scope::collection(179, node),
                Target::WindowsContainer { container, .. } => Scope::container(179, container)?,
            }));
        }
        let collection = match self {
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
            } => Some((*product_id, node)),
            Self::HidNodeMouseCurrentPower {
                node,
                product_id,
                change,
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
            Self::DeviceWrite { target, .. }
            | Self::DeviceDpiStagesWrite { target, .. }
            | Self::DeviceMouseCurrentDpi {
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
