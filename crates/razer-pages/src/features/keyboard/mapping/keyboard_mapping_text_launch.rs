//! Mounted 48693 MapLaunch and MapText text payload, for analog mapping slots.
use super::*;
#[path = "keyboard_mapping_text_emoji.rs"]
mod text_emoji;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState, TextareaState},
    radio::Radio,
};

#[derive(Default)]
pub(super) struct State {
    slots: [Controls; 2],
    generation: u64,
}
#[derive(Default)]
struct Controls {
    text: Option<Entity<TextareaState>>,
    editor: Option<Entity<text_emoji::MappingTextEditor>>,
    path: Option<Entity<InputState>>,
    url: Option<Entity<InputState>>,
    mode: Option<bool>, // Some(true) = program, Some(false) = website.
    subscriptions: Vec<Subscription>,
}
impl State {
    pub(super) fn clear(&mut self) {
        self.slots = Default::default();
        self.generation = self.generation.wrapping_add(1);
    }
}
#[cfg(test)]
fn clip_text(value: &str) -> String {
    let mut units = 0;
    value
        .chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= 250
        })
        .collect()
}
fn normalize_url(value: &str) -> String {
    // 31867.Gz preserves its original alternation/anchor precedence.
    static VALID: OnceLock<regex::Regex> = OnceLock::new();
    let valid = VALID.get_or_init(|| regex::Regex::new(
        r"(?i)^(https?://)?((([a-z\d]([a-z\d-]*[a-z\d])*)\.)+[a-z]{2,}|((\d{1,3}\.){3}\d{1,3}))|localhost(:\d+)?(/[-a-z\d%_.~+]*)*(\?[;&a-z\d%_.~+=-]*)?(#[-a-z\d_]*)?$"
    ).expect("source URL expression"));
    if !value.starts_with("http://") && !value.starts_with("https://") && valid.is_match(value) {
        format!("https://{value}")
    } else {
        value.to_owned()
    }
}
pub(super) fn prepare_save(edit: &mut Edit) {
    for slot in &mut edit.slots {
        if let Some(url) = slot
            .staged
            .pointer("/launchGroup/url")
            .and_then(Value::as_str)
        {
            slot.staged["launchGroup"]["url"] = json!(normalize_url(url));
        }
    }
}
pub(super) fn valid(slot: &Slot) -> bool {
    match slot.category.as_str() {
        "TEXT_FUNCTION" => slot
            .staged
            .pointer("/textBlockGroup/text")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty()),
        "LAUNCH_PROGRAM" => {
            let group = &slot.staged["launchGroup"];
            group
                .get("path")
                .or_else(|| group.get("url"))
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
        }
        _ => true,
    }
}
impl KeyboardProductWorkspace {
    pub(super) fn init_mapping_text_launch(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.analog_gamepad.mapping.text_launch.generation = self
            .analog_gamepad
            .mapping
            .text_launch
            .generation
            .wrapping_add(1);
        let mut controls = Controls::default();
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let entry = &edit.slots[slot];
        if entry.category == "TEXT_FUNCTION" {
            let value = entry
                .staged
                .pointer("/textBlockGroup/text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let input = cx.new(|cx| {
                let mut state = TextareaState::new(window, cx).placeholder(t("ENTER_TEXT"));
                state.set_value(value, window, cx);
                state
            });
            controls.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, event, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let value = input.read(cx).value().to_string();
                    let previous = this
                        .analog_gamepad
                        .mapping
                        .edit
                        .as_ref()
                        .and_then(|edit| edit.slots[slot].staged.pointer("/textBlockGroup/text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let clipped = if let Some((range, limited)) =
                        crate::features::shortcuts::limit_shortcut_text(previous, &value)
                    {
                        input.update(cx, |input, cx| {
                            let caret = range.start;
                            input.set_selected_range(range, cx);
                            input.replace("", window, cx);
                            input.set_selected_range(caret..caret, cx);
                        });
                        limited
                    } else {
                        value
                    };
                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit
                        && edit.slots[slot].category == "TEXT_FUNCTION"
                    {
                        edit.slots[slot].staged["textBlockGroup"]["text"] = json!(clipped);
                        cx.emit(KeyboardProductChanged);
                        cx.notify();
                    }
                },
            ));
            let editor = cx.new(|cx| text_emoji::MappingTextEditor::new(input.clone(), window, cx));
            controls.subscriptions.push(cx.subscribe(
                &editor,
                move |this, _, event: &text_emoji::TextChanged, cx| {
                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit
                        && edit.slots[slot].category == "TEXT_FUNCTION"
                    {
                        edit.slots[slot].staged["textBlockGroup"]["text"] = json!(event.0);
                        cx.emit(KeyboardProductChanged);
                        cx.notify();
                    }
                },
            ));
            controls.subscriptions.push(cx.subscribe(
                &editor,
                |_, _, _: &text_emoji::CharacterMapRequested, cx| {
                    cx.emit(KeyboardCharacterMapRequested);
                },
            ));
            controls.text = Some(input);
            controls.editor = Some(editor);
        } else if entry.category == "LAUNCH_PROGRAM" {
            let group = &entry.staged["launchGroup"];
            controls.mode = if group.get("path").is_some() {
                Some(true)
            } else if group.get("url").is_some() {
                Some(false)
            } else {
                None
            };
            for program in [true, false] {
                let value = group[if program { "path" } else { "url" }]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                let input = cx.new(|cx| {
                    let mut state = InputState::new(window, cx);
                    state.set_value(value, window, cx);
                    state
                });
                if program {
                    controls.path = Some(input);
                } else {
                    controls.subscriptions.push(cx.subscribe_in(
                        &input,
                        window,
                        move |this, _, event, _, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.stage_mapping_launch_value(slot, cx);
                            }
                        },
                    ));
                    controls.url = Some(input);
                }
            }
        }
        self.analog_gamepad.mapping.text_launch.slots[slot] = controls;
    }
    fn stage_mapping_launch_value(&mut self, slot: usize, cx: &mut Context<Self>) {
        let controls = &self.analog_gamepad.mapping.text_launch.slots[slot];
        let Some(program) = controls.mode else { return };
        let input = if program { &controls.path } else { &controls.url };
        let Some(input) = input else { return };
        let value = input.read(cx).value().to_string();
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit
            && edit.slots[slot].category == "LAUNCH_PROGRAM"
        {
            let group = if program {
                json!({"path":value})
            } else {
                json!({"url":value})
            };
            two_tap::replace_slot(
                &mut edit.slots[slot],
                json!({"outputType":"launchGroup","launchGroup":group}),
            );
            cx.emit(KeyboardProductChanged);
            cx.notify();
        }
    }
    fn browse_analog_mapping_program(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let state = &self.analog_gamepad.mapping.text_launch;
        if state.slots[slot].mode != Some(true) {
            return;
        }
        let identity = (state.generation, edit.button["inputID"].clone(), edit.shift);
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = picker.await
                && let Some(path) = paths.first()
            {
                // GPUI's platform picker currently lacks the source .exe filter.
                if !path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
                {
                    return;
                }
                let value = path.to_string_lossy().into_owned();
                let _ = view.update_in(cx, |this, window, cx| {
                    let state = &this.analog_gamepad.mapping.text_launch;
                    if state.generation != identity.0
                        || state.slots[slot].mode != Some(true)
                        || !this
                            .analog_gamepad
                            .mapping
                            .edit
                            .as_ref()
                            .is_some_and(|edit| {
                                edit.button["inputID"] == identity.1
                                    && edit.shift == identity.2
                                    && edit.slots[slot].category == "LAUNCH_PROGRAM"
                            })
                    {
                        return;
                    }
                    if let Some(input) = state.slots[slot].path.clone() {
                        input.update(cx, |input, cx| input.set_value(value, window, cx));
                        this.stage_mapping_launch_value(slot, cx);
                    }
                });
            }
        })
        .detach();
    }
}
pub(super) fn element(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    cx: &Context<KeyboardProductWorkspace>,
) -> Option<AnyElement> {
    let edit = workspace.analog_gamepad.mapping.edit.as_ref()?;
    let controls = &workspace.analog_gamepad.mapping.text_launch.slots[slot];
    if edit.slots[slot].category == "TEXT_FUNCTION" {
        let editor = controls.editor.as_ref()?;
        return Some(
            v_flex()
                .mx(surface::css(20.))
                .mb(surface::css(10.))
                .child(editor.clone())
                .into_any_element(),
        );
    }
    if edit.slots[slot].category != "LAUNCH_PROGRAM" {
        return None;
    }
    let (path, url) = (controls.path.as_ref()?, controls.url.as_ref()?);
    let program = controls.mode == Some(true);
    let website = controls.mode == Some(false);
    Some(
        v_flex()
            .mx(surface::css(20.))
            .mb(surface::css(10.))
            .child(
                Radio::new(SharedString::from(format!("analog-launch-program-{slot}")))
                    .label(t("PROGRAM"))
                    .checked(program)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.analog_gamepad.mapping.text_launch.slots[slot].mode = Some(true);
                        this.stage_mapping_launch_value(slot, cx);
                    })),
            )
            .child(
                h_flex()
                    .ml(surface::css(30.))
                    .mb(surface::css(20.))
                    .child(razer_widgets::keyboard_navigation::focus_input(
                        Input::new(path)
                            .readonly(true)
                            .disabled(!program)
                            .w(surface::css(170.))
                            .h(surface::css(25.)),
                        path,
                        cx,
                    ))
                    .child(
                        BaseButton::new(SharedString::from(format!("analog-launch-browse-{slot}")))
                            .disabled(!program)
                            .ml(surface::css(-25.))
                            .size(surface::css(20.))
                            .child(
                                razer_assets::resource_svg()
                                    .path("synapse/macro/launch-folder.svg")
                                    .size_full(),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.browse_analog_mapping_program(slot, window, cx)
                            })),
                    ),
            )
            .child(
                Radio::new(SharedString::from(format!("analog-launch-website-{slot}")))
                    .label(t("WEBSITE"))
                    .checked(website)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.analog_gamepad.mapping.text_launch.slots[slot].mode = Some(false);
                        this.stage_mapping_launch_value(slot, cx);
                    })),
            )
            .child(razer_widgets::keyboard_navigation::focus_input(
                Input::new(url)
                    .disabled(!website)
                    .ml(surface::css(30.))
                    .w(surface::css(170.))
                    .h(surface::css(25.)),
                url,
                cx,
            ))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn text_limit_counts_utf16_and_preserves_pasted_newlines() {
        let value = format!("{}😀x", "a".repeat(249));
        assert_eq!(clip_text(&value), "a".repeat(249));
        assert_eq!(clip_text("line one\nline two"), "line one\nline two");
    }
    #[::core::prelude::v1::test]
    fn source_url_normalization_preserves_case_sensitive_prefix_check() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
        assert_eq!(normalize_url("http://example.com"), "http://example.com");
        assert_eq!(
            normalize_url("HTTP://example.com"),
            "https://HTTP://example.com"
        );
        assert_eq!(normalize_url("not a host"), "not a host");
    }
    #[::core::prelude::v1::test]
    fn empty_text_and_launch_cannot_be_submitted() {
        assert!(!valid(&Slot::new(
            json!({"outputType":"launchGroup","launchGroup":{}})
        )));
        assert!(!valid(&Slot::new(
            json!({"outputType":"launchGroup","launchGroup":{"url":""}})
        )));
        assert!(valid(&Slot::new(
            json!({"outputType":"launchGroup","launchGroup":{"url":"not a host"}})
        )));
        assert!(!valid(&Slot::new(
            json!({"outputType":"textBlockGroup","textBlockGroup":{"text":""}})
        )));
    }
}
