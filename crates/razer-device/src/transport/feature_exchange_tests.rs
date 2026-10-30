use super::*;
use crate::backend::{ExchangeLease, ExchangeTiming, HidNode, ReportLengths};
use serde_json::Value;
use std::cell::{Cell, RefCell};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LeaseEvent {
    Begin(ExchangeTiming),
    Send,
    Batch,
    Get,
    RetryRelease,
    Finish,
    ForcedDrop,
}

/// Reject raw calls so a protocol test cannot pass while bypassing admission.
pub(crate) struct ExchangeProbe<'a> {
    inner: &'a dyn FeatureTransport,
    events: RefCell<Vec<LeaseEvent>>,
    fail_finish: bool,
    finish_check: Option<&'a dyn Fn() -> anyhow::Result<()>>,
    on_finish: Option<&'a dyn Fn()>,
}

impl<'a> ExchangeProbe<'a> {
    pub(crate) fn new(inner: &'a dyn FeatureTransport) -> Self {
        Self {
            inner,
            events: RefCell::new(Vec::new()),
            fail_finish: false,
            finish_check: None,
            on_finish: None,
        }
    }

    pub(crate) fn with_on_finish(inner: &'a dyn FeatureTransport, on_finish: &'a dyn Fn()) -> Self {
        Self {
            on_finish: Some(on_finish),
            ..Self::new(inner)
        }
    }

    pub(crate) fn failing_finish(inner: &'a dyn FeatureTransport) -> Self {
        Self {
            fail_finish: true,
            ..Self::new(inner)
        }
    }

    pub(crate) fn with_finish_check(
        inner: &'a dyn FeatureTransport,
        finish_check: &'a dyn Fn() -> anyhow::Result<()>,
    ) -> Self {
        Self {
            finish_check: Some(finish_check),
            ..Self::new(inner)
        }
    }

    pub(crate) fn timings(&self) -> Vec<ExchangeTiming> {
        self.events
            .borrow()
            .iter()
            .filter_map(|event| match event {
                LeaseEvent::Begin(timing) => Some(*timing),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn events(&self) -> Vec<LeaseEvent> {
        self.events.borrow().clone()
    }
}

impl FeatureTransport for ExchangeProbe<'_> {
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        ensure!(!options.is_canceled(), "simulated canceled admission");
        self.events
            .borrow_mut()
            .push(LeaseEvent::Begin(options.timing));
        Ok(Box::new(ProbeLease {
            owner: self,
            finished: false,
        }))
    }

    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("protocol bypassed the admitted lease for OUT")
    }

    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        anyhow::bail!("protocol bypassed the admitted lease for GET")
    }

    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.hid_node()
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        self.inner.report_lengths()
    }

    fn metadata(&self) -> Value {
        self.inner.metadata()
    }
}

struct ProbeLease<'a> {
    owner: &'a ExchangeProbe<'a>,
    finished: bool,
}

impl ExchangeLease for ProbeLease<'_> {
    fn transport(&self) -> &dyn FeatureTransport {
        self
    }

    fn retry_release(&self) -> anyhow::Result<()> {
        self.owner
            .events
            .borrow_mut()
            .push(LeaseEvent::RetryRelease);
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> anyhow::Result<()> {
        self.finished = true;
        self.owner.events.borrow_mut().push(LeaseEvent::Finish);
        ensure!(!self.owner.fail_finish, "simulated explicit release failed");
        if let Some(check) = self.owner.finish_check {
            check()?;
        }
        if let Some(on_finish) = self.owner.on_finish {
            on_finish();
        }
        Ok(())
    }
}

impl Drop for ProbeLease<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner.events.borrow_mut().push(LeaseEvent::ForcedDrop);
        }
    }
}

impl FeatureTransport for ProbeLease<'_> {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.owner.events.borrow_mut().push(LeaseEvent::Send);
        self.owner.inner.send_feature(report)
    }

    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Send);
        self.owner.inner.send_feature_counted(report)
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.owner.events.borrow_mut().push(LeaseEvent::Send);
        self.owner.inner.send_feature_at_collection_max(report)
    }

    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Send);
        self.owner
            .inner
            .send_feature_at_collection_max_counted(report)
    }

    fn send_feature_batch(
        &self,
        batch: &crate::backend::FeatureBatch,
    ) -> anyhow::Result<crate::backend::FeatureBatchReceipt> {
        self.owner.events.borrow_mut().push(LeaseEvent::Batch);
        self.owner.inner.send_feature_batch(batch)
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Get);
        self.owner.inner.get_feature(report)
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Get);
        self.owner.inner.get_feature_at_collection_max(report)
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        self.owner.events.borrow_mut().push(LeaseEvent::Send);
        self.owner.inner.write_output(report)
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Send);
        self.owner.inner.write_output_counted(report)
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Get);
        self.owner.inner.get_input(report)
    }

    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        self.owner.events.borrow_mut().push(LeaseEvent::Get);
        self.owner.inner.read_interrupt(report, timeout_ms)
    }

    fn hid_node(&self) -> Option<&HidNode> {
        self.owner.inner.hid_node()
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        self.owner.inner.report_lengths()
    }

    fn metadata(&self) -> Value {
        self.owner.inner.metadata()
    }
}

struct ProtocolIo {
    outgoing: RefCell<Vec<u8>>,
    replies: RefCell<std::collections::VecDeque<(u8, bool)>>,
    reject_first_send: Cell<bool>,
    response_count: Cell<usize>,
    response_length: Cell<u8>,
}

impl ProtocolIo {
    fn new(replies: impl IntoIterator<Item = (u8, bool)>) -> Self {
        Self {
            outgoing: RefCell::new(Vec::new()),
            replies: RefCell::new(replies.into_iter().collect()),
            reject_first_send: Cell::new(false),
            response_count: Cell::new(91),
            response_length: Cell::new(2),
        }
    }
}

impl FeatureTransport for ProtocolIo {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        if self.reject_first_send.replace(false) {
            anyhow::bail!("simulated failed OUT");
        }
        *self.outgoing.borrow_mut() = report.to_vec();
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        report.copy_from_slice(&self.outgoing.borrow());
        let (status, stale) = self.replies.borrow_mut().pop_front().unwrap();
        report[1] = status;
        report[6] = self.response_length.get();
        report[9..11].copy_from_slice(&[23, 42]);
        if stale {
            report[2] ^= 1;
        }
        Ok(self.response_count.get())
    }

    fn metadata(&self) -> Value {
        Value::Null
    }
}

fn capability() -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(182).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap
}

struct ObservedIo<'a> {
    protocol: ProtocolIo,
    returns: RefCell<std::collections::VecDeque<anyhow::Result<usize>>>,
    events: &'a RefCell<Vec<&'static str>>,
    retire_on_send: Option<&'a Cell<bool>>,
    retire_on_get: Option<&'a Cell<bool>>,
}

impl<'a> ObservedIo<'a> {
    fn new(
        events: &'a RefCell<Vec<&'static str>>,
        returns: impl IntoIterator<Item = anyhow::Result<usize>>,
    ) -> Self {
        Self {
            protocol: ProtocolIo::new([(2, false)]),
            returns: RefCell::new(returns.into_iter().collect()),
            events,
            retire_on_send: None,
            retire_on_get: None,
        }
    }
}

impl FeatureTransport for ObservedIo<'_> {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("send observation requires the counted terminal result")
    }

    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        let result = self.returns.borrow_mut().pop_front().unwrap();
        if result.is_ok() {
            self.protocol.send_feature(report)?;
        }
        if let Some(current) = self.retire_on_send {
            current.set(false);
        }
        self.events.borrow_mut().push("send returned");
        result
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let result = self.protocol.get_feature(report);
        if let Some(current) = self.retire_on_get {
            current.set(false);
        }
        self.events.borrow_mut().push("get returned");
        result
    }

    fn metadata(&self) -> Value {
        Value::Null
    }
}

fn observed_send_failure(outcome: Option<crate::backend::NativeIoOutcome>) -> anyhow::Error {
    let error: anyhow::Error = std::io::Error::other("simulated native send failure").into();
    match outcome {
        Some(outcome) => crate::backend::native_io_failure(outcome, error),
        None => error,
    }
}

#[test]
fn send_observers_reconcile_terminal_outcomes_in_source_retry_order() {
    use crate::backend::NativeIoOutcome;
    for (first, expected_attempts) in [
        (Err(observed_send_failure(None)), 1),
        (
            Err(observed_send_failure(Some(NativeIoOutcome::NotCalled))),
            1,
        ),
        (
            Err(observed_send_failure(Some(NativeIoOutcome::Rejected))),
            2,
        ),
        (
            Err(observed_send_failure(Some(NativeIoOutcome::Returned {
                byte_count: 90,
            }))),
            2,
        ),
        (Ok(0), 2),
        (Ok(90), 2),
    ] {
        let events = RefCell::new(Vec::new());
        let io = ObservedIo::new(&events, [first, Ok(91)]);
        let probe = ExchangeProbe::new(&io);
        let attempts = Cell::new(0usize);
        let reply = exchange_with_report_data_size_observing_send(
            &probe,
            &capability(),
            [2, 7, 128],
            &[],
            3,
            2,
            &|| Ok(()),
            &mut || {
                attempts.set(attempts.get() + 1);
                events.borrow_mut().push("send observed");
            },
        )
        .unwrap();
        assert_eq!(reply, [23, 42]);
        assert_eq!(attempts.get(), expected_attempts);
        let expected = if expected_attempts == 2 {
            vec![
                "send returned",
                "send observed",
                "send returned",
                "send observed",
                "get returned",
            ]
        } else {
            vec![
                "send returned",
                "send returned",
                "send observed",
                "get returned",
            ]
        };
        assert_eq!(*events.borrow(), expected);
        assert_eq!(
            probe.events(),
            [
                LeaseEvent::Begin(ExchangeTiming::MutexOut {
                    interval: Duration::ZERO,
                }),
                LeaseEvent::Send,
                LeaseEvent::Send,
                LeaseEvent::Get,
                LeaseEvent::Finish,
            ]
        );
    }
}

#[test]
fn canceled_and_stale_send_owners_keep_terminal_attempts_and_errors_without_retrying() {
    use crate::backend::{NativeIoOutcome, native_io_outcome};
    for owner_error in ["simulated canceled owner", "simulated stale connection"] {
        for outcome in [
            None,
            Some(NativeIoOutcome::NotCalled),
            Some(NativeIoOutcome::Rejected),
            Some(NativeIoOutcome::Returned { byte_count: 90 }),
            Some(NativeIoOutcome::Returned { byte_count: 91 }),
        ] {
            let events = RefCell::new(Vec::new());
            let current = Cell::new(true);
            let first = match outcome {
                Some(NativeIoOutcome::Returned { byte_count: 91 }) => Ok(91),
                _ => Err(observed_send_failure(outcome)),
            };
            let mut io = ObservedIo::new(&events, [first]);
            io.retire_on_send = Some(&current);
            let probe = ExchangeProbe::new(&io);
            let mut attempts = 0usize;
            let result = exchange_with_report_data_size_observing_send(
                &probe,
                &capability(),
                [2, 7, 128],
                &[],
                3,
                2,
                &|| {
                    ensure!(current.get(), "{owner_error}");
                    Ok(())
                },
                &mut || {
                    attempts += 1;
                    events.borrow_mut().push("send observed");
                },
            );
            assert!(acknowledged_reply(&result).is_none());
            let error = result.unwrap_err();
            assert_eq!(native_io_outcome(&error), outcome);
            assert!(format!("{error:#}").contains(owner_error));
            assert_eq!(
                error.downcast_ref::<std::io::Error>().is_some(),
                outcome != Some(NativeIoOutcome::Returned { byte_count: 91 })
            );
            let attempted = matches!(
                outcome,
                Some(NativeIoOutcome::Returned { .. } | NativeIoOutcome::Rejected)
            );
            assert_eq!(attempts, usize::from(attempted));
            assert_eq!(
                *events.borrow(),
                if attempted {
                    vec!["send returned", "send observed"]
                } else {
                    vec!["send returned"]
                }
            );
            assert_eq!(
                probe.events(),
                [
                    LeaseEvent::Begin(ExchangeTiming::MutexOut {
                        interval: Duration::ZERO,
                    }),
                    LeaseEvent::Send,
                    LeaseEvent::ForcedDrop,
                ]
            );
            assert!(crate::backend::exchange_finalization_failed(&error));
        }
    }
}

#[test]
fn cancellation_after_get_retains_validated_ack_as_diagnostics_and_rejects_publication() {
    let events = RefCell::new(Vec::new());
    let current = Cell::new(true);
    let mut io = ObservedIo::new(&events, [Ok(91)]);
    io.retire_on_get = Some(&current);
    let probe = ExchangeProbe::new(&io);
    let result = exchange_with_report_data_size_observing_send(
        &probe,
        &capability(),
        [2, 7, 128],
        &[],
        3,
        2,
        &|| {
            ensure!(current.get(), "simulated stale GET owner");
            Ok(())
        },
        &mut || events.borrow_mut().push("send observed"),
    );
    assert!(result.is_err());
    assert_eq!(acknowledged_reply(&result), Some([23, 42].as_slice()));
    let error = result.unwrap_err();
    assert_eq!(
        crate::backend::native_io_outcome(&error),
        Some(crate::backend::NativeIoOutcome::Returned { byte_count: 91 })
    );
    assert_eq!(crate::backend::native_read_bytes(&error).unwrap().len(), 91);
    assert_eq!(
        *events.borrow(),
        ["send returned", "send observed", "get returned"]
    );
    assert_eq!(
        probe.events(),
        [
            LeaseEvent::Begin(ExchangeTiming::MutexOut {
                interval: Duration::ZERO,
            }),
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::ForcedDrop,
        ]
    );
    assert!(crate::backend::exchange_finalization_failed(&error));
}

#[test]
fn full_feature_retry_chain_keeps_admission_and_marks_only_complete_out_retry_boundaries() {
    let io = ProtocolIo::new([(1, false), (2, true), (2, false)]);
    let probe = ExchangeProbe::new(&io);
    assert_eq!(
        exchange(&probe, &capability(), [2, 7, 128], &[], 3, &|| Ok(())).unwrap(),
        [23, 42]
    );
    assert_eq!(
        *probe.events.borrow(),
        [
            LeaseEvent::Begin(ExchangeTiming::MutexOut {
                interval: Duration::ZERO
            }),
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::Get,
            LeaseEvent::RetryRelease,
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::Finish,
        ]
    );
}

#[test]
fn audited_host_caught_send_failures_retry_without_an_intermediate_hid_release() {
    for source_class in [
        "rzDevice25",
        "rzDevice25DualLinkMouse",
        "rzDevice25DualLinkKeyboard",
    ] {
        let io = ProtocolIo::new([(2, false)]);
        io.reject_first_send.set(true);
        let probe = ExchangeProbe::new(&io);
        let mut cap = capability();
        cap.source_class = source_class.into();
        exchange(&probe, &cap, [2, 7, 128], &[], 3, &|| Ok(())).unwrap();
        assert_eq!(
            *probe.events.borrow(),
            [
                LeaseEvent::Begin(ExchangeTiming::MutexOut {
                    interval: Duration::ZERO
                }),
                LeaseEvent::Send,
                LeaseEvent::Send,
                LeaseEvent::Get,
                LeaseEvent::Finish,
            ],
            "source class {source_class}"
        );
    }
}

#[test]
fn source_out_count_predicate_does_not_read_or_release_for_a_non_91_return() {
    struct CountedIo {
        inner: ProtocolIo,
        counts: RefCell<std::collections::VecDeque<usize>>,
    }
    impl FeatureTransport for CountedIo {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("the source OUT count predicate requires the counted boundary")
        }
        fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
            self.inner.send_feature(report)?;
            Ok(self
                .counts
                .borrow_mut()
                .pop_front()
                .expect("unexpected OUT"))
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.inner.get_feature(report)
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }
    // MW190's sendCommand accepts count === 91 before issuing GET. This tests
    // that predicate, not an unproven short-success branch of Windows SET.
    for count in [0, 90, 92] {
        let io = CountedIo {
            inner: ProtocolIo::new([(2, false)]),
            counts: RefCell::new([count, 91].into()),
        };
        let probe = ExchangeProbe::new(&io);
        exchange(&probe, &capability(), [2, 7, 128], &[], 3, &|| Ok(())).unwrap();
        assert_eq!(
            *probe.events.borrow(),
            [
                LeaseEvent::Begin(ExchangeTiming::MutexOut {
                    interval: Duration::ZERO
                }),
                LeaseEvent::Send,
                LeaseEvent::Send,
                LeaseEvent::Get,
                LeaseEvent::Finish,
            ]
        );
    }
    let mut cap = capability();
    cap.max_retry_out = 2;
    let io = CountedIo {
        inner: ProtocolIo::new([]),
        counts: RefCell::new([90, 90].into()),
    };
    let probe = ExchangeProbe::new(&io);
    let error = exchange(&probe, &cap, [2, 7, 128], &[], 3, &|| Ok(())).unwrap_err();
    assert_eq!(
        crate::backend::native_io_outcome(&error),
        Some(crate::backend::NativeIoOutcome::Returned { byte_count: 90 })
    );
    assert_eq!(
        *probe.events.borrow(),
        [
            LeaseEvent::Begin(ExchangeTiming::MutexOut {
                interval: Duration::ZERO
            }),
            LeaseEvent::Send,
            LeaseEvent::Send,
            LeaseEvent::Finish,
        ]
    );
}

#[test]
fn audited_host_caught_send_failures_exhaust_the_out_budget_and_preserve_final_release_errors() {
    struct FailingOut(Cell<usize>);
    impl FeatureTransport for FailingOut {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            self.0.set(self.0.get() + 1);
            anyhow::bail!("simulated native SET failure swallowed by the source host");
        }
        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            panic!("a failed SET must not issue GET");
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }
    for source_class in [
        "rzDevice25",
        "rzDevice25DualLinkMouse",
        "rzDevice25DualLinkKeyboard",
    ] {
        for fail_release in [false, true] {
            let io = FailingOut(Cell::new(0));
            let mut probe = ExchangeProbe::new(&io);
            probe.fail_finish = fail_release;
            let mut cap = capability();
            cap.source_class = source_class.into();
            let error = exchange(&probe, &cap, [2, 7, 128], &[], 3, &|| Ok(())).unwrap_err();
            let detail = format!("{error:#}");
            assert!(detail.contains("native SET failure swallowed by the source host"));
            assert_eq!(
                detail.contains("simulated explicit release failed"),
                fail_release
            );
            assert_eq!(io.0.get(), usize::from(cap.max_retry_out));
            let events = probe.events.borrow();
            assert_eq!(events.last(), Some(&LeaseEvent::Finish));
            assert_eq!(
                events
                    .iter()
                    .filter(|event| **event == LeaseEvent::Send)
                    .count(),
                usize::from(cap.max_retry_out)
            );
            assert!(!events.contains(&LeaseEvent::Get));
            assert!(!events.contains(&LeaseEvent::RetryRelease));
            assert!(!events.contains(&LeaseEvent::ForcedDrop));
        }
    }
}

#[test]
fn protocol_error_still_finishes_and_retains_an_independent_release_error() {
    let io = ProtocolIo::new([(5, false)]);
    let mut probe = ExchangeProbe::new(&io);
    probe.fail_finish = true;
    let error = exchange(&probe, &capability(), [2, 7, 128], &[], 3, &|| Ok(())).unwrap_err();
    let detail = format!("{error:#}");
    assert!(detail.contains("unsupported or returned status 5"));
    assert!(detail.contains("simulated explicit release failed"));
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert!(acknowledged_error(&error).is_none());
    assert_eq!(probe.events.borrow().last(), Some(&LeaseEvent::Finish));
    assert!(!probe.events.borrow().contains(&LeaseEvent::ForcedDrop));
}

#[test]
fn decoded_reply_survives_release_failure_without_turning_the_exchange_into_success() {
    let io = ProtocolIo::new([(2, false)]);
    let probe = ExchangeProbe::failing_finish(&io);
    let result = exchange(&probe, &capability(), [2, 7, 128], &[], 3, &|| Ok(()));
    assert!(result.is_err());
    assert_eq!(acknowledged_reply(&result), Some([23, 42].as_slice()));
    let error = result.unwrap_err();
    let completed = error.downcast_ref::<CompletedFeatureExchange>().unwrap();
    assert_eq!(completed.command, [2, 7, 128]);
    assert_eq!(completed.transaction, 3);
    assert!(format!("{error:#}").contains("simulated explicit release failed"));
    assert!(io.replies.borrow().is_empty());
    assert_eq!(
        *probe.events.borrow(),
        [
            LeaseEvent::Begin(ExchangeTiming::MutexOut {
                interval: Duration::ZERO
            }),
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::Finish,
        ]
    );
}

#[test]
fn decoded_reply_is_diagnostic_when_the_owner_changes_before_finalization() {
    let io = ProtocolIo::new([(2, false)]);
    let probe = ExchangeProbe::new(&io);
    let current = Cell::new(true);
    let validate = || {
        ensure!(current.get(), "simulated new connection epoch");
        Ok(())
    };
    let result = with_validated_feature_reply(
        &probe,
        ExchangeOptions::plain(),
        [2, 7, 128],
        3,
        &validate,
        |device, _| {
            let data = exchange(device, &capability(), [2, 7, 128], &[], 3, &|| Ok(()))?;
            current.set(false);
            Ok(data)
        },
    );
    assert!(result.is_err());
    assert_eq!(acknowledged_reply(&result), Some([23, 42].as_slice()));
    assert!(format!("{:#}", result.unwrap_err()).contains("new connection epoch"));
    assert_eq!(
        *probe.events.borrow(),
        [
            LeaseEvent::Begin(ExchangeTiming::Plain),
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::ForcedDrop,
        ]
    );
}

#[test]
fn decoded_reply_survives_a_checked_leases_after_release_guard_failure() {
    struct GuardedFinish<'a> {
        probe: &'a ExchangeProbe<'a>,
        current: &'a Cell<bool>,
    }
    impl FeatureTransport for GuardedFinish<'_> {
        fn begin_exchange_validated<'a>(
            &'a self,
            options: ExchangeOptions,
            validate: crate::backend::ExchangeValidation<'a>,
        ) -> anyhow::Result<Box<dyn ExchangeLease + 'a>> {
            Ok(crate::backend::checked_exchange_owned(
                self.probe.begin_exchange(options)?,
                validate,
                std::rc::Rc::new(|| {
                    ensure!(self.current.get(), "simulated after-release cancellation");
                    Ok(())
                }),
                None,
            ))
        }
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("guarded exchange must use its lease")
        }
        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            panic!("guarded exchange must use its lease")
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }
    let io = ProtocolIo::new([(2, false)]);
    let current = Cell::new(true);
    let cancel = || current.set(false);
    let probe = ExchangeProbe::with_on_finish(&io, &cancel);
    let transport = GuardedFinish {
        probe: &probe,
        current: &current,
    };
    let result = exchange(&transport, &capability(), [2, 7, 128], &[], 3, &|| Ok(()));
    assert!(result.is_err());
    assert_eq!(acknowledged_reply(&result), Some([23, 42].as_slice()));
    assert!(crate::backend::exchange_release_completed(
        result.as_ref().unwrap_err()
    ));
    assert!(format!("{:#}", result.unwrap_err()).contains("after-release cancellation"));
    assert_eq!(probe.events.borrow().last(), Some(&LeaseEvent::Finish));
    assert_eq!(probe.events.borrow().len(), 4);
}

#[test]
fn unaccepted_or_unparsed_packets_do_not_create_acknowledgement_evidence() {
    for (status, stale, count, length) in [
        (2, true, 91, 2),
        (3, false, 91, 2),
        (2, false, 90, 2),
        (2, false, 91, 81),
    ] {
        let io = ProtocolIo::new([(status, stale)]);
        io.response_count.set(count);
        io.response_length.set(length);
        let probe = ExchangeProbe::failing_finish(&io);
        let mut cap = capability();
        cap.max_retry_in = 1;
        cap.max_retry_out = 1;
        let result = exchange(&probe, &cap, [2, 7, 128], &[], 3, &|| Ok(()));
        assert!(result.is_err());
        assert!(acknowledged_reply(&result).is_none());
        assert!(format!("{:#}", result.unwrap_err()).contains("explicit release failed"));
        assert_eq!(probe.events.borrow().len(), 4);
    }
}

#[test]
fn canceled_read_receipts_use_the_same_protocol_acceptance_checks() {
    for (status, stale, count, length, accepted) in [
        (2, false, 91, 2, true),
        (2, true, 91, 2, false),
        (1, false, 91, 2, false),
        (3, false, 91, 2, false),
        (5, false, 91, 2, false),
        (2, false, 90, 2, false),
        (2, false, 92, 2, false),
        (2, false, 91, 81, false),
    ] {
        let io = ProtocolIo::new([(status, stale)]);
        io.response_count.set(count);
        io.response_length.set(length);
        let probe = ExchangeProbe::new(&io);
        let result = exchange(&probe, &capability(), [2, 7, 128], &[], 3, &|| {
            ensure!(
                probe.events.borrow().last() != Some(&LeaseEvent::Get),
                "canceled during GET"
            );
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(acknowledged_reply(&result).is_some(), accepted);
        assert!(io.replies.borrow().is_empty());
        assert_eq!(probe.events.borrow().last(), Some(&LeaseEvent::ForcedDrop));
        assert_eq!(probe.events.borrow().len(), 4);
    }
}

#[test]
fn native_count_without_a_read_receipt_never_authenticates_an_error_buffer() {
    struct ReadError<'a> {
        inner: ProtocolIo,
        current: &'a Cell<bool>,
        receipt: bool,
    }
    impl FeatureTransport for ReadError<'_> {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.inner.send_feature(report)
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            let count = self.inner.get_feature(report)?;
            self.current.set(false);
            let error = anyhow::anyhow!("retired read owner");
            Err(if self.receipt {
                crate::backend::complete_native_read(Ok(count), report, error)
            } else {
                crate::backend::native_io_failure(
                    crate::backend::NativeIoOutcome::Returned { byte_count: count },
                    error,
                )
            })
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }
    for receipt in [false, true] {
        let current = Cell::new(true);
        let io = ReadError {
            inner: ProtocolIo::new([(2, false)]),
            current: &current,
            receipt,
        };
        let result = exchange(&io, &capability(), [2, 7, 128], &[], 3, &|| {
            ensure!(current.get(), "caller retired");
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(acknowledged_reply(&result).is_some(), receipt);
        assert!(format!("{:#}", result.unwrap_err()).contains("retired read owner"));
        assert!(io.inner.replies.borrow().is_empty());
    }
}

#[test]
fn canceled_read_receipts_reject_each_unmatched_command_byte() {
    struct ChangedCommand {
        inner: ProtocolIo,
        byte: usize,
        retired: Cell<bool>,
    }
    impl FeatureTransport for ChangedCommand {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.inner.send_feature(report)
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            let count = self.inner.get_feature(report)?;
            report[self.byte] ^= 1;
            self.retired.set(true);
            Ok(count)
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }
    for byte in [7, 8] {
        let io = ChangedCommand {
            inner: ProtocolIo::new([(2, false)]),
            byte,
            retired: Cell::new(false),
        };
        let probe = ExchangeProbe::new(&io);
        let result = exchange(&probe, &capability(), [2, 7, 128], &[], 3, &|| {
            ensure!(!io.retired.get(), "read owner retired");
            Ok(())
        });
        assert!(result.is_err());
        assert!(acknowledged_reply(&result).is_none());
        assert_eq!(
            crate::backend::native_read_bytes(result.as_ref().unwrap_err())
                .unwrap()
                .len(),
            91
        );
        assert_eq!(probe.events().last(), Some(&LeaseEvent::ForcedDrop));
        assert_eq!(probe.events().len(), 4);
    }
}

#[test]
fn checked_lease_preserves_extent_identity_send_observation_and_forced_cleanup() {
    let io = ProtocolIo::new([]);
    let probe = ExchangeProbe::new(&io);
    let current = Cell::new(true);
    let validate = || {
        ensure!(current.get(), "simulated identity lost");
        Ok(())
    };
    let observed = Cell::new(0);
    let checked = crate::backend::checked_exchange_observing_send(
        probe.begin_exchange(ExchangeOptions::plain()).unwrap(),
        &validate,
        &validate,
        Some(91),
        |_| observed.set(observed.get() + 1),
    );
    assert!(
        checked
            .transport()
            .send_feature_at_collection_max(&[0; 90])
            .is_err()
    );
    assert_eq!(observed.get(), 0);
    checked
        .transport()
        .send_feature_at_collection_max(&[0; 91])
        .unwrap();
    checked.retry_release().unwrap();
    assert_eq!(observed.get(), 1);
    current.set(false);
    assert!(
        checked
            .transport()
            .send_feature_at_collection_max(&[0; 91])
            .is_err()
    );
    assert!(
        checked
            .finish()
            .unwrap_err()
            .to_string()
            .contains("identity lost")
    );
    assert_eq!(observed.get(), 1);
    assert_eq!(
        *probe.events.borrow(),
        [
            LeaseEvent::Begin(ExchangeTiming::Plain),
            LeaseEvent::Send,
            LeaseEvent::RetryRelease,
            LeaseEvent::ForcedDrop,
        ]
    );
}

#[test]
fn checked_lease_preserves_transport_error_and_after_validation_error() {
    let io = ProtocolIo::new([]);
    io.reject_first_send.set(true);
    let probe = ExchangeProbe::new(&io);
    let before = || Ok(());
    let after = || anyhow::bail!("simulated post-transfer identity failure");
    let checked = crate::backend::checked_exchange(
        probe.begin_exchange(ExchangeOptions::plain()).unwrap(),
        &before,
        &after,
        None,
    );
    let error = checked.transport().send_feature(&[0; 91]).unwrap_err();
    let detail = format!("{error:#}");
    assert!(detail.contains("failed OUT"));
    assert!(detail.contains("post-transfer identity failure"));
    assert!(checked.finish().is_err());
    assert_eq!(probe.events.borrow().last(), Some(&LeaseEvent::Finish));
}

#[test]
fn invalid_owner_forces_cleanup_without_matched_release_and_preserves_operation_failure() {
    for operation_fails in [false, true] {
        let io = ProtocolIo::new([]);
        let probe = ExchangeProbe::new(&io);
        let current = Cell::new(true);
        let validate = || {
            ensure!(current.get(), "simulated owner canceled");
            Ok(())
        };
        let error = with_validated_exchange(&probe, ExchangeOptions::plain(), &validate, |_, _| {
            current.set(false);
            ensure!(!operation_fails, "simulated operation failure");
            Ok(())
        })
        .unwrap_err();
        let detail = format!("{error:#}");
        assert!(detail.contains("simulated owner canceled"));
        assert_eq!(
            detail.contains("simulated operation failure"),
            operation_fails
        );
        assert_eq!(
            *probe.events.borrow(),
            [
                LeaseEvent::Begin(ExchangeTiming::Plain),
                LeaseEvent::ForcedDrop
            ],
        );
        assert!(
            with_validated_exchange(&probe, ExchangeOptions::plain(), &validate, |_, _| Ok(()),)
                .is_err()
        );
        assert_eq!(probe.events.borrow().len(), 2);
    }
}

#[test]
fn source_scope_follows_the_selected_factory_and_keeps_ordinary_dongles_normal() {
    use crate::backend::{Protocol25Topology, SourceCommandScope};
    let cap = crate::device_reads::capability(190).unwrap();
    for physical in [190, 191] {
        let bound = cap.for_physical_product(physical).unwrap();
        assert_eq!(bound.source_class, "rzDevice25");
        let options = with_protocol25_scope(ExchangeOptions::plain(), &bound.source_class);
        assert_eq!(
            options.source_scope,
            Some(SourceCommandScope::Protocol25 {
                topology: Protocol25Topology::Normal
            })
        );
    }
    for factory in ["rzDevice25DualLinkMouse", "rzDevice25DualLinkKeyboard"] {
        let options = with_protocol25_scope(ExchangeOptions::plain(), factory);
        assert_eq!(
            options.source_scope,
            Some(SourceCommandScope::Protocol25 {
                topology: Protocol25Topology::DualLink
            })
        );
    }
}

#[test]
fn unrelated_factories_do_not_claim_the_audited_protocol25_navigator_scope() {
    for factory in [
        "rzDevice25SagePC",
        "rzDevice25Noa",
        "rzDevice25Marci",
        "rzDevice25Linker",
        "rzDevice25LinkerUma",
        "rzDevice25Other",
        "rzDevice30",
    ] {
        let options = with_protocol25_scope(ExchangeOptions::plain(), factory);
        assert_eq!(options.source_scope, None, "factory {factory}");
        assert_eq!(options.timing, ExchangeTiming::Plain);
    }
}
