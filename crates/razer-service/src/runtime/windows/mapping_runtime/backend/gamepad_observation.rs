//! Host fanout of computed output samples; the first subscriber owns the mode.
use super::*;
use device::gamepad_observation::Control;
use razer_ipc::gamepad_observation::{Batch, Delivery, Receipt, Registration as Request, Target};

pub(super) struct Observer {
    pub(super) scope: Uuid,
    pub(super) caller: Uuid,
    pub(super) epoch: Uuid,
    registration: Request,
    pending_failure: Option<String>,
    pub(super) pending: Inbox<Delivery>,
}
type Observers = BTreeMap<String, Observer>;
fn compatible(first: &Request, next: &Request) -> bool {
    // Host interval=0 is also its normal-mode sentinel, even when the first
    // native registration actually requested an interval of zero.
    let interval = match first {
        Request::Events { .. } => 0,
        Request::Interval { milliseconds } => *milliseconds,
    };
    match next {
        Request::Events { .. } => interval == 0,
        Request::Interval { milliseconds } => interval != 0 && interval == *milliseconds,
    }
}
fn register(
    observers: &mut Observers,
    scope: Uuid,
    caller: Uuid,
    id: String,
    registration: Request,
    activate: impl FnOnce(Uuid, Request) -> Result<()>,
) -> Result<()> {
    Uuid::parse_str(&id)?;
    ensure!(
        !observers.contains_key(&id),
        "Gamepad observer is already registered"
    );
    let previous = observers
        .values()
        .find(|observer| observer.caller == caller);
    if let Some(previous) = previous {
        ensure!(
            compatible(&previous.registration, &registration),
            "Gamepad observation interval conflicts with the current registration"
        );
    }
    let epoch = previous.map_or_else(Uuid::new_v4, |observer| observer.epoch);
    if previous.is_none() {
        activate(epoch, registration.clone())?;
    }
    observers.insert(
        id,
        Observer {
            scope,
            caller,
            epoch,
            registration,
            pending_failure: None,
            pending: Inbox::new(epoch),
        },
    );
    Ok(())
}
fn unregister(
    observers: &mut Observers,
    scope: Uuid,
    id: &str,
    deactivate: impl FnOnce(Uuid, Uuid) -> Result<()>,
) -> Result<()> {
    let Some(observer) = observers.get(id) else {
        return Ok(());
    };
    ensure!(observer.scope == scope, "Gamepad observer scope changed");
    if !observers
        .iter()
        .any(|(other_id, other)| other_id != id && other.caller == observer.caller)
    {
        deactivate(observer.caller, observer.epoch)?;
    }
    observers.remove(id);
    Ok(())
}
pub(super) fn notify(observers: &mut Observers, caller: Uuid, epoch: Uuid, delivery: Delivery) {
    for observer in observers
        .values_mut()
        .filter(|observer| observer.caller == caller && observer.epoch == epoch)
    {
        observer.pending.push(epoch, delivery.clone());
    }
}
fn drain(observer: &mut Observer, scope: Uuid, failure: Option<&str>) -> Result<Vec<Delivery>> {
    ensure!(observer.scope == scope, "Gamepad observer scope changed");
    // Output callbacks precede transport completion. Preserve those samples,
    // then report the independently retained execution failure on the next poll.
    if let Some(error) = &observer.pending_failure {
        anyhow::bail!("{error}");
    }
    let deliveries = observer.pending.drain()?;
    if let Some(error) = failure {
        if deliveries.is_empty() {
            anyhow::bail!("{error}");
        }
        observer.pending_failure = Some(error.into());
    }
    Ok(deliveries)
}
impl<E: device::Executor> Core<E> {
    pub(super) fn start_gamepad_observation(
        &mut self,
        scope: Uuid,
        target: Target,
        id: String,
        registration: Request,
    ) -> Result<Value> {
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
            anyhow::bail!("Gamepad observer has no unique active device owner");
        };
        row.ensure_active()?;
        row.target.revalidate()?;
        register(
            &mut self.gamepad_observers,
            scope,
            row.caller,
            id.clone(),
            registration,
            |epoch, registration| {
                row.worker.gamepad_observation(Control::Register {
                    epoch,
                    registration,
                })
            },
        )?;
        Ok(serde_json::to_value(Receipt::new(id, true))?)
    }
    pub(super) fn poll_gamepad_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let observer = self
            .gamepad_observers
            .get_mut(id)
            .context("Gamepad observer is not registered")?;
        ensure!(observer.scope == scope, "Gamepad observer scope changed");
        let row = self
            .devices
            .values()
            .find(|row| row.caller == observer.caller)
            .context("Gamepad observed device was removed")?;
        row.ensure_active()?;
        Ok(serde_json::to_value(Batch::new(
            id.into(),
            drain(observer, scope, row.failure.as_deref())?,
        ))?)
    }
    pub(super) fn stop_gamepad_observation(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        unregister(&mut self.gamepad_observers, scope, id, |caller, epoch| {
            if let Some(row) = self
                .devices
                .values()
                .find(|row| row.caller == caller && !row.physical_retired)
            {
                row.worker.gamepad_observation(Control::Stop { epoch })?;
            }
            Ok(())
        })?;
        Ok(serde_json::to_value(Receipt::new(id.into(), false))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::mapping_native_output::GamepadOutputState;
    fn delivery(n: u8) -> Delivery {
        Delivery::new(
            vec![GamepadOutputState {
                left_trigger: n,
                ..Default::default()
            }],
            u64::from(n),
        )
    }
    #[test]
    fn scopes_share_first_registration_and_last_stop_preserves_other_device() {
        let mut observers = Observers::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let caller = Uuid::new_v4();
        let other = Uuid::new_v4();
        let ids = [
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
        ];
        register(
            &mut observers,
            a,
            caller,
            ids[0].clone(),
            Request::Events { skip: 50 },
            |_, _| Ok(()),
        )
        .unwrap();
        let epoch = observers[&ids[0]].epoch;
        register(
            &mut observers,
            b,
            caller,
            ids[1].clone(),
            Request::Events { skip: 0 },
            |_, _| panic!("second native registration"),
        )
        .unwrap();
        register(
            &mut observers,
            b,
            other,
            ids[2].clone(),
            Request::Events { skip: 50 },
            |_, _| Ok(()),
        )
        .unwrap();
        notify(&mut observers, caller, epoch, delivery(1));
        assert!(drain(observers.get_mut(&ids[0]).unwrap(), b, None).is_err());
        assert!(unregister(&mut observers, b, &ids[0], |_, _| panic!()).is_err());
        unregister(&mut observers, a, &ids[0], |_, _| {
            panic!("other subscriber still active")
        })
        .unwrap();
        assert_eq!(
            drain(observers.get_mut(&ids[1]).unwrap(), b, None).unwrap(),
            vec![delivery(1)]
        );
        assert!(
            drain(observers.get_mut(&ids[2]).unwrap(), b, None)
                .unwrap()
                .is_empty()
        );
        let mut stopped = false;
        unregister(&mut observers, b, &ids[1], |c, e| {
            assert_eq!((c, e), (caller, epoch));
            stopped = true;
            Ok(())
        })
        .unwrap();
        assert!(stopped);
        assert!(observers.contains_key(&ids[2]));
    }
    #[test]
    fn failed_registration_stop_and_stale_inflight_notification_keep_lifetime_exact() {
        let mut observers = Observers::new();
        let scope = Uuid::new_v4();
        let caller = Uuid::new_v4();
        let id = Uuid::new_v4().to_string();
        assert!(
            register(
                &mut observers,
                scope,
                caller,
                id.clone(),
                Request::Events { skip: 50 },
                |_, _| anyhow::bail!("register failed")
            )
            .is_err()
        );
        assert!(observers.is_empty());
        register(
            &mut observers,
            scope,
            caller,
            id.clone(),
            Request::Events { skip: 50 },
            |_, _| Ok(()),
        )
        .unwrap();
        let retired = observers[&id].epoch;
        notify(&mut observers, caller, retired, delivery(1));
        assert!(
            unregister(&mut observers, scope, &id, |_, _| anyhow::bail!(
                "stop failed"
            ))
            .is_err()
        );
        assert!(observers.contains_key(&id));
        unregister(&mut observers, scope, &id, |_, _| Ok(())).unwrap();
        register(
            &mut observers,
            scope,
            caller,
            id.clone(),
            Request::Events { skip: 50 },
            |_, _| Ok(()),
        )
        .unwrap();
        let current = observers[&id].epoch;
        assert_ne!(retired, current);
        notify(&mut observers, caller, retired, delivery(2));
        assert!(
            drain(observers.get_mut(&id).unwrap(), scope, None)
                .unwrap()
                .is_empty()
        );
        notify(&mut observers, caller, current, delivery(3));
        assert_eq!(
            drain(
                observers.get_mut(&id).unwrap(),
                scope,
                Some("driver failed")
            )
            .unwrap(),
            vec![delivery(3)]
        );
        assert!(
            drain(
                observers.get_mut(&id).unwrap(),
                scope,
                Some("driver failed")
            )
            .unwrap_err()
            .to_string()
            .contains("driver failed")
        );
    }
    #[test]
    fn host_interval_zero_sentinel_and_nonzero_conflicts_follow_source() {
        assert!(compatible(
            &Request::Interval { milliseconds: 0 },
            &Request::Events { skip: 50 }
        ));
        assert!(!compatible(
            &Request::Interval { milliseconds: 0 },
            &Request::Interval { milliseconds: 0 }
        ));
        assert!(!compatible(
            &Request::Events { skip: 50 },
            &Request::Interval { milliseconds: 0 }
        ));
        assert!(compatible(
            &Request::Interval { milliseconds: 5 },
            &Request::Interval { milliseconds: 5 }
        ));
        assert!(!compatible(
            &Request::Interval { milliseconds: 5 },
            &Request::Interval { milliseconds: 10 }
        ));
        assert!(!compatible(
            &Request::Interval { milliseconds: 5 },
            &Request::Events { skip: 50 }
        ));
    }
}
