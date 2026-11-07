use super::*;
use std::{cell::Cell, collections::VecDeque};

// Current Protocol30 source owners are referenced by
// keyboard_lighting_physical_brightness_transport in keyboard-main-audit.json.
// These expected frames are literal source packet layouts, not encoder output.
struct Script {
    replies: VecDeque<anyhow::Result<Vec<u8>>>,
    sent: Vec<Vec<u8>>,
}
impl Script {
    fn new(replies: &[&[u8]]) -> Self {
        Self {
            replies: replies.iter().map(|bytes| Ok(bytes.to_vec())).collect(),
            sent: vec![],
        }
    }
}
impl Transport for Script {
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        selector: u8,
        data: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement> {
        validate()?;
        let transaction = self.sent.len() as u8;
        self.sent
            .push(frame(transaction, command, profile, selector, data)?);
        Acknowledgement::new(
            transaction,
            command,
            profile,
            selector,
            self.replies
                .pop_front()
                .expect("scripted source response")?,
        )
    }
}

#[test]
fn keyboard_ble_brightness_reads_actual_masked_region_and_percent() {
    let mut script = Script::new(&[&[0xc2, 3], &[127]]);
    let report = execute(600, 626, &mut script, None, &|| Ok(())).unwrap();
    assert_eq!(
        script.sent,
        [
            vec![0, 0, 0, 0, 16, 128, 0, 1],
            vec![1, 0, 0, 0, 16, 133, 1, 2]
        ]
    );
    let reading = report.reading().unwrap();
    assert_eq!(
        (reading.region_id, reading.raw_value, reading.percent),
        (2, 127, 50)
    );
    assert_eq!(report.confirmed_enabled(), None);
}

#[test]
fn keyboard_ble_backlight_registration_bypasses_all_region_discovery() {
    for (product, physical) in [(585, 584), (631, 632)] {
        let mut script = Script::new(&[&[127]]);
        let report = execute(product, physical, &mut script, None, &|| Ok(())).unwrap();
        assert_eq!(script.sent, [vec![0, 0, 0, 0, 16, 133, 1, 5]]);
        assert_eq!(report.reading().unwrap().region_id, 5);
        let mut script = Script::new(&[&[], &[127]]);
        let submission = BrightnessSubmission::new(50, true, BrightnessIntent::Value).unwrap();
        let report = execute(product, physical, &mut script, Some(submission), &|| Ok(())).unwrap();
        assert_eq!(
            script.sent,
            [
                vec![0, 1, 0, 0, 16, 5, 1, 5, 127],
                vec![1, 0, 0, 0, 16, 133, 1, 5]
            ]
        );
        report.check_request(product, Some(submission)).unwrap();
    }
}

#[test]
fn keyboard_ble_brightness_write_confirms_readback_instead_of_echo() {
    let mut script = Script::new(&[&[], &[1], &[127]]);
    let setting = BrightnessSubmission::new(50, true, BrightnessIntent::Value).unwrap();
    let report = execute(600, 626, &mut script, Some(setting), &|| Ok(())).unwrap();
    assert_eq!(
        script.sent,
        [
            vec![0, 1, 0, 0, 16, 5, 1, 0, 127],
            vec![1, 0, 0, 0, 16, 128, 0, 1],
            vec![2, 0, 0, 0, 16, 133, 1, 1]
        ]
    );
    assert_eq!(report.reading().unwrap().percent, 50);
    assert_eq!(report.confirmed_enabled(), None);
}

#[test]
fn keyboard_ble_oled_toggle_keeps_the_level_and_confirms_only_explicit_switch() {
    let mut script = Script::new(&[&[], &[1], &[178]]);
    let setting = BrightnessSubmission::new(70, false, BrightnessIntent::Enabled).unwrap();
    let report = execute(691, 693, &mut script, Some(setting), &|| Ok(())).unwrap();
    assert_eq!(
        script.sent,
        [
            vec![0, 2, 0, 0, 23, 17, 1, 0, 0, 0],
            vec![1, 0, 0, 0, 16, 128, 0, 1],
            vec![2, 0, 0, 0, 16, 133, 1, 1]
        ]
    );
    assert_eq!(report.reading().unwrap().percent, 70);
    assert_eq!(report.confirmed_enabled(), Some(false));
}

#[test]
fn keyboard_ble_oled_level_sets_switch_before_value_and_zero_is_not_a_switch_guess() {
    let mut script = Script::new(&[&[], &[], &[1], &[0]]);
    let setting = BrightnessSubmission::new(0, true, BrightnessIntent::Value).unwrap();
    let report = execute(691, 693, &mut script, Some(setting), &|| Ok(())).unwrap();
    assert_eq!(script.sent[0], [0, 2, 0, 0, 23, 17, 1, 0, 0, 1]);
    assert_eq!(script.sent[1], [1, 1, 0, 0, 16, 5, 1, 0, 0]);
    assert_eq!(report.confirmed_enabled(), Some(true));
    assert_eq!(report.reading().unwrap().percent, 0);
}

#[test]
fn keyboard_ble_brightness_rejects_wrong_identity_before_io() {
    let mut script = Script::new(&[]);
    assert!(execute(600, 625, &mut script, None, &|| Ok(())).is_err());
    assert!(script.sent.is_empty());
    for (product, physical) in [
        (585, 584),
        (631, 632),
        (658, 657),
        (717, 718),
        (600, 626),
        (602, 603),
        (664, 663),
        (697, 699),
        (716, 714),
        (724, 723),
        (727, 726),
        (691, 693),
        (708, 707),
        (756, 758),
    ] {
        assert!(available(product, physical));
    }
}

#[test]
fn keyboard_ble_brightness_empty_regions_remain_unobserved() {
    let mut script = Script::new(&[&[]]);
    let error = execute(600, 626, &mut script, None, &|| Ok(())).unwrap_err();
    assert!(
        error
            .downcast_ref::<Failure>()
            .unwrap()
            .report()
            .reading()
            .is_none()
    );
    assert_eq!(script.sent.len(), 1);
}

#[test]
fn keyboard_ble_rejected_region_query_runs_real_source_fallback_getter() {
    let mut script = Script::new(&[&[25]]);
    script
        .replies
        .push_front(Err(anyhow::anyhow!("region command rejected")));
    let report = execute(600, 626, &mut script, None, &|| Ok(())).unwrap();
    assert_eq!(
        script.sent,
        [
            vec![0, 0, 0, 0, 16, 128, 0, 1],
            vec![1, 0, 0, 0, 16, 133, 1, 1]
        ]
    );
    assert_eq!(report.reading().unwrap().percent, 10);
    assert!(
        report
            .region_query_failure
            .as_ref()
            .unwrap()
            .contains("rejected")
    );
    report.check_request(600, None).unwrap();
}

#[test]
fn keyboard_ble_scope_failure_after_region_ack_never_enters_fallback() {
    let mut script = Script::new(&[&[1]]);
    let calls = Cell::new(0);
    let validate = || {
        calls.set(calls.get() + 1);
        anyhow::ensure!(calls.get() != 3, "one-shot scope failure");
        Ok(())
    };
    assert!(execute(600, 626, &mut script, None, &validate).is_err());
    assert_eq!(script.sent.len(), 1);
}

#[test]
fn keyboard_ble_brightness_mismatch_retains_real_readback_as_failure_diagnostics() {
    let mut script = Script::new(&[&[], &[1], &[25]]);
    let setting = BrightnessSubmission::new(50, true, BrightnessIntent::Value).unwrap();
    let error = execute(600, 626, &mut script, Some(setting), &|| Ok(())).unwrap_err();
    let failure = error.downcast_ref::<Failure>().unwrap();
    assert_eq!(failure.report().reading().unwrap().percent, 10);
    assert_eq!(failure.report().acknowledgements.len(), 3);
}

#[test]
fn keyboard_ble_brightness_cancel_after_write_does_not_start_readback() {
    let calls = Cell::new(0);
    let validate = || {
        calls.set(calls.get() + 1);
        anyhow::ensure!(calls.get() < 3, "source scope changed");
        Ok(())
    };
    let mut script = Script::new(&[&[]]);
    let setting = BrightnessSubmission::new(50, true, BrightnessIntent::Value).unwrap();
    let error = execute(600, 626, &mut script, Some(setting), &validate).unwrap_err();
    assert_eq!(script.sent, [vec![0, 1, 0, 0, 16, 5, 1, 0, 127]]);
    let failure = error.downcast_ref::<Failure>().unwrap();
    assert_eq!(failure.report().acknowledgements.len(), 1);
    assert!(failure.report().reading().is_none());
}

#[test]
fn keyboard_ble_brightness_validates_frames_and_deserialized_range() {
    assert!(frame(31, WRITE, 1, 0, &[1]).is_err());
    assert!(frame(0, WRITE, 1, 0, &[]).is_err());
    let invalid: BrightnessSubmission =
        serde_json::from_str(r#"{"value":101,"enabled":true,"intent":"Value"}"#).unwrap();
    let mut script = Script::new(&[]);
    assert!(execute(600, 626, &mut script, Some(invalid), &|| Ok(())).is_err());
    assert!(script.sent.is_empty());
}
