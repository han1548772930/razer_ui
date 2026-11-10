use anyhow::{Context as _, Result, ensure};
use razer_device::{
    backend::HidNode,
    earbuds_auto_pause::{self, Operation},
    earbuds_protocol40::Protocol40Session,
};
use razer_device::{
    earbuds_events::Target,
    earbuds_protocol40::{TransactionCounter, TransactionDeviceType},
    receiver_pairing::HardwareEvents,
};
use razer_hid::with_backend;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

type Key = (u32, Vec<u8>);
struct CounterOwner {
    subscriptions: usize,
    state: Arc<Mutex<CounterState>>,
}
struct CounterState {
    counter: TransactionCounter,
    active: bool,
    anc: Arc<Mutex<razer_device::earbuds_anc::Cache>>,
}
pub(super) struct Transactions(Arc<Mutex<CounterState>>);
impl Transactions {
    pub(super) fn with_anc_cache<R>(
        &self,
        validate: &dyn Fn() -> Result<()>,
        run: impl FnOnce(&razer_device::earbuds_anc::Cache) -> Result<R>,
    ) -> Result<R> {
        let cache = {
            let state = self
                .0
                .lock()
                .map_err(|_| anyhow::anyhow!("Earbud transaction owner poisoned"))?;
            ensure!(state.active, "Earbud transaction owner was released");
            state.anc.clone()
        };
        loop {
            self.validate()?;
            validate()?;
            match cache.try_lock() {
                Ok(cache) => return run(&cache),
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    anyhow::bail!("Earbud physical ANC cache poisoned")
                }
            }
        }
    }
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.0
                .lock()
                .map_err(|_| anyhow::anyhow!("Earbud transaction counter poisoned"))?
                .active,
            "Earbud transaction owner was released"
        );
        Ok(())
    }
    pub(super) fn next(&self, kind: TransactionDeviceType) -> Result<u8> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Earbud transaction counter poisoned"))?;
        ensure!(state.active, "Earbud transaction owner was released");
        Ok(state.counter.next(kind))
    }
}
static COUNTERS: OnceLock<Mutex<BTreeMap<Key, CounterOwner>>> = OnceLock::new();

fn key(target: &Target) -> Result<Key> {
    let pid = target.product_id()?;
    let mut path = match target {
        Target::Collection(node) => node.path.clone(),
        Target::WindowsContainer(target) => target.path.as_bytes().to_vec(),
    };
    ensure!(
        !path.is_empty(),
        "Earbud transaction owner has no collection path"
    );
    if cfg!(windows) {
        path.make_ascii_lowercase();
    }
    Ok((pid, path))
}

struct Subscription(Key);
impl Subscription {
    fn begin(target: &Target) -> Result<Self> {
        let key = key(target)?;
        let mut owners = COUNTERS
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("Earbud counter registry poisoned"))?;
        let owner = owners.entry(key.clone()).or_insert_with(|| CounterOwner {
            subscriptions: 0,
            state: Arc::new(Mutex::new(CounterState {
                counter: TransactionCounter::default(),
                active: true,
                anc: Arc::new(Mutex::new(razer_device::earbuds_anc::Cache::default())),
            })),
        });
        owner.subscriptions += 1;
        Ok(Self(key))
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut owners) = COUNTERS.get_or_init(Mutex::default).lock() {
            if let Some(owner) = owners.get_mut(&self.0) {
                owner.subscriptions -= 1;
                if owner.subscriptions == 0 {
                    if let Ok(mut state) = owner.state.lock() {
                        state.active = false;
                    }
                    owners.remove(&self.0);
                }
            }
        }
    }
}

pub(super) fn borrow_transactions(target: &Target) -> Result<Transactions> {
    let key = key(target)?;
    let state = COUNTERS
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| anyhow::anyhow!("Earbud counter registry poisoned"))?
        .get(&key)
        .context("Earbud initialization requires an active notification owner")?
        .state
        .clone();
    Ok(Transactions(state))
}

struct RegisteredEvents {
    source: Box<dyn HardwareEvents>,
    target: Target,
    subscription: Option<Subscription>,
}
pub(super) fn registered_events(
    source: Box<dyn HardwareEvents>,
    target: Target,
) -> Box<dyn HardwareEvents> {
    Box::new(RegisteredEvents {
        source,
        target,
        subscription: None,
    })
}
impl HardwareEvents for RegisteredEvents {
    fn begin(&mut self) -> Result<()> {
        self.source.begin()?;
        self.subscription = Some(Subscription::begin(&self.target)?);
        Ok(())
    }
    fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        self.source.receive(timeout)
    }
    fn finish(&mut self) -> Result<()> {
        self.source.finish()?;
        self.subscription = None;
        Ok(())
    }
}

pub(super) fn execute(
    node: &HidNode,
    product_id: u32,
    operation: Operation,
) -> Result<serde_json::Value> {
    let cap = earbuds_auto_pause::transport(product_id)?;
    ensure!(
        node.vendor_id == cap.vendor_id
            && u32::from(node.product_id) == product_id
            && node.interface_number == i32::from(cap.claim_interface),
        "Earbud collection identity differs"
    );
    let started = std::time::Instant::now();
    let validate_physical = || {
        with_backend(|backend| {
            ensure!(
                backend
                    .enumerate()?
                    .iter()
                    .filter(|current| *current == node)
                    .count()
                    == 1,
                "Earbud collection changed"
            );
            Ok(())
        })
    };
    let validate = || {
        super::hid_broker::check_current_request()?;
        ensure!(
            started.elapsed() < std::time::Duration::from_secs(30),
            "Earbud operation deadline expired"
        );
        validate_physical()
    };
    validate()?;
    let transport =
        with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
    ensure!(
        transport.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
        "Earbud Feature descriptor differs"
    );
    let target = Target::Collection(node.clone());
    let transactions = borrow_transactions(&target)?;
    let mut next = |kind| transactions.next(kind);
    let source_validate = || {
        validate()?;
        transactions.validate()
    };
    let source_physical = || {
        transactions.validate()?;
        validate_physical()
    };
    let report = transactions
        .with_anc_cache(&source_validate, |cache| {
            earbuds_auto_pause::execute(
                &mut Protocol40Session::with_transaction_source(
                    transport.as_ref(),
                    razer_device::earbuds_protocol40::ThreadSleeper,
                    &mut next,
                ),
                product_id,
                operation,
                cache,
                &source_validate,
                &source_physical,
            )
        })
        .context("Earbud enhancement operation")?;
    encode_report(report, &source_validate)
}

pub(super) fn encode_report(
    report: earbuds_auto_pause::Report,
    validate: &dyn Fn() -> Result<()>,
) -> Result<serde_json::Value> {
    if let Some(submission) = &report.equalizer {
        submission.validate()?;
        let mut errors = Vec::new();
        if let razer_device::earbuds_protocol40::WriteOutcome::Failed(error) = &report.write {
            errors.push(error.clone());
        }
        if let Err(error) = validate() {
            errors.push(format!("{error:#}"));
        }
        if !errors.is_empty() {
            return Err(razer_device::earbuds_equalizer::Failure {
                product_id: report.product_id,
                submission: submission.clone(),
                message: errors.join("; "),
            }.into());
        }
    }
    serde_json::to_value(report).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn earbuds_equalizer_worker_encodes_partial_ack_as_typed_failure_without_success_data() {
        use razer_device::earbuds_equalizer::{Mode, Preset, Setting, Submission};
        for completed in [false, true] {
            let report = earbuds_auto_pause::Report::from_equalizer(2657,
                razer_device::earbuds_equalizer::Report {
                    setting: Setting { mode: Mode::Standard, preset: Preset::Music, bands: [2; 10] },
                    preset: Submission::Confirmed,
                    bands: if completed { Submission::Confirmed } else {
                        Submission::Cancelled { message: "cancelled before band ACK".into(), acknowledged: false }
                    },
                }).unwrap();
            let error = encode_report(report, &|| anyhow::bail!("worker scope expired")).unwrap_err();
            let Some(razer_ipc::FailureDiagnostics::EarbudsEqualizer(failure)) =
                razer_ipc::FailureDiagnostics::from_error(&error) else { panic!("EQ diagnostic domain") };
            assert_eq!(failure.product_id, 2657);
            assert!(failure.submission.preset.acknowledged());
            assert_eq!(failure.submission.bands.acknowledged(), completed);
            assert!(failure.message.contains("worker scope expired"));
        }
    }
    fn target(path: &str) -> Target {
        Target::Collection(HidNode {
            path: path.as_bytes().to_vec(),
            vendor_id: 0x1532,
            product_id: 2660,
            usage_page: 0,
            usage: 0,
            interface_number: 0,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        })
    }

    #[test]
    fn physical_counter_survives_clients_and_old_borrows_fail_after_last_subscription() {
        let target = target("COUNTER-SOURCE-A");
        #[cfg(windows)]
        let alternate = Target::WindowsContainer(razer_device::device_reads::DeviceReadTarget {
            product_id: 2660,
            physical_product_id: 2660,
            peer_product_id: None,
            device_container_id: "test-container".into(),
            path: "counter-source-a".into(),
        });
        #[cfg(not(windows))]
        let alternate = target.clone();
        let first = Subscription::begin(&target).unwrap();
        let second = Subscription::begin(&alternate).unwrap();
        let first_client = borrow_transactions(&target).unwrap();
        let second_client = borrow_transactions(&alternate).unwrap();
        assert_eq!(first_client.next(TransactionDeviceType::Mouse).unwrap(), 1);
        assert_eq!(
            second_client
                .next(TransactionDeviceType::AudioHeadsetSpeakerMic)
                .unwrap(),
            0x62
        );
        drop(first);
        assert_eq!(
            first_client
                .next(TransactionDeviceType::AudioHeadsetSpeakerMic)
                .unwrap(),
            0x63
        );
        drop(second);
        assert!(first_client.validate().is_err());
        assert!(borrow_transactions(&target).is_err());
        let successor = Subscription::begin(&alternate).unwrap();
        let fresh = borrow_transactions(&target).unwrap();
        assert_eq!(
            fresh
                .next(TransactionDeviceType::AudioHeadsetSpeakerMic)
                .unwrap(),
            0x61
        );
        assert!(
            second_client
                .next(TransactionDeviceType::AudioHeadsetSpeakerMic)
                .is_err()
        );
        drop(successor);
        assert!(fresh.validate().is_err());
    }

    #[test]
    fn transaction_registration_follows_actual_event_begin_and_finish() {
        struct Source(bool);
        impl HardwareEvents for Source {
            fn begin(&mut self) -> Result<()> {
                ensure!(self.0, "begin failed");
                Ok(())
            }
            fn receive(&mut self, _: Duration) -> Result<Option<Vec<u8>>> {
                Ok(None)
            }
            fn finish(&mut self) -> Result<()> {
                Ok(())
            }
        }
        let target = target("counter-source-b");
        let mut failed = registered_events(Box::new(Source(false)), target.clone());
        assert!(failed.begin().is_err());
        assert!(borrow_transactions(&target).is_err());
        failed.finish().unwrap();
        let mut active = registered_events(Box::new(Source(true)), target.clone());
        active.begin().unwrap();
        let borrowed = borrow_transactions(&target).unwrap();
        active.finish().unwrap();
        assert!(borrowed.validate().is_err());
        assert!(borrow_transactions(&target).is_err());
    }

    #[test]
    fn canceled_broker_context_prevents_earbud_hid_backend_access() {
        let node = HidNode {
            path: b"not-a-real-device".to_vec(),
            vendor_id: 0x1532,
            product_id: 2660,
            usage_page: 0,
            usage: 0,
            interface_number: 0,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        };
        let canceled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let result =
            super::super::hid_broker::with_request_context(uuid::Uuid::new_v4(), canceled, || {
                execute(&node, 2660, Operation::Read)
            });
        assert!(format!("{:#}", result.unwrap_err()).contains("Device request was canceled"));
    }

    struct NoSleep;
    impl razer_device::earbuds_protocol40::Sleeper for NoSleep {
        fn sleep(&self, _: Duration) {}
    }
    struct AncIo<'a> {
        last: std::cell::RefCell<Vec<u8>>,
        sent: std::cell::Cell<usize>,
        read: Box<dyn Fn() + 'a>,
    }
    impl razer_device::backend::FeatureTransport for AncIo<'_> {
        fn send_feature(&self, bytes: &[u8]) -> Result<()> {
            self.last.replace(bytes.to_vec());
            self.sent.set(self.sent.get() + 1);
            Ok(())
        }
        fn get_feature(&self, bytes: &mut [u8]) -> Result<usize> {
            let last = self.last.borrow();
            bytes.fill(0);
            bytes[0] = 2;
            bytes[1] = 2;
            bytes[2] = last[2];
            bytes[7] = 24;
            bytes[8] = 6;
            (self.read)();
            Ok(64)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
    }
    fn anc_target(path: &str) -> Target {
        let mut target = target(path);
        if let Target::Collection(node) = &mut target {
            node.product_id = 2657;
            node.interface_number = 3;
        }
        target
    }

    #[test]
    fn earbuds_anc_cache_survives_independent_request_clients() {
        use razer_device::earbuds_protocol40::{ANC_PROFILE_DEFAULT, AncSetting, AncWrite};
        let target = anc_target("earbuds-anc-cache-clients");
        let owner = Subscription::begin(&target).unwrap();
        let first = borrow_transactions(&target).unwrap();
        let second = borrow_transactions(&target).unwrap();
        let io = AncIo {
            last: Default::default(),
            sent: Default::default(),
            read: Box::new(|| {}),
        };
        for (connection, setting) in [
            (
                &first,
                AncSetting {
                    enabled: false,
                    ..ANC_PROFILE_DEFAULT
                },
            ),
            (&second, ANC_PROFILE_DEFAULT),
        ] {
            connection
                .with_anc_cache(&|| Ok(()), |cache| {
                    let mut next = |kind| connection.next(kind);
                    let report = cache.execute(
                        &mut Protocol40Session::with_transaction_source(&io, NoSleep, &mut next),
                        2657,
                        setting,
                        &|| Ok(()),
                        &|| connection.validate(),
                    )?;
                    assert_eq!(report.write, AncWrite::Acknowledged);
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(io.sent.get(), 2);
        assert_eq!(io.last.borrow()[2], 0x62);
        drop(owner);
    }

    #[test]
    fn earbuds_anc_old_ack_cannot_update_a_successor_physical_owner() {
        use razer_device::earbuds_protocol40::{ANC_PROFILE_DEFAULT, AncRuntimeState, AncSetting};
        let target = anc_target("earbuds-anc-cache-successor");
        let owner = std::cell::RefCell::new(Some(Subscription::begin(&target).unwrap()));
        let successor = std::cell::RefCell::new(None);
        let old = borrow_transactions(&target).unwrap();
        let io = AncIo {
            last: Default::default(),
            sent: Default::default(),
            read: Box::new(|| {
                owner.borrow_mut().take();
                successor.replace(Some(Subscription::begin(&target).unwrap()));
            }),
        };
        assert!(
            old.with_anc_cache(&|| Ok(()), |cache| {
                let result = cache.execute(
                    &mut Protocol40Session::with_sleeper(&io, NoSleep),
                    2657,
                    AncSetting {
                        enabled: false,
                        ..ANC_PROFILE_DEFAULT
                    },
                    &|| Ok(()),
                    &|| old.validate(),
                );
                assert!(!cache.is_comparable());
                result.map(|_| ())
            })
            .is_err()
        );
        let current = borrow_transactions(&target).unwrap();
        current
            .with_anc_cache(&|| Ok(()), |cache| {
                assert!(cache.is_comparable());
                assert_eq!(cache.runtime(), AncRuntimeState::default());
                Ok(())
            })
            .unwrap();
        assert!(old.with_anc_cache(&|| Ok(()), |_| Ok(())).is_err());
        successor.borrow_mut().take();
    }
}
