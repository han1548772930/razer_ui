//! Kitsune's current QuickEffects/U2A settings. The selected profile payload
//! and per-profile effect cache retain the source's missing duration/speed.
use super::lighting_state::HexColor;
use anyhow::ensure;
use razer_device::controller_lighting::{QuickEffectLevel, SoftwareEffect, WaveDirection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
pub(super) enum QuickEffectId { Breathing, Spectrum, Static, Reactive, Wave }
impl TryFrom<u8> for QuickEffectId {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> anyhow::Result<Self> {
        match value { 2 => Ok(Self::Breathing), 3 => Ok(Self::Spectrum), 1 => Ok(Self::Static), 5 => Ok(Self::Reactive), 4 => Ok(Self::Wave), _ => anyhow::bail!("Effect is not mounted by the current Kitsune page") }
    }
}
impl From<QuickEffectId> for u8 {
    fn from(value: QuickEffectId) -> Self { match value { QuickEffectId::Breathing => 2, QuickEffectId::Spectrum => 3, QuickEffectId::Static => 1, QuickEffectId::Reactive => 5, QuickEffectId::Wave => 4 } }
}
impl QuickEffectId {
    pub(super) const ORDER: [Self; 5] = [Self::Breathing, Self::Spectrum, Self::Static, Self::Reactive, Self::Wave];
    pub(super) fn label(self) -> &'static str { match self { Self::Breathing => "BREATHING", Self::Spectrum => "SPECTRUM_CYCLING", Self::Static => "STATIC", Self::Reactive => "REACTIVE", Self::Wave => "WAVE" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(super) enum QuickColor { None, Rgb(HexColor) }
impl TryFrom<String> for QuickColor {
    type Error = anyhow::Error;
    fn try_from(value: String) -> anyhow::Result<Self> { if value == "no-color" { Ok(Self::None) } else { Ok(Self::Rgb(HexColor::try_from(value)?)) } }
}
impl From<QuickColor> for String {
    fn from(value: QuickColor) -> Self { match value { QuickColor::None => "no-color".into(), QuickColor::Rgb(rgb) => rgb.into() } }
}
impl QuickColor {
    pub(super) fn rgb(self) -> Option<[u8; 3]> { match self { Self::None => None, Self::Rgb(color) => Some(color.0) } }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSetting {
    #[serde(skip_serializing_if = "Option::is_none")] color1: Option<QuickColor>,
    #[serde(skip_serializing_if = "Option::is_none")] color2: Option<QuickColor>,
    #[serde(skip_serializing_if = "Option::is_none")] is_random: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] duration: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] direction: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] speed: Option<u8>,
    #[serde(flatten)] extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum QuickSetting {
    Breathing { color1: QuickColor, color2: QuickColor, random: bool, duration: Option<QuickEffectLevel> },
    Spectrum { duration: Option<QuickEffectLevel> },
    Static { color: HexColor },
    Reactive { color: HexColor, duration: QuickEffectLevel },
    Wave { direction: WaveDirection, speed: Option<QuickEffectLevel> },
}
impl QuickSetting {
    pub(super) fn source_default(id: QuickEffectId) -> Self {
        let green = HexColor([0,255,0]);
        match id {
            QuickEffectId::Breathing => Self::Breathing { color1: QuickColor::Rgb(green), color2: QuickColor::None, random: false, duration: None },
            QuickEffectId::Spectrum => Self::Spectrum { duration: None },
            QuickEffectId::Static => Self::Static { color: green },
            QuickEffectId::Reactive => Self::Reactive { color: green, duration: QuickEffectLevel::Two },
            QuickEffectId::Wave => Self::Wave { direction: WaveDirection::Right, speed: None },
        }
    }
    fn parse(id: QuickEffectId, source: &SourceSetting) -> anyhow::Result<Self> {
        let color = || -> anyhow::Result<HexColor> { match source.color1 { Some(QuickColor::Rgb(color)) => Ok(color), _ => anyhow::bail!("This effect requires an RGB color") } };
        let duration = source.duration.map(QuickEffectLevel::try_from).transpose()?;
        let speed = source.speed.map(QuickEffectLevel::try_from).transpose()?;
        Ok(match id {
            QuickEffectId::Breathing => Self::Breathing { color1: source.color1.ok_or_else(|| anyhow::anyhow!("Breathing first color is missing"))?, color2: source.color2.ok_or_else(|| anyhow::anyhow!("Breathing second color is missing"))?, random: source.is_random.ok_or_else(|| anyhow::anyhow!("Breathing random choice is missing"))?, duration },
            QuickEffectId::Spectrum => Self::Spectrum { duration },
            QuickEffectId::Static => Self::Static { color: color()? },
            QuickEffectId::Reactive => Self::Reactive { color: color()?, duration: duration.ok_or_else(|| anyhow::anyhow!("Reactive duration is missing"))? },
            QuickEffectId::Wave => Self::Wave { direction: match source.direction { Some(1) => WaveDirection::Left, Some(2) => WaveDirection::Right, _ => anyhow::bail!("Invalid or missing Kitsune wave direction") }, speed },
        })
    }
    fn source(&self) -> SourceSetting {
        let mut source = SourceSetting::default();
        match *self {
            Self::Breathing { color1,color2,random,duration } => { source.color1=Some(color1); source.color2=Some(color2); source.is_random=Some(random); source.duration=duration.map(QuickEffectLevel::source_value); },
            Self::Spectrum {duration} => source.duration=duration.map(QuickEffectLevel::source_value),
            Self::Static { color } => source.color1=Some(QuickColor::Rgb(color)),
            Self::Reactive {color,duration} => { source.color1=Some(QuickColor::Rgb(color)); source.duration=Some(duration.source_value()); },
            Self::Wave {direction,speed} => { source.direction=Some(if direction==WaveDirection::Left {1} else {2}); source.speed=speed.map(QuickEffectLevel::source_value); },
        }
        source
    }
    pub(super) fn software(&self) -> SoftwareEffect {
        match *self {
            Self::Breathing {color1,color2,random,duration} => { let (color,color2)=match color1.rgb() { Some(color)=>(Some(color),color2.rgb()), None=>(color2.rgb(),None) }; SoftwareEffect::Breathing {color,color2,random,duration} },
            Self::Spectrum {duration} => SoftwareEffect::Spectrum {duration},
            Self::Static {color} => SoftwareEffect::Static {color:color.0},
            Self::Reactive {color,duration} => SoftwareEffect::Reactive {color:color.0,duration},
            Self::Wave {direction,speed} => SoftwareEffect::Wave {direction,speed},
        }
    }
    pub(super) fn color(&self, channel: usize) -> Option<QuickColor> {
        match *self { Self::Breathing {color1,color2,..} => Some(if channel==0 {color1} else {color2}), Self::Static {color} | Self::Reactive {color,..} if channel==0 => Some(QuickColor::Rgb(color)), _ => None }
    }
    pub(super) fn set_color(&mut self, channel: usize, next: QuickColor) -> anyhow::Result<()> {
        match self {
            Self::Breathing {color1,color2,..} => *if channel==0 {color1} else {color2}=next,
            Self::Static {color} | Self::Reactive {color,..} if channel==0 => { let QuickColor::Rgb(next)=next else {anyhow::bail!("This effect hides no-color")}; *color=next; },
            _ => anyhow::bail!("This effect has no color channel"),
        }
        Ok(())
    }
    pub(super) fn level(&self) -> Option<QuickEffectLevel> {
        match *self { Self::Breathing {duration,..}|Self::Spectrum {duration} => Some(duration.unwrap_or(QuickEffectLevel::Two)), Self::Reactive {duration,..}=>Some(duration), Self::Wave {speed,..}=>Some(speed.unwrap_or(QuickEffectLevel::Two)), _=>None }
    }
    pub(super) fn set_level(&mut self, level: QuickEffectLevel) {
        match self { Self::Breathing {duration,..}|Self::Spectrum {duration}=>*duration=Some(level), Self::Reactive {duration,..}=>*duration=level, Self::Wave {speed,..}=>*speed=Some(level), _=>{} }
    }
}

#[derive(Clone, Debug)]
struct CachedSetting { id: QuickEffectId, setting: QuickSetting, source: SourceSetting }
impl CachedSetting {
    fn source(&self)->SourceSetting {
        let current=self.setting.source();
        let mut source=self.source.clone();
        if current.color1.is_some(){source.color1=current.color1;}
        if current.color2.is_some(){source.color2=current.color2;}
        if current.is_random.is_some(){source.is_random=current.is_random;}
        if current.duration.is_some(){source.duration=current.duration;}
        if current.direction.is_some(){source.direction=current.direction;}
        if current.speed.is_some(){source.speed=current.speed;}
        source
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceCached { effect_id: QuickEffectId, #[serde(flatten)] setting: SourceSetting }
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceQuick {
    selected_effect_id: QuickEffectId,
    #[serde(skip_serializing_if = "Option::is_none")] selected_effect_setting: Option<SourceSetting>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] effect_settings: Vec<SourceCached>,
    #[serde(flatten)] extensions: BTreeMap<String,Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "SourceQuick", into = "SourceQuick")]
pub(super) struct QuickLightingSettings {
    pub(super) selected: QuickEffectId,
    cache: Vec<CachedSetting>,
    extensions: BTreeMap<String,Value>,
}
impl Default for QuickLightingSettings {
    fn default() -> Self { Self {selected:QuickEffectId::Spectrum,cache:Vec::new(),extensions:BTreeMap::new()} }
}
impl TryFrom<SourceQuick> for QuickLightingSettings {
    type Error = anyhow::Error;
    fn try_from(source: SourceQuick) -> anyhow::Result<Self> {
        let mut cache=Vec::<CachedSetting>::new();
        for saved in source.effect_settings {
            ensure!(!cache.iter().any(|c|c.id==saved.effect_id),"Duplicate cached quick effect");
            cache.push(CachedSetting {id:saved.effect_id,setting:QuickSetting::parse(saved.effect_id,&saved.setting)?,source:saved.setting});
        }
        if let Some(selected)=source.selected_effect_setting {
            let setting=QuickSetting::parse(source.selected_effect_id,&selected)?;
            cache.retain(|c|c.id!=source.selected_effect_id);
            cache.push(CachedSetting {id:source.selected_effect_id,setting,source:selected});
        }
        Ok(Self {selected:source.selected_effect_id,cache,extensions:source.extensions})
    }
}
impl From<QuickLightingSettings> for SourceQuick {
    fn from(state: QuickLightingSettings) -> Self {
        let selected_effect_setting=state.cache.iter().find(|c|c.id==state.selected).map(CachedSetting::source);
        let effect_settings=state.cache.into_iter().map(|c|SourceCached {effect_id:c.id,setting:c.source()}).collect();
        Self {selected_effect_id:state.selected,selected_effect_setting,effect_settings,extensions:state.extensions}
    }
}
impl QuickLightingSettings {
    pub(super) fn setting(&self) -> QuickSetting { self.cache.iter().find(|c|c.id==self.selected).map(|c|c.setting.clone()).unwrap_or_else(||QuickSetting::source_default(self.selected)) }
    pub(super) fn select(&mut self,id: QuickEffectId) {
        self.selected=id;
        let setting=self.cache.iter().find(|cached|cached.id==id).cloned().unwrap_or_else(||CachedSetting {id,setting:QuickSetting::source_default(id),source:SourceSetting::default()});
        self.cache.retain(|cached|cached.id!=id);self.cache.push(setting);
    }
    pub(super) fn store(&mut self,setting: QuickSetting) {
        let source=if self.selected==QuickEffectId::Static { SourceSetting::default() } else { self.cache.iter().find(|c|c.id==self.selected).map(CachedSetting::source).unwrap_or_default() };
        self.cache.retain(|c|c.id!=self.selected);
        self.cache.push(CachedSetting {id:self.selected,setting,source});
    }
    pub(super) fn sync_payload(&self) -> Value {
        let mut setting=serde_json::to_value(self.cache.iter().find(|c|c.id==self.selected).map(CachedSetting::source).unwrap_or_else(||self.setting().source())).expect("typed quick effect settings");
        setting["effectId"]=serde_json::json!(u8::from(self.selected));
        let mut payload=serde_json::json!({"selectedEffectId":u8::from(self.selected),"effectSettings":setting});
        if let QuickSetting::Wave {direction,..}=self.setting() {
            payload["indexDirection"]=serde_json::json!(if direction==WaveDirection::Left {0} else {1});
            payload["numberOfWaveDirection"]=serde_json::json!(2);
        }
        payload
    }
}

#[cfg(test)]
mod tests {
    use super::{QuickLightingSettings,QuickEffectId,QuickSetting,QuickColor};
    use razer_device::controller_lighting::{QuickEffectLevel,SoftwareEffect};
    use serde_json::json;
    #[test]
    fn source_defaults_retain_missing_duration_and_cache_each_effect() {
        let mut state:QuickLightingSettings=serde_json::from_value(json!({"selectedEffectId":3})).unwrap();
        assert_eq!(serde_json::to_value(&state).unwrap(),json!({"selectedEffectId":3}));
        state.select(QuickEffectId::Breathing);
        assert_eq!(state.setting().level(),Some(QuickEffectLevel::Two));
        assert!(serde_json::to_value(&state).unwrap()["selectedEffectSetting"].get("duration").is_none());
        let mut next=state.setting(); next.set_level(QuickEffectLevel::Three); state.store(next);
        state.select(QuickEffectId::Wave); state.select(QuickEffectId::Breathing);
        assert_eq!(state.setting().level(),Some(QuickEffectLevel::Three));
    }
    #[test]
    fn no_color_conversion_and_invalid_saved_fields_follow_source_contract() {
        let state:QuickLightingSettings=serde_json::from_value(json!({"selectedEffectId":2,"selectedEffectSetting":{"color1":"no-color","color2":"#112233","isRandom":false}})).unwrap();
        assert_eq!(state.setting().software(),SoftwareEffect::Breathing {color:Some([17,34,51]),color2:None,random:false,duration:None});
        for invalid in [json!({"selectedEffectId":6}),json!({"selectedEffectId":1,"selectedEffectSetting":{"color1":"no-color"}}),json!({"selectedEffectId":4,"selectedEffectSetting":{"direction":3}}),json!({"selectedEffectId":3,"selectedEffectSetting":{"duration":0}})] { assert!(serde_json::from_value::<QuickLightingSettings>(invalid).is_err()); }
        let mut setting=QuickSetting::source_default(QuickEffectId::Static);
        assert!(setting.set_color(0,QuickColor::None).is_err());
    }
}
