/// The transport return is independent of protocol acceptance and of the
/// caller still owning the connection when the result is consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeIoOutcome {
    NotCalled,
    Returned { byte_count: usize },
    Rejected,
}

impl NativeIoOutcome {
    pub const fn was_called(self) -> bool {
        matches!(self, Self::Returned { .. } | Self::Rejected)
    }
}

#[derive(Debug)]
struct NativeIoFailure {
    outcome: NativeIoOutcome,
    read_bytes: Option<Vec<u8>>,
}

impl std::fmt::Display for NativeIoFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.outcome {
            NativeIoOutcome::NotCalled => formatter.write_str("HID native I/O was not called"),
            NativeIoOutcome::Returned { byte_count } => {
                write!(formatter, "HID native I/O returned {byte_count} bytes")
            }
            NativeIoOutcome::Rejected => formatter.write_str("HID native I/O was rejected"),
        }
    }
}

/// Keep the original error, including its typed cause, under the observed
/// native boundary. An unmarked error carries no inferred native outcome.
pub fn native_io_failure(outcome: NativeIoOutcome, error: anyhow::Error) -> anyhow::Error {
    error.context(NativeIoFailure {
        outcome,
        read_bytes: None,
    })
}

pub fn native_io_outcome(error: &anyhow::Error) -> Option<NativeIoOutcome> {
    error
        .downcast_ref::<NativeIoFailure>()
        .map(|failure| failure.outcome)
}

/// Reconcile an attempt after its retained native owner reaches a terminal
/// result. Neither an unmarked failure nor preflight rejection proves I/O.
pub fn native_io_attempted(result: &anyhow::Result<usize>) -> bool {
    match result {
        Ok(_) => true,
        Err(error) => native_io_outcome(error).is_some_and(NativeIoOutcome::was_called),
    }
}

/// Identity and cancellation checks run on the lifecycle owner's thread. A
/// failed post-check must retain a completed native return as diagnostics while
/// preventing its value from being published as the current connection state.
pub fn checked_native_io(
    before: &dyn Fn() -> anyhow::Result<()>,
    operation: impl FnOnce() -> anyhow::Result<usize>,
    after: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<usize> {
    before().map_err(|error| native_io_failure(NativeIoOutcome::NotCalled, error))?;
    match (operation(), after()) {
        (result, Ok(())) => result,
        (result, Err(validation)) => Err(complete_native_io(result, validation)),
    }
}

/// Actual bytes retained by the read owner when publication is rejected. A
/// native byte count alone cannot prove that a proxy filled its caller's buffer.
/// Outcome and bytes belong to the same native boundary; later operations cannot
/// reuse an earlier receipt, even when their byte counts are equal.
pub fn native_read_bytes(error: &anyhow::Error) -> Option<&[u8]> {
    let failure = error.downcast_ref::<NativeIoFailure>()?;
    let NativeIoOutcome::Returned { byte_count } = failure.outcome else {
        return None;
    };
    failure
        .read_bytes
        .as_deref()
        .filter(|bytes| bytes.len() == byte_count)
}

/// Retain only the bytes belonging to a successful native return. Errors must
/// already carry their read owner's receipt; the caller's buffer is not evidence.
pub fn complete_native_read(
    result: anyhow::Result<usize>,
    report: &[u8],
    validation: anyhow::Error,
) -> anyhow::Error {
    match result {
        Ok(byte_count) => validation.context(NativeIoFailure {
            outcome: NativeIoOutcome::Returned { byte_count },
            read_bytes: report.get(..byte_count).map(|bytes| bytes.to_vec()),
        }),
        Err(error) => complete_native_io(Err(error), validation),
    }
}

pub fn checked_native_read(
    before: &dyn Fn() -> anyhow::Result<()>,
    report: &mut [u8],
    operation: impl FnOnce(&mut [u8]) -> anyhow::Result<usize>,
    after: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<usize> {
    before().map_err(|error| native_io_failure(NativeIoOutcome::NotCalled, error))?;
    match (operation(report), after()) {
        (result, Ok(())) => result,
        (result, Err(validation)) => Err(complete_native_read(result, report, validation)),
    }
}

/// Reject publication after ownership loss while retaining the terminal native
/// fact and its original typed failure for the recovery owner.
pub(crate) fn complete_native_io(
    result: anyhow::Result<usize>,
    validation: anyhow::Error,
) -> anyhow::Error {
    match result {
        Ok(byte_count) => native_io_failure(NativeIoOutcome::Returned { byte_count }, validation),
        Err(error) => error.context(format!(
            "HID native I/O validation also failed: {validation:#}"
        )),
    }
}

/// Legacy send methods guarantee a complete transport send. Counted methods
/// expose normal short returns too, so complete-send consumers check here.
pub fn require_complete_send(count: usize, expected: usize) -> anyhow::Result<()> {
    if count == expected {
        Ok(())
    } else {
        Err(native_io_failure(
            NativeIoOutcome::Returned { byte_count: count },
            anyhow::anyhow!("HID send returned {count} bytes; expected {expected}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct NativeCause;

    impl std::fmt::Display for NativeCause {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("native cause")
        }
    }

    impl std::error::Error for NativeCause {}

    #[test]
    fn native_observations_survive_context_without_discarding_the_typed_cause() {
        for outcome in [
            NativeIoOutcome::NotCalled,
            NativeIoOutcome::Returned { byte_count: 0 },
            NativeIoOutcome::Returned { byte_count: 91 },
            NativeIoOutcome::Rejected,
        ] {
            let error = native_io_failure(outcome, NativeCause.into()).context("owner failed");
            assert_eq!(native_io_outcome(&error), Some(outcome));
            assert!(error.downcast_ref::<NativeCause>().is_some());
        }
        assert_eq!(native_io_outcome(&anyhow::anyhow!("unknown")), None);
    }

    #[test]
    fn a_short_send_is_a_normal_native_return_and_a_failed_complete_send() {
        assert!(require_complete_send(91, 91).is_ok());
        for count in [0, 1, 90, 92] {
            let error = require_complete_send(count, 91).unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: count })
            );
        }
    }

    #[test]
    fn only_a_normal_return_or_an_explicit_native_rejection_proves_an_attempt() {
        assert!(native_io_attempted(&Ok(0)));
        assert!(!native_io_attempted(&Err(anyhow::anyhow!(
            "unobserved failure"
        ))));
        for (outcome, attempted) in [
            (NativeIoOutcome::NotCalled, false),
            (NativeIoOutcome::Rejected, true),
            (NativeIoOutcome::Returned { byte_count: 0 }, true),
            (NativeIoOutcome::Returned { byte_count: 91 }, true),
        ] {
            assert_eq!(
                native_io_attempted(&Err(native_io_failure(outcome, NativeCause.into()))),
                attempted
            );
        }
    }

    #[test]
    fn validation_distinguishes_not_called_from_a_return_before_identity_loss() {
        let error = checked_native_io(
            &|| Err(NativeCause.into()),
            || panic!("preflight failure must not call native I/O"),
            &|| panic!("preflight failure must not run post-check"),
        )
        .unwrap_err();
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
        assert!(error.downcast_ref::<NativeCause>().is_some());

        let error =
            checked_native_io(&|| Ok(()), || Ok(17), &|| Err(NativeCause.into())).unwrap_err();
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: 17 })
        );
        assert!(error.downcast_ref::<NativeCause>().is_some());

        for outcome in [
            None,
            Some(NativeIoOutcome::NotCalled),
            Some(NativeIoOutcome::Rejected),
            Some(NativeIoOutcome::Returned { byte_count: 0 }),
            Some(NativeIoOutcome::Returned { byte_count: 91 }),
        ] {
            let error = checked_native_io(
                &|| Ok(()),
                || {
                    Err(match outcome {
                        Some(outcome) => native_io_failure(outcome, NativeCause.into()),
                        None => NativeCause.into(),
                    })
                },
                &|| anyhow::bail!("identity changed"),
            )
            .unwrap_err();
            assert_eq!(native_io_outcome(&error), outcome);
            assert!(error.downcast_ref::<NativeCause>().is_some());
            assert!(format!("{error:#}").contains("identity changed"));
        }
    }

    #[test]
    fn failed_read_publication_retains_only_the_native_returned_prefix() {
        for count in [0, 2, 4, 5] {
            let mut buffer = [0xee; 4];
            let error = checked_native_read(
                &|| Ok(()),
                &mut buffer,
                |report| {
                    report.copy_from_slice(&[11, 22, 33, 44]);
                    Ok(count)
                },
                &|| Err(NativeCause.into()),
            )
            .unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: count })
            );
            assert_eq!(native_read_bytes(&error), [11, 22, 33, 44].get(..count));
            assert!(error.downcast_ref::<NativeCause>().is_some());
        }
    }

    #[test]
    fn a_read_count_without_owned_bytes_cannot_certify_the_callers_buffer() {
        for outcome in [
            None,
            Some(NativeIoOutcome::NotCalled),
            Some(NativeIoOutcome::Rejected),
            Some(NativeIoOutcome::Returned { byte_count: 4 }),
        ] {
            let mut buffer = [0x7f; 4];
            let error = checked_native_read(
                &|| Ok(()),
                &mut buffer,
                |_| {
                    Err(match outcome {
                        Some(outcome) => native_io_failure(outcome, NativeCause.into()),
                        None => NativeCause.into(),
                    })
                },
                &|| anyhow::bail!("owner changed"),
            )
            .unwrap_err();
            assert_eq!(native_io_outcome(&error), outcome);
            assert!(native_read_bytes(&error).is_none());
            assert!(error.downcast_ref::<NativeCause>().is_some());
        }
    }

    #[test]
    fn nested_read_guards_preserve_the_inner_owners_bytes_and_failures() {
        let original = complete_native_read(Ok(2), &[3, 7, 99], NativeCause.into());
        let mut untouched = [0xaa; 3];
        let error = checked_native_read(&|| Ok(()), &mut untouched, |_| Err(original), &|| {
            anyhow::bail!("outer owner changed")
        })
        .unwrap_err();
        assert_eq!(native_read_bytes(&error), Some([3, 7].as_slice()));
        assert_eq!(untouched, [0xaa; 3]);
        assert!(error.downcast_ref::<NativeCause>().is_some());
        assert!(format!("{error:#}").contains("outer owner changed"));
    }

    #[test]
    fn a_later_native_operation_cannot_reuse_a_prior_read_receipt() {
        let error = checked_native_read(
            &|| Err(complete_native_read(Ok(2), &[3, 7], NativeCause.into())),
            &mut [0xaa; 2],
            |_| panic!("preflight failure must not read"),
            &|| panic!("preflight failure must not validate a native result"),
        )
        .unwrap_err();
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
        assert!(native_read_bytes(&error).is_none());
        assert!(error.is::<NativeCause>());

        for outcome in [
            NativeIoOutcome::NotCalled,
            NativeIoOutcome::Rejected,
            NativeIoOutcome::Returned { byte_count: 1 },
            NativeIoOutcome::Returned { byte_count: 2 },
            NativeIoOutcome::Returned { byte_count: 3 },
        ] {
            let prior = complete_native_read(Ok(2), &[3, 7], NativeCause.into());
            let error = native_io_failure(outcome, prior);
            assert!(native_read_bytes(&error).is_none());
            assert!(error.is::<NativeCause>());
        }

        let prior = complete_native_read(Ok(0), &[], NativeCause.into());
        let error = native_io_failure(NativeIoOutcome::Returned { byte_count: 0 }, prior);
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: 0 })
        );
        assert!(native_read_bytes(&error).is_none());
        assert!(error.is::<NativeCause>());
    }

    #[test]
    fn an_oversized_read_cannot_reuse_an_older_receipt_of_the_same_length() {
        let prior = complete_native_read(Ok(3), &[1, 2, 3], NativeCause.into());
        let error = complete_native_read(Ok(3), &[9], prior);
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: 3 })
        );
        assert!(native_read_bytes(&error).is_none());
        assert!(error.is::<NativeCause>());
    }

    #[test]
    fn a_new_read_owns_its_bytes_even_when_an_older_read_has_the_same_length() {
        for count in [0, 3] {
            let prior = complete_native_read(Ok(count), &[1, 2, 3], NativeCause.into());
            let error = complete_native_read(Ok(count), &[9, 8, 7], prior);
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: count })
            );
            assert_eq!(native_read_bytes(&error), [9, 8, 7].get(..count));
            assert!(error.is::<NativeCause>());
        }
    }
}
