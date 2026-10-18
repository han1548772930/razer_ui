use super::*;
use crate::backend::FeatureTransport;
use std::cell::{Cell, RefCell};

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
}
impl FeatureTransport for FrameIo {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("The current owner sends through the collection Feature channel")
    }
    fn send_feature_at_collection_max(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.attempts.borrow_mut().push(bytes.to_vec());
        if self.reject_first_chunk && bytes[12] == 0 {
            anyhow::bail!("Simulated first chunk failure");
        }
        Ok(())
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Native frames do not request a device acknowledgement")
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({ "simulated": true })
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
fn failed_frame_chunk_retries_twenty_times_continues_and_retains_partial_submission() {
    let io = FrameIo {
        reject_first_chunk: true,
        ..Default::default()
    };
    let sent = SoftwareHid::new(&io)
        .send_frame(&rendered_frame(), 7, &mut |_| Ok(()))
        .unwrap();
    assert_eq!(io.attempts.borrow().len(), 21);
    assert_eq!(sent.reports_submitted, 1);
    assert!(!sent.complete);
    assert!(sent.interruption.is_none());
    assert_eq!(sent.errors, ["Simulated first chunk failure"]);
    assert_eq!(io.attempts.borrow().last().unwrap()[12], 25);
}

#[test]
fn interrupted_frame_preserves_accepted_reports_and_never_sends_the_remaining_chunk() {
    for (interrupt_at, accepted, complete) in [(2, 1, false), (4, 2, true)] {
        let io = FrameIo::default();
        let waits = Cell::new(0);
        let sent = SoftwareHid::new(&io)
            .send_frame(&rendered_frame(), 7, &mut |_| {
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
