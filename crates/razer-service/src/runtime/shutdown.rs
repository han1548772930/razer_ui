//! Collect independent owner teardown results before the worker acknowledges exit.
use razer_ipc::ResponseEnvelope;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Default)]
pub(super) struct Cleanup {
    observations: Map<String, Value>,
    errors: Vec<String>,
    resources_pending: bool,
}

pub(super) enum CleanupProgress {
    Pending(ResponseEnvelope),
    Released(ResponseEnvelope),
}

impl CleanupProgress {
    pub(super) fn into_response(self) -> ResponseEnvelope {
        match self {
            Self::Pending(response) | Self::Released(response) => response,
        }
    }
}

/// Cache only terminal owner results. A failed release that still owns native
/// resources must run again on the same owner before it can become terminal.
#[derive(Default)]
pub(super) struct OwnerCleanup {
    terminal: Option<Cleanup>,
    attempted: bool,
    interrupted: bool,
    failures: std::collections::BTreeSet<String>,
}

impl OwnerCleanup {
    pub(super) fn run(&mut self, action: impl FnOnce() -> Cleanup) -> Cleanup {
        if let Some(terminal) = &self.terminal {
            return terminal.clone();
        }
        self.interrupted |= self.attempted;
        self.attempted = true;
        let mut cleanup = action();
        self.attempted = false;
        if self.interrupted {
            cleanup.fail(
                "owner cleanup",
                "An earlier cleanup attempt panicked; its restoration result was not confirmed",
            );
        }
        self.failures.extend(cleanup.errors.drain(..));
        cleanup.errors.extend(self.failures.iter().cloned());
        if cleanup.resources_released() {
            self.terminal = Some(cleanup.clone());
        }
        cleanup
    }

    pub(super) fn run_result<T: Serialize>(
        &mut self,
        owner: &str,
        action: impl FnOnce() -> (anyhow::Result<T>, bool),
    ) -> Cleanup {
        self.run(|| {
            let (result, pending) = action();
            let mut cleanup = Cleanup::default();
            cleanup.record(owner, result);
            if pending {
                cleanup.retain_resources();
            }
            cleanup
        })
    }
}

impl Cleanup {
    pub(super) fn resources_released(&self) -> bool {
        !self.resources_pending
    }

    pub(super) fn retain_resources(&mut self) {
        self.resources_pending = true;
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.observations.extend(other.observations);
        self.errors.extend(other.errors);
        self.resources_pending |= other.resources_pending;
    }

    pub(super) fn progress(mut self, id: u64) -> CleanupProgress {
        let released = self.resources_released();
        if !released && self.errors.is_empty() {
            self.fail("native owners", "release is still pending");
        }
        let mut response = ResponseEnvelope {
            diagnostics: None,
            id,
            data: Some(serde_json::json!({"stopped": released})),
            error: None,
            fatal: false,
        };
        self.apply(&mut response);
        if released {
            CleanupProgress::Released(response)
        } else {
            CleanupProgress::Pending(response)
        }
    }

    #[cfg(test)]
    pub(super) fn succeeded(&self) -> bool {
        self.errors.is_empty()
    }
    pub(super) fn log_failures(&self) {
        for error in &self.errors {
            eprintln!("Service owner cleanup: {error}");
        }
    }

    pub(super) fn record<T: Serialize>(&mut self, owner: &str, result: anyhow::Result<T>) {
        match result.and_then(|value| serde_json::to_value(value).map_err(Into::into)) {
            Ok(value) => {
                self.observations.insert(owner.to_owned(), value);
            }
            Err(error) => self.errors.push(format!("{owner}: {error:#}")),
        }
    }

    pub(super) fn record_submission<
        S: Serialize,
        R: Serialize + super::feature_submission::WorkerReport,
    >(
        &mut self,
        owner: &str,
        result: anyhow::Result<Option<razer_device::feature_submission::Progress<S, R>>>,
    ) {
        if let Ok(Some(progress)) = &result {
            if let Some(error) = &progress.error {
                self.fail(owner, error);
            }
            if let Some(error) = progress
                .report
                .as_ref()
                .and_then(|report| report.cleanup_error())
            {
                self.fail(owner, error);
            }
        }
        self.record(owner, result);
    }

    pub(super) fn fail(&mut self, owner: &str, error: impl std::fmt::Display) {
        self.errors.push(format!("{owner}: {error}"));
    }

    pub(super) fn apply(self, response: &mut ResponseEnvelope) {
        let mut errors = self.errors;
        if let Some(error) = response.error.take() {
            errors.insert(0, error);
        }
        if errors.is_empty() {
            if let Some(data) = response.data.as_mut().and_then(Value::as_object_mut) {
                data.extend(self.observations);
            }
        } else {
            // The IPC envelope permits one outcome. Retain all completed
            // observations in its error detail, including an earlier native
            // shutdown failure; a later owner cannot overwrite it.
            if !self.observations.is_empty() {
                errors.push(format!(
                    "cleanup observations: {}",
                    Value::Object(self.observations)
                ));
            }
            response.data = None;
            response.error = Some(errors.join("; "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_cleanup_retries_retained_resources_then_caches_actual_release() {
        let mut owner = OwnerCleanup::default();
        let mut attempts = 0;
        let pending = owner.run_result("notifications", || {
            attempts += 1;
            (Err::<(), _>(anyhow::anyhow!("unregister failed")), true)
        });
        assert!(!pending.resources_released());
        assert!(!pending.succeeded());
        let released = owner.run_result("notifications", || {
            attempts += 1;
            (Ok(()), false)
        });
        assert!(released.resources_released() && !released.succeeded());
        assert!(
            released
                .errors
                .iter()
                .any(|error| error.contains("unregister failed"))
        );
        let cached = owner.run(|| panic!("a released owner cannot run cleanup again"));
        assert!(cached.resources_released() && !cached.succeeded());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn terminal_restoration_failure_remains_failed_without_repeating_the_consumed_owner() {
        let mut owner = OwnerCleanup::default();
        let first = owner.run_result("lighting", || {
            (
                Err::<(), _>(anyhow::anyhow!("restore readback failed after join")),
                false,
            )
        });
        assert!(first.resources_released());
        let repeated = owner.run(|| panic!("the terminal owner has already been joined"));
        for cleanup in [first, repeated] {
            let CleanupProgress::Released(response) = cleanup.progress(91) else {
                panic!("no live native owner remains");
            };
            assert!(response.data.is_none());
            assert!(
                response
                    .error
                    .unwrap()
                    .contains("restore readback failed after join")
            );
        }
    }

    #[test]
    fn interrupted_cleanup_keeps_the_unknown_restoration_result_after_retry_releases_resources() {
        let mut owner = OwnerCleanup::default();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.run(|| panic!("owner cleanup panicked after consuming its worker"));
            }))
            .is_err()
        );
        let cleanup = owner.run(Cleanup::default);
        let CleanupProgress::Released(response) = cleanup.progress(92) else {
            panic!("the retry released all native resources");
        };
        assert!(
            response
                .error
                .unwrap()
                .contains("earlier cleanup attempt panicked")
        );
    }

    #[test]
    fn independent_failures_keep_native_error_and_completed_observations() {
        let mut cleanup = Cleanup::default();
        cleanup.record::<()>("tracking", Err(anyhow::anyhow!("tracking failed")));
        cleanup.record("wristrest", Ok(serde_json::json!({"released":true})));
        cleanup.record::<()>("audio", Err(anyhow::anyhow!("audio failed")));
        let mut reply = ResponseEnvelope {
            diagnostics: None,
            id: 10,
            data: None,
            error: Some("native cleanup failed".into()),
            fatal: false,
        };
        cleanup.apply(&mut reply);
        let error = reply.error.unwrap();
        for expected in [
            "native cleanup failed",
            "tracking failed",
            "audio failed",
            "wristrest",
            "released",
        ] {
            assert!(error.contains(expected), "{error}");
        }
        assert!(reply.data.is_none());
    }

    #[test]
    fn successful_cleanup_is_part_of_the_shutdown_receipt() {
        let mut cleanup = Cleanup::default();
        cleanup.record(
            "camera",
            Ok(serde_json::json!({"operation":"actual-result"})),
        );
        let mut reply = ResponseEnvelope {
            diagnostics: None,
            id: 11,
            data: Some(serde_json::json!({"stopped":true})),
            error: None,
            fatal: false,
        };
        cleanup.apply(&mut reply);
        assert!(reply.error.is_none());
        assert_eq!(reply.data.unwrap()["camera"]["operation"], "actual-result");
    }
}
