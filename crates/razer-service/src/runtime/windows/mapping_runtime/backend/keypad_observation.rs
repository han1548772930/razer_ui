//! PID747 read-only hardware ownership, independent of mapping publication.
use super::*;
use razer_device::keyboard_keypad_observation::{Throttle, decode_report};
use razer_ipc::keypad_observation::{Batch, Delivery, Receipt, Target};
use serde::Deserialize;

pub(super) trait Input {
    fn validate(&self) -> Result<()>;
    fn inputs(&mut self) -> Result<Vec<Result<hardware::Packet, String>>>;
    fn shutdown(&mut self) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PhysicalIdentity {
    vendor_id: u32,
    product_id: u32,
    device_container_id: String,
    device_instance_id: String,
    path: String,
}
impl PhysicalIdentity {
    fn observe(target: &Target) -> Result<Self> {
        ensure!(
            target.product_id() == 747,
            "Keypad hardware observer requires product 747"
        );
        let (product, physical) =
            super::super::super::native_target(target.container_id(), Some(747))?;
        let physical: Self = serde_json::from_value(physical)?;
        ensure!(
            product == 747 && physical.product_id == 747 && physical.vendor_id == 0x1532,
            "Keypad physical identity differs from the current wired product"
        );
        ensure!(
            physical
                .device_container_id
                .eq_ignore_ascii_case(target.container_id())
                && !physical.device_instance_id.is_empty()
                && !physical.path.is_empty(),
            "Keypad hardware physical identity is incomplete"
        );
        Ok(physical)
    }
}
pub(super) struct HardwareInput {
    target: Target,
    physical: PhysicalIdentity,
    input: hardware::Owner,
}
impl HardwareInput {
    fn open(target: &Target) -> Result<Self> {
        let physical = PhysicalIdentity::observe(target)?;
        let info = identity::DeviceInfo {
            vendor_id: physical.vendor_id,
            product_id: physical.product_id,
            container_id: physical.device_container_id.clone(),
            usb_instance_id: physical.device_instance_id.clone(),
            guid: Uuid::new_v4().to_string(),
        };
        // This owner opens only GENERIC_READ HID handles. It neither creates a
        // filter-driver route nor publishes mappings or sends feature reports.
        let input = hardware::Owner::open(&info)?;
        let mut owner = Self {
            target: target.clone(),
            physical,
            input,
        };
        if let Err(error) = owner.validate() {
            let cleanup = owner.input.shutdown();
            anyhow::bail!("Keypad hardware changed during open: {error:#}; cleanup: {cleanup:?}");
        }
        Ok(owner)
    }
}
impl Input for HardwareInput {
    fn validate(&self) -> Result<()> {
        ensure!(
            PhysicalIdentity::observe(&self.target)? == self.physical,
            "Keypad hardware physical connection changed"
        );
        Ok(())
    }
    fn inputs(&mut self) -> Result<Vec<Result<hardware::Packet, String>>> {
        self.input.inputs()
    }
    fn shutdown(&mut self) -> Result<()> {
        self.input.shutdown()
    }
}

#[derive(Default)]
struct Samples {
    endpoints: BTreeMap<String, hardware::EndpointIdentity>,
    current: Option<hardware::EndpointIdentity>,
    throttle: Throttle,
}
impl Samples {
    fn consume(&mut self, packet: hardware::Packet) -> Result<Vec<Delivery>> {
        let Some(report) = decode_report(&packet.input.bytes) else {
            return Ok(Vec::new());
        };
        if let Some(previous) = self.endpoints.get(&packet.endpoint.path) {
            ensure!(
                previous.instance == packet.endpoint.instance,
                "Keypad hardware endpoint changed physical instance"
            );
            if previous.generation > packet.endpoint.generation {
                return Ok(Vec::new());
            }
        }
        let mut deliveries = Vec::new();
        if self.current.as_ref() != Some(&packet.endpoint) {
            self.throttle = Throttle::default();
            self.current = Some(packet.endpoint.clone());
        } else if let Some(sample) = self.throttle.poll(packet.input.time_tick) {
            deliveries.push(Delivery::new(sample, packet.input.time_tick));
        }
        self.endpoints
            .insert(packet.endpoint.path.clone(), packet.endpoint);
        if let Some(sample) = self.throttle.observe(report.sample, packet.input.time_tick) {
            deliveries.push(Delivery::new(sample, packet.input.time_tick));
        }
        Ok(deliveries)
    }
    fn poll(&mut self, now: u64) -> Option<Delivery> {
        self.throttle
            .poll(now)
            .map(|sample| Delivery::new(sample, now))
    }
}

struct Owner<I> {
    target: Target,
    input: I,
    samples: Samples,
    failure: Option<String>,
}
impl<I: Input> Owner<I> {
    fn poll(&mut self, now: u64) -> Result<Vec<Delivery>> {
        self.input.validate()?;
        let mut deliveries = Vec::new();
        for packet in self.input.inputs()? {
            deliveries.extend(self.samples.consume(packet.map_err(anyhow::Error::msg)?)?);
        }
        if let Some(delivery) = self.samples.poll(now) {
            deliveries.push(delivery);
        }
        Ok(deliveries)
    }
}

struct Observer {
    scope: Uuid,
    caller: Uuid,
    epoch: Uuid,
    stopping: bool,
    pending: Inbox<Delivery>,
}
type Observers = BTreeMap<String, Observer>;

pub(super) struct State<I = HardwareInput> {
    owners: BTreeMap<Uuid, Owner<I>>,
    observers: Observers,
}
impl<I> Default for State<I> {
    fn default() -> Self {
        Self {
            owners: BTreeMap::new(),
            observers: BTreeMap::new(),
        }
    }
}
impl State {
    pub(super) fn start(&mut self, scope: Uuid, target: Target, id: String) -> Result<Value> {
        self.start_with(scope, target, id, HardwareInput::open)
    }
}
impl<I: Input> State<I> {
    fn start_with(
        &mut self,
        scope: Uuid,
        target: Target,
        id: String,
        open: impl FnOnce(&Target) -> Result<I>,
    ) -> Result<Value> {
        ensure!(
            target.product_id() == 747,
            "Keypad hardware observer requires product 747"
        );
        Uuid::parse_str(&id)?;
        ensure!(
            !self.observers.contains_key(&id),
            "Keypad observer is already registered"
        );
        let previous = self.owners.iter().find(|(_, owner)| {
            owner.target.product_id() == target.product_id()
                && owner
                    .target
                    .container_id()
                    .eq_ignore_ascii_case(target.container_id())
        });
        let (caller, epoch) = if let Some((caller, owner)) = previous {
            if let Some(error) = &owner.failure {
                anyhow::bail!("{error}");
            }
            owner.input.validate()?;
            ensure!(
                !self
                    .observers
                    .values()
                    .any(|observer| observer.caller == *caller && observer.stopping),
                "Keypad observation cleanup is still pending"
            );
            let epoch = self
                .observers
                .values()
                .find(|observer| observer.caller == *caller)
                .context("Keypad hardware owner has no subscribing scope")?
                .epoch;
            (*caller, epoch)
        } else {
            let input = open(&target)?;
            let caller = Uuid::new_v4();
            self.owners.insert(
                caller,
                Owner {
                    target,
                    input,
                    samples: Samples::default(),
                    failure: None,
                },
            );
            (caller, Uuid::new_v4())
        };
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
    pub(super) fn poll(&mut self, now: u64) {
        for (caller, owner) in &mut self.owners {
            if owner.failure.is_some() {
                continue;
            }
            let deliveries = match owner.poll(now) {
                Ok(deliveries) => deliveries,
                Err(error) => {
                    let cleanup = owner.input.shutdown();
                    owner.failure = Some(match cleanup {
                        Ok(()) => format!("Keypad hardware observation: {error:#}"),
                        Err(cleanup) => {
                            format!("Keypad hardware observation: {error:#}; cleanup: {cleanup:#}")
                        }
                    });
                    owner.samples = Samples::default();
                    continue;
                }
            };
            for observer in self
                .observers
                .values_mut()
                .filter(|observer| observer.caller == *caller && !observer.stopping)
            {
                for delivery in &deliveries {
                    observer.pending.push(observer.epoch, delivery.clone());
                }
            }
        }
    }
    pub(super) fn deliveries(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let observer = self
            .observers
            .get_mut(id)
            .context("Keypad observer is not registered")?;
        ensure!(observer.scope == scope, "Keypad observer scope changed");
        ensure!(
            !observer.stopping,
            "Keypad observation cleanup is still pending"
        );
        let owner = self
            .owners
            .get(&observer.caller)
            .context("Keypad hardware owner was removed")?;
        if let Some(error) = &owner.failure {
            anyhow::bail!("{error}");
        }
        Ok(serde_json::to_value(Batch::new(
            id.into(),
            observer.pending.drain()?,
        ))?)
    }
    pub(super) fn stop(&mut self, scope: Uuid, id: &str) -> Result<Value> {
        let Some(observer) = self.observers.get_mut(id) else {
            return Ok(serde_json::to_value(Receipt::new(id.into(), false))?);
        };
        ensure!(observer.scope == scope, "Keypad observer scope changed");
        observer.stopping = true;
        let caller = observer.caller;
        if !self
            .observers
            .iter()
            .any(|(other_id, other)| other_id != id && other.caller == caller)
        {
            if let Some(owner) = self.owners.get_mut(&caller) {
                owner.input.shutdown()?;
            }
            self.owners.remove(&caller);
        }
        self.observers.remove(id);
        Ok(serde_json::to_value(Receipt::new(id.into(), false))?)
    }
    pub(super) fn release(&mut self, scope: Uuid) -> Result<()> {
        let ids = self
            .observers
            .iter()
            .filter_map(|(id, observer)| (observer.scope == scope).then_some(id.clone()))
            .collect::<Vec<_>>();
        let mut errors = Vec::new();
        for id in ids {
            if let Err(error) = self.stop(scope, &id) {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    pub(super) fn shutdown(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for owner in self.owners.values_mut() {
            if let Err(error) = owner.input.shutdown() {
                errors.push(format!("{error:#}"));
            }
        }
        if errors.is_empty() {
            self.owners.clear();
            self.observers.clear();
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[derive(Default)]
    struct MockInput {
        frames: Vec<Result<hardware::Packet, String>>,
        closes: Arc<AtomicUsize>,
        cleanup_failure: bool,
        changed: bool,
    }
    impl Input for MockInput {
        fn validate(&self) -> Result<()> {
            ensure!(!self.changed, "physical connection changed");
            Ok(())
        }
        fn inputs(&mut self) -> Result<Vec<Result<hardware::Packet, String>>> {
            Ok(std::mem::take(&mut self.frames))
        }
        fn shutdown(&mut self) -> Result<()> {
            self.closes.fetch_add(1, Ordering::SeqCst);
            ensure!(!self.cleanup_failure, "cleanup failed");
            Ok(())
        }
    }
    fn packet(x: u16, generation: u64, tick: u64) -> hardware::Packet {
        let [lo, hi] = x.to_le_bytes();
        hardware::Packet {
            endpoint: hardware::EndpointIdentity {
                path: "keypad".into(),
                instance: "physical".into(),
                generation,
            },
            input: driver::Packet {
                bytes: vec![0, 0, 128, 0, 128, lo, hi, 0, 128, 0, 128, 0, 0, 0, 0],
                modifiers: Default::default(),
                time_tick: tick,
            },
        }
    }
    fn values(state: &mut State<MockInput>, scope: Uuid, id: &str) -> Vec<Delivery> {
        serde_json::from_value::<Batch>(state.deliveries(scope, id).unwrap())
            .unwrap()
            .into_deliveries(id)
            .unwrap()
    }
    #[test]
    fn keypad_read_only_session_works_without_mapping_owner_and_shares_physical_stream() {
        let mut state = State::<MockInput>::default();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let ids = [Uuid::new_v4().to_string(), Uuid::new_v4().to_string()];
        let closes = Arc::new(AtomicUsize::new(0));
        let count = closes.clone();
        let target = Target::new(747, "container".into());
        state
            .start_with(a, target.clone(), ids[0].clone(), |_| {
                Ok(MockInput {
                    frames: vec![Ok(packet(65535, 1, 0)), Ok(packet(0, 1, 10_000))],
                    closes,
                    ..Default::default()
                })
            })
            .unwrap();
        state
            .start_with(b, target, ids[1].clone(), |_| {
                panic!("duplicate physical HID owner")
            })
            .unwrap();
        state.poll(15_000);
        let samples = values(&mut state, a, &ids[0]);
        assert_eq!(
            samples
                .iter()
                .map(|d| d.sample().right_thumb_x())
                .collect::<Vec<_>>(),
            [1000, -1000]
        );
        assert_eq!(values(&mut state, b, &ids[1]), samples);
        assert!(state.stop(b, &ids[0]).is_err());
        state.stop(a, &ids[0]).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        state.release(b).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(state.owners.is_empty());
    }
    #[test]
    fn keypad_reconnect_drops_pending_old_endpoint_and_retired_packets() {
        let mut samples = Samples::default();
        assert_eq!(
            samples.consume(packet(65535, 1, 0)).unwrap()[0]
                .sample()
                .right_thumb_x(),
            1000
        );
        assert!(samples.consume(packet(0, 1, 10_000)).unwrap().is_empty());
        assert_eq!(
            samples.consume(packet(65535, 2, 11_000)).unwrap()[0]
                .sample()
                .right_thumb_x(),
            1000
        );
        assert!(samples.consume(packet(0, 1, 12_000)).unwrap().is_empty());
        assert!(samples.poll(15_000).is_none());
        assert!(samples.poll(26_000).is_none());
    }
    #[test]
    fn keypad_failure_closes_read_owner_reports_failure_and_cannot_emit_pending_sample() {
        let mut state = State::<MockInput>::default();
        let scope = Uuid::new_v4();
        let id = Uuid::new_v4().to_string();
        let closes = Arc::new(AtomicUsize::new(0));
        let count = closes.clone();
        state
            .start_with(
                scope,
                Target::new(747, "container".into()),
                id.clone(),
                |_| {
                    Ok(MockInput {
                        frames: vec![Ok(packet(65535, 1, 0)), Err("read failed".into())],
                        closes,
                        ..Default::default()
                    })
                },
            )
            .unwrap();
        state.poll(15_000);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(
            state
                .deliveries(scope, &id)
                .unwrap_err()
                .to_string()
                .contains("read failed")
        );
        assert!(
            state
                .start_with(
                    scope,
                    Target::new(747, "container".into()),
                    Uuid::new_v4().to_string(),
                    |_| panic!()
                )
                .is_err()
        );
        state.stop(scope, &id).unwrap();
    }
    #[test]
    fn keypad_unconfirmed_cleanup_blocks_successor_and_keeps_scope_ownership() {
        let mut state = State::<MockInput>::default();
        let scope = Uuid::new_v4();
        let id = Uuid::new_v4().to_string();
        state
            .start_with(
                scope,
                Target::new(747, "container".into()),
                id.clone(),
                |_| {
                    Ok(MockInput {
                        cleanup_failure: true,
                        ..Default::default()
                    })
                },
            )
            .unwrap();
        assert!(state.stop(Uuid::new_v4(), &id).is_err());
        assert!(state.stop(scope, &id).is_err());
        assert!(
            state
                .start_with(
                    scope,
                    Target::new(747, "container".into()),
                    Uuid::new_v4().to_string(),
                    |_| panic!("cleanup pending")
                )
                .is_err()
        );
        assert!(state.deliveries(scope, &id).is_err());
        state
            .owners
            .values_mut()
            .next()
            .unwrap()
            .input
            .cleanup_failure = false;
        state.stop(scope, &id).unwrap();
        assert!(state.owners.is_empty());
    }
    #[test]
    fn keypad_stopped_generation_cannot_replay_after_new_read_session() {
        let mut state = State::<MockInput>::default();
        let scope = Uuid::new_v4();
        let old = Uuid::new_v4().to_string();
        let new = Uuid::new_v4().to_string();
        let target = Target::new(747, "container".into());
        state
            .start_with(scope, target.clone(), old.clone(), |_| {
                Ok(MockInput {
                    frames: vec![Ok(packet(65535, 1, 0)), Ok(packet(0, 1, 10_000))],
                    ..Default::default()
                })
            })
            .unwrap();
        state.poll(10_000);
        state.stop(scope, &old).unwrap();
        state
            .start_with(scope, target, new.clone(), |_| Ok(MockInput::default()))
            .unwrap();
        state.poll(50_000);
        state.stop(scope, &old).unwrap();
        assert!(values(&mut state, scope, &new).is_empty());
    }
    #[test]
    fn keypad_wire_failure_and_overflow_are_not_empty_or_partial_success() {
        let mut state = State::<MockInput>::default();
        let scope = Uuid::new_v4();
        let id = Uuid::new_v4().to_string();
        state
            .start_with(
                scope,
                Target::new(747, "container".into()),
                id.clone(),
                |_| {
                    Ok(MockInput {
                        changed: true,
                        ..Default::default()
                    })
                },
            )
            .unwrap();
        state.poll(0);
        assert!(
            state
                .deliveries(scope, &id)
                .unwrap_err()
                .to_string()
                .contains("physical connection changed")
        );
        let epoch = Uuid::new_v4();
        let mut inbox = Inbox::new(epoch);
        let sample = decode_report(&packet(32768, 1, 0).input.bytes)
            .unwrap()
            .sample;
        for _ in 0..513 {
            inbox.push(epoch, Delivery::new(sample, 0));
        }
        assert!(
            inbox
                .drain()
                .unwrap_err()
                .to_string()
                .contains("incomplete")
        );
    }
}
