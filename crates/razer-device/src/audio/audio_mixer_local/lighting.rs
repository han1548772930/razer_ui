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
