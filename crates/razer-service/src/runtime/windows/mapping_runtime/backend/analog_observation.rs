//! Host-side analog callback fanout, scoped to the active device generation.
use super::*;
use device::analog_observation::{Control, same_key};
use razer_ipc::analog_observation::{Batch, Delivery, Receipt, Registration as Request, Target};

pub(super) struct Observer {
    pub(super) scope: Uuid,
    pub(super) caller: Uuid,
    pub(super) epoch: Uuid,
    registration: Request,
    pub(super) pending: Inbox<Delivery>,
}
impl<E: device::Executor> Core<E> {
    pub(super) fn start_analog_observation(
        &mut self,
        scope: Uuid,
        target: Target,
        id: String,
        registration: Request,
    ) -> Result<Value> {
        Uuid::parse_str(&id)?;
        ensure!(
            !self.analog_observers.contains_key(&id),
            "Analog observer is already registered"
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
            anyhow::bail!("Analog observer has no unique active device owner")
        };
        row.ensure_active()?;
        row.target.revalidate()?;
        let caller = row.caller;
        let previous = self
            .analog_observers
            .values()
            .find(|observer| observer.caller == caller);
        if let Some(previous) = previous {
            ensure!(
                compatible(&previous.registration, &registration),
                "Analog observation interval conflicts with the current registration"
            );
        }
        let epoch = previous.map_or_else(Uuid::new_v4, |observer| observer.epoch);
        // The host calls native registration for every single-key request, but
        // all-key/interval requests only install it for the first subscriber.
        if previous.is_none() || matches!(registration, Request::Key { .. }) {
            row.worker.analog_observation(Control::Register {
                epoch,
                registration: registration.clone(),
            })?;
        }
        self.analog_observers.insert(
            id.clone(),
            Observer {
                scope,
                caller,
                epoch,
                registration,
                pending: Inbox::new(epoch),
            },
        );
        Ok(serde_json::to_value(Receipt::new(id, true))?)
    }
    pub(super) fn poll_analog_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let observer = self
            .analog_observers
            .get_mut(id)
            .context("Analog observer is not registered")?;
        ensure!(observer.scope == scope, "Analog observer scope changed");
        let row = self
            .devices
            .values()
            .find(|row| row.caller == observer.caller)
            .context("Analog observed device was removed")?;
        row.ensure_active()?;
        if let Some(error) = &row.failure {
            anyhow::bail!("{error}");
        }
        Ok(serde_json::to_value(Batch::new(
            id.into(),
            observer.pending.drain()?,
        ))?)
    }
    pub(super) fn stop_analog_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let Some(observer) = self.analog_observers.get(id) else {
            return Ok(serde_json::to_value(Receipt::new(id.into(), false))?);
        };
        ensure!(observer.scope == scope, "Analog observer scope changed");
        let other = self
            .analog_observers
            .iter()
            .filter(|(other_id, other)| other_id.as_str() != id && other.caller == observer.caller)
            .map(|(_, other)| other)
            .collect::<Vec<_>>();
        if let Some(row) = self
            .devices
            .values()
            .find(|row| row.caller == observer.caller && !row.physical_retired)
        {
            if other.is_empty() {
                row.worker.analog_observation(Control::Stop {
                    epoch: observer.epoch,
                })?;
            } else if let Request::Key { input, .. } = &observer.registration {
                if !other.iter().any(|other| matches!(&other.registration, Request::Key { input: other_input, .. } if same_key(input, other_input))) {
                    row.worker.analog_observation(Control::RemoveKey { epoch: observer.epoch, input: input.clone() })?;
                }
            }
        }
        self.analog_observers.remove(id);
        Ok(serde_json::to_value(Receipt::new(id.into(), false))?)
    }
}
fn compatible(left: &Request, right: &Request) -> bool {
    match (left, right) {
        (Request::Interval { milliseconds: a }, Request::Interval { milliseconds: b }) => a == b,
        (Request::Interval { .. }, _) | (_, Request::Interval { .. }) => false,
        _ => true,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_subscriber_aliases_share_native_registration_identity() {
        use razer_device::mapping_native_lookup::{Input, InputKind};
        let keyboard = Input::new(InputKind::Keyboard {
            scancode: 30,
            flag: 2,
        });
        let analog = Input::new(InputKind::AnalogKey {
            scancode: 30,
            flag: 3,
        });
        let ordinary = Input::new(InputKind::AnalogKey {
            scancode: 30,
            flag: 0,
        });
        assert!(same_key(&keyboard, &analog));
        assert!(!same_key(&keyboard, &ordinary));
    }
    #[test]
    fn host_rejects_interval_mode_or_duration_conflicts() {
        assert!(compatible(
            &Request::All { skip: 25 },
            &Request::All { skip: 50 }
        ));
        assert!(!compatible(
            &Request::All { skip: 25 },
            &Request::Interval { milliseconds: 25 }
        ));
        assert!(!compatible(
            &Request::Interval { milliseconds: 25 },
            &Request::Interval { milliseconds: 50 }
        ));
        assert!(compatible(
            &Request::Interval { milliseconds: 25 },
            &Request::Interval { milliseconds: 25 }
        ));
    }
}
