use super::*;
use crate::backend::{
    ExchangeLease, ExchangeOptions, ExchangeTiming, FeatureTransport, NativeIoOutcome,
    Protocol25Topology, SourceCommandScope,
};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

#[test]
fn software_quick_levels_preserve_fractional_wave_speed_and_source_duration_tables() {
    for (level, breath, spectrum, reactive, wave) in [
        (QuickEffectLevel::One, 3500, 18870, 500, 12.5),
        (QuickEffectLevel::Two, 7000, 37740, 1500, 25.0),
        (QuickEffectLevel::Three, 11200, 56610, 2000, 43.75),
    ] {
        assert_eq!(
            SoftwareEffect::Breathing {
                color: Some([1, 2, 3]),
                color2: None,
                random: false,
                duration: Some(level)
            }
            .engine_effect()
            .parameters
            .duration,
            Some(breath)
        );
        assert_eq!(
            SoftwareEffect::Spectrum {
                duration: Some(level)
            }
            .engine_effect()
            .parameters
            .duration,
            Some(spectrum)
        );
        let e = SoftwareEffect::Reactive {
            color: [1, 2, 3],
            duration: level,
        }
        .engine_effect();
        assert_eq!(e.parameters.duration, Some(reactive));
        assert_eq!(e.parameters.mode, Some(4128));
        assert_eq!(
            SoftwareEffect::Wave {
                direction: WaveDirection::Left,
                speed: Some(level)
            }
            .engine_effect()
            .parameters
            .speed,
            Some(wave)
        );
    }
    assert!(QuickEffectLevel::try_from(0).is_err());
    assert!(QuickEffectLevel::try_from(4).is_err());
}

#[test]
fn breathing_none_swap_two_colors_random_and_colorref_follow_complete_source_conversion() {
    let make = |color, color2, random| {
        SoftwareEffect::Breathing {
            color,
            color2,
            random,
            duration: None,
        }
        .engine_effect()
    };
    let swapped = make(None, Some([0x12, 0x34, 0x56]), false);
    let single = make(Some([0x12, 0x34, 0x56]), None, false);
    assert_eq!(swapped, single);
    assert_eq!(
        single.parameters.color_stops.as_ref().unwrap()[2],
        ColorStop {
            stop: 50,
            color: 0x563412
        }
    );
    let both = make(Some([1, 2, 3]), Some([4, 5, 6]), false);
    assert_eq!(both.parameters.duration, Some(14000));
    assert_eq!(
        both.parameters.color_stops.unwrap()[6],
        ColorStop {
            stop: 75,
            color: 0x060504
        }
    );
    let none = make(None, None, false);
    assert_eq!(
        none.parameters.color_stops.unwrap()[2],
        ColorStop {
            stop: 50,
            color: 0x6400_0000
        }
    );
    let random = make(None, Some([1, 2, 3]), true);
    assert_eq!(random.parameters.mode, Some(1));
    assert_eq!(random.parameters.color_stops, Some(Vec::new()));
    assert_eq!(random.parameters.duration, Some(7000));
}

#[test]
fn all_current_kitsune_editions_have_exact_perimeter_and_no_invented_led_input_mapping() {
    for edition in [0, 128, 129, 130, 131, 132, 133] {
        let config = led_configuration(edition).unwrap();
        assert_eq!(config.led_matrix.len(), 32);
        let mut indices = config
            .led_matrix
            .iter()
            .map(|m| m.device_pos[1])
            .collect::<Vec<_>>();
        indices.sort();
        assert_eq!(indices, (0..32).collect::<Vec<_>>());
        assert_eq!(
            config
                .led_matrix
                .iter()
                .find(|m| m.matrix_pos == [1, 0])
                .unwrap()
                .device_pos,
            [0, 31]
        );
        assert_eq!(
            config
                .led_matrix
                .iter()
                .find(|m| m.matrix_pos == [7, 9])
                .unwrap()
                .device_pos,
            [0, 16]
        );
        let json = serde_json::to_value(config).unwrap();
        assert_eq!(json["EID"], edition);
        assert_eq!(json["LedInputMap"], serde_json::json!([]));
    }
    assert!(led_configuration(1).is_err());
    assert!(led_configuration(134).is_err());
}

#[test]
fn software_frames_are_chunked_in_device_order_and_do_not_fabricate_acknowledgements() {
    let frame = EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 1,
        num_col: 32,
        frame: (0..96).collect(),
    };
    let mut transaction = 30;
    let reports = frame_reports(&frame, 7, &mut transaction).unwrap();
    assert_eq!(transaction, 1);
    assert_eq!(reports.len(), 2);
    assert_eq!(&reports[0][6..14], &[80, 15, 3, 0, 0, 0, 0, 24]);
    assert_eq!(&reports[0][14..89], &frame.frame[..75]);
    assert_eq!(&reports[1][6..14], &[26, 15, 3, 0, 0, 0, 25, 31]);
    assert_eq!(&reports[1][14..35], &frame.frame[75..]);
    for report in reports {
        assert_eq!(report[89], report[3..89].iter().fold(0, |xor, b| xor ^ b));
    }
    assert!(frame_reports(&frame, 8, &mut transaction).is_err());
    let mut invalid = frame;
    invalid.frame.pop();
    assert!(frame_reports(&invalid, 7, &mut transaction).is_err());
    assert_eq!(SoftwareStatus::starting(1).acknowledged_writes, 0);
}

#[derive(Default)]
struct FrameIo {
    attempts: RefCell<Vec<Vec<u8>>>,
    reject_first_chunk: bool,
    returns: RefCell<VecDeque<Result<usize, NativeIoOutcome>>>,
}
impl FeatureTransport for FrameIo {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("The current owner sends through the collection Feature channel")
    }
    fn send_feature_at_collection_max(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Source count checks must receive the actual native return")
    }
    fn send_feature_at_collection_max_counted(&self, bytes: &[u8]) -> anyhow::Result<usize> {
        self.attempts.borrow_mut().push(bytes.to_vec());
        if let Some(returned) = self.returns.borrow_mut().pop_front() {
            return returned
                .map_err(|outcome| crate::backend::native_io_failure(outcome, FrameCause.into()));
        }
        if self.reject_first_chunk && bytes[12] == 0 {
            anyhow::bail!("Simulated first chunk failure");
        }
        Ok(bytes.len())
    }
    fn send_feature_batch(
        &self,
        _: &crate::backend::FeatureBatch,
    ) -> anyhow::Result<crate::backend::FeatureBatchReceipt> {
        panic!("The controller override sends ordinary commands without a native batch")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Native frames do not request a device acknowledgement")
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({ "simulated": true })
    }
}

#[derive(Debug)]
struct FrameCause;

impl std::fmt::Display for FrameCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Simulated frame owner ended")
    }
}

impl std::error::Error for FrameCause {}

struct FrameOptions<'a> {
    inner: &'a dyn FeatureTransport,
    observed: RefCell<Vec<ExchangeOptions>>,
}

impl FeatureTransport for FrameOptions<'_> {
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        self.observed.borrow_mut().push(options.clone());
        self.inner.begin_exchange(options)
    }

    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Frame bypassed command admission")
    }

    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Frame must not invent an acknowledgement")
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated": true})
    }
}

fn rendered_frame() -> EngineFrame {
    EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 1,
        num_col: 32,
        frame: (0..96).collect(),
    }
}

#[test]
fn software_effect_uses_the_engine_instance_five_ms_clock_and_clamped_ten_in_retries() {
    use crate::feature_exchange::tests::{ExchangeProbe, LeaseEvent};

    #[derive(Default)]
    struct EffectIo {
        reports: RefCell<Vec<Vec<u8>>>,
        reads: Cell<usize>,
    }
    impl FeatureTransport for EffectIo {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("Software effect OUT requires its native byte count")
        }
        fn send_feature_at_collection_max_counted(&self, bytes: &[u8]) -> anyhow::Result<usize> {
            self.reports.borrow_mut().push(bytes.to_vec());
            Ok(bytes.len())
        }
        fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
            self.reads.set(self.reads.get() + 1);
            let reports = self.reports.borrow();
            bytes.copy_from_slice(reports.last().unwrap());
            bytes[0] = 0xff;
            bytes[1] = if self.reads.get() <= 10 { 1 } else { 2 };
            bytes[6] = 0;
            Ok(91)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::json!({"simulated": true})
        }
    }

    for (restore, expected) in [
        (false, [6, 15, 2, 0, 0, 8, 0, 0, 0]),
        (true, [6, 15, 2, 1, 0, 3, 0, 0, 0]),
    ] {
        let io = EffectIo::default();
        let probe = ExchangeProbe::new(&io);
        let waits = RefCell::new(Vec::new());
        let mut wait = |duration: std::time::Duration| {
            if !duration.is_zero() {
                waits.borrow_mut().push(duration);
            }
            Ok(())
        };
        let mut hid = SoftwareHid::new(&probe);
        if restore {
            hid.restore_on_exit(&mut wait).unwrap();
        } else {
            hid.select_custom(&mut wait).unwrap();
        }
        assert_eq!(io.reads.get(), 11);
        let reports = io.reports.borrow();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0], reports[1]);
        assert_eq!(&reports[0][6..15], &expected);
        assert_eq!(reports[0][0], 4);
        assert_eq!(reports[0][2], 0);
        assert_eq!(hid.transaction, 1);
        assert_eq!(
            probe.timings(),
            [ExchangeTiming::MutexOut {
                interval: std::time::Duration::from_millis(5)
            }]
        );
        assert_eq!(
            probe
                .events()
                .iter()
                .filter(|event| **event == LeaseEvent::RetryRelease)
                .count(),
            1
        );
        assert_eq!(probe.events().last(), Some(&LeaseEvent::Finish));
        assert_eq!(*waits.borrow(), [std::time::Duration::from_millis(5); 11]);
    }
}

#[test]
fn failed_frame_chunk_retries_twenty_times_continues_and_retains_partial_submission() {
    let io = FrameIo {
        reject_first_chunk: true,
        ..Default::default()
    };
    let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
    let waits = RefCell::new(Vec::new());
    let sent = SoftwareHid::new(&probe)
        .send_frame(&rendered_frame(), 7, &mut |duration| {
            waits.borrow_mut().push(duration);
            Ok(())
        })
        .unwrap();
    assert_eq!(io.attempts.borrow().len(), 21);
    assert_eq!(
        probe.timings(),
        [ExchangeTiming::MutexOut {
            interval: std::time::Duration::from_millis(5)
        }; 2]
    );
    assert_eq!(
        waits
            .borrow()
            .iter()
            .copied()
            .filter(|duration| !duration.is_zero())
            .collect::<Vec<_>>(),
        [std::time::Duration::from_millis(1)]
    );
    assert_eq!(sent.reports_submitted, 1);
    assert!(!sent.complete);
    assert!(sent.interruption.is_none());
    assert_eq!(sent.errors, ["Simulated first chunk failure"]);
    assert_eq!(io.attempts.borrow().last().unwrap()[12], 25);
}

#[test]
fn frame_release_failure_preserves_transferred_reports_and_returns_an_error() {
    let io = FrameIo::default();
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&io);
    let error = SoftwareHid::new(&probe)
        .send_frame(&rendered_frame(), 7, &mut |_| Ok(()))
        .unwrap_err();
    let progress = error.downcast_ref::<FrameSubmissionFailure>().unwrap();
    assert_eq!(progress.reports_submitted, 1);
    assert!(progress.errors.is_empty());
    assert!(progress.interruption.is_none());
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(io.attempts.borrow().len(), 1);
}

#[test]
fn interrupted_frame_preserves_accepted_reports_and_never_sends_the_remaining_chunk() {
    for (interrupt_at, accepted, complete) in [(1, 1, false), (2, 2, true)] {
        let io = FrameIo::default();
        let waits = Cell::new(0);
        let sent = SoftwareHid::new(&io)
            .send_frame(&rendered_frame(), 7, &mut |duration| {
                if duration.is_zero() {
                    return Ok(());
                }
                waits.set(waits.get() + 1);
                if waits.get() == interrupt_at {
                    anyhow::bail!("Simulated cancellation or lost identity");
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(io.attempts.borrow().len(), usize::from(accepted));
        assert_eq!(sent.reports_submitted, accepted);
        assert_eq!(sent.complete, complete);
        assert!(sent.errors.is_empty());
        assert_eq!(
            sent.interruption.unwrap().to_string(),
            "Simulated cancellation or lost identity"
        );
    }
}

#[test]
fn each_source_frame_chunk_owns_one_command_lock_and_releases_before_its_wait() {
    use crate::feature_exchange::tests::{ExchangeProbe, LeaseEvent};

    let io = FrameIo::default();
    let probe = ExchangeProbe::new(&io);
    let transport = FrameOptions {
        inner: &probe,
        observed: RefCell::new(Vec::new()),
    };
    let waits = Cell::new(0);
    let submitted = SoftwareHid::new(&transport)
        .send_frame(&rendered_frame(), 7, &mut |duration| {
            if duration.is_zero() {
                return Ok(());
            }
            assert_eq!(duration, std::time::Duration::from_millis(1));
            waits.set(waits.get() + 1);
            assert_eq!(
                probe
                    .events()
                    .iter()
                    .filter(|event| **event == LeaseEvent::Finish)
                    .count(),
                waits.get()
            );
            Ok(())
        })
        .unwrap();
    let timing = ExchangeTiming::MutexOut {
        interval: std::time::Duration::from_millis(5),
    };
    assert_eq!(
        probe.events(),
        [
            LeaseEvent::Begin(timing),
            LeaseEvent::Send,
            LeaseEvent::Finish,
            LeaseEvent::Begin(timing),
            LeaseEvent::Send,
            LeaseEvent::Finish,
        ]
    );
    assert_eq!(waits.get(), 2);
    assert_eq!(submitted.reports_submitted, 2);
    assert!(submitted.complete);
    for options in transport.observed.borrow().iter() {
        assert_eq!(options.timing, timing);
        assert_eq!(
            options.source_scope,
            Some(SourceCommandScope::Protocol25 {
                topology: Protocol25Topology::Normal
            })
        );
    }
}

#[test]
fn normal_incomplete_frame_returns_retry_the_same_report_without_an_ack_or_extra_wait() {
    for count in [0, 1, 90, 92] {
        let io = FrameIo {
            returns: RefCell::new([Ok(count), Ok(91), Ok(91)].into()),
            ..Default::default()
        };
        let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
        let waits = RefCell::new(Vec::new());
        let submitted = SoftwareHid::new(&probe)
            .send_frame(&rendered_frame(), 7, &mut |duration| {
                if !duration.is_zero() {
                    waits.borrow_mut().push(duration);
                }
                Ok(())
            })
            .unwrap();
        assert!(submitted.complete);
        assert_eq!(submitted.reports_submitted, 2);
        assert!(submitted.errors.is_empty());
        assert_eq!(*waits.borrow(), [std::time::Duration::from_millis(1); 2]);
        let reports = io.attempts.borrow();
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0], reports[1]);
        assert_eq!(reports[2][12], 25);
    }
}

#[test]
fn short_frame_exhaustion_keeps_the_second_chunk_and_skips_the_failed_chunks_wait() {
    let io = FrameIo {
        returns: RefCell::new(std::iter::repeat_n(Ok(90), 20).collect()),
        ..Default::default()
    };
    let waits = RefCell::new(Vec::new());
    let submitted = SoftwareHid::new(&io)
        .send_frame(&rendered_frame(), 7, &mut |duration| {
            if !duration.is_zero() {
                waits.borrow_mut().push(duration);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(submitted.reports_submitted, 1);
    assert!(!submitted.complete);
    assert_eq!(submitted.errors.len(), 1);
    assert!(submitted.errors[0].contains("returned 90 bytes"));
    assert_eq!(io.attempts.borrow().len(), 21);
    assert_eq!(*waits.borrow(), [std::time::Duration::from_millis(1)]);
}

#[test]
fn a_terminal_frame_owner_error_preserves_actual_submission_without_retry_or_next_chunk() {
    for (outcome, submitted) in [
        (NativeIoOutcome::NotCalled, 0),
        (NativeIoOutcome::Returned { byte_count: 90 }, 0),
        (NativeIoOutcome::Returned { byte_count: 91 }, 1),
    ] {
        let io = FrameIo {
            returns: RefCell::new([Err(outcome)].into()),
            ..Default::default()
        };
        let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
        let error = SoftwareHid::new(&probe)
            .send_frame(&rendered_frame(), 7, &mut |_| Ok(()))
            .unwrap_err();
        assert_eq!(crate::backend::native_io_outcome(&error), Some(outcome));
        assert!(error.is::<FrameCause>());
        let progress = error.downcast_ref::<FrameSubmissionFailure>().unwrap();
        assert_eq!(progress.reports_submitted, submitted);
        assert_eq!(io.attempts.borrow().len(), 1);
        assert_eq!(probe.events().len(), 3);
    }
}

#[test]
fn cancellation_immediately_after_a_return_keeps_its_count_and_stops_later_io() {
    let io = FrameIo::default();
    let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
    let error = SoftwareHid::new(&probe)
        .send_frame(&rendered_frame(), 7, &mut |_| {
            if io.attempts.borrow().is_empty() {
                Ok(())
            } else {
                Err(FrameCause.into())
            }
        })
        .unwrap_err();
    assert!(error.is::<FrameCause>());
    assert_eq!(
        crate::backend::native_io_outcome(&error),
        Some(NativeIoOutcome::Returned { byte_count: 91 })
    );
    assert_eq!(
        error
            .downcast_ref::<FrameSubmissionFailure>()
            .unwrap()
            .reports_submitted,
        1
    );
    assert_eq!(io.attempts.borrow().len(), 1);
    assert_eq!(probe.events().len(), 3);
}

#[test]
fn an_unsent_chunk_does_not_consume_the_next_source_transaction_id() {
    let io = FrameIo::default();
    let mut hid = SoftwareHid::new(&io);
    let interrupted = hid
        .send_frame(&rendered_frame(), 7, &mut |duration| {
            if duration.is_zero() {
                Ok(())
            } else {
                Err(FrameCause.into())
            }
        })
        .unwrap();
    assert_eq!(interrupted.reports_submitted, 1);
    assert!(interrupted.interruption.unwrap().is::<FrameCause>());
    let resumed = hid
        .send_frame(&rendered_frame(), 7, &mut |_| Ok(()))
        .unwrap();
    assert!(resumed.complete);
    assert_eq!(
        io.attempts
            .borrow()
            .iter()
            .map(|report| report[2])
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}
