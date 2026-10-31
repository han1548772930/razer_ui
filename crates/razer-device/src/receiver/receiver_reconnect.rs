//! Current164/241/756 Unpair's independent connectDevice retry path.
//! The initial firmware command only opens the host's Feature handle; it
//! never reads a firmware reply or establishes a device acknowledgement.
use anyhow::ensure;
use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuation {
    ResultField,
    ///756 tests the returned object itself, even when its result is false.
    ReturnedObject,
}

/// Windows connectDevice prepends claimInterface, unlike ordinary commands
/// that use reportId0. Current HID.node forwards all bytes to HidD_SetFeature.
pub fn initial_report(claim_interface: u8, transaction: u8) -> [u8; 91] {
    let mut report = [0; 91];
    report[0] = claim_interface;
    report[2] = transaction;
    report[6..9].copy_from_slice(&[2, 0, 129]);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    report
}

/// Native enumeration/handles and interruptible waits belong to the adapter.
/// `enumerate` distinguishes an absent physical device from a present device
/// whose current command collection is unavailable or ambiguous.
pub trait Adapter {
    type Target;
    fn check_owner(&self) -> anyhow::Result<()>;
    fn wait(&mut self, duration: Duration) -> anyhow::Result<()>;
    fn reset_runtime_ids(&mut self) -> anyhow::Result<()>;
    fn enumerate(&mut self) -> anyhow::Result<Option<Vec<Self::Target>>>;
    fn report(&mut self) -> anyhow::Result<[u8; 91]>;
    /// A complete Feature send, without a subsequent GET_FEATURE.
    fn open_and_send(&mut self, target: &Self::Target, report: &[u8]) -> anyhow::Result<()>;
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Outcome {
    outer_attempts: u8,
    connect_attempts: u8,
    enumeration_rounds: u16,
    feature_send_complete: bool,
    source_returned_object: bool,
    source_retry_stopped: bool,
    source_runtime_ids_reset: bool,
    last_error: Option<String>,
}
impl Outcome {
    pub fn feature_send_complete(&self) -> bool {
        self.feature_send_complete
    }
}

/// Preserve the source's20 outer attempts,3 connect attempts and10 missing
/// device enumeration rounds. The source's `index===20` branch is unreachable
/// inside `index<20`, so even the last failed outer attempt sleeps.
pub fn run(adapter: &mut impl Adapter, continuation: Continuation) -> anyhow::Result<Outcome> {
    run_with_remaining_delay(adapter, continuation, Duration::from_millis(500))
}

/// The owning shell has already scheduled the source timer before creating
/// its dedicated IPC client. Carry only the remaining local delay to avoid
/// adding another500ms after worker-process startup.
pub fn run_with_remaining_delay(
    adapter: &mut impl Adapter,
    continuation: Continuation,
    remaining: Duration,
) -> anyhow::Result<Outcome> {
    ensure!(
        remaining <= Duration::from_millis(500),
        "独立重连初始等待超过原500ms计时器"
    );
    adapter.check_owner()?;
    adapter.wait(remaining)?;
    adapter.check_owner()?;
    // Source launches its async retry, then resets these runtime fields without
    // awaiting success. Reset at the same500ms timer boundary, never at ACK.
    adapter.reset_runtime_ids()?;
    let mut outcome = Outcome {
        outer_attempts: 0,
        connect_attempts: 0,
        enumeration_rounds: 0,
        feature_send_complete: false,
        source_returned_object: false,
        source_retry_stopped: false,
        source_runtime_ids_reset: true,
        last_error: None,
    };
    let mut delay_ms = 1000;
    for outer in 0..20 {
        adapter.check_owner()?;
        outcome.outer_attempts = outer + 1;
        // _createDataSend is called once per connectDevice, before its3 retries.
        let report = adapter.report()?;
        for _ in 0..3 {
            adapter.check_owner()?;
            outcome.connect_attempts += 1;
            let mut targets = None;
            for _ in 0..10 {
                adapter.check_owner()?;
                outcome.enumeration_rounds += 1;
                match adapter.enumerate() {
                    Ok(Some(found)) => {
                        targets = Some(found);
                        break;
                    }
                    Ok(None) => {
                        outcome.last_error = Some("当前物理接收器尚未枚举到".into());
                        adapter.wait(Duration::from_millis(500))?;
                    }
                    Err(error) => {
                        if crate::backend::exchange_interrupted(&error) {
                            return Err(error);
                        }
                        outcome.last_error = Some(format!("{error:#}"));
                        break;
                    }
                }
            }
            if let Some(targets) = targets {
                ensure!(targets.len() <= 1, "接收器重连集合存在歧义，未发送命令");
                if targets.is_empty() {
                    outcome.last_error = Some("物理接收器存在，但源命令接口不可用".into());
                }
                for target in targets {
                    adapter.check_owner()?;
                    match adapter.open_and_send(&target, &report) {
                        Ok(()) => {
                            outcome.feature_send_complete = true;
                            outcome.last_error = None;
                        }
                        Err(error) => {
                            if crate::backend::exchange_interrupted(&error) {
                                return Err(error);
                            }
                            outcome.last_error = Some(format!("{error:#}"));
                        }
                    }
                }
            }
            if outcome.feature_send_complete {
                break;
            }
        }
        // All current connectDevice paths return their result object after the
        // inner retries.756's truthiness does not mean its HID write succeeded.
        outcome.source_returned_object = true;
        if outcome.feature_send_complete || continuation == Continuation::ReturnedObject {
            outcome.source_retry_stopped = true;
            return Ok(outcome);
        }
        adapter.wait(Duration::from_millis(delay_ms))?;
        delay_ms *= 2;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::VecDeque};

    #[derive(Default)]
    struct Simulation {
        waits: Vec<Duration>,
        writes: Vec<Vec<u8>>,
        available: VecDeque<bool>,
        fail_writes: bool,
        reset: bool,
        canceled: Cell<bool>,
        cancel_on_wait: Option<Duration>,
        native_failure: Option<crate::backend::NativeIoOutcome>,
    }
    impl Adapter for Simulation {
        type Target = ();
        fn check_owner(&self) -> anyhow::Result<()> {
            ensure!(!self.canceled.get(), "simulated owner disposed");
            Ok(())
        }
        fn wait(&mut self, duration: Duration) -> anyhow::Result<()> {
            self.waits.push(duration);
            if self.cancel_on_wait == Some(duration) {
                self.canceled.set(true);
            }
            self.check_owner()
        }
        fn reset_runtime_ids(&mut self) -> anyhow::Result<()> {
            assert_eq!(self.waits.len(), 1);
            self.reset = true;
            Ok(())
        }
        fn enumerate(&mut self) -> anyhow::Result<Option<Vec<()>>> {
            Ok(self.available.pop_front().unwrap_or(true).then(|| vec![()]))
        }
        fn report(&mut self) -> anyhow::Result<[u8; 91]> {
            Ok(initial_report(3, 128))
        }
        fn open_and_send(&mut self, _: &(), report: &[u8]) -> anyhow::Result<()> {
            assert!(self.reset);
            self.writes.push(report.to_vec());
            if let Some(outcome) = self.native_failure.take() {
                return Err(crate::backend::native_io_failure(
                    outcome,
                    anyhow::anyhow!("one-shot receiver reconnect interruption"),
                ));
            }
            ensure!(!self.fail_writes, "simulated Feature send failure");
            Ok(())
        }
    }

    #[test]
    fn native_interruption_stops_both_source_reconnect_continuations() {
        use crate::backend::{NativeIoOutcome, native_io_outcome};
        for continuation in [Continuation::ResultField, Continuation::ReturnedObject] {
            for outcome in [
                NativeIoOutcome::NotCalled,
                NativeIoOutcome::Returned { byte_count: 91 },
            ] {
                let mut simulated = Simulation {
                    native_failure: Some(outcome),
                    ..Default::default()
                };
                let error = run(&mut simulated, continuation).unwrap_err();
                assert_eq!(native_io_outcome(&error), Some(outcome));
                assert_eq!(simulated.writes.len(), 1);
                assert_eq!(simulated.waits, [Duration::from_millis(500)]);
            }
        }
    }

    #[test]
    fn initial_open_preserves_windows_interface_and_never_requires_firmware_read() {
        let mut simulated = Simulation::default();
        let result = run(&mut simulated, Continuation::ResultField).unwrap();
        assert!(result.feature_send_complete);
        assert_eq!(result.outer_attempts, 1);
        assert_eq!(simulated.writes.len(), 1);
        assert_eq!(simulated.writes[0][0], 3);
        assert_eq!(simulated.writes[0][2], 128);
        assert_eq!(&simulated.writes[0][6..9], &[2, 0, 129]);
        assert_eq!(simulated.writes[0][89], 2 ^ 129);
        assert_eq!(simulated.waits, [Duration::from_millis(500)]);
    }

    #[test]
    fn returned_false_object_stops756_without_claiming_a_complete_feature_send() {
        let mut simulated = Simulation {
            fail_writes: true,
            ..Default::default()
        };
        let result = run(&mut simulated, Continuation::ReturnedObject).unwrap();
        assert_eq!(result.outer_attempts, 1);
        assert_eq!(result.connect_attempts, 3);
        assert!(result.source_returned_object && result.source_retry_stopped);
        assert!(!result.feature_send_complete);
        assert_eq!(simulated.waits, [Duration::from_millis(500)]);
        assert!(result.last_error.as_ref().unwrap().contains("simulated"));
    }

    #[test]
    fn linker_failed_twentieth_retry_keeps_its_last_source_delay() {
        let mut simulated = Simulation {
            fail_writes: true,
            ..Default::default()
        };
        let result = run(&mut simulated, Continuation::ResultField).unwrap();
        assert_eq!(result.outer_attempts, 20);
        assert_eq!(result.connect_attempts, 60);
        assert!(!result.feature_send_complete && !result.source_retry_stopped);
        assert_eq!(simulated.waits.len(), 21);
        assert_eq!(simulated.waits[20], Duration::from_millis(1000 * (1 << 19)));
    }

    #[test]
    fn missing_device_enumeration_waits_and_owner_disposal_interrupts_long_delay() {
        let mut simulated = Simulation {
            available: VecDeque::from([false; 10]),
            ..Default::default()
        };
        let result = run(&mut simulated, Continuation::ResultField).unwrap();
        assert!(result.feature_send_complete);
        assert_eq!(result.enumeration_rounds, 11);
        assert_eq!(simulated.waits, vec![Duration::from_millis(500); 11]);
        let mut canceled = Simulation {
            fail_writes: true,
            cancel_on_wait: Some(Duration::from_millis(1000)),
            ..Default::default()
        };
        assert!(run(&mut canceled, Continuation::ResultField).is_err());
        assert_eq!(canceled.writes.len(), 3);
    }

    #[test]
    fn dedicated_worker_startup_consumes_existing_source_timer() {
        let mut simulated = Simulation::default();
        run_with_remaining_delay(
            &mut simulated,
            Continuation::ResultField,
            Duration::from_millis(125),
        )
        .unwrap();
        assert_eq!(simulated.waits, [Duration::from_millis(125)]);
        let mut invalid = Simulation::default();
        assert!(
            run_with_remaining_delay(
                &mut invalid,
                Continuation::ResultField,
                Duration::from_millis(501)
            )
            .is_err()
        );
        assert!(!invalid.reset && invalid.writes.is_empty());
    }
}
