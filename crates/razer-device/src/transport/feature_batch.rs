use super::{NativeIoOutcome, native_io_failure, native_io_outcome};
use anyhow::{Context as _, ensure};

const HEADER: [u8; 4] = [0x37, 0x13, 0xfe, 0xca];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeatureBatchOperation {
    Report { bytes: Vec<u8> },
    Sleep { milliseconds: u8 },
}

/// An owned, bounds-checked stream for the source F0/F1/F2 batch interpreter.
/// Report bytes include their ID. Unknown bytecodes are source no-ops.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureBatch {
    operations: Vec<FeatureBatchOperation>,
}

impl FeatureBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operations(&self) -> &[FeatureBatchOperation] {
        &self.operations
    }

    pub fn push_report(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        ensure!(!bytes.is_empty(), "Feature batch report is empty");
        u32::try_from(bytes.len()).context("Feature batch report exceeds its length field")?;
        self.operations.push(FeatureBatchOperation::Report {
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    pub fn push_sleep(&mut self, milliseconds: u8) {
        self.operations
            .push(FeatureBatchOperation::Sleep { milliseconds });
    }

    pub fn from_bytecode(bytes: &[u8]) -> anyhow::Result<Self> {
        let parse = || {
            let mut batch = Self::new();
            let mut offset = 0;
            while let Some(&opcode) = bytes.get(offset) {
                offset += 1;
                let length = match opcode {
                    0xf0 => {
                        let length = *bytes
                            .get(offset)
                            .context("Feature batch F0 length is missing")?;
                        offset += 1;
                        usize::from(length)
                    }
                    0xf1 => {
                        let milliseconds = *bytes
                            .get(offset)
                            .context("Feature batch sleep is missing")?;
                        offset += 1;
                        batch.push_sleep(milliseconds);
                        continue;
                    }
                    0xf2 => {
                        let end = offset
                            .checked_add(4)
                            .context("Feature batch length overflow")?;
                        let field: [u8; 4] = bytes
                            .get(offset..end)
                            .context("Feature batch F2 length is incomplete")?
                            .try_into()?;
                        offset = end;
                        usize::try_from(u32::from_le_bytes(field))?
                    }
                    _ => continue,
                };
                let end = offset
                    .checked_add(length)
                    .context("Feature batch extent overflow")?;
                // The source parser does not establish this body bound. Rust
                // validates the complete owned stream before any native send.
                batch.push_report(
                    bytes
                        .get(offset..end)
                        .context("Feature batch report is truncated")?,
                )?;
                offset = end;
            }
            Ok(batch)
        };
        parse().map_err(|error| native_io_failure(NativeIoOutcome::NotCalled, error))
    }

    pub fn to_bytecode(&self) -> Vec<u8> {
        let mut bytes = HEADER.to_vec();
        for operation in &self.operations {
            match operation {
                FeatureBatchOperation::Report { bytes: report } => {
                    if let Ok(length) = u8::try_from(report.len()) {
                        bytes.extend_from_slice(&[0xf0, length]);
                    } else {
                        bytes.push(0xf2);
                        bytes.extend_from_slice(&(report.len() as u32).to_le_bytes());
                    }
                    bytes.extend_from_slice(report);
                }
                FeatureBatchOperation::Sleep { milliseconds } => {
                    bytes.extend_from_slice(&[0xf1, *milliseconds]);
                }
            }
        }
        bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureBatchReportReturn {
    pub operation_index: usize,
    pub requested_bytes: usize,
    pub returned_bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureBatchProgress {
    pub report_attempts: Vec<FeatureBatchReportAttempt>,
    pub report_returns: Vec<FeatureBatchReportReturn>,
    pub completed_operations: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureBatchReportAttempt {
    pub operation_index: usize,
    pub outcome: Option<NativeIoOutcome>,
}

impl FeatureBatchProgress {
    pub fn reports_submitted(&self) -> usize {
        self.report_returns
            .iter()
            .filter(|report| report.returned_bytes == report.requested_bytes)
            .count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureBatchReceipt {
    /// Unsigned bits of the native accumulator, separate from its signed
    /// numeric conversion at the source asynchronous worker boundary.
    byte_count: usize,
    raw_return: i32,
    progress: FeatureBatchProgress,
}

impl FeatureBatchReceipt {
    pub fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub fn raw_return(&self) -> i32 {
        self.raw_return
    }

    pub fn progress(&self) -> &FeatureBatchProgress {
        &self.progress
    }

    pub fn into_failure(self, error: anyhow::Error) -> anyhow::Error {
        native_io_failure(
            NativeIoOutcome::Returned {
                byte_count: self.byte_count,
            },
            error.context(FeatureBatchFailure {
                progress: self.progress,
                raw_return: Some(self.raw_return),
            }),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureBatchFailure {
    pub progress: FeatureBatchProgress,
    pub raw_return: Option<i32>,
}

impl FeatureBatchFailure {
    pub fn from_error(error: &anyhow::Error) -> Option<&Self> {
        error.downcast_ref()
    }
}

impl std::fmt::Display for FeatureBatchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Feature batch stopped after {} complete report submissions",
            self.progress.reports_submitted()
        )
    }
}

fn rejected(
    progress: FeatureBatchProgress,
    raw_return: Option<i32>,
    error: anyhow::Error,
) -> anyhow::Error {
    // A report's normal return followed by adapter failure is not a normal
    // return of the enclosing batch. Keep the report fact in progress instead.
    let error = if super::exchange_interrupted(&error) {
        let phase = error
            .downcast_ref::<super::ExchangeInterruption>()
            .copied()
            .unwrap_or(super::ExchangeInterruption::Native);
        super::interrupt_exchange(error, phase)
    } else {
        error
    };
    native_io_failure(
        NativeIoOutcome::Rejected,
        error.context(FeatureBatchFailure {
            progress,
            raw_return,
        }),
    )
}

/// Execute the source interpreter with injected I/O and waits. The caller owns
/// the entire operation until its terminal return; release is a separate step.
pub fn execute_feature_batch(
    batch: &FeatureBatch,
    mut send: impl FnMut(&[u8]) -> anyhow::Result<usize>,
    mut wait: impl FnMut(u8) -> anyhow::Result<()>,
) -> anyhow::Result<FeatureBatchReceipt> {
    let mut progress = FeatureBatchProgress::default();
    let mut accumulator = 0u32;
    for (operation_index, operation) in batch.operations.iter().enumerate() {
        match operation {
            FeatureBatchOperation::Report { bytes } => {
                let count = match send(bytes) {
                    Ok(count) => {
                        progress.report_attempts.push(FeatureBatchReportAttempt {
                            operation_index,
                            outcome: Some(NativeIoOutcome::Returned { byte_count: count }),
                        });
                        count
                    }
                    Err(error) => {
                        let outcome = native_io_outcome(&error);
                        progress.report_attempts.push(FeatureBatchReportAttempt {
                            operation_index,
                            outcome,
                        });
                        if let Some(NativeIoOutcome::Returned { byte_count }) = outcome {
                            progress.report_returns.push(FeatureBatchReportReturn {
                                operation_index,
                                requested_bytes: bytes.len(),
                                returned_bytes: byte_count,
                            });
                        }
                        return Err(rejected(progress, None, error));
                    }
                };
                progress.report_returns.push(FeatureBatchReportReturn {
                    operation_index,
                    requested_bytes: bytes.len(),
                    returned_bytes: count,
                });
                let count = match u32::try_from(count) {
                    Ok(count) => count,
                    Err(error) => return Err(rejected(progress, None, error.into())),
                };
                let callback_return = count as i32;
                if callback_return <= 0 {
                    return terminal_receipt(count, progress);
                }
                accumulator = accumulator.wrapping_add(count);
            }
            FeatureBatchOperation::Sleep { milliseconds } => {
                if let Err(error) = wait(*milliseconds) {
                    return Err(rejected(
                        progress,
                        None,
                        super::interrupt_exchange(error, super::ExchangeInterruption::Wait),
                    ));
                }
            }
        }
        progress.completed_operations = operation_index + 1;
    }
    terminal_receipt(accumulator, progress)
}

/// Reconcile write-attempt consumers after the retained batch owner has drained
/// its native operation. A later report or a failed preflight is not an attempt.
pub fn observe_feature_batch(
    batch: &FeatureBatch,
    result: &anyhow::Result<FeatureBatchReceipt>,
    mut observe: impl FnMut(&[u8]),
) {
    let progress = match result {
        Ok(receipt) => &receipt.progress,
        Err(error) => match FeatureBatchFailure::from_error(error) {
            Some(failure) => &failure.progress,
            None => return,
        },
    };
    for attempt in &progress.report_attempts {
        if attempt.outcome.is_some_and(NativeIoOutcome::was_called) {
            if let Some(FeatureBatchOperation::Report { bytes }) =
                batch.operations.get(attempt.operation_index)
            {
                observe(bytes);
            }
        }
    }
}

fn terminal_receipt(
    accumulator: u32,
    progress: FeatureBatchProgress,
) -> anyhow::Result<FeatureBatchReceipt> {
    let raw_return = accumulator as i32;
    if raw_return == -1 {
        Err(rejected(
            progress,
            Some(raw_return),
            anyhow::anyhow!("Source native batch returned -1"),
        ))
    } else {
        Ok(FeatureBatchReceipt {
            byte_count: accumulator as usize,
            raw_return,
            progress,
        })
    }
}

pub fn checked_feature_batch(
    before: &dyn Fn() -> anyhow::Result<()>,
    operation: impl FnOnce() -> anyhow::Result<FeatureBatchReceipt>,
    after: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<FeatureBatchReceipt> {
    before().map_err(|error| native_io_failure(NativeIoOutcome::NotCalled, error))?;
    match (operation(), after()) {
        (Ok(receipt), Ok(())) => Ok(receipt),
        (Ok(receipt), Err(error)) => Err(receipt.into_failure(error)),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(validation)) => {
            let phase = validation
                .downcast_ref::<super::ExchangeInterruption>()
                .copied()
                .unwrap_or(super::ExchangeInterruption::Validation);
            Err(super::interrupt_exchange(
                error.context(format!(
                    "Feature batch validation also failed: {validation:#}"
                )),
                phase,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_bytecodes_skip_header_and_preserve_short_long_reports_and_zero_sleeps() {
        let mut bytecode = vec![
            0x37, 0x13, 0xfe, 0xca, 0xf0, 3, 4, 1, 2, 0xf1, 0, 0xf2, 0, 1, 0, 0,
        ];
        bytecode.extend_from_slice(&[0x7a; 256]);
        let batch = FeatureBatch::from_bytecode(&bytecode).unwrap();
        assert_eq!(batch.operations.len(), 3);
        assert_eq!(batch.to_bytecode(), bytecode);
        let mut lengths = Vec::new();
        let mut waits = Vec::new();
        let receipt = execute_feature_batch(
            &batch,
            |bytes| {
                lengths.push(bytes.len());
                Ok(bytes.len())
            },
            |ms| {
                waits.push(ms);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(lengths, [3, 256]);
        assert_eq!(waits, [0]);
        assert_eq!(receipt.byte_count, 259);
        assert_eq!(receipt.raw_return, 259);
        assert_eq!(receipt.progress.reports_submitted(), 2);
        assert_eq!(receipt.progress.completed_operations, 3);
    }

    #[test]
    fn empty_header_only_and_sleep_only_batches_really_return_zero() {
        for bytes in [
            vec![],
            HEADER.to_vec(),
            vec![0xf1, 0],
            vec![0xf1, 7],
            vec![0x42],
        ] {
            let batch = FeatureBatch::from_bytecode(&bytes).unwrap();
            let mut waits = Vec::new();
            let receipt = execute_feature_batch(
                &batch,
                |_| panic!("no report exists"),
                |ms| {
                    waits.push(ms);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(receipt.byte_count, 0);
            assert_eq!(receipt.raw_return, 0);
            assert!(receipt.progress.report_returns.is_empty());
            assert_eq!(waits.len(), usize::from(bytes.first() == Some(&0xf1)));
        }
    }

    #[test]
    fn malformed_stream_is_rejected_before_any_native_operation() {
        for bytes in [
            vec![0xf0],
            vec![0xf0, 0],
            vec![0xf0, 3, 4],
            vec![0xf1],
            vec![0xf2, 1, 0, 0],
            vec![0xf2, 0, 0, 0, 0],
            vec![0xf2, 3, 0, 0, 0, 4],
        ] {
            let error = FeatureBatch::from_bytecode(&bytes).unwrap_err();
            assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
        }
    }

    #[test]
    fn a_failed_second_report_preserves_the_first_submission_without_a_batch_return() {
        let batch =
            FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf1, 1, 0xf0, 2, 0, 2, 0xf0, 2, 0, 3])
                .unwrap();
        for return_then_fail in [false, true] {
            let mut calls = 0;
            let error = execute_feature_batch(
                &batch,
                |_| {
                    calls += 1;
                    if calls == 1 {
                        return Ok(2);
                    }
                    let error = anyhow::anyhow!("second report failed");
                    Err(native_io_failure(
                        if return_then_fail {
                            NativeIoOutcome::Returned { byte_count: 2 }
                        } else {
                            NativeIoOutcome::Rejected
                        },
                        error,
                    ))
                },
                |_| Ok(()),
            )
            .unwrap_err();
            assert_eq!(calls, 2);
            assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
            assert_eq!(super::super::exchange_interrupted(&error), return_then_fail);
            let failure = FeatureBatchFailure::from_error(&error).unwrap();
            assert_eq!(failure.progress.completed_operations, 2);
            assert_eq!(
                failure.progress.reports_submitted(),
                if return_then_fail { 2 } else { 1 }
            );
            assert_eq!(failure.raw_return, None);
        }
    }

    #[test]
    fn zero_callback_stops_without_inventing_completed_reports_or_running_later_steps() {
        let batch = FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf1, 1, 0xf0, 2, 0, 2]).unwrap();
        let receipt =
            execute_feature_batch(&batch, |_| Ok(0), |_| panic!("zero stops the interpreter"))
                .unwrap();
        assert_eq!(receipt.raw_return, 0);
        assert_eq!(receipt.progress.completed_operations, 0);
        assert_eq!(receipt.progress.reports_submitted(), 0);
        assert_eq!(receipt.progress.report_returns[0].returned_bytes, 0);
    }

    #[test]
    fn signed_worker_conversion_preserves_normal_negative_values_and_rejects_only_minus_one() {
        let batch = FeatureBatch::from_bytecode(&[0xf0, 1, 0]).unwrap();
        let receipt = execute_feature_batch(&batch, |_| Ok(0xffff_fffe), |_| Ok(())).unwrap();
        assert_eq!(receipt.raw_return, -2);
        assert_eq!(receipt.byte_count, 0xffff_fffe);
        let error = execute_feature_batch(&batch, |_| Ok(0xffff_ffff), |_| Ok(())).unwrap_err();
        assert_eq!(
            FeatureBatchFailure::from_error(&error).unwrap().raw_return,
            Some(-1)
        );
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
        let batch = FeatureBatch::from_bytecode(&[0xf0, 1, 0, 0xf0, 1, 0, 0xf0, 1, 0]).unwrap();
        let mut counts = [0x7fff_ffff, 0x7fff_ffff, 2].into_iter();
        let receipt =
            execute_feature_batch(&batch, |_| Ok(counts.next().unwrap()), |_| Ok(())).unwrap();
        assert_eq!(receipt.raw_return, 0);
        assert_eq!(receipt.progress.completed_operations, 3);
        let mut counts = [0x7fff_ffff, 0x7fff_ffff, 1].into_iter();
        let error =
            execute_feature_batch(&batch, |_| Ok(counts.next().unwrap()), |_| Ok(())).unwrap_err();
        assert_eq!(
            FeatureBatchFailure::from_error(&error).unwrap().raw_return,
            Some(-1)
        );
    }

    #[test]
    fn ownership_loss_after_a_normal_batch_keeps_progress_but_returns_failure() {
        let batch = FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1]).unwrap();
        let error = checked_feature_batch(
            &|| Ok(()),
            || execute_feature_batch(&batch, |_| Ok(2), |_| Ok(())),
            &|| anyhow::bail!("owner canceled"),
        )
        .unwrap_err();
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: 2 })
        );
        assert_eq!(
            FeatureBatchFailure::from_error(&error)
                .unwrap()
                .progress
                .reports_submitted(),
            1
        );
    }

    #[test]
    fn batch_attempt_consumers_only_see_reports_the_native_owner_reached() {
        let batch = FeatureBatch::from_bytecode(&[0xf0, 1, 1, 0xf0, 1, 2]).unwrap();
        for outcome in [
            None,
            Some(NativeIoOutcome::NotCalled),
            Some(NativeIoOutcome::Rejected),
            Some(NativeIoOutcome::Returned { byte_count: 0 }),
        ] {
            let mut calls = 0;
            let result = execute_feature_batch(
                &batch,
                |_| {
                    calls += 1;
                    let error = anyhow::anyhow!("first report failed");
                    Err(match outcome {
                        Some(outcome) => native_io_failure(outcome, error),
                        None => error,
                    })
                },
                |_| Ok(()),
            );
            let mut observed = Vec::new();
            observe_feature_batch(&batch, &result, |bytes| observed.push(bytes.to_vec()));
            assert_eq!(calls, 1);
            assert_eq!(
                observed,
                if matches!(
                    outcome,
                    Some(NativeIoOutcome::Returned { .. } | NativeIoOutcome::Rejected)
                ) {
                    vec![vec![1]]
                } else {
                    vec![]
                }
            );
            let failure = FeatureBatchFailure::from_error(result.as_ref().unwrap_err()).unwrap();
            assert_eq!(
                failure.progress.report_attempts,
                [FeatureBatchReportAttempt {
                    operation_index: 0,
                    outcome
                }]
            );
        }
        let result = execute_feature_batch(&batch, |_| Ok(0), |_| Ok(()));
        let mut observed = Vec::new();
        observe_feature_batch(&batch, &result, |bytes| observed.push(bytes.to_vec()));
        assert_eq!(observed, [vec![1]]);
    }

    #[test]
    fn failed_batch_wait_keeps_prior_attempts_without_marking_a_later_report() {
        let batch = FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf1, 1, 0xf0, 2, 0, 2]).unwrap();
        let events = std::cell::RefCell::new(Vec::new());
        let result = execute_feature_batch(
            &batch,
            |_| {
                events.borrow_mut().push("send returned");
                Ok(2)
            },
            |milliseconds| {
                assert_eq!(milliseconds, 1);
                events.borrow_mut().push("wait canceled");
                anyhow::bail!("simulated row wait cancellation");
            },
        );
        observe_feature_batch(&batch, &result, |report| {
            assert_eq!(report, [0, 1]);
            events.borrow_mut().push("send observed");
        });
        assert_eq!(
            *events.borrow(),
            ["send returned", "wait canceled", "send observed"]
        );
        let error = result.unwrap_err();
        assert!(super::super::exchange_interrupted(&error));
        assert!(matches!(
            error.downcast_ref::<super::super::ExchangeInterruption>(),
            Some(super::super::ExchangeInterruption::Wait)
        ));
        assert!(!super::super::exchange_finalization_failed(&error));
        let failure = FeatureBatchFailure::from_error(&error).unwrap();
        assert_eq!(failure.raw_return, None);
        assert_eq!(failure.progress.completed_operations, 1);
        assert_eq!(failure.progress.reports_submitted(), 1);
        assert_eq!(
            failure.progress.report_attempts,
            [FeatureBatchReportAttempt {
                operation_index: 0,
                outcome: Some(NativeIoOutcome::Returned { byte_count: 2 }),
            }]
        );
        assert!(format!("{error:#}").contains("simulated row wait cancellation"));
    }

    #[test]
    fn batch_attempts_survive_partial_failure_and_stale_publication_checks() {
        let batch =
            FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf1, 1, 0xf0, 2, 0, 2, 0xf0, 2, 0, 3])
                .unwrap();
        for second_outcome in [
            None,
            Some(NativeIoOutcome::NotCalled),
            Some(NativeIoOutcome::Rejected),
            Some(NativeIoOutcome::Returned { byte_count: 1 }),
        ] {
            let calls = std::cell::Cell::new(0);
            let result = checked_feature_batch(
                &|| Ok(()),
                || {
                    execute_feature_batch(
                        &batch,
                        |_| {
                            calls.set(calls.get() + 1);
                            if calls.get() == 1 {
                                return Ok(2);
                            }
                            let error: anyhow::Error =
                                std::io::Error::other("simulated partial native failure").into();
                            Err(match second_outcome {
                                Some(outcome) => native_io_failure(outcome, error),
                                None => error,
                            })
                        },
                        |_| Ok(()),
                    )
                },
                &|| anyhow::bail!("simulated stale batch owner"),
            );
            let mut observed = Vec::new();
            observe_feature_batch(&batch, &result, |report| observed.push(report.to_vec()));
            assert_eq!(calls.get(), 2);
            let expected = match second_outcome {
                Some(NativeIoOutcome::Rejected | NativeIoOutcome::Returned { .. }) => {
                    vec![vec![0, 1], vec![0, 2]]
                }
                _ => vec![vec![0, 1]],
            };
            assert_eq!(observed, expected);
            let error = result.unwrap_err();
            assert!(super::super::exchange_interrupted(&error));
            assert!(matches!(
                error.downcast_ref::<super::super::ExchangeInterruption>(),
                Some(super::super::ExchangeInterruption::Validation)
            ));
            assert!(!super::super::exchange_finalization_failed(&error));
            let failure = FeatureBatchFailure::from_error(&error).unwrap();
            assert_eq!(failure.raw_return, None);
            assert_eq!(failure.progress.completed_operations, 2);
            assert_eq!(failure.progress.reports_submitted(), 1);
            assert_eq!(
                failure.progress.report_attempts,
                [
                    FeatureBatchReportAttempt {
                        operation_index: 0,
                        outcome: Some(NativeIoOutcome::Returned { byte_count: 2 }),
                    },
                    FeatureBatchReportAttempt {
                        operation_index: 2,
                        outcome: second_outcome,
                    },
                ]
            );
            assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
            assert!(error.downcast_ref::<std::io::Error>().is_some());
            let detail = format!("{error:#}");
            assert!(detail.contains("simulated partial native failure"));
            assert!(detail.contains("simulated stale batch owner"));
        }
    }
}
