use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
struct Reply {
    status: u8,
    data: Vec<u8>,
}
struct Simulated {
    sent: Mutex<Vec<Vec<u8>>>,
    replies: Mutex<VecDeque<Reply>>,
    canceled: AtomicBool,
    cancel_after: Option<usize>,
}
impl Simulated {
    fn new(data: Vec<Vec<u8>>) -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            replies: Mutex::new(
                data.into_iter()
                    .map(|data| Reply { status: 2, data })
                    .collect(),
            ),
            canceled: AtomicBool::new(false),
            cancel_after: None,
        }
    }
    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().unwrap().clone()
    }
}
impl FeatureTransport for Simulated {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("Missing simulated EQ response")?;
        let sent = self.sent.lock().unwrap();
        let latest = sent.last().unwrap();
        report.fill(0);
        report[1] = reply.status;
        report[2] = latest[2];
        report[6] = reply.data.len() as u8;
        report[7..9].copy_from_slice(&latest[7..9]);
        report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
        if self.cancel_after == Some(sent.len()) {
            self.canceled.store(true, Ordering::Release);
        }
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn cap(pid: u32) -> DeviceReadCapability {
    let mut cap = capability(pid).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}
fn next() -> impl FnMut() -> anyhow::Result<u8> {
    let mut index = 0;
    move || {
        let current = index;
        index = (index + 1) % 31;
        Ok(current)
    }
}
fn band() -> Band {
    serde_json::from_value(serde_json::json!({"direction":0,"band":2,"bandStatus":1,"freq":1234,"gain":-3.7,"qFactor":0.7,"filterType":5})).unwrap()
}
fn preset() -> Preset {
    serde_json::from_value(serde_json::json!({"eqName":"custom","parametricEqSettings":[band()],"highPassFilter":{"isEnabled":true,"value":3},
    "vocalBass":{"isEnabled":true,"value":20},"vocalExciter":{"isEnabled":false,"value":80}})).unwrap()
}

#[test]
fn band_queries_actual_preset_before_writing_signed_gain_and_big_endian_frequency() {
    for pid in [1422, 1446] {
        let device = Simulated::new(vec![vec![0, 16], vec![0; 9]]);
        let report = execute(
            pid,
            &device,
            &cap(pid),
            &Change::Band(band()),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.confirmed());
        assert!(report.setter_sent());
        let sent = device.sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(&sent[0][6..11], &[2, 8, 130, 0, 0]);
        assert_eq!(&sent[1][6..18], &[9, 8, 26, 6, 0, 2, 1, 4, 210, 219, 7, 5]);
        assert_eq!(
            sent[1][89],
            sent[1][3..89].iter().fold(0, |sum, byte| sum ^ byte)
        );
    }
}
#[test]
fn unknown_actual_preset_stops_the_single_band_without_a_setter() {
    let device = Simulated::new(vec![vec![0, 11]]);
    let report = execute(
        1422,
        &device,
        &cap(1422),
        &Change::Band(band()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.confirmed());
    assert!(!report.source_result());
    assert!(!report.setter_sent());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn microphone_noise_compares_decoded_source_values_and_writes_signed_wire_fields() {
    let change=Change::from_source("ON_SET_NOISE_GATE",serde_json::json!({"isEnabled":true,"thresholdDB":-65,"atTime":4,"rlTime":400,"holdTime":40,"decayDB":-120})).unwrap();
    let observed = vec![7, 2, 191, 4, 1, 144, 40, 251, 80];
    let device = Simulated::new(vec![vec![1, 2, 1], vec![1, 2, 1], observed.clone()]);
    let report = execute(1422, &device, &cap(1422), &change, next(), || Ok(())).unwrap();
    assert!(report.source_result() && !report.setter_sent());
    // Profile bytes and non-zero mode are not fields compared by Marie_FW.
    assert_eq!(device.sent().len(), 3);
    let device = Simulated::new(vec![
        vec![1, 2, 0],
        vec![1, 2, 1],
        vec![1, 2, 1],
        vec![0; 9],
        vec![0; 9],
    ]);
    let report = execute(1422, &device, &cap(1422), &change, next(), || Ok(())).unwrap();
    assert!(report.confirmed() && report.setter_sent());
    let sent = device.sent();
    assert_eq!(&sent[1][6..12], &[3, 8, 22, 1, 2, 1]);
    assert_eq!(
        &sent[4][6..18],
        &[9, 8, 27, 1, 1, 191, 4, 1, 144, 40, 251, 80]
    );
}

#[test]
fn microphone_limiter_and_reverb_keep_current_source_comparison_quirks() {
    let limiter =
        Change::from_source("ON_SET_MIC_LIMITER", serde_json::json!({"isEnabled":true})).unwrap();
    let device = Simulated::new(vec![vec![1, 5, 1]]);
    let report = execute(1446, &device, &cap(1446), &limiter, next(), || Ok(())).unwrap();
    assert!(!report.source_result() && !report.setter_sent());
    let reverb = Change::from_source(
        "ON_SET_MIC_REVERB",
        serde_json::json!({"isEnabled":true,"roomMode":0,"intensity":50}),
    )
    .unwrap();
    let device = Simulated::new(vec![vec![1, 6, 1], vec![1, 0, 50], vec![1, 0, 50]]);
    let report = execute(1446, &device, &cap(1446), &reverb, next(), || Ok(())).unwrap();
    assert!(report.confirmed() && report.setter_sent());
    assert_eq!(&device.sent()[2][6..12], &[3, 8, 31, 1, 0, 50]);
    assert!(Change::from_source("ON_MIC_RETURN_SWITCH", serde_json::json!({})).is_err());
}
#[test]
fn preset_reset_then_selection_bands_vocal_effects_and_highpass_follow_source_order() {
    let device = Simulated::new(vec![
        vec![0; 11],
        vec![1, 16],
        vec![0; 9],
        vec![0; 9],
        vec![0; 9],
        vec![1, 3],
    ]);
    let report = execute(
        1422,
        &device,
        &cap(1422),
        &Change::Preset(preset()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.confirmed());
    let sent = device.sent();
    assert_eq!(sent.len(), 6);
    assert_eq!(
        sent.iter().map(|packet| packet[8]).collect::<Vec<_>>(),
        [33, 2, 26, 26, 26, 21]
    );
    assert_eq!(&sent[0][9..20], &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&sent[1][9..11], &[1, 16]);
    assert_eq!(&sent[3][9..18], &[6, 0, 8, 1, 1, 44, 226, 7, 3]);
    assert_eq!(&sent[4][9..18], &[6, 0, 9, 0, 23, 112, 30, 7, 4]);
    assert_eq!(&sent[5][9..11], &[1, 3]);
}
#[test]
fn unchecked_preset_stage_failure_keeps_source_order_but_never_confirms_all_device_writes() {
    let mut preset = preset();
    preset.vocal_bass = None;
    preset.vocal_exciter = None;
    let device = Simulated::new(vec![vec![0; 11], vec![1, 16], vec![], vec![1, 3]]);
    let report = execute(
        1422,
        &device,
        &cap(1422),
        &Change::Preset(preset),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.source_result());
    assert!(!report.confirmed());
    assert_eq!(device.sent().len(), 4);
    assert!(report.stages()[2].error().unwrap().contains("truncated"));
}
#[test]
fn failed_vocal_confirmation_stops_remaining_effects_and_highpass() {
    let device = Simulated::new(vec![vec![0; 11], vec![1, 16], vec![0; 9], vec![]]);
    let report = execute(
        1422,
        &device,
        &cap(1422),
        &Change::Preset(preset()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.source_result());
    assert!(!report.confirmed());
    assert_eq!(device.sent().len(), 4);
}
#[test]
fn disabling_highpass_and_deesser_writes_zero_without_inventing_a_getter() {
    for change in [
        Change::HighPass(Effect::new(false, 5)),
        Change::DeEsser(Effect::new(false, 100)),
    ] {
        let device = Simulated::new(vec![vec![1, 0]]);
        let report = execute(1446, &device, &cap(1446), &change, next(), || Ok(())).unwrap();
        assert!(report.confirmed());
        assert_eq!(device.sent().len(), 1);
        assert_eq!(&device.sent()[0][9..11], &[1, 0]);
    }
}
#[test]
fn canceled_device_response_stops_the_next_band_even_when_source_ignores_stage_results() {
    let mut device = Simulated::new(vec![vec![0; 11], vec![1, 16]]);
    device.cancel_after = Some(1);
    let error = execute(
        1422,
        &device,
        &cap(1422),
        &Change::Preset(preset()),
        next(),
        || {
            ensure!(
                !device.canceled.load(Ordering::Acquire),
                "Canceled simulated EQ task"
            );
            Ok(())
        },
    )
    .unwrap_err();
    let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
    assert!(progress.setter_sent());
    assert!(!progress.confirmed());
    assert!(!progress.source_result());
    assert_eq!(progress.stages().len(), 1);
    assert_eq!(progress.stages()[0].command(), [11, 8, 33]);
    assert_eq!(progress.stages()[0].response(), Some([0; 11].as_slice()));
    assert!(
        progress.stages()[0]
            .error()
            .unwrap()
            .contains("Canceled simulated EQ task")
    );
    assert_eq!(device.sent().len(), 1);
    assert_eq!(device.replies.lock().unwrap().len(), 1);
}

#[test]
fn seiren_parsed_setter_ack_survives_release_failure_without_the_next_preset_stage() {
    let acknowledged = vec![0x71; 11];
    let device = Simulated::new(vec![acknowledged.clone(), vec![1, 16]]);
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
    let error = execute(
        1422,
        &probe,
        &cap(1422),
        &Change::Preset(preset()),
        next(),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(
        error.root_cause().to_string(),
        "simulated explicit release failed"
    );
    let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
    assert!(progress.setter_sent());
    assert!(!progress.confirmed());
    assert!(!progress.source_result());
    assert_eq!(progress.stages().len(), 1);
    assert_eq!(
        progress.stages()[0].response(),
        Some(acknowledged.as_slice())
    );
    assert!(progress.stages()[0].error().is_some());
    assert_eq!(device.sent().len(), 1);
    assert_eq!(device.replies.lock().unwrap().len(), 1);
}

#[test]
fn seiren_owner_loss_after_release_keeps_the_ack_but_stops_the_next_stage() {
    let acknowledged = vec![0x72; 11];
    let device = Simulated::new(vec![acknowledged.clone(), vec![1, 16]]);
    let cancel = || device.canceled.store(true, Ordering::Release);
    let probe = crate::feature_exchange::tests::ExchangeProbe::with_on_finish(&device, &cancel);
    let error = execute(
        1446,
        &probe,
        &cap(1446),
        &Change::Preset(preset()),
        next(),
        || {
            ensure!(
                !device.canceled.load(Ordering::Acquire),
                "owner replaced after release"
            );
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(
        error.root_cause().to_string(),
        "owner replaced after release"
    );
    let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
    assert_eq!(progress.stages().len(), 1);
    assert_eq!(
        progress.stages()[0].response(),
        Some(acknowledged.as_slice())
    );
    assert!(progress.setter_sent());
    assert!(!progress.confirmed());
    assert_eq!(device.sent().len(), 1);
    assert_eq!(device.replies.lock().unwrap().len(), 1);
}
#[test]
fn invalid_ui_ranges_do_not_submit_a_packet() {
    let mut bad = band();
    bad.q_factor = 15.1;
    let device = Simulated::new(vec![]);
    assert!(
        execute(
            1422,
            &device,
            &cap(1422),
            &Change::Band(bad),
            next(),
            || Ok(())
        )
        .is_err()
    );
    assert!(device.sent().is_empty());
    assert_eq!(preset_ids("custom").unwrap(), (16, 6));
}

#[test]
fn actual_float_json_from_retained_ui_preserves_frequency_and_whole_effect_amount() {
    let change = Change::from_source(
        "ON_SET_PARAMETRIC_EQ_SETTINGS",
        serde_json::json!({
            "parametricEQ":{"direction":0,"band":2,"bandStatus":1,"freq":1234.0,
            "gain":-3.7,"qFactor":0.7,"filterType":5}
        }),
    )
    .unwrap();
    assert_eq!(change, Change::Band(band()));
    let mut fractional = band();
    fractional.freq = 1234.9;
    assert_eq!(&fractional.wire(6).unwrap()[4..6], &[4, 210]);
    assert_eq!(
        Change::from_source(
            "ON_SET_VOCAL_BASS",
            serde_json::json!({
                "vocalBass":{"isEnabled":true,"value":80.0}
            })
        )
        .unwrap(),
        Change::VocalBass(Effect::new(true, 80))
    );
    assert!(
        Change::from_source(
            "ON_SET_VOCAL_BASS",
            serde_json::json!({
                "isEnabled":true,"value":80.5
            })
        )
        .is_err()
    );
}

#[test]
fn mic_agc_coercion_and_command_follow_current_feature_without_initial_getter() {
    for enabled in [
        serde_json::json!(true),
        serde_json::json!(1),
        serde_json::json!("true"),
        serde_json::json!("1"),
    ] {
        let change =
            Change::from_source("ON_SET_MIC_AGC", serde_json::json!({"isEnabled":enabled}))
                .unwrap();
        assert_eq!(change, Change::MicAgc(true));
        let device = Simulated::new(vec![vec![1, 1, 1]]);
        let report = execute(1422, &device, &cap(1422), &change, next(), || Ok(())).unwrap();
        assert!(report.confirmed());
        assert_eq!(device.sent().len(), 1);
        assert_eq!(&device.sent()[0][6..12], &[3, 8, 22, 1, 1, 1]);
    }
    assert_eq!(
        Change::from_source("ON_SET_MIC_AGC", serde_json::json!({"isEnabled":"false"})).unwrap(),
        Change::MicAgc(false)
    );
}

#[test]
fn dial_commit_directly_sets_but_profile_restore_reads_and_skips_only_when_equal() {
    for pid in [1422, 1446] {
        let device = Simulated::new(vec![vec![1, 1]]);
        let report = execute(
            pid,
            &device,
            &cap(pid),
            &Change::DialControl(1),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.confirmed());
        assert!(report.setter_sent());
        assert_eq!(&device.sent()[0][6..11], &[2, 8, 24, 1, 1]);
        let device = Simulated::new(vec![vec![1, 1]]);
        let report = execute(
            pid,
            &device,
            &cap(pid),
            &Change::RestoreDialControl(1),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.confirmed());
        assert!(!report.setter_sent());
        assert_eq!(&device.sent()[0][6..11], &[2, 8, 152, 1, 0]);
        let device = Simulated::new(vec![vec![1, 0], vec![1, 1]]);
        let report = execute(
            pid,
            &device,
            &cap(pid),
            &Change::RestoreDialControl(1),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.confirmed());
        assert!(report.setter_sent());
        assert_eq!(device.sent().len(), 2);
    }
}

#[test]
fn failed_dial_restore_getter_still_sets_and_retains_read_failure_without_false_confirmation() {
    let device = Simulated::new(vec![vec![], vec![1, 1]]);
    let report = execute(
        1422,
        &device,
        &cap(1422),
        &Change::RestoreDialControl(1),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.source_result());
    assert!(!report.confirmed());
    assert!(report.setter_sent());
    assert_eq!(device.sent().len(), 2);
    assert!(report.stages()[0].error().is_some());
}
