//! Retained framework controls. Domain values remain in ProfileSettings.
use gpui_kit::component::{
    color_picker::ColorPickerState,
    input::{InputState, TextareaState},
    select::{SelectItem, SelectState},
    slider::SliderState,
};
use gpui_kit::*;
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Choice {
    id: String,
    title: SharedString,
    source_key: Option<String>,
    translations: BTreeMap<String, String>,
}
impl Choice {
    pub fn new(id: impl Into<String>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            source_key: None,
            translations: BTreeMap::new(),
        }
    }
    pub fn localized(id: impl Into<String>, source_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: SharedString::default(),
            source_key: Some(source_key.into()),
            translations: BTreeMap::new(),
        }
    }
    pub(super) fn localized_with_translations(
        id: impl Into<String>,
        source_key: impl Into<String>,
        translations: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        let mut choice = Self::localized(id, source_key);
        choice.translations = translations.into_iter().collect();
        choice
    }
    pub(super) fn title_for_locale(&self, locale: &str) -> SharedString {
        let normalized = locale.replace('_', "-");
        if let Some(title) = self
            .translations
            .iter()
            .find(|(language, _)| language.eq_ignore_ascii_case(&normalized))
            .map(|(_, title)| title)
            .or_else(|| self.translations.get("en"))
        {
            return title.clone().into();
        }
        self.source_key
            .as_ref()
            .map(|key| razer_i18n::t_locale(key, locale).into())
            .unwrap_or_else(|| self.title.clone())
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
}
impl SelectItem for Choice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.title_for_locale(&razer_i18n::locale())
    }
    fn value(&self) -> &String {
        &self.id
    }
}
pub(super) type Choices = SelectState<Vec<Choice>>;
#[cfg(test)]
mod locale_tests {
    use super::Choice;
    use gpui_kit::component::select::SelectItem;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn retained_source_choices_translate_without_changing_value_or_custom_names() {
        let choice = Choice::localized("cancel-action", "CANCEL");
        let selected = choice.clone();
        for locale in ["zh-CN", "zh-cn", "ZH_CN"] {
            assert_eq!(selected.title_for_locale(locale).as_ref(), "取消");
            assert_eq!(selected.value(), "cancel-action");
        }
        assert_eq!(selected.title_for_locale("en").as_ref(), "Cancel");
        let custom = Choice::new("user-preset", "CANCEL");
        assert_eq!(custom.title_for_locale("zh-CN").as_ref(), "CANCEL");
        assert_eq!(custom.value(), "user-preset");
        let product = Choice::localized_with_translations(
            "source-mode",
            "PRODUCT_MODE",
            [
                ("en".into(), "Source mode".into()),
                ("zh-CN".into(), "源模式".into()),
            ],
        );
        assert_eq!(product.title_for_locale("zh-cn").as_ref(), "源模式");
        assert_eq!(product.title_for_locale("de").as_ref(), "Source mode");
        assert_eq!(product.value(), "source-mode");
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Control {
    Volume,
    Tracking,
    Lift,
    Landing,
    Idle,
    LowPower,
    Brightness,
    LightingIdle,
    EffectDuration,
    Audio(u32),
    Mic(u32),
}
pub(super) struct Controls {
    pub(super) sliders: BTreeMap<Control, Entity<SliderState>>,
    pub(super) profile: Entity<Choices>,
    pub(super) effect: Entity<Choices>,
    pub(super) mapping: Entity<Choices>,
    pub(super) mapping_text: Entity<InputState>,
    pub(super) mapping_paragraph: Entity<TextareaState>,
    pub(super) mapping_group: Entity<Choices>,
    pub(super) mapping_x: Entity<InputState>,
    pub(super) mapping_y: Entity<InputState>,
    pub(super) mapping_rate: Entity<razer_widgets::stepper::Stepper>,
    pub(super) mapping_sliders: [Entity<SliderState>; 3],
    pub(super) snap_left: Entity<InputState>,
    pub(super) snap_right: Entity<InputState>,
    pub(super) color_boost: Entity<InputState>,
    pub(super) color: Entity<ColorPickerState>,
    pub(super) color2: Entity<ColorPickerState>,
}
