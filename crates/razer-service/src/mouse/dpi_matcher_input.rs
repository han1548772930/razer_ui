//! Input-monitor receipts own the partial-start state independently of the
//! calibration state machine. A lost start acknowledgement still needs stop.
use razer_device::dpi_matcher::InputBatch;
use razer_ipc::{ServiceRequest, mouse_input_monitoring::Session};
use serde_json::Value;

pub(super) struct InputMonitor {
    session: Session,
}

impl InputMonitor {
    pub(super) fn new(owner: String) -> Self {
        Self {
            session: Session::new(owner).expect("retained Matcher owner is nonempty"),
        }
    }

    pub(super) fn possibly_started(&self) -> bool {
        self.session.possibly_started()
    }

    pub(super) fn start(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<()> {
        self.session.start(request)
    }

    pub(super) fn stop(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<()> {
        self.session.stop(request)
    }

    pub(super) fn events(
        &self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<Vec<InputBatch>> {
        self.session
            .poll(request)?
            .into_iter()
            .map(|event| {
                InputBatch::from_event(event.product_id(), event.container_id(), event.json_event())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dpi_matcher_partial_start_cannot_restart_until_its_own_stop_is_acknowledged() {
        let mut monitor = InputMonitor::new("matcher-owner".into());
        assert!(
            monitor
                .start(|_| anyhow::bail!("lost start acknowledgement"))
                .is_err()
        );
        assert!(monitor.possibly_started());
        assert!(
            monitor
                .start(|_| panic!("must not start over a partial owner"))
                .is_err()
        );
        assert!(
            monitor
                .stop(|_| Ok(json!({"owner_guid":"old-owner","monitoring":false})))
                .is_err()
        );
        assert!(monitor.possibly_started());
        monitor.stop(|request| {
            assert!(matches!(request, ServiceRequest::MouseInputMonitoringStop { owner_guid } if owner_guid == "matcher-owner"));
            Ok(json!({"owner_guid":"matcher-owner","monitoring":false}))
        }).unwrap();
        assert!(!monitor.possibly_started());
        monitor
            .stop(|_| panic!("acknowledged stop must not run again"))
            .unwrap();
    }

    #[test]
    fn dpi_matcher_monitor_rejects_stale_polls_and_incomplete_event_streams() {
        let mut monitor = InputMonitor::new("matcher-owner".into());
        monitor
            .start(|_| Ok(json!({"owner_guid":"matcher-owner","monitoring":true})))
            .unwrap();
        for receipt in [
            json!({"owner_guid":"old-owner","events":[],"failure":null}),
            json!({"owner_guid":"matcher-owner","events":[],"failure":"callback overflow"}),
        ] {
            assert!(monitor.events(|_| Ok(receipt.clone())).is_err());
            assert!(monitor.possibly_started());
        }
        monitor
            .stop(|_| Ok(json!({"owner_guid":"matcher-owner","monitoring":false})))
            .unwrap();
        assert!(
            monitor
                .events(|_| panic!("closed monitor must not poll"))
                .is_err()
        );
    }
}
