//! Current AudioMixer setBrightness/setChromaEffectCommon/Vn/Cn and current
//! Lighting Engine UI-to-hardware conversion. PID1342 manifest is hardware-only.
use super::*;
use std::time::Duration;

pub const REGIONS: [u8; 10] = [4, 5, 6, 7, 8, 9, 16, 10, 33, 32];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Brightness {
        region: u8,
        value: u8,
    },
    Effect {
        region: u8,
        effect: u8,
        flags: u8,
        colors: Vec<u8>,
    },
    MeterFrame {
        peaks: [u8; 4],
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Sample {
    pub sources: [u8; 4],
    pub peaks: [u8; 4],
    pub raw_peak: [Option<f32>; 7],
    pub device_container_id: String,
    /// A completed source Feature send, not an independent device readback.
    pub feature_written: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SampleFailure {
    pub progress: Sample,
    pub transfer: TransferProgress,
}

impl std::fmt::Display for SampleFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mixer meter sample failed; actual peaks and Feature progress retained")
    }
}

impl std::error::Error for SampleFailure {}

/// Native RVA 0x368c0 truncates left-channel peaks; RVA 0x36ca0 sends a
/// Feature7/64 frame, retrying native send failures ten times after 10 ms each.
pub fn submit_sample(
    device: &dyn FeatureTransport,
    sources: [u8; 4],
    raw_peak: [Option<f32>; 7],
    device_container_id: String,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Sample> {
    ensure!(
        sources.iter().all(|source| *source <= 6),
        "Mixer fader input source outside enum"
    );
    let mut peaks = [0; 4];
    for (index, source) in sources.iter().enumerate() {
        let raw = raw_peak[usize::from(*source)].context("Mixer input peak was not observed")?;
        ensure!(
            raw.is_finite() && (0. ..=1.).contains(&raw),
            "Mixer input peak outside source range"
        );
        peaks[index] = (raw * 100.).trunc() as u8;
    }
    let report = reports(&Action::MeterFrame { peaks })?.remove(0).0;
    let mut progress = Sample {
        sources,
        peaks,
        raw_peak,
        device_container_id,
        feature_written: false,
    };
    let mut transfer = TransferProgress {
        channel: ReportChannel::Feature,
        report: report.to_vec(),
        sends_attempted: 0,
        sends_completed: 0,
        input: None,
    };
    let result = (|| {
        validate()?;
        let lease = crate::backend::checked_exchange(
            device.begin_exchange_validated(
                crate::backend::ExchangeOptions::plain(),
                std::rc::Rc::new(validate),
            )?,
            validate,
            validate,
            None,
        );
        let result = (|| {
            let mut last = None;
            for _ in 0..10 {
                std::thread::sleep(Duration::from_millis(10));
                validate()?;
                let sent = lease.transport().send_feature_counted(&report);
                transfer.observe_send(&sent);
                progress.feature_written = transfer.sends_completed > 0;
                if let Err(validation) = validate() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                match sent
                    .and_then(|count| crate::backend::require_complete_send(count, report.len()))
                {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(error) => {
                        if crate::backend::exchange_finalization_failed(&error) {
                            return Err(error);
                        }
                        last = Some(error);
                    }
                }
            }
            Err(last.context("Mixer lighting send was not attempted")?)
        })();
        crate::backend::finish_exchange(lease, result)
    })();
    match result {
        Ok(()) => Ok(progress),
        Err(error) => Err(error.context(SampleFailure { progress, transfer })),
    }
}

pub fn handles(path: &str) -> bool {
    path == "/profile/brightness"
        || path == "/profile/quickEffects"
        || path.starts_with("/profile/brightness/")
        || path.starts_with("/profile/quickEffects/")
        || path.starts_with("/device/fader/")
}

fn region(value: &Value) -> anyhow::Result<u8> {
    let id = u8::try_from(
        value["regionId"]
            .as_u64()
            .context("Lighting region absent")?,
    )?;
    ensure!(
        id == 0 || REGIONS.contains(&id),
        "Unknown Mixer lighting region {id}"
    );
    Ok(id)
}
fn color(value: &Value) -> anyhow::Result<Vec<u8>> {
    if value.is_null() || value == "no-color" {
        return Ok(vec![0, 0, 0]);
    }
    let text = value
        .as_str()
        .context("Lighting color is not a source hex string")?;
    let text = text.strip_prefix('#').unwrap_or(text);
    ensure!(
        text.len() == 6 && text.is_ascii(),
        "Lighting hex color invalid"
    );
    Ok(vec![
        u8::from_str_radix(&text[0..2], 16)?,
        u8::from_str_radix(&text[2..4], 16)?,
        u8::from_str_radix(&text[4..6], 16)?,
    ])
}
fn effect(value: &Value) -> anyhow::Result<Command> {
    let region = region(value)?;
    let id = value["selectedEffectId"]
        .as_u64()
        .context("Lighting effect ID absent")?;
    let setting = &value["selectedEffectSetting"];
    ensure!(setting.is_object(), "Lighting settings absent");
    let (effect, flags, colors) = match id {
        0 => (0, 0, vec![]),
        1 => {
            let colors = if let Some(list) = setting["color1"].as_array() {
                let mut colors = Vec::new();
                for item in list {
                    colors.extend(color(item)?);
                }
                colors
            } else {
                let colors = color(&setting["color1"])?;
                // generateSetChromaEffectCommonData uses a truthy scalar test.
                if colors == [0, 0, 0] { vec![] } else { colors }
            };
            (1, 0, colors)
        }
        2 => {
            if setting["isRandom"] == true {
                (2, 3, vec![])
            } else {
                let first = if setting["color1"] == "no-color" {
                    &setting["color2"]
                } else {
                    &setting["color1"]
                };
                let second = if setting["color1"] == "no-color" {
                    Value::Null
                } else {
                    setting["color2"].clone()
                };
                let mut colors = color(first)?;
                let second = color(&second)?;
                if second != [0, 0, 0] {
                    colors.extend(second);
                }
                (2, (colors.len() / 3) as u8, colors)
            }
        }
        3 => (3, 0, vec![]),
        4 => (4, if setting["direction"] == 1 { 2 } else { 1 }, vec![]),
        12 => {
            let colors = color(&setting["color1"])?;
            (
                10,
                setting["colorBoost"].as_u64().unwrap_or(0) as u8,
                if colors == [0, 0, 0] { vec![] } else { colors },
            )
        }
        _ => anyhow::bail!("Effect {id} outside current mounted Mixer choices"),
    };
    Ok(Command::Lighting {
        action: Action::Effect {
            region,
            effect,
            flags,
            colors,
        },
    })
}

pub fn compile(path: &str, profile: &Value) -> anyhow::Result<Vec<Command>> {
    if matches!(path, "/profile/brightness" | "/profile/quickEffects") {
        let entries = profile
            .pointer(path)
            .and_then(Value::as_array)
            .context("Lighting restore collection absent")?;
        let mut result = Vec::new();
        for index in 0..entries.len() {
            result.extend(compile(&format!("{path}/{index}"), profile)?);
        }
        return Ok(result);
    }
    if path.starts_with("/profile/brightness/") {
        let selected = profile
            .pointer(path)
            .context("Lighting brightness selection absent")?;
        let value = selected["value"]
            .as_f64()
            .context("Lighting brightness absent")?;
        ensure!(
            value.is_finite() && (0. ..=100.).contains(&value),
            "Lighting brightness outside source range"
        );
        return Ok(vec![Command::Lighting {
            action: Action::Brightness {
                region: region(selected)?,
                value: if yes(selected, "isEnabled")? {
                    value.trunc() as u8
                } else {
                    0
                },
            },
        }]);
    }
    if path.starts_with("/profile/quickEffects/") {
        return Ok(vec![effect(
            profile
                .pointer(path)
                .context("Lighting effect selection absent")?,
        )?]);
    }
    let fader = &profile["device"]["fader"];
    let mode = fader["useMode"]
        .as_u64()
        .context("Fader lighting mode absent")?;
    ensure!(mode <= 1, "Fader lighting mode outside source enum");
    let selected = if mode == 0 { "quickEffects" } else { "meterEffects" };
    let mut commands = Vec::new();
    // taskMakerSetFader Ir publishes the complete active collection as ports.
    for entry in fader[selected]
        .as_array()
        .context("Fader effect collection absent")?
    {
        commands.push(effect(entry)?);
    }
    if path == "/device/fader/useMode" {
        for entry in fader["quickEffects"]
            .as_array()
            .context("Fader region collection absent")?
        {
            commands.push(Command::Fader {
                region: region(entry)?,
                mode: mode as u8 + 1,
            });
        }
    }
    // dynamicSpeeds/isEnabled are local fields: the current tn setter only
    // writes on useMode and deliberately passes speed 0. Do not invent a setter.
    Ok(commands)
}

fn header(prefix: &[u8]) -> [u8; 64] {
    let mut report = [0; 64];
    report[..prefix.len()].copy_from_slice(prefix);
    report
}
pub fn reports(action: &Action) -> anyhow::Result<Vec<([u8; 64], Option<u8>, bool)>> {
    let (region, all) = match action {
        Action::Brightness { region, .. } | Action::Effect { region, .. } => {
            (*region, *region == 0)
        }
        Action::MeterFrame { peaks } => {
            ensure!(
                peaks.iter().all(|v| *v <= 100),
                "Meter peak outside source percent range"
            );
            let mut report = header(&[7, 0, 31, 0, 0, 0, 4, 15, 19]);
            report[9..13].copy_from_slice(peaks);
            return Ok(vec![(report, None, false)]);
        }
    };
    ensure!(
        all || REGIONS.contains(&region),
        "Invalid Mixer lighting region"
    );
    let regions = if all { REGIONS.to_vec() } else { vec![region] };
    let mut reports = Vec::new();
    for region in regions {
        let mut reply = None;
        let report = match action {
            Action::Brightness { value, .. } => {
                ensure!(*value <= 100, "Mixer brightness exceeds source range");
                header(&[7, 0, 31, 0, 0, 0, 3, 15, 4, 1, region, *value])
            }
            Action::Effect {
                effect,
                flags,
                colors,
                ..
            } => {
                ensure!(
                    colors.len() <= 24 && colors.len() % 3 == 0,
                    "Mixer color array invalid"
                );
                if *effect == 0 {
                    header(&[7, 0, 31, 0, 0, 0, 0, 15, 4, 0, region])
                } else if *effect == 10 && (6..=9).contains(&region) {
                    let mut report = header(&[7, 0, 31, 0, 0, 0, 5, 15, 18, 1, region]);
                    report[11..11 + colors.len()].copy_from_slice(colors);
                    // Cn description has no Ft mapping; sendCommand uses the
                    // AudioMixer reportId 18 fallback (not an invented 7 ACK).
                    reply = Some(18);
                    report
                } else if *effect == 1 && matches!(region, 16 | 32 | 33) {
                    let count = if region == 16 { 8 } else { 2 };
                    let mut report = header(&[
                        7,
                        0,
                        31,
                        0,
                        0,
                        0,
                        if region == 16 { 27 } else { 30 },
                        15,
                        2,
                        0,
                        region,
                        1,
                        0,
                        0,
                        count,
                    ]);
                    if colors.len() >= 3 {
                        for index in 0..usize::from(count) {
                            let start = if index % 2 == 1 && colors.len() >= 6 { 3 } else { 0 };
                            report[15 + index * 3..18 + index * 3]
                                .copy_from_slice(&colors[start..start + 3]);
                        }
                    }
                    if region == 16 && colors.len() >= 24 {
                        report[15..39].copy_from_slice(&colors[..24]);
                    }
                    report
                } else {
                    ensure!(
                        *effect <= 4,
                        "Mixer effect lacks this region's source template"
                    );
                    let lengths = match region {
                        4 => [0, 27, 30, 24, 0],
                        5 => [0, 36, 30, 24, 24],
                        6..=9 => [0, 27, 30, 24, 24],
                        10 => [0, 36, 16, 24, 0],
                        _ => [0; 5],
                    };
                    let mut report = header(&[
                        7,
                        0,
                        31,
                        0,
                        0,
                        0,
                        lengths[*effect as usize],
                        15,
                        2,
                        0,
                        region,
                        *effect,
                        *flags,
                        if *effect == 4 { 40 } else { 0 },
                        (colors.len() / 3) as u8,
                    ]);
                    report[15..15 + colors.len()].copy_from_slice(colors);
                    if region == 5 && *effect == 1 && colors.len() <= 3 {
                        report[14] = 4;
                        for index in 1..4 {
                            report[15 + 3 * index..15 + 3 * index + colors.len()]
                                .copy_from_slice(colors);
                        }
                    }
                    report
                }
            }
            Action::MeterFrame { .. } => unreachable!(),
        };
        reports.push((report, reply, all));
    }
    Ok(reports)
}
pub(super) fn execute(
    device: &dyn FeatureTransport,
    action: &Action,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let mut observations = Vec::new();
    for (report, reply, delay) in reports(action)? {
        if delay {
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(bytes) = super::reports::feature(device, &report, reply, validate)? {
            observations.push(json!({"feature_reply":bytes}));
        }
    }
    Ok(json!({"observations":observations}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
    };

    enum MeterSend {
        Completed,
        Failed,
        FinalizationFailed,
    }

    struct MeterTransport {
        steps: RefCell<VecDeque<MeterSend>>,
        reports: RefCell<Vec<Vec<u8>>>,
    }

    impl MeterTransport {
        fn new(steps: impl IntoIterator<Item = MeterSend>) -> Self {
            Self {
                steps: RefCell::new(steps.into_iter().collect()),
                reports: RefCell::new(Vec::new()),
            }
        }
    }

    impl FeatureTransport for MeterTransport {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            // Independent RVA 0x36ca0 Feature7/64 template and 0x368c0
            // left*100 truncation for the four requested sample sources.
            let mut expected = [0; 64];
            expected[..13].copy_from_slice(&[7, 0, 31, 0, 0, 0, 4, 15, 19, 23, 56, 99, 23]);
            assert_eq!(report, expected);
            self.reports.borrow_mut().push(report.to_vec());
            match self.steps.borrow_mut().pop_front().unwrap() {
                MeterSend::Completed => Ok(()),
                MeterSend::Failed => Err(crate::backend::native_io_failure(
                    crate::backend::NativeIoOutcome::Rejected,
                    anyhow::anyhow!("ordinary native Feature failure"),
                )),
                MeterSend::FinalizationFailed => Err(crate::backend::native_io_failure(
                    crate::backend::NativeIoOutcome::Rejected,
                    crate::backend::finalization_failure(anyhow::anyhow!(
                        "inner exchange ownership was lost"
                    )),
                )),
            }
        }

        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            panic!("the source meter frame has no Feature readback")
        }

        fn metadata(&self) -> Value {
            Value::Null
        }
    }

    fn raw_meter_peaks() -> [Option<f32>; 7] {
        [
            Some(0.237),
            None,
            None,
            None,
            Some(0.569),
            Some(0.991),
            None,
        ]
    }

    #[test]
    fn meter_sample_retries_ordinary_native_failure_without_changing_the_source_frame() {
        let io = MeterTransport::new([MeterSend::Failed, MeterSend::Completed]);
        let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
        let result = submit_sample(
            &probe,
            [0, 4, 5, 0],
            raw_meter_peaks(),
            "actual-container".into(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(result.peaks, [23, 56, 99, 23]);
        assert_eq!(result.raw_peak, raw_meter_peaks());
        assert!(result.feature_written);
        assert_eq!(io.reports.borrow().len(), 2);
        assert!(io.steps.borrow().is_empty());
        assert_eq!(probe.timings(), [crate::backend::ExchangeTiming::Plain]);
    }

    #[test]
    fn meter_sample_stops_after_ten_ordinary_failures_with_actual_peaks_in_diagnostics() {
        let io = MeterTransport::new((0..10).map(|_| MeterSend::Failed));
        let error = submit_sample(
            &io,
            [0, 4, 5, 0],
            raw_meter_peaks(),
            "actual-container".into(),
            &|| Ok(()),
        )
        .unwrap_err();
        let failure = error.downcast_ref::<SampleFailure>().unwrap();
        assert_eq!(failure.progress.raw_peak, raw_meter_peaks());
        assert_eq!(failure.progress.peaks, [23, 56, 99, 23]);
        assert!(!failure.progress.feature_written);
        assert_eq!(failure.transfer.sends_attempted, 10);
        assert_eq!(failure.transfer.sends_completed, 0);
        assert!(failure.transfer.input.is_none());
        assert_eq!(io.reports.borrow().len(), 10);
        assert!(io.steps.borrow().is_empty());
        assert_eq!(
            error.root_cause().to_string(),
            "ordinary native Feature failure"
        );
    }

    #[test]
    fn meter_sample_does_not_retry_a_terminal_exchange_failure() {
        let io = MeterTransport::new([MeterSend::FinalizationFailed, MeterSend::Completed]);
        let error = submit_sample(
            &io,
            [0, 4, 5, 0],
            raw_meter_peaks(),
            "actual-container".into(),
            &|| Ok(()),
        )
        .unwrap_err();
        assert!(crate::backend::exchange_finalization_failed(&error));
        let failure = error.downcast_ref::<SampleFailure>().unwrap();
        assert_eq!(failure.transfer.sends_attempted, 1);
        assert_eq!(failure.transfer.sends_completed, 0);
        assert!(!failure.progress.feature_written);
        assert_eq!(io.reports.borrow().len(), 1);
        assert_eq!(io.steps.borrow().len(), 1);
        assert_eq!(
            error.root_cause().to_string(),
            "inner exchange ownership was lost"
        );
    }

    #[test]
    fn meter_sample_preserves_completed_feature_and_peaks_when_release_or_final_owner_fails() {
        for release_failure in [true, false] {
            let io = MeterTransport::new([MeterSend::Completed]);
            let stale = Cell::new(false);
            let replace = || stale.set(true);
            let probe = if release_failure {
                crate::feature_exchange::tests::ExchangeProbe::failing_finish(&io)
            } else {
                crate::feature_exchange::tests::ExchangeProbe::with_on_finish(&io, &replace)
            };
            let error = submit_sample(
                &probe,
                [0, 4, 5, 0],
                raw_meter_peaks(),
                "actual-container".into(),
                &|| {
                    ensure!(!stale.get(), "meter sample owner replaced during release");
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(crate::backend::exchange_finalization_failed(&error));
            let failure = error.downcast_ref::<SampleFailure>().unwrap();
            assert_eq!(failure.progress.sources, [0, 4, 5, 0]);
            assert_eq!(failure.progress.raw_peak, raw_meter_peaks());
            assert_eq!(failure.progress.peaks, [23, 56, 99, 23]);
            assert_eq!(failure.progress.device_container_id, "actual-container");
            assert!(failure.progress.feature_written);
            assert_eq!(failure.transfer.sends_attempted, 1);
            assert_eq!(failure.transfer.sends_completed, 1);
            assert!(failure.transfer.input.is_none());
            assert_eq!(io.reports.borrow().len(), 1);
            assert!(io.steps.borrow().is_empty());
            assert_eq!(
                error.root_cause().to_string(),
                if release_failure {
                    "simulated explicit release failed"
                } else {
                    "meter sample owner replaced during release"
                }
            );
        }
    }

    #[test]
    fn meter_sample_keeps_observed_peaks_when_admission_is_canceled_without_writing() {
        let io = MeterTransport::new([]);
        let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
        let error = submit_sample(
            &probe,
            [0, 4, 5, 0],
            raw_meter_peaks(),
            "actual-container".into(),
            &|| anyhow::bail!("meter sample canceled before admission"),
        )
        .unwrap_err();
        let failure = error.downcast_ref::<SampleFailure>().unwrap();
        assert_eq!(failure.progress.raw_peak, raw_meter_peaks());
        assert!(!failure.progress.feature_written);
        assert_eq!(failure.transfer.sends_attempted, 0);
        assert_eq!(failure.transfer.sends_completed, 0);
        assert!(failure.transfer.input.is_none());
        assert!(io.reports.borrow().is_empty());
        assert!(probe.timings().is_empty());
    }

    #[test]
    fn meter_sample_retains_a_completed_send_only_as_diagnostics_after_owner_loss() {
        let io = MeterTransport::new([MeterSend::Completed]);
        let error = submit_sample(
            &io,
            [0, 4, 5, 0],
            raw_meter_peaks(),
            "actual-container".into(),
            &|| {
                ensure!(
                    io.reports.borrow().is_empty(),
                    "meter owner changed during send"
                );
                Ok(())
            },
        )
        .unwrap_err();
        let failure = error.downcast_ref::<SampleFailure>().unwrap();
        assert!(failure.progress.feature_written);
        assert_eq!(failure.progress.peaks, [23, 56, 99, 23]);
        assert_eq!(
            (
                failure.transfer.sends_attempted,
                failure.transfer.sends_completed
            ),
            (1, 1)
        );
        assert_eq!(
            crate::backend::native_io_outcome(&error),
            Some(crate::backend::NativeIoOutcome::Returned { byte_count: 64 })
        );
        assert_eq!(io.reports.borrow().len(), 1);
        assert!(format!("{error:#}").contains("meter owner changed during send"));
    }

    #[test]
    fn source_region_templates_and_mute_pairs() {
        let command=effect(&json!({"regionId":16,"selectedEffectId":1,"selectedEffectSetting":{"color1":["#010203","#040506"]}})).unwrap();
        let Command::Lighting { action } = command else {
            panic!()
        };
        let packets = reports(&action).unwrap();
        assert_eq!(
            &packets[0].0[..15],
            &[7, 0, 31, 0, 0, 0, 27, 15, 2, 0, 16, 1, 0, 0, 8]
        );
        assert_eq!(
            &packets[0].0[15..39],
            &[
                1, 2, 3, 4, 5, 6, 1, 2, 3, 4, 5, 6, 1, 2, 3, 4, 5, 6, 1, 2, 3, 4, 5, 6
            ]
        );
    }
    #[test]
    fn meter_color_uses_source_fallback_reply_and_all_region_order() {
        let packets = reports(&Action::Effect {
            region: 7,
            effect: 10,
            flags: 0,
            colors: vec![0, 255, 0],
        })
        .unwrap();
        assert_eq!(
            &packets[0].0[..14],
            &[7, 0, 31, 0, 0, 0, 5, 15, 18, 1, 7, 0, 255, 0]
        );
        assert_eq!(packets[0].1, Some(18));
        let packets = reports(&Action::Brightness {
            region: 0,
            value: 61,
        })
        .unwrap();
        assert_eq!(packets.iter().map(|p| p.0[10]).collect::<Vec<_>>(), REGIONS);
        assert!(packets.iter().all(|p| p.2));
    }
}
