//! Current 1422/1446 UI 8193 defaults and main entry `fi`, `gi`, `vi`.
//! Stable band identity is independent of its position in the preset array.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) const PRESETS: [&str; 5] = ["podcast", "arena", "fpscallout", "studio", "custom"];
pub(super) const FILTERS: [&str; 5] = ["lowpass", "highpass", "peaking", "lowshelf", "highshelf"];
pub(super) const EFFECTS: [&str; 3] = ["mic_deesser", "vocal_bass", "vocal_exciter"];
pub(super) const FREQUENCIES: [f64; 10] = [
    20., 50., 100., 200., 500., 1000., 2000., 5000., 10000., 20000.,
];
pub(super) const HIGH_PASS: [(u8, u32); 3] = [(1, 40), (3, 80), (5, 160)];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(super) struct Band {
    pub id: u8,
    pub frequency: f64,
    pub gain: f64,
    pub q: f64,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
}
impl Band {
    pub fn pass(&self) -> bool {
        matches!(self.kind.as_str(), "lowpass" | "highpass")
    }
    pub fn shelf(&self) -> bool {
        matches!(self.kind.as_str(), "lowshelf" | "highshelf")
    }
    pub fn payload(&self) -> Value {
        json!({"direction":0,"band":self.id,"bandStatus":self.status.unwrap_or(1),
            "freq":self.frequency,"gain":self.gain,"qFactor":self.q,
            "filterType":match self.kind.as_str(){"highpass"=>1,"lowpass"=>2,"lowshelf"=>3,"highshelf"=>4,_=>5}})
    }
    pub fn update(&mut self, field: &str, value: &Value) {
        match field {
            "type" => {
                if let Some(kind) = value.as_str().filter(|kind| FILTERS.contains(kind)) {
                    self.kind = kind.into();
                    if self.pass() {
                        self.gain = 0.;
                    }
                }
            }
            "frequency" => {
                if let Some(value) = value.as_f64().filter(|v| v.is_finite()) {
                    self.frequency = value.clamp(20., 20000.);
                }
            }
            "gain" if !self.pass() => {
                if let Some(value) = value.as_f64().filter(|v| v.is_finite()) {
                    self.gain = value.clamp(-12., 12.);
                }
            }
            "q" => {
                if let Some(value) = value.as_f64().filter(|v| v.is_finite()) {
                    self.q = value.clamp(0.3, 15.);
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(super) struct HighPass {
    #[serde(rename = "isEnabled")]
    pub enabled: bool,
    pub value: u8,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(super) struct Preset {
    pub preset: String,
    #[serde(rename = "selectedBandId")]
    pub selected: u8,
    #[serde(rename = "highPassFilter")]
    pub high_pass: HighPass,
    pub bands: Vec<Band>,
}
impl Preset {
    pub fn selected(&self) -> Option<&Band> {
        self.bands
            .iter()
            .find(|band| band.id == self.selected)
            .or_else(|| self.bands.first())
    }
    pub fn first_unused(&self) -> Option<u8> {
        (0..8).find(|id| self.bands.iter().all(|band| band.id != *id))
    }
    pub fn add(&mut self) -> Option<Vec<Band>> {
        if self.bands.len() >= 8 {
            return None;
        }
        let id = self.first_unused()?;
        let original = data()
            .defaults
            .data
            .iter()
            .find(|p| p.preset == self.preset)
            .and_then(|p| p.bands.iter().find(|band| band.id == id));
        let band = original
            .map(|band| Band {
                gain: 0.,
                status: None,
                ..band.clone()
            })
            .unwrap_or_else(|| Band {
                id,
                frequency: self
                    .bands
                    .last()
                    .map_or(1000., |band| 1.44 * band.frequency)
                    .clamp(20., 20000.),
                gain: 0.,
                q: 1.,
                kind: "peaking".into(),
                status: None,
            });
        self.bands.push(band);
        self.bands.sort_by_key(|band| band.id);
        self.selected = id;
        Some(self.bands.clone())
    }
    pub fn remove_selected(&mut self) -> Option<Band> {
        if self.bands.len() <= 1 {
            return None;
        }
        let position = self
            .bands
            .iter()
            .position(|band| band.id == self.selected)?;
        let next = if position + 1 == self.bands.len() {
            position - 1
        } else {
            position + 1
        };
        self.selected = self.bands[next].id;
        let mut removed = self.bands.remove(position);
        removed.status = Some(0);
        Some(removed)
    }
    pub fn reset(&mut self) -> Vec<Band> {
        let default = data()
            .defaults
            .data
            .iter()
            .find(|p| p.preset == self.preset)
            .expect("current preset identity");
        self.selected = default.selected;
        self.high_pass = default.high_pass.clone();
        self.bands = default
            .bands
            .iter()
            .filter(|band| band.status != Some(0))
            .cloned()
            .collect();
        default.bands.clone()
    }
    pub fn flatten(&mut self) -> Vec<Band> {
        self.bands = data()
            .flat
            .iter()
            .filter(|band| band.status != Some(0))
            .cloned()
            .collect();
        // TO_FLAT deliberately preserves high pass and selectedBandId.
        data().flat.clone()
    }
    pub fn is_default(&self) -> bool {
        data()
            .defaults
            .data
            .iter()
            .find(|p| p.preset == self.preset)
            .is_some_and(|p| {
                p.high_pass == self.high_pass
                    && p.bands
                        .iter()
                        .filter(|band| band.status != Some(0))
                        .eq(self.bands.iter())
            })
    }
    pub fn payload(&self, effects: &Value) -> Value {
        json!({"eqName":self.preset,"parametricEqSettings":self.bands.iter().map(Band::payload).collect::<Vec<_>>(),
            "highPassFilter":self.high_pass,
            "vocalBass":{"isEnabled":effects["vocal_bass"]["isEnabled"],"value":effects["vocal_bass"]["value"]},
            "vocalExciter":{"isEnabled":effects["vocal_exciter"]["isEnabled"],"value":effects["vocal_exciter"]["value"]}})
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(super) struct Model {
    #[serde(rename = "selectedPreset")]
    pub selected: String,
    pub data: Vec<Preset>,
}
impl Model {
    pub fn active(&self) -> Option<&Preset> {
        self.data.iter().find(|p| p.preset == self.selected)
    }
    pub fn active_mut(&mut self) -> Option<&mut Preset> {
        self.data.iter_mut().find(|p| p.preset == self.selected)
    }
    pub fn from_draft(value: &Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }
    pub fn initial() -> Self {
        let mut model = data().defaults.clone();
        for preset in &mut model.data {
            preset.bands.retain(|band| band.status != Some(0));
        }
        model
    }
    /// Restore a dynamic collection whole; index-wise `merge_known` loses bands.
    pub fn restore(value: &Value) -> Option<Self> {
        let mut model = Self::from_draft(value)?;
        if !PRESETS.contains(&model.selected.as_str()) || model.data.len() != PRESETS.len() {
            return None;
        }
        for name in PRESETS {
            let mut matches = model.data.iter_mut().filter(|p| p.preset == name);
            let preset = matches.next()?;
            if matches.next().is_some() || preset.bands.is_empty() || preset.bands.len() > 8 {
                return None;
            }
            let mut seen = [false; 8];
            for band in &mut preset.bands {
                if band.id >= 8
                    || seen[usize::from(band.id)]
                    || band.status == Some(0)
                    || !FILTERS.contains(&band.kind.as_str())
                    || !band.frequency.is_finite()
                    || !band.gain.is_finite()
                    || !band.q.is_finite()
                {
                    return None;
                }
                seen[usize::from(band.id)] = true;
                band.frequency = band.frequency.clamp(20., 20000.);
                band.q = band.q.clamp(0.3, 15.);
                band.gain = if band.pass() {
                    0.
                } else {
                    band.gain.clamp(-12., 12.)
                };
            }
            if !HIGH_PASS
                .iter()
                .any(|(value, _)| *value == preset.high_pass.value)
            {
                return None;
            }
        }
        Some(model)
    }
}

#[derive(Deserialize)]
pub(super) struct FilterAsset {
    pub kind: String,
    pub asset: String,
    pub width: f32,
    pub height: f32,
}
#[derive(Deserialize)]
pub(super) struct Data {
    pub defaults: Model,
    pub flat: Vec<Band>,
    pub labels: BTreeMap<String, String>,
    pub filters: Vec<FilterAsset>,
}
pub(super) fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| read_audio_seiren_eq_catalog())
}
pub(super) fn label(alias: &str) -> String {
    razer_i18n::t(&data().labels[alias])
}

/// Module8951 interpolation, including source's +4 display offset.
pub(super) fn gain_db(percent: f64) -> f64 {
    interpolate(percent.clamp(0., 100.), false) + 4.
}
pub(super) fn gain_percent(db: f64) -> f64 {
    interpolate((db - 4.).clamp(0., 36.), true)
}
fn interpolate(value: f64, inverse: bool) -> f64 {
    const POINTS: [(f64, f64); 9] = [
        (0., 0.),
        (14., 2.1),
        (40., 6.9),
        (60., 12.),
        (70., 15.3),
        (80., 19.7),
        (84., 21.8),
        (87., 23.7),
        (100., 36.),
    ];
    for pair in POINTS.windows(2) {
        let [(a, b), (c, d)] = [pair[0], pair[1]];
        let (x0, y0, x1, y1) = if inverse { (b, a, d, c) } else { (a, b, c, d) };
        if value >= x0 && value <= x1 {
            return y0 + (value - x0) / (x1 - x0) * (y1 - y0);
        }
    }
    if inverse { 100. } else { 36. }
}

pub(super) fn log_position(frequency: f64) -> f64 {
    (frequency.log10() - 20_f64.log10()) / 3.
}
pub(super) fn drag_band(band: &Band, x: f64, y: f64) -> Band {
    let mut next = band.clone();
    next.frequency = (20. * 1000_f64.powf(x.clamp(0., 1.))).round();
    if band.pass() {
        next.gain = 0.;
        next.q = (((1. - y.clamp(0., 1.)) * 14.7 + 0.3) * 10.).round() / 10.;
    } else {
        next.gain = (((1. - y.clamp(0., 1.)) * 24. - 12.) * 10.).round() / 10.;
    }
    next
}
pub(super) fn q_handle_distance(q: f64) -> f64 {
    20. + 60. * (1. - (q.ln() - 0.3_f64.ln()) / (15_f64.ln() - 0.3_f64.ln()))
}
pub(super) fn q_from_handle_distance(distance: f64) -> f64 {
    let exponent = (distance.clamp(20., 79.99) - 20.) / 60.;
    ((15. * (0.3_f64 / 15.).powf(exponent)) * 10.)
        .round()
        .clamp(3., 150.)
        / 10.
}

#[cfg(test)]
#[path = "audio_seiren_eq_tests.rs"]
mod tests;

// Generated catalog reader: razer-pages/audio_seiren_eq_data.
fn read_audio_seiren_eq_catalog() -> Data {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                defaults: r.child(decode_1)?,
                flat: r.child(decode_8)?,
                labels: r.child(decode_12)?,
                filters: r.child(decode_13)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Model> {
        let value = match r.tag()? {
            0 => Model {
                selected: r.child(decode_2)?,
                data: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Preset>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Preset> {
        let value = match r.tag()? {
            0 => Preset {
                preset: r.child(decode_2)?,
                selected: r.child(decode_5)?,
                high_pass: r.child(decode_6)?,
                bands: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<HighPass> {
        let value = match r.tag()? {
            0 => HighPass {
                enabled: r.child(decode_7)?,
                value: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Band>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Band> {
        let value = match r.tag()? {
            0 => Band {
                id: r.child(decode_5)?,
                frequency: r.child(decode_10)?,
                gain: r.child(decode_10)?,
                q: r.child(decode_10)?,
                kind: r.child(decode_2)?,
                status: None,
            },
            1 => Band {
                id: r.child(decode_5)?,
                frequency: r.child(decode_10)?,
                gain: r.child(decode_10)?,
                q: r.child(decode_10)?,
                kind: r.child(decode_2)?,
                status: r.child(decode_11)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f64> {
        let value = match r.tag()? {
            0 => r.f64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u8>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_5)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<FilterAsset>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_14)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FilterAsset> {
        let value = match r.tag()? {
            0 => FilterAsset {
                kind: r.child(decode_2)?,
                asset: r.child(decode_2)?,
                width: r.child(decode_15)?,
                height: r.child(decode_15)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_AUDIO_SEIREN_EQ_DATA,
        0,
        "5a1558e6e0a34a58c1ab2c06829b4bd430a1413c432b928cfe868007a9ed6192",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod audio_seiren_eq_data_catalog_tests {
    #[test]
    fn packaged_catalog_audio_seiren_eq_data_decodes() {
        let _ = super::read_audio_seiren_eq_catalog();
    }
}
// End generated catalog reader: razer-pages/audio_seiren_eq_data.
