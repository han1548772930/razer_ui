//! Snap Tap callback registration belongs to an observed device generation.
use super::*;
use razer_ipc::mapping_observation::{Batch, Receipt, SnapTapEvent, Target};

pub(super) struct Observer<E = SnapTapEvent> {
    pub(super) scope: Uuid,
    pub(super) caller: Uuid,
    pub(super) epoch: Uuid,
    stopping: bool,
    pub(super) pending: Inbox<E>,
}
impl<E> Observer<E> {
    pub(super) fn push(&mut self, caller: Uuid, epoch: Uuid, event: E) {
        if self.caller == caller && self.epoch == epoch && !self.stopping {
            self.pending.push(epoch, event);
        }
    }
}

impl<E: device::Executor> Core<E> {
    pub(super) fn start_observation(
        &mut self,
        scope: Uuid,
        target: Target,
        id: String,
    ) -> Result<Value> {
        Uuid::parse_str(&id)?;
        ensure!(
            !self.observers.contains_key(&id),
            "Mapping observer is already registered"
        );
        let rows = self
            .devices
            .values()
            .filter(|row| {
                row.target.logical_product.product_id() == target.product_id()
                    && row
                        .target
                        .info
                        .container_id
                        .eq_ignore_ascii_case(target.container_id())
                    && !row.physical_retired
            })
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            anyhow::bail!("Mapping observer has no unique active device owner")
        };
        row.ensure_active()?;
        row.target.revalidate()?;
        let caller = row.caller;
        ensure!(
            !self
                .observers
                .values()
                .any(|observer| observer.caller == caller && observer.stopping),
            "Mapping observation cleanup is still pending"
        );
        let epoch = self
            .observers
            .values()
            .find(|observer| observer.caller == caller)
            .map(|observer| observer.epoch)
            .unwrap_or_else(Uuid::new_v4);
        // The first consumer installs both native-equivalent registration and
        // callback. Later consumers share that device-owned registration.
        row.worker.snaptap_observation(Some(epoch))?;
        self.observers.insert(
            id.clone(),
            Observer {
                scope,
                caller,
                epoch,
                stopping: false,
                pending: Inbox::new(epoch),
            },
        );
        Ok(serde_json::to_value(Receipt::new(id, true))?)
    }

    pub(super) fn poll_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let observer = self
            .observers
            .get_mut(id)
            .context("Mapping observer is not registered")?;
        ensure!(observer.scope == scope, "Mapping observer scope changed");
        ensure!(
            !observer.stopping,
            "Mapping observation cleanup is still pending"
        );
        let row = self
            .devices
            .values()
            .find(|row| row.caller == observer.caller)
            .context("Mapping observed device was removed")?;
        row.ensure_active()?;
        if let Some(error) = &row.failure {
            anyhow::bail!("{error}");
        }
        Ok(serde_json::to_value(Batch::new(
            id.into(),
            observer.pending.drain()?,
        ))?)
    }

    pub(super) fn stop_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let Some(observer) = self.observers.get_mut(id) else {
            return Ok(serde_json::to_value(Receipt::new(id.into(), false))?);
        };
        ensure!(observer.scope == scope, "Mapping observer scope changed");
        observer.stopping = true;
        let caller = observer.caller;
        if !self
            .observers
            .iter()
            .any(|(other_id, other)| other_id != id && other.caller == caller)
        {
            if let Some(row) = self
                .devices
                .values()
                .find(|row| row.caller == caller && !row.physical_retired)
            {
                row.worker.snaptap_observation(None)?;
            }
        }
        self.observers.remove(id);
        Ok(serde_json::to_value(Receipt::new(id.into(), false))?)
    }

    pub(super) fn start_input_observation(
        &mut self,
        scope: Uuid,
        target: Target,
        id: String,
    ) -> Result<Value> {
        Uuid::parse_str(&id)?;
        ensure!(
            !self.input_observers.contains_key(&id),
            "Input observer is already registered"
        );
        let rows = self
            .devices
            .values()
            .filter(|row| {
                row.target.logical_product.product_id() == target.product_id()
                    && row
                        .target
                        .info
                        .container_id
                        .eq_ignore_ascii_case(target.container_id())
                    && !row.physical_retired
            })
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            anyhow::bail!("Input observer has no unique active device owner")
        };
        row.ensure_active()?;
        row.target.revalidate()?;
        let caller = row.caller;
        ensure!(
            !self
                .input_observers
                .values()
                .any(|observer| observer.caller == caller && observer.stopping),
            "Input observation cleanup is still pending"
        );
        let lease = Lease::Device {
            scope: row.scope,
            caller,
        };
        let epoch = self
            .input_observers
            .values()
            .find(|observer| observer.caller == caller)
            .map(|observer| observer.epoch)
            .unwrap_or_else(Uuid::new_v4);
        // AAE10 sets the callback registration flag on the already valid driver
        // owner. This does not redirect input or suspend its mapping execution.
        self.connections.notifications(&lease, true)?;
        if let Err(error) = row.worker.input_subscriber(Some(epoch)) {
            let mut errors = vec![format!("Input callback registration: {error:#}")];
            if !row.input_registered
                && !self
                    .input_observers
                    .values()
                    .any(|observer| observer.caller == caller)
            {
                if let Err(error) = self.connections.notifications(&lease, false) {
                    errors.push(format!("Input callback cleanup: {error:#}"));
                }
            }
            anyhow::bail!("{}", errors.join("; "));
        }
        self.input_observers.insert(
            id.clone(),
            Observer {
                scope,
                caller,
                epoch,
                stopping: false,
                pending: Inbox::new(epoch),
            },
        );
        Ok(serde_json::to_value(Receipt::new(id, true))?)
    }

    pub(super) fn poll_input_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let observer = self
            .input_observers
            .get_mut(id)
            .context("Input observer is not registered")?;
        ensure!(observer.scope == scope, "Input observer scope changed");
        ensure!(
            !observer.stopping,
            "Input observation cleanup is still pending"
        );
        let row = self
            .devices
            .values()
            .find(|row| row.caller == observer.caller)
            .context("Input observed device was removed")?;
        row.ensure_active()?;
        if let Some(error) = &row.failure {
            anyhow::bail!("{error}");
        }
        Ok(serde_json::to_value(
            razer_ipc::input_observation::Batch::new(id.into(), observer.pending.drain()?),
        )?)
    }

    pub(super) fn stop_input_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let Some(observer) = self.input_observers.get_mut(id) else {
            return Ok(serde_json::to_value(Receipt::new(id.into(), false))?);
        };
        ensure!(observer.scope == scope, "Input observer scope changed");
        observer.stopping = true;
        let caller = observer.caller;
        if !self
            .input_observers
            .iter()
            .any(|(other_id, other)| other_id != id && other.caller == caller)
        {
            if let Some(row) = self
                .devices
                .values()
                .find(|row| row.caller == caller && !row.physical_retired)
            {
                row.worker.input_subscriber(None)?;
                // The retained OTFM listener has the same native callback;
                // removing a page must not disable its notification ownership.
                if !row.input_registered {
                    self.connections.notifications(
                        &Lease::Device {
                            scope: row.scope,
                            caller,
                        },
                        false,
                    )?;
                }
            }
        }
        self.input_observers.remove(id);
        Ok(serde_json::to_value(Receipt::new(id.into(), false))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_observer_fanout_requires_both_device_and_subscription_generation() {
        let caller = Uuid::from_u128(1);
        let epoch = Uuid::from_u128(2);
        let mut first = Observer {
            scope: Uuid::from_u128(3),
            caller,
            epoch,
            stopping: false,
            pending: Inbox::new(epoch),
        };
        let mut second = Observer {
            scope: Uuid::from_u128(4),
            caller,
            epoch,
            stopping: false,
            pending: Inbox::new(epoch),
        };
        for observer in [&mut first, &mut second] {
            observer.push(Uuid::from_u128(5), epoch, "stale device");
            observer.push(caller, Uuid::from_u128(6), "stale subscription");
            observer.push(caller, epoch, "physical key");
        }
        assert_eq!(first.pending.drain().unwrap(), ["physical key"]);
        assert_eq!(second.pending.drain().unwrap(), ["physical key"]);
    }
}
