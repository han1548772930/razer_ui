use super::*;
use crate::backend::{
    ExchangeLease, ExchangeOptions, ExchangeTiming, Protocol25Topology, SourceCommandScope,
    native_io_failure,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

#[derive(Debug)]
struct NativeCause;

impl std::fmt::Display for NativeCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("scripted controller native error")
    }
}

impl std::error::Error for NativeCause {}

fn native_error(outcome: Option<NativeIoOutcome>) -> anyhow::Error {
    match outcome {
        Some(outcome) => native_io_failure(outcome, NativeCause.into()),
        None => NativeCause.into(),
    }
}

enum Step {
    Send(anyhow::Result<usize>),
    Get(Vec<u8>, anyhow::Result<usize>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Begin(ExchangeTiming, Option<SourceCommandScope>),
    Send,
    Get,
    RetryRelease,
    Finish,
    ForcedDrop,
}

struct Transport {
    steps: RefCell<VecDeque<Step>>,
    events: RefCell<Vec<Event>>,
    fail_retry: bool,
    fail_finish: bool,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            events: RefCell::default(),
            fail_retry: false,
            fail_finish: false,
        }
    }

    fn finish(&self, expected: &[Event]) {
        assert_eq!(*self.events.borrow(), expected);
        assert!(
            self.steps.borrow().is_empty(),
            "unexpected remaining native I/O"
        );
    }
}

impl FeatureTransport for Transport {
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        self.events
            .borrow_mut()
            .push(Event::Begin(options.timing, options.source_scope));
        Ok(Box::new(Lease {
            owner: self,
            finished: false,
        }))
    }

    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("controller OUT bypassed its retained lease")
    }

    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("controller IN bypassed its retained lease")
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

struct Lease<'a> {
    owner: &'a Transport,
    finished: bool,
}

impl FeatureTransport for Lease<'_> {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("controller source OUT requires the native count")
    }

    fn send_feature_at_collection_max_counted(&self, outgoing: &[u8]) -> anyhow::Result<usize> {
        assert_eq!(outgoing, request());
        self.owner.events.borrow_mut().push(Event::Send);
        let Some(Step::Send(result)) = self.owner.steps.borrow_mut().pop_front() else {
            panic!("unexpected controller OUT")
        };
        result
    }

    fn get_feature(&self, incoming: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(incoming.len(), 91);
        assert_eq!(incoming[0], 4);
        self.owner.events.borrow_mut().push(Event::Get);
        let Some(Step::Get(bytes, result)) = self.owner.steps.borrow_mut().pop_front() else {
            panic!("unexpected controller IN")
        };
        assert!(bytes.len() <= incoming.len());
        incoming[..bytes.len()].copy_from_slice(&bytes);
        result
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

impl ExchangeLease for Lease<'_> {
    fn transport(&self) -> &dyn FeatureTransport {
        self
    }

    fn retry_release(&self) -> anyhow::Result<()> {
        self.owner.events.borrow_mut().push(Event::RetryRelease);
        if self.owner.fail_retry {
            return Err(crate::backend::finalization_failure(anyhow::anyhow!(
                "controller retry release failed"
            )));
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> anyhow::Result<()> {
        self.finished = true;
        self.owner.events.borrow_mut().push(Event::Finish);
        ensure!(!self.owner.fail_finish, "controller final release failed");
        Ok(())
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner.events.borrow_mut().push(Event::ForcedDrop);
        }
    }
}

// Independent Protocol25 connection-probe bytes: report 4, transaction 7,
// command [4, 0, 147], checksum 151. Replies need no matching returned ID.
fn request() -> [u8; 91] {
    let mut outgoing = [0; 91];
    outgoing[0] = 4;
    outgoing[2] = 7;
    outgoing[6] = 4;
    outgoing[8] = 147;
    outgoing[89] = 151;
    outgoing
}

fn reply(status: u8) -> Vec<u8> {
    let mut incoming = vec![0; 91];
    incoming[..13].copy_from_slice(&[0xfe, status, 7, 0, 0, 0, 4, 0, 147, 2, 17, 0, 29]);
    incoming
}

fn get(bytes: Vec<u8>) -> Step {
    let count = bytes.len();
    Step::Get(bytes, Ok(count))
}

fn begin() -> Event {
    Event::Begin(
        ExchangeTiming::MutexOut {
            interval: Duration::from_millis(11),
        },
        Some(SourceCommandScope::Protocol25 {
            topology: Protocol25Topology::Normal,
        }),
    )
}

fn run(
    transport: &Transport,
    max_retry_in: u8,
    wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    exchange_with_timing(
        transport,
        &request(),
        Duration::from_millis(11),
        Duration::from_millis(17),
        Duration::from_millis(23),
        wait,
        max_retry_in,
    )
}

#[test]
fn source_normal_short_sends_retry_without_input_wait_or_retry_release() {
    let transport = Transport::new([
        Step::Send(Ok(0)),
        Step::Send(Ok(1)),
        Step::Send(Ok(90)),
        Step::Send(Ok(92)),
        Step::Send(Ok(91)),
        get(reply(2)),
    ]);
    let mut waits = Vec::new();
    let data = run(&transport, 10, &mut |delay| {
        if !delay.is_zero() {
            waits.push(delay);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(data, [2, 17, 0, 29]);
    assert_eq!(waits, [Duration::from_millis(17)]);
    transport.finish(&[
        begin(),
        Event::Send,
        Event::Send,
        Event::Send,
        Event::Send,
        Event::Send,
        Event::Get,
        Event::Finish,
    ]);
}

#[test]
fn source_short_sends_and_swallowed_native_errors_are_bounded_at_twenty_outputs() {
    for outcome in [None, Some(NativeIoOutcome::Rejected)] {
        let transport = Transport::new((0..20).map(|_| Step::Send(Err(native_error(outcome)))));
        let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
        assert!(error.downcast_ref::<NativeCause>().is_some());
        assert_eq!(native_io_outcome(&error), outcome);
        let mut expected = vec![begin()];
        expected.extend((0..20).map(|_| Event::Send));
        expected.push(Event::Finish);
        transport.finish(&expected);
    }
    for count in [0, 1, 90, 92] {
        let transport = Transport::new((0..20).map(|_| Step::Send(Ok(count))));
        let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: count })
        );
        let mut expected = vec![begin()];
        expected.extend((0..20).map(|_| Event::Send));
        expected.push(Event::Finish);
        transport.finish(&expected);
    }
}

#[test]
fn source_terminal_send_facts_never_read_or_retry() {
    for outcome in [
        NativeIoOutcome::NotCalled,
        NativeIoOutcome::Returned { byte_count: 0 },
        NativeIoOutcome::Returned { byte_count: 90 },
        NativeIoOutcome::Returned { byte_count: 91 },
        NativeIoOutcome::Returned { byte_count: 92 },
    ] {
        let transport = Transport::new([Step::Send(Err(native_error(Some(outcome))))]);
        let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
        assert_eq!(native_io_outcome(&error), Some(outcome));
        assert!(error.downcast_ref::<NativeCause>().is_some());
        transport.finish(&[begin(), Event::Send, Event::Finish]);
    }
}

#[test]
fn source_normal_oversized_send_count_keeps_the_strict_equality_retry_predicate() {
    let transport = Transport::new([Step::Send(Ok(92)), Step::Send(Ok(91)), get(reply(2))]);
    assert_eq!(
        run(&transport, 10, &mut |_| Ok(())).unwrap(),
        [2, 17, 0, 29]
    );
    transport.finish(&[begin(), Event::Send, Event::Send, Event::Get, Event::Finish]);
}

#[test]
fn source_resend_statuses_and_mismatched_headers_release_before_the_next_output() {
    let mut replies = [0, 3, 4].map(reply).to_vec();
    for offset in [2, 7, 8] {
        let mut bytes = reply(2);
        bytes[offset] ^= 1;
        replies.push(bytes);
    }
    replies.extend([Vec::new(), reply(2)[..8].to_vec()]);
    for first in replies {
        let transport = Transport::new([
            Step::Send(Ok(91)),
            get(first),
            Step::Send(Ok(91)),
            get(reply(2)),
        ]);
        assert_eq!(
            run(&transport, 10, &mut |_| Ok(())).unwrap(),
            [2, 17, 0, 29]
        );
        transport.finish(&[
            begin(),
            Event::Send,
            Event::Get,
            Event::RetryRelease,
            Event::Send,
            Event::Get,
            Event::Finish,
        ]);
    }
}

#[test]
fn source_busy_reads_continue_until_success_or_the_input_budget_is_exhausted() {
    for exhausted in [false, true] {
        let mut steps = vec![Step::Send(Ok(91)), get(reply(1))];
        if exhausted {
            steps.extend([get(reply(1)), Step::Send(Ok(91))]);
        }
        steps.push(get(reply(2)));
        let transport = Transport::new(steps);
        let mut waits = Vec::new();
        assert_eq!(
            run(&transport, 2, &mut |delay| {
                if !delay.is_zero() {
                    waits.push(delay.as_millis());
                }
                Ok(())
            })
            .unwrap(),
            [2, 17, 0, 29]
        );
        let mut events = vec![begin(), Event::Send, Event::Get];
        if exhausted {
            events.extend([Event::Get, Event::RetryRelease, Event::Send]);
        }
        events.extend([Event::Get, Event::Finish]);
        transport.finish(&events);
        assert_eq!(
            waits,
            if exhausted { vec![17, 23, 17] } else { vec![17, 23] }
        );
    }
}

#[test]
fn source_unsupported_and_unknown_statuses_fail_without_requesting_a_resend() {
    for status in [5, 6, 255] {
        let transport = Transport::new([Step::Send(Ok(91)), get(reply(status))]);
        let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
        assert!(error.to_string().contains(&format!("status {status}")));
        assert!(crate::feature_exchange::acknowledged_error(&error).is_none());
        transport.finish(&[begin(), Event::Send, Event::Get, Event::Finish]);
    }
}

#[test]
fn source_success_slices_only_actual_payload_without_requiring_a_full_collection() {
    for (count, announced) in [(9, 4), (10, 4), (12, 4), (90, 4), (91, 4), (11, 255)] {
        let mut bytes = reply(2);
        bytes[6] = announced;
        let expected = bytes[9..(9 + usize::from(announced)).min(count)].to_vec();
        bytes.truncate(count);
        let transport = Transport::new([Step::Send(Ok(91)), get(bytes)]);
        assert_eq!(run(&transport, 10, &mut |_| Ok(())).unwrap(), expected);
        transport.finish(&[begin(), Event::Send, Event::Get, Event::Finish]);
    }
}

#[test]
fn source_swallowed_get_errors_request_a_resend_but_terminal_reads_do_not() {
    for outcome in [None, Some(NativeIoOutcome::Rejected)] {
        let transport = Transport::new([
            Step::Send(Ok(91)),
            Step::Get(Vec::new(), Err(native_error(outcome))),
            Step::Send(Ok(91)),
            get(reply(2)),
        ]);
        assert_eq!(
            run(&transport, 10, &mut |_| Ok(())).unwrap(),
            [2, 17, 0, 29]
        );
        transport.finish(&[
            begin(),
            Event::Send,
            Event::Get,
            Event::RetryRelease,
            Event::Send,
            Event::Get,
            Event::Finish,
        ]);
    }
    for outcome in [
        NativeIoOutcome::NotCalled,
        NativeIoOutcome::Returned { byte_count: 0 },
        NativeIoOutcome::Returned { byte_count: 91 },
    ] {
        let transport = Transport::new([
            Step::Send(Ok(91)),
            Step::Get(reply(2), Err(native_error(Some(outcome)))),
        ]);
        let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
        assert!(crate::feature_exchange::acknowledged_error(&error).is_none());
        assert_eq!(native_io_outcome(&error), Some(outcome));
        transport.finish(&[begin(), Event::Send, Event::Get, Event::Finish]);
    }
}

#[test]
fn source_cancellation_after_a_normal_send_prevents_input_and_preserves_its_count() {
    for count in [63, 91] {
        let transport = Transport::new([Step::Send(Ok(count))]);
        let error = run(&transport, 10, &mut |_| {
            ensure!(
                !transport.events.borrow().contains(&Event::Send),
                "controller owner canceled after OUT"
            );
            Ok(())
        })
        .unwrap_err();
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: count })
        );
        assert!(crate::feature_exchange::acknowledged_error(&error).is_none());
        transport.finish(&[begin(), Event::Send, Event::Finish]);
    }
}

#[test]
fn source_actual_ack_remains_diagnostic_after_read_cancellation_or_native_owner_failure() {
    for short in [false, true] {
        let mut bytes = reply(2);
        if short {
            bytes.truncate(11);
        }
        let expected = bytes[9..].iter().copied().take(4).collect::<Vec<_>>();
        for native_failure in [false, true] {
            let returned = bytes.len();
            let received = if native_failure {
                Err(complete_native_read(
                    Ok(returned),
                    &bytes,
                    anyhow::anyhow!("controller read owner replaced"),
                ))
            } else {
                Ok(returned)
            };
            let transport =
                Transport::new([Step::Send(Ok(91)), Step::Get(bytes.clone(), received)]);
            let error = run(&transport, 10, &mut |_| {
                ensure!(
                    native_failure || !transport.events.borrow().contains(&Event::Get),
                    "controller owner canceled after IN"
                );
                Ok(())
            })
            .unwrap_err();
            let completed = crate::feature_exchange::CompletedFeatureExchange::from_error(&error)
                .expect("actual ACK must remain diagnostic");
            assert_eq!(completed.command(), [4, 0, 147]);
            assert_eq!(completed.transaction(), 7);
            assert_eq!(completed.data(), expected);
            assert_eq!(native_read_bytes(&error), Some(bytes.as_slice()));
            transport.finish(&[begin(), Event::Send, Event::Get, Event::Finish]);
        }
    }
}

#[test]
fn source_retry_and_final_release_failures_cannot_be_hidden_by_a_later_success() {
    let mut transport = Transport::new([Step::Send(Ok(91)), get(reply(0))]);
    transport.fail_retry = true;
    let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
    assert!(exchange_finalization_failed(&error));
    assert!(crate::feature_exchange::acknowledged_error(&error).is_none());
    transport.finish(&[
        begin(),
        Event::Send,
        Event::Get,
        Event::RetryRelease,
        Event::Finish,
    ]);

    let mut transport = Transport::new([Step::Send(Ok(91)), get(reply(2))]);
    transport.fail_finish = true;
    let error = run(&transport, 10, &mut |_| Ok(())).unwrap_err();
    assert!(exchange_finalization_failed(&error));
    assert_eq!(
        crate::feature_exchange::acknowledged_error(&error),
        Some(&[2, 17, 0, 29][..])
    );
    transport.finish(&[begin(), Event::Send, Event::Get, Event::Finish]);
}

#[test]
fn source_wait_cancellation_before_io_and_during_busy_prevents_later_packets() {
    let transport = Transport::new([]);
    assert!(
        run(&transport, 10, &mut |_| anyhow::bail!(
            "canceled before OUT"
        ))
        .is_err()
    );
    transport.finish(&[begin(), Event::Finish]);

    let transport = Transport::new([Step::Send(Ok(91)), get(reply(1))]);
    let canceled = Cell::new(false);
    let error = run(&transport, 10, &mut |delay| {
        if delay == Duration::from_millis(23) {
            canceled.set(true);
        }
        ensure!(!canceled.get(), "canceled before the next busy read");
        Ok(())
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("canceled before the next busy read")
    );
    transport.finish(&[begin(), Event::Send, Event::Get, Event::Finish]);
}
