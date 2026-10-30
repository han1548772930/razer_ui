use super::{
    FeatureBatch, FeatureBatchOperation, FeatureBatchReceipt, FeatureTransport, HidNode,
    ReportLengths, checked_feature_batch, checked_native_io, checked_native_read,
    native_io_attempted, observe_feature_batch, require_complete_send,
};
use serde_json::Value;
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Validation runs on the caller while waiting for admission and is retained
/// across retries. It never crosses to a collection's native actor thread.
pub type ExchangeValidation<'a> = Rc<dyn Fn() -> anyhow::Result<()> + 'a>;

/// Timing belongs to the source operation, not a generic transport default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExchangeTiming {
    Plain,
    MutexOut { interval: Duration },
    Batch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol25Topology {
    Normal,
    DualLink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceCommandScope {
    Protocol25 { topology: Protocol25Topology },
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ExchangeOptions {
    pub timing: ExchangeTiming,
    pub canceled: Option<Arc<AtomicBool>>,
    pub source_scope: Option<SourceCommandScope>,
}

impl ExchangeOptions {
    pub fn plain() -> Self {
        Self {
            timing: ExchangeTiming::Plain,
            canceled: None,
            source_scope: None,
        }
    }

    pub fn mutex_out(interval: Duration) -> Self {
        Self {
            timing: ExchangeTiming::MutexOut { interval },
            canceled: None,
            source_scope: None,
        }
    }

    pub fn batch() -> Self {
        Self {
            timing: ExchangeTiming::Batch,
            canceled: None,
            source_scope: None,
        }
    }

    pub fn with_cancellation(mut self, canceled: Arc<AtomicBool>) -> Self {
        self.canceled = Some(canceled);
        self
    }

    pub fn with_source_scope(mut self, scope: SourceCommandScope) -> Self {
        self.source_scope = Some(scope);
        self
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled
            .as_ref()
            .is_some_and(|canceled| canceled.load(Ordering::Acquire))
    }
}

/// Explicit finish is a source release boundary. Drop only gives up ownership;
/// it cannot claim a matching explicit release or a device acknowledgement.
pub trait ExchangeLease {
    fn transport(&self) -> &dyn FeatureTransport;
    /// End the current source HID cycle and acquire the next one through the
    /// same queue. The outer command and its recovery owner remain retained.
    fn retry_release(&self) -> anyhow::Result<()>;
    fn finish(self: Box<Self>) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct ExchangeFinalizationFailure {
    detail: String,
}

impl std::fmt::Display for ExchangeFinalizationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "HID exchange finalization failed: {}",
            self.detail
        )
    }
}

pub fn exchange_finalization_failed(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ExchangeFinalizationFailure>()
        .is_some()
}

pub(crate) fn finalization_failure(error: anyhow::Error) -> anyhow::Error {
    finalization_failure_after_release(error, false)
}

fn finalization_failure_after_release(
    error: anyhow::Error,
    release_completed: bool,
) -> anyhow::Error {
    let detail = format!("{error:#}");
    release_failure(error, release_completed).context(ExchangeFinalizationFailure { detail })
}

#[derive(Debug)]
struct ExchangeReleaseStatus {
    completed: bool,
    detail: String,
}

impl std::fmt::Display for ExchangeReleaseStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

fn release_failure(error: anyhow::Error, completed: bool) -> anyhow::Error {
    let detail = error.to_string();
    error.context(ExchangeReleaseStatus { completed, detail })
}

/// Reject the current release before executing it, regardless of older release
/// facts retained in the validation error. This does not mark finalization.
pub fn reject_exchange_release(error: anyhow::Error) -> anyhow::Error {
    release_failure(error, false)
}

/// The outermost release scope owns completion; older causes remain diagnostic.
pub fn exchange_release_completed(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ExchangeReleaseStatus>()
        .is_some_and(|release| release.completed)
}

/// Combine an executed release and its subsequent owner validation. Preflight
/// rejection uses `reject_exchange_release` because no release was executed.
pub fn validate_exchange_release(
    release: anyhow::Result<()>,
    validation: anyhow::Result<()>,
) -> anyhow::Result<()> {
    let completed = match &release {
        Ok(()) => true,
        Err(error) => exchange_release_completed(error),
    };
    checked_result(release, validation).map_err(|error| release_failure(error, completed))
}

/// Complete the source finally release without discarding either failure.
pub fn finish_exchange<T>(
    lease: Box<dyn ExchangeLease + '_>,
    result: anyhow::Result<T>,
) -> anyhow::Result<T> {
    match (result, lease.finish()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(release_failure(error, true)),
        (Ok(_), Err(error)) => {
            let completed = exchange_release_completed(&error);
            Err(finalization_failure_after_release(error, completed))
        }
        (Err(error), Err(release)) => {
            let completed = exchange_release_completed(&release);
            Err(finalization_failure_after_release(
                error.context(format!("HID exchange release also failed: {release:#}")),
                completed,
            ))
        }
    }
}

/// Preserve a checked adapter's identity guards and observed report extent
/// while delegating admission and native ownership to the inner transport.
pub fn checked_exchange<'a>(
    inner: Box<dyn ExchangeLease + 'a>,
    before: &'a dyn Fn() -> anyhow::Result<()>,
    after: &'a dyn Fn() -> anyhow::Result<()>,
    feature_report_bytes: Option<usize>,
) -> Box<dyn ExchangeLease + 'a> {
    checked_exchange_owned(inner, Rc::new(before), Rc::new(after), feature_report_bytes)
}

pub fn checked_exchange_owned<'a>(
    inner: Box<dyn ExchangeLease + 'a>,
    before: ExchangeValidation<'a>,
    after: ExchangeValidation<'a>,
    feature_report_bytes: Option<usize>,
) -> Box<dyn ExchangeLease + 'a> {
    Box::new(CheckedExchange {
        inner,
        before,
        after,
        feature_report_bytes,
        observe_send: None,
    })
}

/// Reconcile a source write-attempt consumer from the retained native owner's
/// terminal facts, before the caller publishes or restores connection state.
pub fn checked_exchange_observing_send<'a>(
    inner: Box<dyn ExchangeLease + 'a>,
    before: &'a dyn Fn() -> anyhow::Result<()>,
    after: &'a dyn Fn() -> anyhow::Result<()>,
    feature_report_bytes: Option<usize>,
    observe_send: impl Fn(&[u8]) + 'a,
) -> Box<dyn ExchangeLease + 'a> {
    Box::new(CheckedExchange {
        inner,
        before: Rc::new(before),
        after: Rc::new(after),
        feature_report_bytes,
        observe_send: Some(Box::new(observe_send)),
    })
}

struct CheckedExchange<'a> {
    inner: Box<dyn ExchangeLease + 'a>,
    before: ExchangeValidation<'a>,
    after: ExchangeValidation<'a>,
    feature_report_bytes: Option<usize>,
    observe_send: Option<Box<dyn Fn(&[u8]) + 'a>>,
}

fn checked_result<T>(
    result: anyhow::Result<T>,
    validation: anyhow::Result<()>,
) -> anyhow::Result<T> {
    match (result, validation) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(validation)) => Err(error.context(format!(
            "HID exchange validation also failed: {validation:#}"
        ))),
    }
}

impl CheckedExchange<'_> {
    fn feature_length(&self, bytes: usize) -> anyhow::Result<()> {
        if let Some(expected) = self.feature_report_bytes {
            anyhow::ensure!(
                bytes == expected,
                "Feature report extent {bytes} differs from observed interface length {expected}"
            );
        }
        Ok(())
    }
}

impl ExchangeLease for CheckedExchange<'_> {
    fn transport(&self) -> &dyn FeatureTransport {
        self
    }

    fn retry_release(&self) -> anyhow::Result<()> {
        let result = (|| {
            (self.before)().map_err(reject_exchange_release)?;
            validate_exchange_release(self.inner.retry_release(), (self.after)())
        })();
        result.map_err(|error| {
            let completed = exchange_release_completed(&error);
            finalization_failure_after_release(error, completed)
        })
    }

    fn finish(self: Box<Self>) -> anyhow::Result<()> {
        let Self {
            inner,
            before,
            after,
            ..
        } = *self;
        // Identity loss or cancellation cannot claim a matched explicit
        // release. Dropping the retained inner owner requests forced cleanup.
        before().map_err(reject_exchange_release)?;
        validate_exchange_release(inner.finish(), after())
    }
}

impl FeatureTransport for CheckedExchange<'_> {
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        self.begin_exchange_validated(options, Rc::new(|| Ok(())))
    }

    fn begin_exchange_validated<'a>(
        &'a self,
        options: ExchangeOptions,
        validate: ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn ExchangeLease + 'a>> {
        let before = self.before.clone();
        let before_validate = validate.clone();
        let checked_before: ExchangeValidation<'a> = Rc::new(move || {
            before()?;
            before_validate()
        });
        let after = self.after.clone();
        let checked_after: ExchangeValidation<'a> = Rc::new(move || {
            after()?;
            validate()
        });
        checked_before()?;
        // This is already an admitted lease's transport. Retain its ownership
        // and validate nested users without entering the collection queue again.
        Ok(checked_exchange_owned(
            already_exclusive(self as &dyn FeatureTransport, options)?,
            checked_before,
            checked_after,
            self.feature_report_bytes,
        ))
    }
    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.transport().hid_node()
    }

    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        require_complete_send(self.send_feature_counted(report)?, report.len())
    }

    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            &|| {
                self.feature_length(report.len())?;
                (self.before)()
            },
            || {
                let result = self.inner.transport().send_feature_counted(report);
                if native_io_attempted(&result) {
                    if let Some(observe) = &self.observe_send {
                        observe(report);
                    }
                }
                result
            },
            self.after.as_ref(),
        )
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        require_complete_send(
            self.send_feature_at_collection_max_counted(report)?,
            report.len(),
        )
    }

    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            &|| {
                self.feature_length(report.len())?;
                (self.before)()
            },
            || {
                let result = self
                    .inner
                    .transport()
                    .send_feature_at_collection_max_counted(report);
                if native_io_attempted(&result) {
                    if let Some(observe) = &self.observe_send {
                        observe(report);
                    }
                }
                result
            },
            self.after.as_ref(),
        )
    }

    fn send_feature_batch(&self, batch: &FeatureBatch) -> anyhow::Result<FeatureBatchReceipt> {
        checked_feature_batch(
            &|| {
                for operation in batch.operations() {
                    if let FeatureBatchOperation::Report { bytes } = operation {
                        self.feature_length(bytes.len())?;
                    }
                }
                (self.before)()
            },
            || {
                let result = self.inner.transport().send_feature_batch(batch);
                if let Some(observe) = &self.observe_send {
                    observe_feature_batch(batch, &result, observe.as_ref());
                }
                result
            },
            self.after.as_ref(),
        )
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let bytes = report.len();
        checked_native_read(
            &|| {
                self.feature_length(bytes)?;
                (self.before)()
            },
            report,
            |report| self.inner.transport().get_feature(report),
            self.after.as_ref(),
        )
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let bytes = report.len();
        checked_native_read(
            &|| {
                self.feature_length(bytes)?;
                (self.before)()
            },
            report,
            |report| self.inner.transport().get_feature_at_collection_max(report),
            self.after.as_ref(),
        )
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        checked_native_io(
            self.before.as_ref(),
            || {
                self.inner
                    .transport()
                    .write_output(report)
                    .map(|()| report.len())
            },
            self.after.as_ref(),
        )
        .map(|_| ())
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            self.before.as_ref(),
            || self.inner.transport().write_output_counted(report),
            self.after.as_ref(),
        )
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            self.before.as_ref(),
            report,
            |report| self.inner.transport().get_input(report),
            self.after.as_ref(),
        )
    }

    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        checked_native_read(
            self.before.as_ref(),
            report,
            |report| self.inner.transport().read_interrupt(report, timeout_ms),
            self.after.as_ref(),
        )
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        self.inner.transport().report_lengths()
    }

    fn metadata(&self) -> Value {
        self.inner.transport().metadata()
    }
}

struct AlreadyExclusive<'a, T: FeatureTransport + ?Sized> {
    inner: &'a T,
    options: ExchangeOptions,
}

pub(super) fn already_exclusive<T: FeatureTransport + ?Sized>(
    inner: &T,
    options: ExchangeOptions,
) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
    anyhow::ensure!(
        !options.is_canceled(),
        "HID exchange was canceled before admission"
    );
    Ok(Box::new(AlreadyExclusive { inner, options }))
}

impl<T: FeatureTransport + ?Sized> AlreadyExclusive<'_, T> {
    fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.options.is_canceled(), "HID exchange was canceled");
        Ok(())
    }
}

impl<T: FeatureTransport + ?Sized> ExchangeLease for AlreadyExclusive<'_, T> {
    fn transport(&self) -> &dyn FeatureTransport {
        self
    }

    fn retry_release(&self) -> anyhow::Result<()> {
        // A raw exclusive handle has no competing queue to re-enter.
        self.check().map_err(finalization_failure)
    }

    fn finish(self: Box<Self>) -> anyhow::Result<()> {
        self.check().map_err(reject_exchange_release)
    }
}

impl<T: FeatureTransport + ?Sized> FeatureTransport for AlreadyExclusive<'_, T> {
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        self.begin_exchange_validated(options, Rc::new(|| Ok(())))
    }

    fn begin_exchange_validated<'a>(
        &'a self,
        options: ExchangeOptions,
        validate: ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn ExchangeLease + 'a>> {
        let before_validate = validate.clone();
        let before: ExchangeValidation<'a> = Rc::new(move || {
            self.check()?;
            before_validate()
        });
        let after: ExchangeValidation<'a> = Rc::new(move || {
            self.check()?;
            validate()
        });
        before()?;
        // Raw transports may be unsized, so their first exclusive wrapper is
        // generic. Reborrow only through the trait object to keep subsequent
        // lease types finite and retain this owner's cancellation guard.
        Ok(checked_exchange_owned(
            already_exclusive(self as &dyn FeatureTransport, options)?,
            before,
            after,
            None,
        ))
    }
    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.hid_node()
    }

    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        require_complete_send(self.send_feature_counted(report)?, report.len())
    }

    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            &|| self.check(),
            || self.inner.send_feature_counted(report),
            &|| self.check(),
        )
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        require_complete_send(
            self.send_feature_at_collection_max_counted(report)?,
            report.len(),
        )
    }

    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            &|| self.check(),
            || self.inner.send_feature_at_collection_max_counted(report),
            &|| self.check(),
        )
    }

    fn send_feature_batch(&self, batch: &FeatureBatch) -> anyhow::Result<FeatureBatchReceipt> {
        checked_feature_batch(
            &|| self.check(),
            || self.inner.send_feature_batch(batch),
            &|| self.check(),
        )
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            &|| self.check(),
            report,
            |report| self.inner.get_feature(report),
            &|| self.check(),
        )
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            &|| self.check(),
            report,
            |report| self.inner.get_feature_at_collection_max(report),
            &|| self.check(),
        )
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        checked_native_io(
            &|| self.check(),
            || self.inner.write_output(report).map(|()| report.len()),
            &|| self.check(),
        )
        .map(|_| ())
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            &|| self.check(),
            || self.inner.write_output_counted(report),
            &|| self.check(),
        )
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            &|| self.check(),
            report,
            |report| self.inner.get_input(report),
            &|| self.check(),
        )
    }

    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        checked_native_read(
            &|| self.check(),
            report,
            |report| self.inner.read_interrupt(report, timeout_ms),
            &|| self.check(),
        )
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        self.inner.report_lengths()
    }

    fn metadata(&self) -> Value {
        self.inner.metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn an_earlier_completed_release_does_not_hide_the_current_lease_release_failure() {
        struct FailingRelease;
        impl ExchangeLease for FailingRelease {
            fn transport(&self) -> &dyn FeatureTransport {
                panic!("no I/O in release classification")
            }
            fn retry_release(&self) -> anyhow::Result<()> {
                panic!("no retry")
            }
            fn finish(self: Box<Self>) -> anyhow::Result<()> {
                anyhow::bail!("current release failed")
            }
        }
        let earlier =
            validate_exchange_release(Ok(()), Err(anyhow::anyhow!("earlier owner retired")))
                .unwrap_err();
        assert!(exchange_release_completed(&earlier));
        let result: anyhow::Result<()> = finish_exchange(Box::new(FailingRelease), Err(earlier));
        let error = result.unwrap_err();
        assert!(!exchange_release_completed(&error));
        assert!(exchange_finalization_failed(&error));
        assert!(format!("{error:#}").contains("earlier owner retired"));
        assert!(format!("{error:#}").contains("current release failed"));
    }

    #[derive(Debug)]
    struct CallerIdentityLost;

    impl std::fmt::Display for CallerIdentityLost {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("caller identity changed during the retained operation")
        }
    }

    impl std::error::Error for CallerIdentityLost {}

    #[derive(Debug)]
    struct CurrentReleaseFailure;

    impl std::fmt::Display for CurrentReleaseFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("current native release failed")
        }
    }

    impl std::error::Error for CurrentReleaseFailure {}

    struct ReleaseScope<'a> {
        finishes: &'a Cell<usize>,
        retries: &'a Cell<usize>,
        drops: &'a Cell<usize>,
        fails: bool,
    }

    impl ReleaseScope<'_> {
        fn result(&self) -> anyhow::Result<()> {
            if self.fails {
                Err(CurrentReleaseFailure.into())
            } else {
                Ok(())
            }
        }
    }

    impl ExchangeLease for ReleaseScope<'_> {
        fn transport(&self) -> &dyn FeatureTransport {
            panic!("release classification must not perform packet I/O")
        }

        fn retry_release(&self) -> anyhow::Result<()> {
            self.retries.set(self.retries.get() + 1);
            self.result()
        }

        fn finish(self: Box<Self>) -> anyhow::Result<()> {
            self.finishes.set(self.finishes.get() + 1);
            self.result()
        }
    }

    impl Drop for ReleaseScope<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn release_classification_preserves_the_original_error_display_and_typed_cause() {
        for completed in [false, true] {
            let cause: anyhow::Error = CallerIdentityLost.into();
            let expected = cause.to_string();
            let error = if completed {
                validate_exchange_release(Ok(()), Err(cause)).unwrap_err()
            } else {
                reject_exchange_release(cause)
            };
            assert_eq!(error.to_string(), expected);
            assert_eq!(exchange_release_completed(&error), completed);
            assert!(!exchange_finalization_failed(&error));
            assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
        }

        let finishes = Cell::new(0);
        let retries = Cell::new(0);
        let drops = Cell::new(0);
        let lease = Box::new(ReleaseScope {
            finishes: &finishes,
            retries: &retries,
            drops: &drops,
            fails: false,
        });
        let cause = std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "native write returned 66 bytes; source requires 65",
        );
        let expected = cause.to_string();
        let result: anyhow::Result<()> = finish_exchange(lease, Err(cause.into()));
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert!(exchange_release_completed(&error));
        assert!(!exchange_finalization_failed(&error));
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::Unsupported
        );
        assert_eq!((finishes.get(), retries.get(), drops.get()), (1, 0, 1));
    }

    #[test]
    fn a_completed_current_release_overrides_an_earlier_failed_scope_in_its_after_guard() {
        for retry in [false, true] {
            for nested in [false, true] {
                let finishes = Cell::new(0);
                let retries = Cell::new(0);
                let drops = Cell::new(0);
                let current = || Ok(());
                let after = || {
                    let earlier = finalization_failure(CallerIdentityLost.into());
                    assert!(!exchange_release_completed(&earlier));
                    Err(earlier.context("earlier failed release retained by the after guard"))
                };
                let mut lease = checked_exchange(
                    Box::new(ReleaseScope {
                        finishes: &finishes,
                        retries: &retries,
                        drops: &drops,
                        fails: false,
                    }),
                    &current,
                    &after,
                    None,
                );
                if nested {
                    lease = checked_exchange(lease, &current, &current, None);
                }
                let error = if retry {
                    let error = lease.retry_release().unwrap_err();
                    drop(lease);
                    error
                } else {
                    finish_exchange(lease, Ok(())).unwrap_err()
                };
                assert!(exchange_release_completed(&error));
                assert!(exchange_finalization_failed(&error));
                assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
                assert!(format!("{error:#}").contains("earlier failed release retained"));
                assert_eq!(finishes.get(), usize::from(!retry));
                assert_eq!(retries.get(), usize::from(retry));
                assert_eq!(drops.get(), 1);
            }
        }
    }

    #[test]
    fn a_current_preflight_rejection_overrides_an_earlier_completed_release_scope() {
        for retry in [false, true] {
            let finishes = Cell::new(0);
            let retries = Cell::new(0);
            let drops = Cell::new(0);
            let before = || {
                let earlier =
                    validate_exchange_release(Ok(()), Err(CallerIdentityLost.into())).unwrap_err();
                assert!(exchange_release_completed(&earlier));
                Err(earlier.context("earlier completed release retained by the before guard"))
            };
            let after = || -> anyhow::Result<()> {
                panic!("preflight rejection must not run the after guard")
            };
            let lease = checked_exchange(
                Box::new(ReleaseScope {
                    finishes: &finishes,
                    retries: &retries,
                    drops: &drops,
                    fails: false,
                }),
                &before,
                &after,
                None,
            );
            let error = if retry {
                let error = lease.retry_release().unwrap_err();
                drop(lease);
                error
            } else {
                finish_exchange(lease, Ok(())).unwrap_err()
            };
            assert!(!exchange_release_completed(&error));
            assert!(exchange_finalization_failed(&error));
            assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
            assert!(format!("{error:#}").contains("earlier completed release retained"));
            assert_eq!((finishes.get(), retries.get(), drops.get()), (0, 0, 1));
        }
    }

    #[test]
    fn a_failed_current_release_keeps_its_typed_cause_and_ignores_after_guard_completion() {
        for retry in [false, true] {
            let finishes = Cell::new(0);
            let retries = Cell::new(0);
            let drops = Cell::new(0);
            let current = || Ok(());
            let after = || {
                validate_exchange_release(Ok(()), Err(CallerIdentityLost.into())).map_err(|error| {
                    error.context("after guard retains an earlier completed release")
                })
            };
            let lease = checked_exchange(
                Box::new(ReleaseScope {
                    finishes: &finishes,
                    retries: &retries,
                    drops: &drops,
                    fails: true,
                }),
                &current,
                &after,
                None,
            );
            let error = if retry {
                let error = lease.retry_release().unwrap_err();
                drop(lease);
                error
            } else {
                finish_exchange(lease, Ok(())).unwrap_err()
            };
            assert!(!exchange_release_completed(&error));
            assert!(exchange_finalization_failed(&error));
            assert!(error.downcast_ref::<CurrentReleaseFailure>().is_some());
            let message = format!("{error:#}");
            assert!(message.contains("current native release failed"));
            assert!(message.contains("after guard retains an earlier completed release"));
            assert!(message.contains("caller identity changed"));
            assert_eq!(finishes.get(), usize::from(!retry));
            assert_eq!(retries.get(), usize::from(retry));
            assert_eq!(drops.get(), 1);
        }
    }

    #[test]
    fn final_release_status_comes_from_the_current_lease_and_preserves_the_operation_cause() {
        for fails in [false, true] {
            let finishes = Cell::new(0);
            let retries = Cell::new(0);
            let drops = Cell::new(0);
            let current = || Ok(());
            let after = || {
                anyhow::ensure!(
                    !fails,
                    "owner validation failed after the current release failure"
                );
                Ok(())
            };
            let lease = checked_exchange(
                Box::new(ReleaseScope {
                    finishes: &finishes,
                    retries: &retries,
                    drops: &drops,
                    fails,
                }),
                &current,
                &after,
                None,
            );
            let earlier = finalization_failure(CallerIdentityLost.into());
            assert!(!exchange_release_completed(&earlier));
            let result: anyhow::Result<()> = finish_exchange(lease, Err(earlier));
            let error = result.unwrap_err();
            assert_eq!(exchange_release_completed(&error), !fails);
            assert!(exchange_finalization_failed(&error));
            assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
            let message = format!("{error:#}");
            assert!(message.contains("caller identity changed"));
            if fails {
                assert!(message.contains("current native release failed"));
                assert!(
                    message.contains("owner validation failed after the current release failure")
                );
            }
            assert_eq!((finishes.get(), retries.get(), drops.get()), (1, 0, 1));
        }
    }

    struct ExclusiveTransport(Cell<usize>);

    impl FeatureTransport for ExclusiveTransport {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }

        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            report.fill(7);
            Ok(report.len())
        }

        fn metadata(&self) -> Value {
            Value::Null
        }
    }

    #[test]
    fn a_retry_release_failure_remains_terminal_when_the_final_finish_succeeds() {
        struct FailedRetry<'a>(&'a ExclusiveTransport);
        impl ExchangeLease for FailedRetry<'_> {
            fn transport(&self) -> &dyn FeatureTransport {
                self.0
            }
            fn retry_release(&self) -> anyhow::Result<()> {
                anyhow::bail!("simulated retry release failure")
            }
            fn finish(self: Box<Self>) -> anyhow::Result<()> {
                Ok(())
            }
        }
        let raw = ExclusiveTransport(Cell::new(0));
        let validate = || Ok(());
        let lease = checked_exchange(Box::new(FailedRetry(&raw)), &validate, &validate, None);
        let retry = lease.retry_release();
        let error = finish_exchange(lease, retry).unwrap_err();
        assert!(exchange_finalization_failed(&error));
        assert!(format!("{error:#}").contains("simulated retry release failure"));
        assert_eq!(raw.0.get(), 0);
    }

    #[test]
    fn default_exclusive_reborrows_keep_parent_cancellation_through_nested_validated_scopes() {
        let raw = ExclusiveTransport(Cell::new(0));
        let canceled = Arc::new(AtomicBool::new(false));
        let parent = raw
            .begin_exchange(ExchangeOptions::plain().with_cancellation(canceled.clone()))
            .unwrap();
        let child = parent
            .transport()
            .begin_exchange(ExchangeOptions::plain())
            .unwrap();
        let nested = child
            .transport()
            .begin_exchange_validated(ExchangeOptions::plain(), Rc::new(|| Ok(())))
            .unwrap();
        nested.transport().send_feature(&[1]).unwrap();
        canceled.store(true, Ordering::Release);
        assert!(nested.transport().send_feature(&[2]).is_err());
        assert!(nested.finish().is_err());
        assert!(child.finish().is_err());
        assert!(parent.finish().is_err());
        assert_eq!(raw.0.get(), 1);
    }

    #[test]
    fn canceling_an_exclusive_reborrow_keeps_the_parent_scope_usable() {
        let raw = ExclusiveTransport(Cell::new(0));
        let parent = raw.begin_exchange(ExchangeOptions::plain()).unwrap();
        let canceled = Arc::new(AtomicBool::new(false));
        let child = parent
            .transport()
            .begin_exchange(ExchangeOptions::plain().with_cancellation(canceled.clone()))
            .unwrap();
        canceled.store(true, Ordering::Release);
        assert!(child.transport().send_feature(&[1]).is_err());
        assert!(child.finish().is_err());
        parent.transport().send_feature(&[2]).unwrap();
        parent.finish().unwrap();
        assert_eq!(raw.0.get(), 1);
    }

    #[test]
    fn validated_scopes_reject_identity_changes_during_native_packets_and_whole_batches() {
        #[derive(Clone, Copy)]
        enum Operation {
            Send,
            CollectionSend,
            Get,
            CollectionGet,
            Batch,
        }

        enum Scope {
            Direct,
            Exclusive,
            Checked,
        }

        struct ChangingIdentity {
            current: Cell<bool>,
            calls: Cell<usize>,
        }
        impl FeatureTransport for ChangingIdentity {
            fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
                require_complete_send(self.send_feature_counted(report)?, report.len())
            }
            fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
                self.calls.set(self.calls.get() + 1);
                self.current.set(false);
                Ok(report.len())
            }
            fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
                self.calls.set(self.calls.get() + 1);
                report.fill(7);
                self.current.set(false);
                Ok(report.len())
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }

        fn exercise(transport: &dyn FeatureTransport, io: &ChangingIdentity, operation: Operation) {
            let owner = std::thread::current().id();
            let validate = Rc::new(|| {
                assert_eq!(std::thread::current().id(), owner);
                if io.current.get() {
                    Ok(())
                } else {
                    Err(CallerIdentityLost.into())
                }
            });
            let lease = transport
                .begin_exchange_validated(ExchangeOptions::plain(), validate)
                .unwrap();
            let mut report = [1; 4];
            let error = match operation {
                Operation::Send => lease.transport().send_feature_counted(&report).unwrap_err(),
                Operation::CollectionSend => lease
                    .transport()
                    .send_feature_at_collection_max_counted(&report)
                    .unwrap_err(),
                Operation::Get => lease.transport().get_feature(&mut report).unwrap_err(),
                Operation::CollectionGet => lease
                    .transport()
                    .get_feature_at_collection_max(&mut report)
                    .unwrap_err(),
                Operation::Batch => {
                    let mut batch = FeatureBatch::new();
                    batch.push_report(&report).unwrap();
                    batch.push_report(&report).unwrap();
                    let error = lease.transport().send_feature_batch(&batch).unwrap_err();
                    let failure = super::super::FeatureBatchFailure::from_error(&error).unwrap();
                    assert_eq!(failure.progress.completed_operations, 2);
                    assert_eq!(failure.progress.reports_submitted(), 2);
                    assert_eq!(failure.raw_return, Some(8));
                    error
                }
            };
            let is_batch = matches!(operation, Operation::Batch);
            assert_eq!(io.calls.get(), if is_batch { 2 } else { 1 });
            assert_eq!(
                super::super::native_io_outcome(&error),
                Some(super::super::NativeIoOutcome::Returned {
                    byte_count: if is_batch { 8 } else { 4 },
                })
            );
            assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
            assert!(lease.finish().is_err());
        }

        for operation in [
            Operation::Send,
            Operation::CollectionSend,
            Operation::Get,
            Operation::CollectionGet,
            Operation::Batch,
        ] {
            for scope in [Scope::Direct, Scope::Exclusive, Scope::Checked] {
                let io = ChangingIdentity {
                    current: Cell::new(true),
                    calls: Cell::new(0),
                };
                let own = || Ok(());
                match scope {
                    Scope::Direct => exercise(&io, &io, operation),
                    Scope::Exclusive => {
                        let parent = io.begin_exchange(ExchangeOptions::plain()).unwrap();
                        exercise(parent.transport(), &io, operation);
                        parent.finish().unwrap();
                    }
                    Scope::Checked => {
                        let parent = checked_exchange(
                            io.begin_exchange(ExchangeOptions::plain()).unwrap(),
                            &own,
                            &own,
                            None,
                        );
                        exercise(parent.transport(), &io, operation);
                        parent.finish().unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn validated_default_leases_recheck_identity_after_retry_release_and_finish() {
        struct ReleaseChangesIdentity {
            current: Cell<bool>,
            releases: Cell<usize>,
        }
        struct Release<'a>(&'a ReleaseChangesIdentity);
        impl ExchangeLease for Release<'_> {
            fn transport(&self) -> &dyn FeatureTransport {
                self.0
            }
            fn retry_release(&self) -> anyhow::Result<()> {
                self.0.releases.set(self.0.releases.get() + 1);
                self.0.current.set(false);
                Ok(())
            }
            fn finish(self: Box<Self>) -> anyhow::Result<()> {
                self.retry_release()
            }
        }
        impl FeatureTransport for ReleaseChangesIdentity {
            fn begin_exchange(
                &self,
                _: ExchangeOptions,
            ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
                Ok(Box::new(Release(self)))
            }
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("release validation must not send a report")
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                panic!("release validation must not read a report")
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }

        for retry in [false, true] {
            let io = ReleaseChangesIdentity {
                current: Cell::new(true),
                releases: Cell::new(0),
            };
            let validate = Rc::new(|| {
                if io.current.get() {
                    Ok(())
                } else {
                    Err(CallerIdentityLost.into())
                }
            });
            let lease = io
                .begin_exchange_validated(ExchangeOptions::plain(), validate)
                .unwrap();
            let error = if retry {
                lease.retry_release().unwrap_err()
            } else {
                lease.finish().unwrap_err()
            };
            assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
            assert_eq!(io.releases.get(), 1);
        }
    }

    #[test]
    fn an_admitted_batch_drains_its_native_operation_before_rejecting_canceled_publication() {
        struct BatchIo {
            sends: Cell<usize>,
            canceled: Arc<AtomicBool>,
        }
        impl FeatureTransport for BatchIo {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                self.sends.set(self.sends.get() + 1);
                self.canceled.store(true, Ordering::Release);
                Ok(())
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                panic!("a source batch has no Feature GET")
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }
        let canceled = Arc::new(AtomicBool::new(false));
        let io = BatchIo {
            sends: Cell::new(0),
            canceled: canceled.clone(),
        };
        let batch = FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf0, 2, 0, 2]).unwrap();
        let lease = io
            .begin_exchange(ExchangeOptions::batch().with_cancellation(canceled))
            .unwrap();
        let error = lease.transport().send_feature_batch(&batch).unwrap_err();
        assert_eq!(io.sends.get(), 2);
        assert_eq!(
            super::super::native_io_outcome(&error),
            Some(super::super::NativeIoOutcome::Returned { byte_count: 4 })
        );
        assert_eq!(
            super::super::FeatureBatchFailure::from_error(&error)
                .unwrap()
                .progress
                .reports_submitted(),
            2
        );
        assert!(lease.finish().is_err());
    }

    #[test]
    fn checked_batch_validates_all_report_extents_before_dispatching_any_report() {
        let io = ExclusiveTransport(Cell::new(0));
        let before = || Ok(());
        let checked = checked_exchange(
            io.begin_exchange(ExchangeOptions::batch()).unwrap(),
            &before,
            &before,
            Some(2),
        );
        let batch = FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf0, 1, 0]).unwrap();
        let error = checked.transport().send_feature_batch(&batch).unwrap_err();
        assert_eq!(
            super::super::native_io_outcome(&error),
            Some(super::super::NativeIoOutcome::NotCalled)
        );
        assert_eq!(io.0.get(), 0);
        checked.finish().unwrap();
    }

    #[test]
    fn counted_feature_sends_keep_the_actual_return_through_nested_exclusive_and_checked_leases() {
        struct Counted;
        impl FeatureTransport for Counted {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("a counted lease must retain the actual count")
            }
            fn send_feature_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
                Ok(1)
            }
            fn send_feature_at_collection_max_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
                Ok(1)
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                Ok(1)
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }
        let io = Counted;
        let parent = io.begin_exchange(ExchangeOptions::plain()).unwrap();
        let validate = || Ok(());
        let checked = checked_exchange(parent, &validate, &validate, None);
        let child = checked
            .transport()
            .begin_exchange(ExchangeOptions::plain())
            .unwrap();
        assert_eq!(child.transport().send_feature_counted(&[0, 1]).unwrap(), 1);
        assert_eq!(
            child
                .transport()
                .send_feature_at_collection_max_counted(&[0, 1])
                .unwrap(),
            1
        );
        let error = child.transport().send_feature(&[0, 1]).unwrap_err();
        assert_eq!(
            super::super::native_io_outcome(&error),
            Some(super::super::NativeIoOutcome::Returned { byte_count: 1 })
        );
        child.finish().unwrap();
        checked.finish().unwrap();
    }

    #[test]
    fn checked_send_observes_only_terminal_native_attempts_including_failed_attempts() {
        struct Scripted(Option<super::super::NativeIoOutcome>);
        impl FeatureTransport for Scripted {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                let error = anyhow::anyhow!("scripted send failure");
                Err(match self.0 {
                    Some(outcome) => super::super::native_io_failure(outcome, error),
                    None => error,
                })
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                unreachable!()
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }
        for (outcome, count) in [
            (None, 0),
            (Some(super::super::NativeIoOutcome::NotCalled), 0),
            (Some(super::super::NativeIoOutcome::Rejected), 1),
            (
                Some(super::super::NativeIoOutcome::Returned { byte_count: 1 }),
                1,
            ),
        ] {
            let io = Scripted(outcome);
            let observed = Cell::new(0);
            let validate = || Ok(());
            let checked = checked_exchange_observing_send(
                io.begin_exchange(ExchangeOptions::plain()).unwrap(),
                &validate,
                &validate,
                None,
                |_| observed.set(observed.get() + 1),
            );
            assert!(checked.transport().send_feature(&[0, 1]).is_err());
            assert_eq!(observed.get(), count);
            checked.finish().unwrap();

            let batch = FeatureBatch::from_bytecode(&[0xf0, 2, 0, 1, 0xf0, 2, 0, 2]).unwrap();
            let observed = Cell::new(0);
            let checked = checked_exchange_observing_send(
                io.begin_exchange(ExchangeOptions::batch()).unwrap(),
                &validate,
                &validate,
                None,
                |_| observed.set(observed.get() + 1),
            );
            assert!(checked.transport().send_feature_batch(&batch).is_err());
            assert_eq!(observed.get(), count);
            checked.finish().unwrap();
        }
    }
}
