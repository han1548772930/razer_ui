use super::*;
use std::collections::VecDeque;

fn raw(value: f64) -> RawAnalogInput {
    RawAnalogInput {
        lx: 0.,
        ly: 0.,
        rx: 0.,
        ry: 0.,
        lt: value,
        rt: value,
    }
}
struct Transport {
    samples: VecDeque<f64>,
    writes: Vec<(TriggerPart, TriggerCalibrationRange)>,
    readback: Option<TriggerCalibrationRange>,
}
impl TriggerCalibrationTransport for Transport {
    fn raw_analog_input(&mut self) -> anyhow::Result<RawAnalogInput> {
        self.samples
            .pop_front()
            .map(raw)
            .context("Fixture exhausted")
    }
    fn set_trigger_calibration(
        &mut self,
        part: TriggerPart,
        range: TriggerCalibrationRange,
    ) -> anyhow::Result<()> {
        self.writes.push((part, range));
        Ok(())
    }
    fn trigger_calibration(
        &mut self,
        _part: TriggerPart,
    ) -> anyhow::Result<TriggerCalibrationRange> {
        self.readback.context("Missing actual fixture readback")
    }
}
#[test]
fn native_masks_are_distinct_from_ui_parts_and_product_limits_differ() {
    assert_eq!(TriggerPart::from_ui_part(3).unwrap().analog_mask(), 4);
    assert_eq!(TriggerPart::from_ui_part(4).unwrap().analog_mask(), 8);
    assert!(TriggerPart::from_ui_part(8).is_none());
    let range = TriggerCalibrationRange {
        min: 450,
        max: 1450,
    };
    assert!(
        !TriggerCalibrationSession::new(2676, TriggerPart::Left)
            .unwrap()
            .validate_range(range)
    );
    assert!(
        !TriggerCalibrationSession::new(2684, TriggerPart::Left)
            .unwrap()
            .validate_range(range)
    );
    let range = TriggerCalibrationRange {
        min: 390,
        max: 1450,
    };
    assert!(
        !TriggerCalibrationSession::new(2676, TriggerPart::Left)
            .unwrap()
            .validate_range(range)
    );
    assert!(
        TriggerCalibrationSession::new(2684, TriggerPart::Left)
            .unwrap()
            .validate_range(range)
    );
}
#[test]
fn stability_requires_five_differences_then_invalid_input_cancels_hold() {
    let mut io = Transport {
        samples: [2000., 2000.].into(),
        writes: vec![],
        readback: None,
    };
    let mut s = TriggerCalibrationSession::new(2676, TriggerPart::Left).unwrap();
    s.start(&mut io).unwrap();
    for _ in 0..5 {
        assert_eq!(s.poll(Ok(raw(300.))).unwrap(), HoldChange::Unchanged);
    }
    assert_eq!(
        s.poll(Ok(raw(300.))).unwrap(),
        HoldChange::Start(Duration::from_secs(2))
    );
    assert_eq!(s.poll(Ok(raw(500.))).unwrap(), HoldChange::Cancel);
    assert_eq!(s.step(), TriggerStep::Press);
    assert!(io.writes.is_empty());
}
#[test]
fn sampling_waits_and_rounds_margins_then_requires_exact_readback() {
    for readback in [
        TriggerCalibrationRange {
            min: 198,
            max: 2041,
        },
        TriggerCalibrationRange {
            min: 198,
            max: 2040,
        },
    ] {
        let mut io = Transport {
            samples: [
                2000., 2000., 300., 300., 300., 300., 300., 2000., 2000., 2000., 2000., 2000.,
                2000.,
            ]
            .into(),
            writes: vec![],
            readback: Some(readback),
        };
        let mut s = TriggerCalibrationSession::new(2676, TriggerPart::Left).unwrap();
        s.start(&mut io).unwrap();
        for _ in 0..6 {
            s.poll(Ok(raw(300.))).unwrap();
        }
        let mut waits = vec![];
        s.hold_complete(&mut io, |d| {
            waits.push(d);
            Ok(())
        })
        .unwrap();
        assert_eq!(s.step(), TriggerStep::Release);
        assert!(!s.valid());
        assert_eq!(waits.len(), 4);
        for _ in 0..6 {
            s.poll(Ok(raw(2000.))).unwrap();
        }
        let result = s.hold_complete(&mut io, |d| {
            waits.push(d);
            Ok(())
        });
        assert_eq!(waits.len(), 8);
        assert!(waits.iter().all(|d| *d == Duration::from_millis(10)));
        assert_eq!(
            io.writes,
            vec![(
                TriggerPart::Left,
                TriggerCalibrationRange {
                    min: 198,
                    max: 2041
                }
            )]
        );
        assert_eq!(
            s.step(),
            if result.is_ok() {
                TriggerStep::Complete
            } else {
                TriggerStep::Error
            }
        );
        assert_eq!(result.is_ok(), readback.max == 2041);
        assert!(!s.active);
        assert_eq!(s.step, TriggerStep::None);
        assert_eq!(s.progress().valid(), result.is_ok());
        assert_eq!(s.stable_polls, 0);
        assert!(s.sample_min.is_none() && s.sample_max.is_none());
        assert_eq!(s.poll(Ok(raw(300.))).unwrap(), HoldChange::Unchanged);
        // A new source start resets the internal native operation but does not
        // preserve the previous terminal progress in place of Step10.
        io.samples.extend([2000., 2000.]);
        s.start(&mut io).unwrap();
        assert_eq!(s.step(), TriggerStep::Press);
        assert!(!s.progress().valid());
    }
}
#[test]
fn invalid_samples_never_write_or_claim_calibration_success() {
    let mut io = Transport {
        samples: [2000., 2000., f64::NAN].into(),
        writes: vec![],
        readback: None,
    };
    let mut s = TriggerCalibrationSession::new(2684, TriggerPart::Right).unwrap();
    s.start(&mut io).unwrap();
    for _ in 0..6 {
        s.poll(Ok(raw(300.))).unwrap();
    }
    assert!(s.hold_complete(&mut io, |_| Ok(())).is_err());
    assert_eq!(s.step(), TriggerStep::Error);
    assert!(io.writes.is_empty());
}

struct ScriptedReads {
    readings: VecDeque<anyhow::Result<RawAnalogInput>>,
    calibration: Transport,
}
impl TriggerCalibrationTransport for ScriptedReads {
    fn raw_analog_input(&mut self) -> anyhow::Result<RawAnalogInput> {
        self.readings.pop_front().context("Fixture exhausted")?
    }

    fn set_trigger_calibration(
        &mut self,
        part: TriggerPart,
        range: TriggerCalibrationRange,
    ) -> anyhow::Result<()> {
        self.calibration.set_trigger_calibration(part, range)
    }

    fn trigger_calibration(
        &mut self,
        part: TriggerPart,
    ) -> anyhow::Result<TriggerCalibrationRange> {
        self.calibration.trigger_calibration(part)
    }
}

#[test]
fn start_uses_only_its_source_range_baseline_and_prepare_retains_reset_movement() {
    // Current module 36840 start/prepareNextStep: missing/invalid first reads
    // choose Range.max; missing/invalid second reads keep resetTriggerMovement.
    for pid in [2676, 2684] {
        for part in [TriggerPart::Left, TriggerPart::Right] {
            for resting in [None, Some(f64::NAN), Some(-1.), Some(8193.), Some(2000.)] {
                for initial in [None, Some(f64::NAN), Some(-1.), Some(8193.)] {
                    let mut io = ScriptedReads {
                        readings: [resting, initial]
                            .into_iter()
                            .map(|value| value.map(raw).context("Raw query unavailable"))
                            .collect(),
                        calibration: Transport {
                            samples: VecDeque::new(),
                            writes: vec![],
                            readback: None,
                        },
                    };
                    let mut session = TriggerCalibrationSession::new(pid, part).unwrap();
                    assert_eq!(session.movement_percentage(), None);
                    session.start(&mut io).unwrap();
                    assert_eq!(session.step(), TriggerStep::Press);
                    assert!(!session.valid());
                    assert_eq!(session.movement_percentage(), Some(100.));
                    let movement = session.movement.as_ref().unwrap();
                    assert!(matches!(movement.value, TriggerMovementValue::ResetResting));
                    assert_eq!(
                        movement.resting(8192.),
                        if resting == Some(2000.) { 2000. } else { 8192. }
                    );
                    assert!(
                        matches!(movement.resting, RestingTriggerValue::Observed(_))
                            == (resting == Some(2000.))
                    );
                    assert!(session.hold_complete(&mut io, |_| Ok(())).is_err());
                    assert!(io.calibration.writes.is_empty());
                    assert!(io.readings.is_empty());
                }
            }
        }
    }
}

#[test]
fn unavailable_resting_query_does_not_replace_a_distinct_valid_initial_movement() {
    let mut io = ScriptedReads {
        readings: [Err(anyhow::anyhow!("Raw query failed")), Ok(raw(1700.))].into(),
        calibration: Transport {
            samples: VecDeque::new(),
            writes: vec![],
            readback: None,
        },
    };
    let mut session = TriggerCalibrationSession::new(2676, TriggerPart::Right).unwrap();
    session.start(&mut io).unwrap();
    assert_eq!(session.movement_percentage(), Some(17.));
    assert!(!session.valid());
    assert!(matches!(
        session.movement.as_ref().unwrap().resting,
        RestingTriggerValue::SourceRangeMaximum
    ));
    assert!(matches!(
        session.movement.as_ref().unwrap().value,
        TriggerMovementValue::Observed(1700.)
    ));
}

#[test]
fn unavailable_poll_keeps_source_movement_and_stability_without_starting_a_new_hold() {
    for pid in [2676, 2684] {
        let mut io = Transport {
            samples: [2000., 2000.].into(),
            writes: vec![],
            readback: None,
        };
        let mut session = TriggerCalibrationSession::new(pid, TriggerPart::Left).unwrap();
        session.start(&mut io).unwrap();
        for _ in 0..6 {
            assert_eq!(
                session
                    .poll(Err(anyhow::anyhow!("Raw query failed")))
                    .unwrap(),
                HoldChange::Unchanged
            );
        }
        assert!(!session.valid());
        assert!(session.previous.is_none());
        assert_eq!(session.stable_polls, 0);
        for _ in 0..6 {
            session.poll(Ok(raw(300.))).unwrap();
        }
        assert!(session.valid());
        assert_eq!(session.movement_percentage(), Some(0.));
        for read in [
            Err(anyhow::anyhow!("Raw query failed")),
            Ok(raw(f64::NAN)),
            Ok(raw(-1.)),
            Ok(raw(8193.)),
        ] {
            assert_eq!(session.poll(read).unwrap(), HoldChange::Unchanged);
            assert_eq!(session.step(), TriggerStep::Press);
            assert!(session.valid());
            assert_eq!(session.previous, Some(300.));
            assert_eq!(session.stable_polls, 5);
            assert_eq!(session.movement_percentage(), Some(0.));
        }
        assert_eq!(session.poll(Ok(raw(500.))).unwrap(), HoldChange::Cancel);
        assert!(!session.valid());
        assert!(io.writes.is_empty());
    }
}

#[test]
fn missing_release_prepare_read_retains_reset_baseline_but_requires_fresh_stability_and_samples() {
    let mut io = Transport {
        samples: [2000., 2000., 300., 300., 300., 300., 300.].into(),
        writes: vec![],
        readback: Some(TriggerCalibrationRange {
            min: 198,
            max: 2041,
        }),
    };
    let mut session = TriggerCalibrationSession::new(2676, TriggerPart::Left).unwrap();
    session.start(&mut io).unwrap();
    for _ in 0..6 {
        session.poll(Ok(raw(300.))).unwrap();
    }
    session.hold_complete(&mut io, |_| Ok(())).unwrap();
    assert_eq!(session.step(), TriggerStep::Release);
    assert_eq!(session.movement_percentage(), Some(100.));
    assert!(matches!(
        session.movement.as_ref().unwrap().value,
        TriggerMovementValue::ResetResting
    ));
    for _ in 0..6 {
        assert_eq!(
            session
                .poll(Err(anyhow::anyhow!("Raw query failed")))
                .unwrap(),
            HoldChange::Unchanged
        );
    }
    assert!(!session.valid());
    assert_eq!(session.stable_polls, 0);
    assert!(session.hold_complete(&mut io, |_| Ok(())).is_err());
    assert!(io.writes.is_empty());
    for _ in 0..5 {
        assert_eq!(session.poll(Ok(raw(2000.))).unwrap(), HoldChange::Unchanged);
    }
    assert_eq!(
        session.poll(Ok(raw(2000.))).unwrap(),
        HoldChange::Start(Duration::from_secs(3))
    );
    io.samples.extend([2000.; 5]);
    session.hold_complete(&mut io, |_| Ok(())).unwrap();
    assert_eq!(session.step(), TriggerStep::Complete);
    assert_eq!(
        io.writes,
        vec![(
            TriggerPart::Left,
            TriggerCalibrationRange {
                min: 198,
                max: 2041
            }
        )]
    );
}

#[test]
fn cancellation_during_sampling_clears_partial_samples_and_restart_uses_only_new_reads() {
    let mut io = Transport {
        samples: [2000., 2000., 300.].into(),
        writes: vec![],
        readback: None,
    };
    let mut session = TriggerCalibrationSession::new(2684, TriggerPart::Right).unwrap();
    session.start(&mut io).unwrap();
    for _ in 0..6 {
        session.poll(Ok(raw(300.))).unwrap();
    }
    let error = session
        .hold_complete(&mut io, |_| {
            Err(anyhow::anyhow!("Canceled by current owner"))
        })
        .unwrap_err();
    assert!(error.to_string().contains("Canceled by current owner"));
    assert_eq!(session.step(), TriggerStep::Error);
    assert_eq!(session.movement_percentage(), None);
    assert!(session.sample_min.is_none() && session.sample_max.is_none());
    assert!(io.writes.is_empty());
    io.samples.extend([1800., 1700.]);
    session.start(&mut io).unwrap();
    assert_eq!(session.step(), TriggerStep::Press);
    assert_eq!(session.movement_percentage(), Some(94.));
    assert!(!session.valid());
    assert_eq!(session.stable_polls, 0);
    session.stop();
    assert_eq!(session.poll(Ok(raw(300.))).unwrap(), HoldChange::Unchanged);
    assert_eq!(session.movement_percentage(), None);
    assert!(session.hold_complete(&mut io, |_| Ok(())).is_err());
    assert!(io.writes.is_empty());
}
