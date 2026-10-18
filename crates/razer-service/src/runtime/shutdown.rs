//! Collect independent owner teardown results before the worker acknowledges exit.
use razer_ipc::ResponseEnvelope;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Default)]
pub(super) struct Cleanup {
    observations: Map<String, Value>,
    errors: Vec<String>,
}

impl Cleanup {
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
    fn independent_failures_keep_native_error_and_completed_observations() {
        let mut cleanup = Cleanup::default();
        cleanup.record::<()>("tracking", Err(anyhow::anyhow!("tracking failed")));
        cleanup.record("wristrest", Ok(serde_json::json!({"released":true})));
        cleanup.record::<()>("audio", Err(anyhow::anyhow!("audio failed")));
        let mut reply = ResponseEnvelope {
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
