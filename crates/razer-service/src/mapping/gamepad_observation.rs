//! Gamepad subscriptions use the same acknowledged lifecycle as Snap Tap.
use super::{
    Client,
    observation::{self, Stream},
};
use razer_ipc::{
    ServiceRequest,
    gamepad_observation::{Batch, Delivery, Registration, Target},
};

pub type Reply = observation::Reply<Delivery>;
pub struct Session(observation::Session<Delivery>);
struct GamepadStream {
    target: Target,
    registration: Registration,
}
impl Stream for GamepadStream {
    type Event = Delivery;
    fn name() -> &'static str {
        "mapping-gamepad-observer"
    }
    fn start(self, observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingGamepadObservationStart {
            target: self.target,
            observer_guid,
            registration: self.registration,
        }
    }
    fn poll(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingGamepadObservationPoll { observer_guid }
    }
    fn stop(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingGamepadObservationStop { observer_guid }
    }
    fn decode(value: serde_json::Value, observer: &str) -> anyhow::Result<Vec<Delivery>> {
        serde_json::from_value::<Batch>(value)?.into_deliveries(observer)
    }
}
impl Session {
    pub fn start(target: Target, registration: Registration) -> anyhow::Result<Self> {
        Ok(Self(observation::Session::start_stream(
            GamepadStream {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::test_transport::{Mock, TIMEOUT};
    use serde_json::json;
    #[test]
    fn stop_revokes_inflight_samples_and_reports_cleanup_failure() {
        for fail_cleanup in [false, true] {
            let (entered, waiting) = std::sync::mpsc::sync_channel(1);
            let (release, resume) = std::sync::mpsc::sync_channel(1);
            let (mock, trace, cleanup) = Mock::new(move |request| match request {
                ServiceRequest::MappingGamepadObservationStart { observer_guid, .. } => {
                    Ok(json!({"observer_guid":observer_guid,"registered":true}))
                }
                ServiceRequest::MappingGamepadObservationPoll { observer_guid } => {
                    entered.send(()).unwrap();
                    resume.recv_timeout(TIMEOUT).unwrap();
                    Ok(serde_json::to_value(Batch::new(
                        observer_guid.clone(),
                        vec![Delivery::new(
                            vec![razer_device::mapping_native_output::GamepadOutputState {
                                buttons: 16,
                                ..Default::default()
                            }],
                            42,
                        )],
                    ))
                    .unwrap())
                }
                ServiceRequest::MappingGamepadObservationStop { observer_guid } => {
                    Ok(json!({"observer_guid":observer_guid,"registered":fail_cleanup}))
                }
                ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
                request => panic!("Unexpected gamepad request: {request:?}"),
            });
            let session = observation::Session::start_stream(
                GamepadStream {
                    target: Target::new(679, "container".into()),
                    registration: Registration::Events { skip: 50 },
                },
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
            if fail_cleanup {
                assert!(
                    stopped
                        .unwrap_err()
                        .contains("cleanup was not acknowledged")
                );
            } else {
                stopped.unwrap();
            }
            let trace = trace.lock().unwrap();
            assert_eq!(
                trace
                    .iter()
                    .filter(|request| matches!(
                        request,
                        ServiceRequest::MappingGamepadObservationStop { .. }
                    ))
                    .count(),
                1
            );
            assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
        }
    }
    #[test]
    fn gamepad_subscription_preserves_delivery_and_unregisters_on_stale_batch() {
        let mut delivered = false;
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingGamepadObservationStart {
                target,
                observer_guid,
                registration,
            } => {
                assert_eq!(target.product_id(), 679);
                assert_eq!(registration, Registration::Interval { milliseconds: 25 });
                Ok(json!({"observer_guid":observer_guid,"registered":true}))
            }
            ServiceRequest::MappingGamepadObservationPoll { observer_guid } if !delivered => {
                delivered = true;
                Ok(
                    json!({"observer_guid":observer_guid,"deliveries":[{"events":[{"buttons":4112,"left_trigger":23,"right_trigger":201,"left_thumb":[-32768,32767],"right_thumb":[-1234,4567]}],"time_tick":17}]}),
                )
            }
            ServiceRequest::MappingGamepadObservationPoll { .. } => {
                Ok(json!({"observer_guid":"retired","deliveries":[]}))
            }
            ServiceRequest::MappingGamepadObservationStop { observer_guid } => {
                Ok(json!({"observer_guid":observer_guid,"registered":false}))
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("Unexpected gamepad request: {request:?}"),
        });
        let session = observation::Session::start_stream(
            GamepadStream {
                target: Target::new(679, "container".into()),
                registration: Registration::Interval { milliseconds: 25 },
            },
            || Ok(mock),
        )
        .unwrap();
        cleanup.recv_timeout(TIMEOUT).unwrap();
        let events = session.drain();
        assert!(matches!(events.first(), Some(Reply::Ready)));
        assert!(
            matches!(&events[1], Reply::Event(delivery) if delivery.time_tick() == 17 && delivery.events() == [razer_device::mapping_native_output::GamepadOutputState { buttons:4112,left_trigger:23,right_trigger:201,left_thumb:[-32768,32767],right_thumb:[-1234,4567] }])
        );
        assert!(
            matches!(&events[2], Reply::Failed(message) if message.contains("identity changed"))
        );
        assert!(session.stop().unwrap_err().contains("identity changed"));
        let trace = trace.lock().unwrap();
        assert!(matches!(
            &trace[trace.len() - 2],
            ServiceRequest::MappingGamepadObservationStop { .. }
        ));
        assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
    }
}
