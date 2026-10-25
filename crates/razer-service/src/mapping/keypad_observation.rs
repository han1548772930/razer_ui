//! PID747 raw controller subscriptions with acknowledged cancellation and join.
use super::{
    Client,
    observation::{self, Stream},
};
use razer_ipc::{
    ServiceRequest,
    keypad_observation::{Batch, Delivery, Target},
};

pub type Reply = observation::Reply<Delivery>;
pub struct Session(observation::Session<Delivery>);
struct KeypadStream(Target);
impl Stream for KeypadStream {
    type Event = Delivery;
    fn name() -> &'static str {
        "mapping-keypad-observer"
    }
    fn start(self, observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingKeypadObservationStart {
            target: self.0,
            observer_guid,
        }
    }
    fn poll(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingKeypadObservationPoll { observer_guid }
    }
    fn stop(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingKeypadObservationStop { observer_guid }
    }
    fn decode(value: serde_json::Value, observer: &str) -> anyhow::Result<Vec<Delivery>> {
        serde_json::from_value::<Batch>(value)?.into_deliveries(observer)
    }
}
impl Session {
    pub fn start(target: Target) -> anyhow::Result<Self> {
        Ok(Self(observation::Session::start_stream(
            KeypadStream(target),
            Client::spawn,
        )?))
    }
    pub fn drain(&self) -> Vec<Reply> {
        self.0.drain()
    }
    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
    pub fn request_stop(&self) {
        self.0.request_stop();
    }
    pub fn stop(self) -> Result<(), String> {
        self.0.stop()
    }
    pub fn stop_with_receipt(self) -> observation::Completion {
        self.0.stop_with_receipt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::test_transport::{Mock, TIMEOUT};
    use serde_json::json;

    #[test]
    fn keypad_subscription_preserves_normalized_hardware_values_and_rejects_stale_batch() {
        let mut delivered = false;
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingKeypadObservationStart {
                target,
                observer_guid,
            } => {
                assert_eq!(target.product_id(), 747);
                Ok(json!({"observer_guid":observer_guid,"registered":true}))
            }
            ServiceRequest::MappingKeypadObservationPoll { observer_guid } if !delivered => {
                delivered = true;
                Ok(
                    json!({"observer_guid":observer_guid,"deliveries":[{"sample":{
                    "left_thumb_x":-1000,"left_thumb_y":1000,"right_thumb_x":-333,
                    "right_thumb_y":500,"left_trigger":0,"right_trigger":1000},"time_tick":17}]}),
                )
            }
            ServiceRequest::MappingKeypadObservationPoll { .. } => {
                Ok(json!({"observer_guid":"retired","deliveries":[]}))
            }
            ServiceRequest::MappingKeypadObservationStop { observer_guid } => {
                Ok(json!({"observer_guid":observer_guid,"registered":false}))
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("Unexpected keypad request: {request:?}"),
        });
        let session = observation::Session::start_stream(
            KeypadStream(Target::new(747, "container".into())),
            || Ok(mock),
        )
        .unwrap();
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert!(matches!(session.receive_for_test(), Reply::Ready));
        assert!(matches!(session.receive_for_test(), Reply::Event(delivery)
            if delivery.sample().right_thumb_x() == -333 && delivery.sample().right_trigger() == 1000 && delivery.time_tick() == 17));
        assert!(
            matches!(session.receive_for_test(), Reply::Failed(error) if error.contains("identity changed"))
        );
        assert!(session.stop().unwrap_err().contains("identity changed"));
        let trace = trace.lock().unwrap();
        assert!(matches!(
            &trace[trace.len() - 2],
            ServiceRequest::MappingKeypadObservationStop { .. }
        ));
        assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
    }

    #[test]
    fn keypad_stop_suppresses_inflight_event_and_reports_unacknowledged_cleanup() {
        use std::sync::mpsc;
        for cleanup_failure in [false, true] {
            let (entered, waiting) = mpsc::sync_channel(1);
            let (release, resumed) = mpsc::sync_channel(1);
            let (mock, trace, cleanup) = Mock::new(move |request| match request {
                ServiceRequest::MappingKeypadObservationStart { observer_guid, .. } => {
                    Ok(json!({"observer_guid":observer_guid,"registered":true}))
                }
                ServiceRequest::MappingKeypadObservationPoll { observer_guid } => {
                    entered.send(()).unwrap();
                    resumed.recv_timeout(TIMEOUT).unwrap();
                    Ok(
                        json!({"observer_guid":observer_guid,"deliveries":[{"sample":{
                        "left_thumb_x":1,"left_thumb_y":2,"right_thumb_x":3,"right_thumb_y":4,
                        "left_trigger":5,"right_trigger":6},"time_tick":17}]}),
                    )
                }
                ServiceRequest::MappingKeypadObservationStop { observer_guid } => {
                    Ok(json!({"observer_guid":observer_guid,"registered":cleanup_failure}))
                }
                ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
                request => panic!("Unexpected keypad request: {request:?}"),
            });
            let session = observation::Session::start_stream(
                KeypadStream(Target::new(747, "container".into())),
                || Ok(mock),
            )
            .unwrap();
            waiting.recv_timeout(TIMEOUT).unwrap();
            session.request_stop();
            release.send(()).unwrap();
            cleanup.recv_timeout(TIMEOUT).unwrap();
            assert!(
                session
                    .drain()
                    .iter()
                    .all(|reply| !matches!(reply, Reply::Event(_)))
            );
            let stopped = session.stop();
            if cleanup_failure {
                assert!(
                    stopped
                        .unwrap_err()
                        .contains("cleanup was not acknowledged")
                );
            } else {
                stopped.unwrap();
            }
            assert_eq!(
                trace
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|request| matches!(
                        request,
                        ServiceRequest::MappingKeypadObservationStop { .. }
                    ))
                    .count(),
                1
            );
        }
    }
}
