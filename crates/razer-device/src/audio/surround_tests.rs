use super::*;
use std::collections::VecDeque;

struct Script {
    calls: VecDeque<(NativeCall, bool)>,
}
impl Script {
    fn new(calls: Vec<(NativeCall, bool)>) -> Self {
        Self {
            calls: calls.into(),
        }
    }
}
impl Transport for Script {
    fn call(&mut self, call: &NativeCall) -> Result<bool> {
        let (expected, accepted) = self.calls.pop_front().expect("unexpected native call");
        assert_eq!(*call, expected);
        Ok(accepted)
    }
}
fn endpoint() -> Endpoint {
    Endpoint::new("endpoint-front".into(), "Front speakers".into()).unwrap()
}
fn select_front(runtime: &mut Runtime) {
    let mut transport = Script::new(vec![(
        NativeCall::SetFrontEndpoint("endpoint-front".into()),
        true,
    )]);
    assert!(
        runtime
            .execute(&Change::FrontSpeaker(endpoint()), &mut transport, || Ok(()))
            .is_completed()
    );
    assert!(transport.calls.is_empty());
}

#[test]
fn source_ranges_and_external_boundaries_reject_invalid_values() {
    assert!(Volume::new(101).is_err());
    assert!(Delay::new(-101).is_err());
    assert!(Delay::new(101).is_err());
    assert!(Equalizer::new([0, -7, 0]).is_err());
    assert!(serde_json::from_str::<Change>(r#"{"setting":"delay","value":101}"#).is_err());
    assert!(serde_json::from_str::<Endpoint>(r#"{"id":"","name":"Speaker"}"#).is_err());
    assert!(serde_json::from_str::<Product>("1352").is_err());
    assert_eq!(
        Product::try_from(1392).unwrap().library_id(),
        "ThxVADCarolNative"
    );
    assert_eq!(Delay::new(-75).unwrap().native_value(), -0.75);
    assert_eq!(Volume::new(37).unwrap().native_value(), 0.37);
    assert_eq!(Volume::from_native(0.374).unwrap().percent(), 37);
    assert!(Volume::from_native(f32::NAN).is_err());
    assert!(Volume::from_native(-1.0).is_err());
}

#[test]
fn enable_without_front_preserves_source_optimistic_runtime_without_native_success() {
    let mut runtime = Runtime::default();
    let mut transport = Script::new(vec![]);
    let outcome = runtime.execute(&Change::Enabled(true), &mut transport, || Ok(()));
    assert!(!outcome.is_completed());
    assert!(runtime.settings().is_enabled());
    assert!(outcome.calls().is_empty());
    assert_eq!(outcome.failure(), Some("Front speaker is not set"));
}

#[test]
fn mode_ack_and_followup_eq_bools_keep_the_source_failure_semantics() {
    let mut runtime = Runtime::default();
    select_front(&mut runtime);
    let mut transport = Script::new(vec![
        (NativeCall::SetEnabled(true), true),
        (
            NativeCall::SetEqualizer {
                channel: Channel::Rear,
                equalizer: Equalizer::FLAT,
            },
            false,
        ),
        (
            NativeCall::SetEqualizer {
                channel: Channel::Front,
                equalizer: Equalizer::FLAT,
            },
            true,
        ),
    ]);
    let outcome = runtime.execute(&Change::Enabled(true), &mut transport, || Ok(()));
    assert!(outcome.is_completed());
    assert!(!outcome.calls()[1].is_accepted());
    assert!(transport.calls.is_empty());
    let mut rejected = Script::new(vec![(NativeCall::SetEnabled(false), false)]);
    assert!(
        !runtime
            .execute(&Change::Enabled(false), &mut rejected, || Ok(()))
            .is_completed()
    );
    assert!(!runtime.settings().is_enabled());
}

#[test]
fn rear_volume_unmutes_first_and_stops_after_a_rejected_unmute() {
    let mut runtime = Runtime::default();
    let volume = Volume::new(65).unwrap();
    let change = Change::Volume {
        channel: Channel::Rear,
        volume,
    };
    let mut rejected = Script::new(vec![(NativeCall::SetRearMute(false), false)]);
    assert!(
        !runtime
            .execute(&change, &mut rejected, || Ok(()))
            .is_completed()
    );
    assert_eq!(runtime.volume(Channel::Rear), None);
    let mut accepted = Script::new(vec![
        (NativeCall::SetRearMute(false), true),
        (
            NativeCall::SetVolume {
                channel: Channel::Rear,
                volume,
            },
            true,
        ),
    ]);
    assert!(
        runtime
            .execute(&change, &mut accepted, || Ok(()))
            .is_completed()
    );
    assert_eq!(runtime.volume(Channel::Rear), Some(volume));
    let zero = Volume::new(0).unwrap();
    let mut silent = Script::new(vec![(
        NativeCall::SetVolume {
            channel: Channel::Rear,
            volume: zero,
        },
        true,
    )]);
    assert!(
        runtime
            .execute(
                &Change::Volume {
                    channel: Channel::Rear,
                    volume: zero
                },
                &mut silent,
                || Ok(())
            )
            .is_completed()
    );
    assert!(silent.calls.is_empty());
}

#[test]
fn stored_preferences_do_not_claim_volume_test_or_device_writeback() {
    let mut settings = Settings::default();
    assert!(!settings.is_front_stereo_replicated());
    settings.apply(&Change::Volume {
        channel: Channel::Rear,
        volume: Volume::new(80).unwrap(),
    });
    settings.apply(&Change::TestSignal(true));
    assert_eq!(settings, Settings::default());
    settings.apply(&Change::Delay(Delay::new(42).unwrap()));
    let mut runtime = Runtime::default();
    let mut rejected = Script::new(vec![(NativeCall::SetDelay(Delay::new(42).unwrap()), false)]);
    assert!(
        !runtime
            .execute(&Change::Delay(settings.delay()), &mut rejected, || Ok(()))
            .is_completed()
    );
    assert_eq!(settings.delay().milliseconds(), 42);
    assert_eq!(runtime.settings().delay(), Delay::ZERO);
}

#[test]
fn cancellation_and_late_identity_failure_preserve_real_responses() {
    let mut runtime = Runtime::default();
    let mut transport = Script::new(vec![]);
    let cancelled = runtime.execute(&Change::TestSignal(true), &mut transport, || {
        bail!("cancelled")
    });
    assert!(cancelled.calls().is_empty());
    assert_eq!(runtime.test_signal(), None);
    let mut checks = 0;
    let mut transport = Script::new(vec![(NativeCall::SetTestSignal(true), true)]);
    let late = runtime.execute(&Change::TestSignal(true), &mut transport, || {
        checks += 1;
        ensure!(checks == 1, "owner changed");
        Ok(())
    });
    assert!(!late.is_completed());
    assert!(late.calls()[0].is_accepted());
    assert_eq!(runtime.test_signal(), None);
    assert_eq!(runtime.test_signal_requested(), Some(true));
}

#[test]
fn replacing_front_queues_reenable_without_making_its_ack_the_endpoint_ack() {
    let mut runtime = Runtime::default();
    let mut no_calls = Script::new(vec![]);
    runtime.execute(&Change::Enabled(true), &mut no_calls, || Ok(()));
    let mut transport = Script::new(vec![(
        NativeCall::SetFrontEndpoint("endpoint-front".into()),
        true,
    )]);
    let endpoint_outcome =
        runtime.execute(&Change::FrontSpeaker(endpoint()), &mut transport, || Ok(()));
    assert!(endpoint_outcome.is_completed());
    assert_eq!(endpoint_outcome.follow_ups(), &[Change::Enabled(true)]);
    assert!(transport.calls.is_empty());
    let mut rejected = Script::new(vec![(NativeCall::SetEnabled(true), false)]);
    let follow_up = runtime.execute(&endpoint_outcome.follow_ups()[0], &mut rejected, || Ok(()));
    assert!(!follow_up.is_completed());
    assert!(endpoint_outcome.is_completed());
    assert_eq!(runtime.settings().front_speaker(), Some(&endpoint()));
}
