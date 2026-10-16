//! PID1342 main.2623357c.js bP/useWP: complete channel/button mapping.
//! Current CSS .wrapper-Customize owns the 65px label and 38% columns;
//! useWP mounts the 250px product image above one 600px left widget.
use super::audio_mixer_theme::Colors as C;
use super::*;

const PREFIX: &str = "/device/mixerBind/";
const INPUTS: [(&str, &str); 7] = [
    ("mic", "MIC_LOWER"),
    ("chat", "CHAT"),
    ("music", "MUSIC"),
    ("game", "GAME"),
    ("optical", "OPTICAL"),
    ("lineIn", "LINE_IN"),
    ("system", "SYSTEM"),
];
const MUTES: [(&str, &str); 5] = [
    ("muteAll", "MUTE_ALL"),
    ("playbackMix", "MUTE_PLAYBACK_MIX"),
    ("streamMix", "MUTE_STREAM_MIX"),
    ("lineOut", "MUTE_LINE_OUT"),
    ("voiceChat", "MUTE_VOICE_CHAT"),
];
const FUNCTIONS: [(&str, &str); 17] = [
    ("muteAll", "TOGGLE_MUTE_ALL"),
    ("mic", "TOGGLE_MUTE_MIC"),
    ("chat", "TOGGLE_MUTE_CHAT"),
    ("music", "TOGGLE_MUTE_MUSIC"),
    ("game", "TOGGLE_MUTE_GAME"),
    ("optical", "TOGGLE_MUTE_OPTICAL"),
    ("lineIn", "TOGGLE_MUTE_LINE_IN"),
    ("system", "TOGGLE_MUTE_SYSTEM"),
    ("streamMix", "TOGGLE_MUTE_STREAM_MIX"),
    ("playbackMix", "TOGGLE_MUTE_PLAYBACK_MIX"),
    ("lineOut", "TOGGLE_MUTE_LINE_OUT"),
    ("voiceChat", "TOGGLE_MUTE_VOICE_CHAT"),
    ("voiceChanger", "TOGGLE_VOICE_CHANGER"),
    ("echoReverb", "TOGGLE_ECHO_REVERB"),
    ("keyShifter", "TOGGLE_KEY_SHIFTER"),
    ("vocalFading", "TOGGLE_VOCAL_FADING"),
    ("bleep", "BLEEP"),
];

#[derive(Default)]
pub(super) struct State {
    selects: BTreeMap<String, Entity<SelectState<Vec<Choice>>>>,
    /// Keep popup selection/scroll stable while unrelated audio meters update.
    choices: BTreeMap<String, Vec<(String, String)>>,
}

fn choices(key: &str, draft: &Value, presets: &[(String, String)]) -> Vec<(String, String)> {
    let entries: &[(&str, &str)] = if key.starts_with("inputSource/") {
        &INPUTS
    } else if key.starts_with("muteButton/") {
        &MUTES
    } else {
        &FUNCTIONS
    };
    let chat = key
        .strip_prefix("muteButton/")
        .is_some_and(|channel| draft["device"]["mixerBind"]["inputSource"][channel] == "chat");
    let mut choices = entries
        .iter()
        .filter(|(id, _)| {
            !(chat && *id == "voiceChat" || key == "longPressMicMuteButton" && *id == "bleep")
        })
        .map(|(id, label)| ((*id).to_owned(), (*label).to_owned()))
        .collect::<Vec<_>>();
    if matches!(key, "bleepButton" | "longPressMicMuteButton") {
        // bP appends Redux presets in their existing order; it does not sort
        // them like the separate Effects preset chooser.
        choices.extend_from_slice(presets);
    }
    choices
}

fn selector_choices(key: &str, items: &[(String, String)], preset_count: usize) -> Vec<Choice> {
    let builtin_count = if matches!(key, "bleepButton" | "longPressMicMuteButton") {
        items.len() - preset_count
    } else {
        items.len()
    };
    items
        .iter()
        .enumerate()
        .map(|(index, (id, label))| {
            if index < builtin_count {
                Choice::localized(id.clone(), label.clone())
            } else {
                Choice::new(id.clone(), label.clone())
            }
        })
        .collect()
}

fn chat_mute_resets(draft: &Value) -> Vec<String> {
    (1..=4)
        .map(|index| format!("channel{index}"))
        .filter(|channel| {
            draft["device"]["mixerBind"]["inputSource"][channel] == "chat"
                && draft["device"]["mixerBind"]["muteButton"][channel] == "voiceChat"
        })
        .collect()
}

impl AudioProductWorkspace {
    pub(super) fn initialize_mixer_customize(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1342 {
            return;
        }
        let keys = (1..=4)
            .flat_map(|index| {
                [
                    format!("inputSource/channel{index}"),
                    format!("muteButton/channel{index}"),
                ]
            })
            .chain([
                "bleepButton".to_owned(),
                "longPressMicMuteButton".to_owned(),
            ]);
        for key in keys {
            let state = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
            let path = format!("{PREFIX}{key}");
            self.subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing {
                        return;
                    }
                    if let SelectEvent::Confirm(Some(value)) = event {
                        this.edit(&path, json!(value.as_str()), window, cx);
                    }
                },
            ));
            self.mixer_customize.selects.insert(key, state);
        }
    }

    pub(super) fn sync_mixer_customize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 1342 {
            return;
        }
        // bP.u dispatches S(channel,0) when the mounted source sees chat +
        // voiceChat. Do it in the state update phase, never during rendering.
        if self.page == "TAB_CUSTOMIZE" {
            let resets = chat_mute_resets(&self.draft);
            for channel in &resets {
                self.draft["device"]["mixerBind"]["muteButton"][channel] = json!("muteAll");
            }
            if !resets.is_empty() {
                cx.emit(AudioProductChanged);
            }
        }
        let presets = self.mixer_customization_presets();
        for (key, select) in &self.mixer_customize.selects {
            let items = choices(key, &self.draft, &presets);
            let selected = self
                .draft
                .pointer(&format!("{PREFIX}{key}"))
                .and_then(Value::as_str)
                .filter(|value| items.iter().any(|(id, _)| id == value))
                .unwrap_or(&items[0].0)
                .to_owned();
            if self.mixer_customize.choices.get(key) != Some(&items) {
                select.update(cx, |select, cx| {
                    select.set_items(selector_choices(key, &items, presets.len()), window, cx)
                });
                self.mixer_customize.choices.insert(key.clone(), items);
            }
            if select.read(cx).selected_value().map(String::as_str) != Some(selected.as_str()) {
                select.update(cx, |select, cx| {
                    select.set_selected_value(&selected, window, cx)
                });
            }
        }
    }

    fn customize_select(&self, key: &str, label: String) -> AnyElement {
        let state = self
            .mixer_customize
            .selects
            .get(key)
            .expect("retained mixer mapping chooser");
        Select::new(state)
            .accessibility_label(label)
            .w_full()
            .small()
            .h(surface::css(27.))
            .into_any_element()
    }

    pub(super) fn mixer_customize_page(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let header = h_flex()
            .justify_between()
            .items_center()
            .child(div().w(surface::css(65.)).flex_shrink_0())
            .children(
                ["INPUT_SOURCE", "MUTE_BUTTON_MAPPING"]
                    .into_iter()
                    .map(|key| div().w(relative(0.38)).child(t(key).to_uppercase())),
            );
        let rows = (1..=4).map(|index| {
            h_flex()
                .justify_between()
                .items_center()
                .mt(surface::css(10.))
                .child(
                    div()
                        .w(surface::css(65.))
                        .flex_shrink_0()
                        .child(format!("{} {index}:", t("CHANNEL"))),
                )
                .child(div().w(surface::css(200.)).child(self.customize_select(
                    &format!("inputSource/channel{index}"),
                    format!("{} {index}: {}", t("CHANNEL"), t("INPUT_SOURCE")),
                )))
                .child(div().w(surface::css(200.)).child(self.customize_select(
                    &format!("muteButton/channel{index}"),
                    format!("{} {index}: {}", t("CHANNEL"), t("MUTE_BUTTON_MAPPING")),
                )))
        });
        let footer =
            h_flex()
                .justify_between()
                .mt(surface::css(20.))
                .child(
                    v_flex()
                        .w(surface::css(200.))
                        .child(
                            img("synapse/audio-mixer-bleep-button.svg")
                                .w(surface::css(150.))
                                .h(surface::css(40.)),
                        )
                        .child(div().child(t("BLEEP_BUTTON_FUNCTION").to_uppercase()))
                        .child(div().mt(surface::css(10.)).child(self.customize_select(
                            "bleepButton",
                            t("BLEEP_BUTTON_FUNCTION").to_string(),
                        ))),
                )
                .child(
                    v_flex()
                        .w(surface::css(200.))
                        .child(
                            img("synapse/audio-mixer-mic-mute-button.svg")
                                .w(surface::css(150.))
                                .h(surface::css(40.)),
                        )
                        .child(div().child(t("LONG_PRESS").to_uppercase()))
                        .child(div().child(t("MIC_MUTE_BUTTON").to_uppercase()))
                        .child(div().mt(surface::css(10.)).child(self.customize_select(
                            "longPressMicMuteButton",
                            format!("{} {}", t("LONG_PRESS"), t("MIC_MUTE_BUTTON")),
                        ))),
                );
        v_flex()
            .w_full()
            .text_color(C::text())
            .text_size(surface::css(14.))
            .child(
                div()
                    .relative()
                    .flex()
                    .justify_center()
                    .items_center()
                    .w_full()
                    .h(surface::css(250.))
                    .max_w(surface::css(1220.))
                    .min_w(surface::css(1024.))
                    .mx_auto()
                    .my(surface::css(10.))
                    .child(surface::dot_background(cx))
                    .child(
                        img(if window.scale_factor() > 1. {
                            "synapse/audio-mixer-product-3x.png"
                        } else {
                            "synapse/audio-mixer-product-1x.png"
                        })
                        .relative()
                        .w(surface::css(325.))
                        .h(surface::css(250.))
                        .object_fit(ObjectFit::Contain),
                    ),
            )
            .child(
                surface::page_columns().child(surface::page_column(
                    surface::panel_with_help(
                        t("CHANNEL_BUTTON_MAPPING"),
                        surface::help_control(
                            "mixer-customize-help",
                            t("CHANNEL_BUTTON_MAPPING_TOOLTIP"),
                        ),
                        cx,
                    )
                    .child(header)
                    .children(rows)
                    .child(footer),
                )),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{chat_mute_resets, choices, selector_choices};
    use gpui_kit::component::select::SelectItem;
    use serde_json::{Value, json};
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn chat_only_removes_voice_chat_and_resets_the_conflicting_mute() {
        let draft = json!({"device":{"mixerBind":{"inputSource":{"channel1":"chat","channel2":"mic"},"muteButton":{"channel1":"voiceChat","channel2":"voiceChat"}}}});
        assert_eq!(chat_mute_resets(&draft), ["channel1"]);
        assert_eq!(choices("muteButton/channel1", &draft, &[]).len(), 4);
        assert_eq!(choices("muteButton/channel2", &draft, &[]).len(), 5);
    }
    #[test]
    fn button_lists_preserve_preset_identity_and_source_order() {
        let presets = vec![("uuid-z".into(), "Z".into()), ("uuid-a".into(), "A".into())];
        let bleep = choices("bleepButton", &Value::Null, &presets);
        let mic = choices("longPressMicMuteButton", &Value::Null, &presets);
        assert_eq!(bleep[16].0, "bleep");
        assert_eq!(&bleep[17..], &presets);
        assert!(!mic.iter().any(|(id, _)| id == "bleep"));
        assert_eq!(&mic[16..], &presets);
    }

    #[test]
    fn retained_mapping_titles_localize_builtins_but_never_preset_names() {
        let presets = vec![("mic".into(), "TOGGLE_MUTE_MIC".into())];
        for key in ["bleepButton", "longPressMicMuteButton"] {
            let items = choices(key, &Value::Null, &presets);
            let retained = selector_choices(key, &items, presets.len());
            for locale in ["en", "zh-CN"] {
                for (item, (id, label)) in retained.iter().zip(&items).take(items.len() - 1) {
                    assert_eq!(item.value(), id);
                    assert_eq!(
                        item.title_for_locale(locale).as_ref(),
                        razer_i18n::t_locale(label, locale)
                    );
                }
                let preset = retained.last().unwrap();
                assert_eq!(preset.value(), "mic");
                assert_eq!(preset.title_for_locale(locale).as_ref(), "TOGGLE_MUTE_MIC");
            }
        }
    }
}
