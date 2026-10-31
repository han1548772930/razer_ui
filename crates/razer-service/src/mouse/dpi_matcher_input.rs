//! Input-monitor receipts own the partial-start state independently of the
//! calibration state machine. A lost start acknowledgement still needs stop.
use razer_device::dpi_matcher::InputBatch;
use razer_ipc::{
    ServiceRequest,
    mouse_input_monitoring::{InputEvent, Session},
};
use serde_json::Value;
use std::collections::VecDeque;

pub(super) struct InputMonitor {
    session: Session,
    pending: VecDeque<InputEvent>,
}

impl InputMonitor {
    pub(super) fn new(owner: String) -> Self {
        Self {
            session: Session::new(owner).expect("retained Matcher owner is nonempty"),
            pending: VecDeque::new(),
        }
    }

    pub(super) fn possibly_started(&self) -> bool {
        self.session.possibly_started()
    }

    pub(super) fn is_capturing(&self) -> bool {
        self.session.is_capturing()
    }

    pub(super) fn start(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<()> {
        if self.session.is_capturing() {
            self.session.repeat_start(request).map(|_| ())
        } else {
            self.session.start(request)
        }
    }

    pub(super) fn stop(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<()> {
        self.pending.clear();
        self.session.stop(request)
    }

    pub(super) fn stop_with(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
        mut observe: impl FnMut(InputBatch) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let pending = &mut self.pending;
        let stopped = self.session.stop_with(request, |events| {
            observe_pending(pending, &mut observe)?;
            for event in events {
                observe(decode(event)?)?;
            }
            Ok(())
        });
        let queued = observe_pending(pending, &mut observe);
        super::merge_results([stopped, queued])
    }

    pub(super) fn poll(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<()> {
        self.pending.extend(self.session.poll(request)?);
        Ok(())
    }

    pub(super) fn next_event(&mut self) -> anyhow::Result<Option<InputBatch>> {
        self.pending.pop_front().map(decode).transpose()
    }

    pub(super) fn discard_events(
        &mut self,
        request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<()> {
        self.pending.clear();
        self.session.poll(request).map(|_| ())
    }
}

fn decode(event: InputEvent) -> anyhow::Result<InputBatch> {
    InputBatch::from_event(event.product_id(), event.container_id(), event.json_event())
}

fn observe_pending(
    pending: &mut VecDeque<InputEvent>,
    observe: &mut impl FnMut(InputBatch) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for event in std::mem::take(pending) {
        observe(decode(event)?)?;
    }
    Ok(())
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
        assert!(!monitor.is_capturing());
        assert!(
            monitor
                .start(|_| panic!("must not start over a partial owner"))
                .is_err()
        );
        assert!(
            monitor
                .stop(|_| Ok(json!({"owner_guid":"old-owner","monitoring":false,
                    "events":[],"more":false})))
                .is_err()
        );
        assert!(monitor.possibly_started());
        monitor.stop(|request| {
            assert!(matches!(request, ServiceRequest::MouseInputMonitoringStop { owner_guid } if owner_guid == "matcher-owner"));
            Ok(json!({"owner_guid":"matcher-owner","monitoring":false,"events":[],"more":false}))
        }).unwrap();
        assert!(!monitor.possibly_started());
        assert!(!monitor.is_capturing());
        monitor
            .stop(|_| panic!("acknowledged stop must not run again"))
            .unwrap();
    }

    #[test]
    fn dpi_matcher_monitor_rejects_stale_polls_and_incomplete_event_streams() {
        let mut monitor = InputMonitor::new("matcher-owner".into());
        monitor
            .start(|_| Ok(json!({"owner_guid":"matcher-owner","result":true,"reason":""})))
            .unwrap();
        for receipt in [
            json!({"owner_guid":"old-owner","events":[],"failure":null}),
            json!({"owner_guid":"matcher-owner","events":[],"failure":"callback overflow"}),
        ] {
            assert!(monitor.poll(|_| Ok(receipt.clone())).is_err());
            assert!(monitor.possibly_started());
        }
        monitor
            .stop(|_| {
                Ok(json!({"owner_guid":"matcher-owner","monitoring":false,
                "events":[],"more":false}))
            })
            .unwrap();
        assert!(
            monitor
                .poll(|_| panic!("closed monitor must not poll"))
                .is_err()
        );
    }

    #[test]
    fn dpi_matcher_detached_observer_drains_final_pages_without_restarting_or_polling() {
        let mut monitor = InputMonitor::new("matcher-owner".into());
        monitor
            .start(|_| Ok(json!({"owner_guid":"matcher-owner","result":true,"reason":""})))
            .unwrap();
        let mut stops = 0;
        monitor
            .stop(|request| {
                assert!(matches!(request,
                    ServiceRequest::MouseInputMonitoringStop { owner_guid } if owner_guid == "matcher-owner"));
                stops += 1;
                Ok(json!({"owner_guid":"matcher-owner","monitoring":false,
                    "events":[{"containerId":"actual","productId":190,"vendorId":5426,
                        "usbInstanceId":null,"inputMonitoringEventType":1,
                        "jsonEvent":r#"[{"timeTick":"88","data":"{\"type\":\"mouseMove\",\"x\":600,\"y\":900}"}]"#}],
                    "more":stops == 1}))
            })
            .unwrap();
        assert_eq!(stops, 2);
        assert!(!monitor.possibly_started());
    }

    #[test]
    fn dpi_matcher_confirmed_repeat_start_keeps_the_capture_and_its_stop_obligation() {
        let mut monitor = InputMonitor::new("matcher-owner".into());
        let mut starts = 0;
        for acknowledgement in [
            json!({"owner_guid":"matcher-owner","result":true,"reason":""}),
            json!({"owner_guid":"matcher-owner","result":false,"reason":"already started"}),
        ] {
            monitor.start(|request| {
                assert!(matches!(request, ServiceRequest::MouseInputMonitoringStart { owner_guid } if owner_guid == "matcher-owner"));
                starts += 1;
                Ok(acknowledgement.clone())
            }).unwrap();
            assert!(monitor.is_capturing() && monitor.possibly_started());
        }
        assert_eq!(starts, 2);
        assert!(
            monitor
                .start(|_| Ok(
                    json!({"owner_guid":"matcher-owner","result":false,"reason":"native failure"})
                ))
                .is_err()
        );
        assert!(monitor.is_capturing() && monitor.possibly_started());
        assert!(
            monitor
                .stop(|_| anyhow::bail!("lost STOP acknowledgement"))
                .is_err()
        );
        assert!(!monitor.is_capturing() && monitor.possibly_started());
        assert!(
            monitor
                .start(|_| panic!("unacknowledged STOP must prohibit START"))
                .is_err()
        );
        assert!(
            monitor
                .poll(|_| panic!("unacknowledged STOP must prohibit POLL"))
                .is_err()
        );
        monitor.stop(|_| Ok(json!({"owner_guid":"matcher-owner","monitoring":false,"events":[],"more":false}))).unwrap();
        assert!(!monitor.is_capturing() && !monitor.possibly_started());
    }

    #[test]
    fn dpi_matcher_detached_listener_validates_transport_without_parsing_movement() {
        let mut monitor = InputMonitor::new("matcher-owner".into());
        monitor
            .start(|_| Ok(json!({"owner_guid":"matcher-owner","result":true,"reason":""})))
            .unwrap();
        let event = json!({"containerId":"actual","productId":190,"vendorId":5426,
            "usbInstanceId":null,"inputMonitoringEventType":1,"jsonEvent":"invalid movement JSON"});
        let poll = json!({"owner_guid":"matcher-owner","events":[event.clone()],"failure":null});
        monitor.discard_events(|_| Ok(poll.clone())).unwrap();
        monitor.poll(|_| Ok(poll.clone())).unwrap();
        assert!(monitor.next_event().is_err());
        assert!(monitor.is_capturing());
        assert!(
            monitor
                .discard_events(|_| Ok(
                    json!({"owner_guid":"another-owner","events":[event.clone()],"failure":null})
                ))
                .is_err()
        );
        monitor.stop(|_| Ok(json!({"owner_guid":"matcher-owner","monitoring":false,"events":[event.clone()],"more":false}))).unwrap();
        assert!(!monitor.possibly_started());
    }

    #[test]
    fn dpi_matcher_attached_stop_delivers_all_callback_pages_and_drains_after_observer_failure() {
        for product_id in [
            190, 191, 192, 193, 222, 223, 226, 227, 228, 229, 230, 239, 240,
        ] {
            for observer_fails in [false, true] {
                let mut monitor = InputMonitor::new("matcher-owner".into());
                monitor
                    .start(|_| Ok(json!({"owner_guid":"matcher-owner","result":true,"reason":""})))
                    .unwrap();
                let mut stops = 0;
                let mut observed = 0;
                let result = monitor.stop_with(|request| {
                    assert!(matches!(request, ServiceRequest::MouseInputMonitoringStop { owner_guid } if owner_guid == "matcher-owner"));
                    stops += 1;
                    Ok(json!({"owner_guid":"matcher-owner","monitoring":false,
                        "events":[{"containerId":"actual","productId":product_id,"vendorId":5426,
                            "usbInstanceId":null,"inputMonitoringEventType":1,
                            "jsonEvent":r#"[{"timeTick":"88","data":"{\"type\":\"mouseMove\",\"x\":600,\"y\":900}"}]"#}],
                        "more":stops == 1}))
                }, |batch| {
                    observed += 1;
                    assert_eq!(batch.container_id, "actual");
                    assert_eq!(batch.product_id, product_id);
                    assert_eq!(batch.movement.len(), 1);
                    assert_eq!(batch.movement[0].time_tick, 88.);
                    assert_eq!(batch.movement[0].x, 600.);
                    anyhow::ensure!(!observer_fails, "observer failure");
                    Ok(())
                });
                assert_eq!(result.is_err(), observer_fails);
                assert_eq!(stops, 2);
                assert_eq!(observed, if observer_fails { 1 } else { 2 });
                assert!(!monitor.is_capturing() && !monitor.possibly_started());
                monitor
                    .stop(|_| panic!("all final pages were already acknowledged"))
                    .unwrap();
            }
        }
    }

    #[test]
    fn dpi_matcher_stop_keeps_callbacks_already_polled_ahead_of_its_final_page() {
        for has_final_page in [false, true] {
            for observer_fails in [false, true] {
                let mut monitor = InputMonitor::new("matcher-owner".into());
                monitor
                    .start(|_| Ok(json!({"owner_guid":"matcher-owner","result":true,"reason":""})))
                    .unwrap();
                let event = |container: &str| {
                    json!({"containerId":container,"productId":190,"vendorId":5426,
                    "usbInstanceId":null,"inputMonitoringEventType":1,
                    "jsonEvent":r#"[{"timeTick":"88","data":"{\"type\":\"mouseMove\",\"x\":600}"}]"#})
                };
                monitor.poll(|_| Ok(json!({"owner_guid":"matcher-owner","events":[event("first"),event("second")],"failure":null}))).unwrap();
                assert_eq!(monitor.next_event().unwrap().unwrap().container_id, "first");
                let requested_stop = std::cell::Cell::new(false);
                let mut observed = vec![];
                let result = monitor.stop_with(|request| {
                    assert!(matches!(request, ServiceRequest::MouseInputMonitoringStop { owner_guid } if owner_guid == "matcher-owner"));
                    requested_stop.set(true);
                    let events = if has_final_page { vec![event("final")] } else { vec![] };
                    Ok(json!({"owner_guid":"matcher-owner","monitoring":false,"events":events,"more":false}))
                }, |batch| {
                    assert!(requested_stop.get());
                    observed.push(batch.container_id);
                    anyhow::ensure!(!observer_fails, "queued observer failure");
                    Ok(())
                });
                assert_eq!(result.is_err(), observer_fails);
                assert_eq!(
                    observed,
                    if has_final_page && !observer_fails {
                        vec!["second", "final"]
                    } else {
                        vec!["second"]
                    }
                );
                assert!(monitor.next_event().unwrap().is_none());
                assert!(!monitor.is_capturing() && !monitor.possibly_started());
            }
        }
    }
}
