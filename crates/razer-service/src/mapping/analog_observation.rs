//! Analog subscriptions use the same acknowledged lifecycle as Snap Tap.
use super::{
    Client,
    observation::{self, Stream},
};
use razer_ipc::{
    ServiceRequest,
    analog_observation::{Batch, Delivery, Registration, Target},
};

pub type Reply = observation::Reply<Delivery>;
pub struct Session(observation::Session<Delivery>);
struct AnalogStream {
    target: Target,
    registration: Registration,
}
impl Stream for AnalogStream {
    type Event = Delivery;
    fn name() -> &'static str {
        "mapping-analog-observer"
    }
    fn start(self, observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingAnalogObservationStart {
            target: self.target,
            observer_guid,
            registration: self.registration,
        }
    }
    fn poll(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingAnalogObservationPoll { observer_guid }
    }
    fn stop(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingAnalogObservationStop { observer_guid }
    }
    fn decode(value: serde_json::Value, observer: &str) -> anyhow::Result<Vec<Delivery>> {
        serde_json::from_value::<Batch>(value)?.into_deliveries(observer)
    }
}
impl Session {
    pub fn start(target: Target, registration: Registration) -> anyhow::Result<Self> {
        Ok(Self(observation::Session::start_stream(
            AnalogStream {
                target,
                registration,
            },
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
    fn analog_subscription_preserves_delivery_and_unregisters_on_stale_batch() {
        let mut delivered = false;
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingAnalogObservationStart {
                target,
                observer_guid,
                registration,
            } => {
                assert_eq!(target.product_id(), 756);
                assert_eq!(registration, Registration::Interval { milliseconds: 25 });
                Ok(json!({"observer_guid":observer_guid,"registered":true}))
            }
            ServiceRequest::MappingAnalogObservationPoll { observer_guid } if !delivered => {
                delivered = true;
                Ok(
                    json!({"observer_guid":observer_guid,"deliveries":[{"events":[],"time_tick":17}]}),
                )
            }
            ServiceRequest::MappingAnalogObservationPoll { .. } => {
                Ok(json!({"observer_guid":"retired","deliveries":[]}))
            }
            ServiceRequest::MappingAnalogObservationStop { observer_guid } => {
                Ok(json!({"observer_guid":observer_guid,"registered":false}))
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("Unexpected analog request: {request:?}"),
        });
        let session = observation::Session::start_stream(
            AnalogStream {
                target: Target::new(756, "container".into()),
                registration: Registration::Interval { milliseconds: 25 },
            },
            || Ok(mock),
        )
        .unwrap();
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert!(matches!(session.receive_for_test(), Reply::Ready));
        assert!(
            matches!(session.receive_for_test(), Reply::Event(delivery) if delivery.time_tick() == 17)
        );
        assert!(
            matches!(session.receive_for_test(), Reply::Failed(message) if message.contains("identity changed"))
        );
        assert!(session.stop().unwrap_err().contains("identity changed"));
        let trace = trace.lock().unwrap();
        assert!(matches!(
            &trace[trace.len() - 2],
            ServiceRequest::MappingAnalogObservationStop { .. }
        ));
        assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
    }
}
