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
    s.start(&mut io);
    for _ in 0..5 {
        assert_eq!(s.poll(Some(raw(300.))), HoldChange::Unchanged);
    }
    assert_eq!(
        s.poll(Some(raw(300.))),
        HoldChange::Start(Duration::from_secs(2))
    );
    assert_eq!(s.poll(Some(raw(500.))), HoldChange::Cancel);
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
        s.start(&mut io);
        for _ in 0..6 {
            s.poll(Some(raw(300.)));
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
            s.poll(Some(raw(2000.)));
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
        assert_eq!(s.poll(Some(raw(300.))), HoldChange::Unchanged);
        // A new source start resets the internal native operation but does not
        // preserve the previous terminal progress in place of Step10.
        io.samples.extend([2000., 2000.]);
        s.start(&mut io);
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
    s.start(&mut io);
    for _ in 0..6 {
        s.poll(Some(raw(300.)));
    }
    assert!(s.hold_complete(&mut io, |_| Ok(())).is_err());
    assert_eq!(s.step(), TriggerStep::Error);
    assert!(io.writes.is_empty());
}
