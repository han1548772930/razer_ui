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
    #[serde(skip_serializing_if = "Option::is_none")] effect_id: Option<QuickEffectId>,
    #[serde(skip_serializing_if = "Option::is_none")] color1: Option<QuickColor>,
    #[serde(skip_serializing_if = "Option::is_none")] color2: Option<QuickColor>,
    #[serde(skip_serializing_if = "Option::is_none")] is_random: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] duration: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] direction: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] speed: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] index_direction: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] number_of_wave_direction: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] total_idx_direction: Option<u8>,
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
struct SourceCustomColor {
    color: String,
    #[serde(flatten)] extensions: BTreeMap<String,Value>,
}
impl SourceCustomColor {
    fn validate(&self)->anyhow::Result<()> {if self.color=="#111" {Ok(())}else{HexColor::try_from(self.color.clone()).map(|_|())}}
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceQuick {
    selected_effect_id: QuickEffectId,
    #[serde(skip_serializing_if = "Option::is_none")] selected_effect_setting: Option<SourceSetting>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] effect_settings: Vec<SourceCached>,
    #[serde(skip_serializing_if = "Option::is_none")] custom_colors: Option<Vec<SourceCustomColor>>,
    #[serde(flatten)] extensions: BTreeMap<String,Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "SourceQuick", into = "SourceQuick")]
pub(super) struct QuickLightingSettings {
    pub(super) selected: QuickEffectId,
    cache: Vec<CachedSetting>,
    custom_colors: Option<Vec<SourceCustomColor>>,
    extensions: BTreeMap<String,Value>,
}
impl Default for QuickLightingSettings {
    fn default() -> Self { Self {selected:QuickEffectId::Spectrum,cache:Vec::new(),custom_colors:None,extensions:BTreeMap::new()} }
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
            ensure!(selected.effect_id.is_none_or(|id|id==source.selected_effect_id),"Selected quick effect identity does not match its settings");
            let setting=QuickSetting::parse(source.selected_effect_id,&selected)?;
            cache.retain(|c|c.id!=source.selected_effect_id);
            cache.push(CachedSetting {id:source.selected_effect_id,setting,source:selected});
        }
        if let Some(colors)=&source.custom_colors {for color in colors {color.validate()?;}}
        Ok(Self {selected:source.selected_effect_id,cache,custom_colors:source.custom_colors,extensions:source.extensions})
    }
}
impl From<QuickLightingSettings> for SourceQuick {
    fn from(state: QuickLightingSettings) -> Self {
        let selected_effect_setting=state.cache.iter().find(|c|c.id==state.selected).map(CachedSetting::source);
        let effect_settings=state.cache.into_iter().map(|c| {let mut setting=c.source();setting.effect_id=None;SourceCached {effect_id:c.id,setting}}).collect();
        Self {selected_effect_id:state.selected,selected_effect_setting,effect_settings,custom_colors:state.custom_colors,extensions:state.extensions}
    }
}
impl QuickLightingSettings {
    pub(super) fn apply_sync(&mut self,payload:&Value)->anyhow::Result<bool> {
        #[derive(Deserialize)]
        #[serde(rename_all="camelCase")]
        struct Identity {selected_effect_id:u8}
        #[derive(Deserialize)]
        #[serde(rename_all="camelCase")]
        struct Envelope {effect_settings:SourceSetting,custom_colors:Option<Vec<SourceCustomColor>>}
        let identity:Identity=serde_json::from_value(payload.clone())?;
        let Ok(id)=QuickEffectId::try_from(identity.selected_effect_id) else{return Ok(false);};
        let mut envelope:Envelope=serde_json::from_value(payload.clone())?;
        let source=&mut envelope.effect_settings;
        ensure!(source.effect_id.is_none_or(|effect_id|effect_id==id),"Synchronized quick effect identity does not match its settings");
        // Current ON_SET_EFFECTS calls Rp only for indexDirection + totalIdxDirection.
        // 4115's LeftRight mapping has no WAVE_DIRECTION_ID_AVAILABLE override.
        if let (Some(ix),Some(total))=(source.index_direction,source.total_idx_direction) {
            source.direction=match total {2=>Some(if ix==1 {2}else{1}),4=>Some(ix%2+1),_=>source.direction};
            source.index_direction=None;source.total_idx_direction=None;
        }
        let setting=QuickSetting::parse(id,source)?;
        if let Some(colors)=&envelope.custom_colors {for color in colors {color.validate()?;}}
        if let Some(colors)=envelope.custom_colors {
            let current=self.custom_colors.get_or_insert_with(Vec::new);
            let known= current.iter().map(|entry|entry.color.clone()).collect::<std::collections::BTreeSet<_>>();
            current.extend(colors.into_iter().filter(|entry|!known.contains(&entry.color)));
        }
        self.selected=id;
        self.cache.retain(|cached|cached.id!=id);
        self.cache.push(CachedSetting {id,setting,source:envelope.effect_settings});
        Ok(true)
    }
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
    #[test]
    fn synchronized_effect_replaces_only_its_cache_and_retains_source_extensions() {
        let mut state:QuickLightingSettings=serde_json::from_value(json!({
            "selectedEffectId":1,"profileExtension":{"revision":7},
            "selectedEffectSetting":{"color1":"#112233","oldExtension":true},
            "effectSettings":[{"effectId":4,"direction":2,"cachedExtension":"wave"}],
            "customColors":[{"color":"#112233","picked":false,"slotExtension":2}]
        })).unwrap();
        assert!(state.apply_sync(&json!({"selectedEffectId":2,"effectSettings":{"effectId":2,"color1":"#abcdef","color2":"no-color","isRandom":false,"sourceExtension":{"keep":true}},"customColors":[{"color":"#112233","picked":true},{"color":"#abcdef","picked":true}],"timerTick":7})).unwrap());
        let value=serde_json::to_value(&state).unwrap();
        assert_eq!(value["selectedEffectId"],2);
        assert_eq!(value["profileExtension"],json!({"revision":7}));
        assert_eq!(value["effectSettings"].as_array().unwrap().iter().map(|entry|entry["effectId"].as_u64().unwrap()).collect::<Vec<_>>(),[4,1,2]);
        assert_eq!(value["effectSettings"][0]["cachedExtension"],"wave");
        assert_eq!(value["selectedEffectSetting"]["sourceExtension"],json!({"keep":true}));
        assert!(value["selectedEffectSetting"].get("duration").is_none());
        assert_eq!(value["customColors"],json!([{"color":"#112233","picked":false,"slotExtension":2},{"color":"#abcdef","picked":true}]));
        // Cached effects serialize one effectId even when the incoming selected
        // settings include the vendor envelope's own effectId field.
        let encoded=serde_json::to_string(&state).unwrap();
        let roundtrip:QuickLightingSettings=serde_json::from_str(&encoded).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(),value);
        assert!(state.apply_sync(&json!({"selectedEffectId":1,"effectSettings":{"color1":"#445566"}})).unwrap());
        assert!(serde_json::to_value(state).unwrap()["selectedEffectSetting"].get("oldExtension").is_none());
    }
    #[test]
    fn synchronized_effect_validates_atomically_and_ignores_unmounted_effects() {
        let mut state=QuickLightingSettings::default();
        let before=serde_json::to_value(&state).unwrap();
        assert!(!state.apply_sync(&json!({"selectedEffectId":7,"effectSettings":{"color1":{}}})).unwrap());
        for invalid in [json!({"selectedEffectId":1}),json!({"selectedEffectId":1,"effectSettings":{"color1":"no-color"}}),json!({"selectedEffectId":2,"effectSettings":{"color1":"#00ff00","color2":"no-color","isRandom":false,"duration":4}}),json!({"selectedEffectId":1,"effectSettings":{"color1":"#112233","effectId":4}}),json!({"selectedEffectId":1,"effectSettings":{"color1":"#112233"},"customColors":[{"color":"invalid"}]})] {
            assert!(state.apply_sync(&invalid).is_err());
            assert_eq!(serde_json::to_value(&state).unwrap(),before);
        }
    }
    #[test]
    fn synchronized_direction_uses_only_the_current_receiver_field_pair() {
        let mut state=QuickLightingSettings::default();
        state.apply_sync(&json!({"selectedEffectId":4,"effectSettings":{"effectId":4,"direction":2,"indexDirection":0,"numberOfWaveDirection":2}})).unwrap();
        assert_eq!(state.setting(),QuickSetting::Wave {direction:razer_device::controller_lighting::WaveDirection::Right,speed:None});
        let value=serde_json::to_value(&state).unwrap();
        assert_eq!(value["selectedEffectSetting"]["indexDirection"],0);
        assert_eq!(value["selectedEffectSetting"]["numberOfWaveDirection"],2);
        state.apply_sync(&json!({"selectedEffectId":4,"effectSettings":{"direction":2,"indexDirection":2,"totalIdxDirection":4}})).unwrap();
        assert_eq!(state.setting(),QuickSetting::Wave {direction:razer_device::controller_lighting::WaveDirection::Left,speed:None});
        let value=serde_json::to_value(state).unwrap();
        assert!(value["selectedEffectSetting"].get("indexDirection").is_none());
        assert!(value["selectedEffectSetting"].get("totalIdxDirection").is_none());
        assert!(value["selectedEffectSetting"].get("speed").is_none());
    }
}
