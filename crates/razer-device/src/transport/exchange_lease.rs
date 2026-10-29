use super::{FeatureTransport, HidNode, ReportLengths};
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
    let detail = format!("{error:#}");
    error.context(ExchangeFinalizationFailure { detail })
}

/// Complete the source finally release without discarding either failure.
pub fn finish_exchange<T>(
    lease: Box<dyn ExchangeLease + '_>,
    result: anyhow::Result<T>,
) -> anyhow::Result<T> {
    match (result, lease.finish()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(finalization_failure(error)),
        (Err(error), Err(release)) => Err(finalization_failure(
            error.context(format!("HID exchange release also failed: {release:#}")),
        )),
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

/// Retain a source write-attempt consumer at the actual native call boundary.
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
    fn run<T>(&self, operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
        (self.before)()?;
        checked_result(operation(), (self.after)())
    }

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
        self.run(|| self.inner.retry_release())
            .map_err(finalization_failure)
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
        before()?;
        checked_result(inner.finish(), after())
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
        let checked_after = self.after.clone();
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
        self.feature_length(report.len())?;
        self.run(|| {
            if let Some(observe) = &self.observe_send {
                observe(report);
            }
            self.inner.transport().send_feature(report)
        })
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.feature_length(report.len())?;
        self.run(|| {
            if let Some(observe) = &self.observe_send {
                observe(report);
            }
            self.inner
                .transport()
                .send_feature_at_collection_max(report)
        })
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.feature_length(report.len())?;
        self.run(|| self.inner.transport().get_feature(report))
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.feature_length(report.len())?;
        self.run(|| self.inner.transport().get_feature_at_collection_max(report))
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        self.run(|| self.inner.transport().write_output(report))
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.run(|| self.inner.transport().write_output_counted(report))
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.run(|| self.inner.transport().get_input(report))
    }

    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        self.run(|| self.inner.transport().read_interrupt(report, timeout_ms))
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
        self.check()
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
        let before: ExchangeValidation<'a> = Rc::new(move || {
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
            Rc::new(|| Ok(())),
            None,
        ))
    }
    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.hid_node()
    }

    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.check()?;
        self.inner.send_feature(report)
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.check()?;
        self.inner.send_feature_at_collection_max(report)
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.check()?;
        self.inner.get_feature(report)
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.check()?;
        self.inner.get_feature_at_collection_max(report)
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        self.check()?;
        self.inner.write_output(report)
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.check()?;
        self.inner.write_output_counted(report)
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.check()?;
        self.inner.get_input(report)
    }

    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        self.check()?;
        self.inner.read_interrupt(report, timeout_ms)
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
}
