//! The current host's collection-wide command timestamp and OUT polling rule.
use razer_device::backend::NativeIoOutcome;
use std::time::Duration;

const OUT_POLL_INTERVAL: Duration = Duration::from_millis(2);

/// Normal native returns refresh the timestamp regardless of device ACK status.
/// A thrown native call is classified by its `NativeIoOutcome`, not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeReturn {
    Send,
    Get,
    Batch,
}

/// Releases completed by the collection actor. The queue validates the complete
/// holder identity and URL before authorizing ExplicitMatched. A rejected
/// explicit release, cancellation or forced lease drop never reaches the actor,
/// so it leaves the clock unchanged as the source release branches require.
/// These events never acquire or release the queue's lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReleaseReason {
    ExplicitMatched,
    BatchFinally,
}

/// Retained by one collection incarnation, including across temporary handle
/// closes. `now` comes from that owner's single monotonic origin.
#[derive(Debug, Default)]
pub(super) struct SourceClock {
    last_command: Option<Duration>,
}

impl SourceClock {
    pub(super) fn native_outcome(
        &mut self,
        now: Duration,
        returned: NativeReturn,
        outcome: Option<NativeIoOutcome>,
    ) {
        if matches!(outcome, Some(NativeIoOutcome::Returned { .. })) {
            self.native_return(now, returned);
        }
    }

    /// UsbRzDeviceAction's mutex OUT loop polls in complete 2 ms steps. The
    /// caller supplies its source-selected interval, including an explicit zero;
    /// plain OUT does not call this method. OUT-IN and IN waits remain separate.
    pub(super) fn out_wait(&self, now: Duration, interval: Duration) -> Option<Duration> {
        let last = self.last_command?;
        (now.saturating_sub(last) < interval).then_some(OUT_POLL_INTERVAL)
    }

    pub(super) fn native_return(&mut self, now: Duration, returned: NativeReturn) {
        match returned {
            NativeReturn::Send | NativeReturn::Get | NativeReturn::Batch => {
                self.last_command = Some(now);
            }
        }
    }

    /// The explicit host release updates its clock before releasing a valid
    /// holder. The batch finally release carries no additional timestamp.
    pub(super) fn release(&mut self, now: Duration, reason: ReleaseReason) {
        match reason {
            ReleaseReason::ExplicitMatched => self.last_command = Some(now),
            ReleaseReason::BatchFinally => {}
        }
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
