//! Observe input notifications on the existing device; no redirect or suppression.
use super::{Client, Transport, observation};
use razer_ipc::{
    ServiceRequest,
    input_observation::{Batch, InputEvent, Target},
};
pub struct Session(observation::Session<InputEvent>);
pub type Reply = observation::Reply<InputEvent>;

struct InputStream(Target);
impl observation::Stream for InputStream {
    type Event = InputEvent;
    fn name() -> &'static str {
        "mapping-input-observer"
    }
    fn start(self, observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingInputObservationStart {
            target: self.0,
            observer_guid,
        }
    }
    fn poll(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingInputObservationPoll { observer_guid }
    }
    fn stop(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingInputObservationStop { observer_guid }
    }
    fn decode(value: serde_json::Value, observer: &str) -> anyhow::Result<Vec<InputEvent>> {
        serde_json::from_value::<Batch>(value)?.into_events(observer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::test_transport::{Mock, TIMEOUT};
    use razer_device::mapping_native_lookup::InputKind;
    use serde_json::json;
    use std::sync::mpsc;

    #[test]
    fn input_observation_delivers_native_input_without_redirect_or_suppression() {
        let mut delivered = false;
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingInputObservationStart {
                target,
                observer_guid,
            } => {
                assert_eq!(target.product_id(), 679);
                assert_eq!(target.container_id(), "input-container");
                Ok(json!({"observer_guid":observer_guid,"registered":true}))
            }
            ServiceRequest::MappingInputObservationPoll { observer_guid } if !delivered => {
                delivered = true;
                Ok(
                    json!({"observer_guid":observer_guid,"events":[{"input":{"type":"keyboard","scancode":56,"flag":3,"modifiers":9},"timeTick":55}]}),
                )
            }
            ServiceRequest::MappingInputObservationPoll { .. } => {
                Ok(json!({"observer_guid":"retired","events":[]}))
            }
            ServiceRequest::MappingInputObservationStop { observer_guid } => {
                Ok(json!({"observer_guid":observer_guid,"registered":false}))
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => {
                panic!("Input observation must not redirect or suppress mappings: {request:?}")
            }
        });
        let session =
            Session::start_input_with(Target::new(679, "input-container".into()), || Ok(mock))
                .unwrap();
        cleanup.recv_timeout(TIMEOUT).unwrap();
        // Transport release precedes the worker's terminal reply publication.
        assert!(matches!(session.0.receive_for_test(), Reply::Ready));
        assert!(
            matches!(session.0.receive_for_test(), Reply::Event(event) if event.time_tick() == 55 && event.input().kind == InputKind::Keyboard { scancode:56,flag:3 })
        );
        assert!(
            matches!(session.0.receive_for_test(), Reply::Failed(error) if error.contains("identity changed"))
        );
        assert!(session.drain().is_empty());
        assert!(session.stop().unwrap_err().contains("identity changed"));
        let trace = trace.lock().unwrap();
        assert!(matches!(
            trace.as_slice(),
            [
                ServiceRequest::MappingInputObservationStart { .. },
                ServiceRequest::MappingInputObservationPoll { .. },
                ServiceRequest::MappingInputObservationPoll { .. },
                ServiceRequest::MappingInputObservationStop { .. },
                ServiceRequest::Shutdown
            ]
        ));
    }

    #[test]
    fn input_observation_cancellation_discards_inflight_batch_before_scope_release() {
        let (entered, waiting) = mpsc::channel();
        let (resume, blocked) = mpsc::channel();
        let (mock, _, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingInputObservationStart { observer_guid, .. } => {
                Ok(json!({"observer_guid":observer_guid,"registered":true}))
            }
            ServiceRequest::MappingInputObservationPoll { observer_guid } => {
                entered.send(()).unwrap();
                blocked.recv_timeout(TIMEOUT).unwrap();
                Ok(
                    json!({"observer_guid":observer_guid,"events":[{"input":{"type":"analogKey","scancode":30,"flag":0},"timeTick":9}]}),
                )
            }
            ServiceRequest::MappingInputObservationStop { observer_guid } => {
                Ok(json!({"observer_guid":observer_guid,"registered":false}))
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("Unexpected input observer request: {request:?}"),
        });
        let session =
            Session::start_input_with(Target::new(679, "input-container".into()), || Ok(mock))
                .unwrap();
        waiting.recv_timeout(TIMEOUT).unwrap();
        session.request_stop();
        resume.send(()).unwrap();
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert!(matches!(session.drain().as_slice(), [Reply::Ready]));
        session.stop().unwrap();
    }
}
impl Session {
    pub fn start(target: Target) -> anyhow::Result<Self> {
        Self::start_input_with(target, Client::spawn)
    }
    fn start_input_with<T: Transport + 'static>(
        target: Target,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<Self> {
        Ok(Self(observation::Session::start_stream(
            InputStream(target),
            factory,
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
