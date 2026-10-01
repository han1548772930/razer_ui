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
pub(crate) struct Choice {
    id: String,
    title: SharedString,
}
impl Choice {
    pub(crate) fn new(id: impl Into<String>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
}
impl SelectItem for Choice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn value(&self) -> &String {
        &self.id
    }
}
pub(super) type Choices = SelectState<Vec<Choice>>;
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
    pub(super) mapping_rate: Entity<InputState>,
    pub(super) mapping_sliders: [Entity<SliderState>; 3],
    pub(super) snap_left: Entity<InputState>,
    pub(super) snap_right: Entity<InputState>,
    pub(super) color_boost: Entity<InputState>,
    pub(super) color: Entity<ColorPickerState>,
    pub(super) color2: Entity<ColorPickerState>,
}
