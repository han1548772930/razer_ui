use super::*;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    radio::Radio,
    select::{Select, SelectEvent, SelectState},
};

pub(super) struct QuickMacroSaved(pub(super) Value);
impl EventEmitter<QuickMacroSaved> for QuickMacroEditor {}

pub(super) struct QuickMacroEditor {
    catalog_names: Vec<String>,
    name: Entity<InputState>,
    value: Entity<InputState>,
    website: Entity<InputState>,
    program_path: String,
    picker_pending: bool,
    picker_generation: u64,
    picker_task: Option<Task<()>>,
    picker_error: Option<String>,
    text: Entity<TextareaState>,
    type_select: Entity<SelectState<Vec<Choice>>>,
    kind: String,
    launch_kind: String,
    keys: Vec<String>,
    capture: FocusHandle,
    subscriptions: Vec<Subscription>,
}

impl QuickMacroEditor {
    pub(super) fn new(names: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let next_name = Self::next_name_for(&names);
        let name = cx.new(|cx| InputState::new(window, cx).default_value(next_name));
        let value = cx.new(|cx| InputState::new(window, cx));
        let website = cx.new(|cx| InputState::new(window, cx));
        let text = cx.new(|cx| TextareaState::new(window, cx));
        let type_select = cx.new(|cx| SelectState::new(Self::type_choices(), None, window, cx));
        let mut this = Self {
            catalog_names: names,
            name,
            value,
            website,
            program_path: String::new(),
            picker_pending: false,
            picker_generation: 0,
            picker_task: None,
            picker_error: None,
            text,
            type_select,
            kind: String::new(),
            launch_kind: "PROGRAM".into(),
            keys: vec![],
            capture: cx.focus_handle(),
            subscriptions: vec![],
        };
        this.subscriptions
            .push(cx.observe(&this.name, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.value, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.website, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.text, |_, _, cx| cx.notify()));
        this.subscriptions.push(cx.subscribe_in(
            &this.text,
            window,
            |_this, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    if value.encode_utf16().count() > 250 {
                        let mut length = 0;
                        let clipped = value
                            .chars()
                            .take_while(|character| {
                                length += character.len_utf16();
                                length <= 250
                            })
                            .collect::<String>();
                        input.update(cx, |state, cx| state.set_value(clipped, window, cx));
                    }
                    cx.notify();
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.type_select,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(kind)) = event {
                    this.set_kind(kind, window, cx);
                }
            },
        ));
        this
    }

    fn type_choices() -> Vec<Choice> {
        vec![
            Choice::new("keyboard", "Keyboard function"),
            Choice::new("launch", "Launch"),
            Choice::new("runCommand", "Run command"),
            Choice::new("text", "Text function"),
        ]
    }

    fn next_name_for(names: &[String]) -> String {
        for index in 1..=names.len() + 1 {
            let candidate = format!("Macro {index}");
            if !names.iter().any(|name| name == &candidate) {
                return candidate;
            }
        }
        "Macro 1".into()
    }

    fn unique_name(&self, raw: &str) -> String {
        let mut candidate = raw.trim().to_owned();
        for _ in 0..1000 {
            if !self.catalog_names.contains(&candidate) {
                return candidate;
            }
            candidate = if let Some((prefix, index)) = candidate.rsplit_once(' ')
                && let Ok(index) = index.parse::<u64>()
                && let Some(index) = index.checked_add(1)
            {
                format!("{prefix} {index}")
            } else {
                format!("{candidate} 1")
            };
        }
        format!(
            "{candidate}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_millis())
                .unwrap_or_default()
        )
    }

    fn ready(&self, cx: &Context<Self>) -> bool {
        if self.picker_pending || self.name.read(cx).value().trim().is_empty() {
            return false;
        }
        match self.kind.as_str() {
            "keyboard" => !self.keys.is_empty(),
            "launch" if self.launch_kind == "PROGRAM" => !self.program_path.trim().is_empty(),
            "launch" => !self.website.read(cx).value().trim().is_empty(),
            "runCommand" => !self.value.read(cx).value().trim().is_empty(),
            "text" => !self.text.read(cx).value().trim().is_empty(),
            _ => false,
        }
    }

    fn capture_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.kind != "keyboard" {
            return;
        }
        let key = match event.keystroke.key.as_str() {
            "meta" => "Windows".into(),
            "control" => "Ctrl".into(),
            "alt" => "Alt".into(),
            "shift" => "Shift".into(),
            " " => "Space".into(),
            "arrowup" => "Arrow Up".into(),
            "arrowdown" => "Arrow Down".into(),
            "arrowleft" => "Arrow Left".into(),
            "arrowright" => "Arrow Right".into(),
            value if value.len() == 1 => value.to_uppercase(),
            value => {
                let mut chars = value.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            }
        };
        if self.keys.len() < 10 && !self.keys.iter().any(|value| value == &key) {
            self.keys.push(key);
            cx.notify();
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let name = self.unique_name(&self.name.read(cx).value());
        let value = self.value.read(cx).value().trim().to_owned();
        let text = self.text.read(cx).value().to_owned();
        let id = format!(
            "local-macro-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default()
        );
        let mut result = json!({"id":id,"name":name,"type":self.kind});
        match self.kind.as_str() {
            "keyboard" => {
                result["keys"] = json!(self.keys);
                result["description"] = json!(self.keys.join(" + "));
            }
            "launch" => {
                result["mode"] = json!(self.launch_kind.to_lowercase());
                let value = if self.launch_kind == "PROGRAM" {
                    self.program_path.trim().to_owned()
                } else {
                    self.website.read(cx).value().trim().to_owned()
                };
                let target = if self.launch_kind == "WEBSITE"
                    && !value.starts_with("http://")
                    && !value.starts_with("https://")
                {
                    format!("https://{value}")
                } else {
                    value
                };
                result["target"] = json!(target);
            }
            "runCommand" => result["command"] = json!(value),
            "text" => result["text"] = json!(text),
            _ => {}
        }
        cx.emit(QuickMacroSaved(result));
        window.close_dialog(cx);
    }

    fn set_kind(&mut self, kind: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.kind = kind.into();
        self.clear(window, cx);
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A picker response from before Clear or a type change must not
        // repopulate the new draft. The native picker itself remains open.
        self.picker_generation = self.picker_generation.wrapping_add(1);
        self.picker_error = None;
        self.program_path.clear();
        self.launch_kind = "PROGRAM".into();
        self.keys.clear();
        self.value
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.website
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.text
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    fn browse_program(&mut self, cx: &mut Context<Self>) {
        if self.kind != "launch" || self.launch_kind != "PROGRAM" || self.picker_pending {
            return;
        }
        let generation = self.picker_generation;
        self.picker_pending = true;
        self.picker_error = None;
        // Current aH.ie requests one .exe. GPUI's path prompt has no extension
        // filter, so validate the returned path before changing the draft.
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(format!("{} (.exe)", text("PROGRAM")).into()),
        });
        self.picker_task = Some(cx.spawn(async move |view, cx| {
            let result = picker.await;
            let _ = view.update(cx, |this, cx| {
                this.picker_pending = false;
                this.picker_task = None;
                if this.picker_generation == generation && this.kind == "launch" {
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.first() {
                                if path.extension().and_then(|value| value.to_str())
                                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                                {
                                    // Store only the path. Never inspect or launch the executable.
                                    this.program_path = path.to_string_lossy().into_owned();
                                } else {
                                    this.picker_error = Some(Self::picker_message(
                                        "请选择 .exe 文件。原路径已保留。",
                                        "Choose an .exe file. The previous path is retained.",
                                    ));
                                }
                            }
                        }
                        Ok(Ok(None)) => {} // Native Cancel preserves all draft fields.
                        _ => this.picker_error = Some(Self::picker_message(
                            "无法打开文件选择器。原路径已保留，请重试。",
                            "Could not open the file picker. The previous path is retained; try again.",
                        )),
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn picker_message(zh: &str, en: &str) -> String {
        if i18n::locale().to_ascii_lowercase().starts_with("zh") {
            zh.into()
        } else {
            en.into()
        }
    }

    fn launch_field(&self, cx: &Context<Self>) -> AnyElement {
        let program = self.launch_kind == "PROGRAM";
        v_flex()
            .w_full()
            .min_w_0()
            .gap(surface::css(10.))
            .child(
                v_flex()
                    .gap(surface::css(6.))
                    .child(
                        Radio::new("quick-macro-launch-PROGRAM")
                            .label(text("PROGRAM"))
                            .checked(program)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.launch_kind = "PROGRAM".into();
                                cx.notify();
                            })),
                    )
                    .child(
                        gpui_kit::base::Button::new("quick-macro-program-browse")
                            .accessibility_label(format!(
                                "{}: {}",
                                text("PROGRAM"),
                                self.program_path
                            ))
                            .ml(surface::css(30.))
                            .h(surface::css(27.))
                            .flex()
                            .items_center()
                            .px(surface::css(5.))
                            .gap(surface::css(5.))
                            .border_1()
                            .border_color(Colors::input_border())
                            .hover(|style| style.border_color(Colors::primary()))
                            .focus_visible(|style| style.border_color(Colors::primary()))
                            .disabled(!program || self.picker_pending)
                            .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_left()
                                    .child(self.program_path.clone()),
                            )
                            .child(
                                img("synapse/automation-icon_folder.svg")
                                    .size(surface::css(16.67))
                                    .flex_shrink_0(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.browse_program(cx))),
                    ),
            )
            .child(
                v_flex()
                    .gap(surface::css(6.))
                    .child(
                        Radio::new("quick-macro-launch-WEBSITE")
                            .label(text("WEBSITE"))
                            .checked(!program)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.launch_kind = "WEBSITE".into();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .ml(surface::css(30.))
                            .when(program, |view| view.opacity(0.3))
                            .child(
                                Input::new(&self.website)
                                    .appearance(false)
                                    .aria_label(text("WEBSITE"))
                                    .disabled(program)
                                    .w_full()
                                    .h(surface::css(27.))
                                    .px(surface::css(5.))
                                    .border_1()
                                    .border_color(Colors::input_border()),
                            ),
                    ),
            )
            .when_some(self.picker_error.clone(), |view, error| {
                view.child(
                    div()
                        .text_size(surface::css(12.))
                        .text_color(Colors::danger())
                        .child(error),
                )
            })
            .into_any_element()
    }

    fn field(&self, cx: &Context<Self>) -> AnyElement {
        match self.kind.as_str() {
            "launch" => self.launch_field(cx),
            "keyboard" => div()
                .id("quick-macro-key-capture")
                .track_focus(&self.capture)
                .w_full()
                .min_h(surface::css(38.))
                .px(surface::css(6.))
                .flex()
                .items_center()
                .justify_start()
                .flex_wrap()
                .gap(surface::css(5.))
                .border_1()
                .border_color(Colors::border())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.capture.focus(window, cx);
                    }),
                )
                .on_key_down(
                    cx.listener(|this, event: &KeyDownEvent, _, cx| this.capture_key(event, cx)),
                )
                .children(if self.keys.is_empty() {
                    vec![
                        div()
                            .text_color(Colors::muted())
                            .child("+")
                            .into_any_element(),
                    ]
                } else {
                    self.keys
                        .iter()
                        .map(|key| {
                            let key_to_remove = key.clone();
                            gpui_kit::base::Button::new(SharedString::from(format!(
                                "quick-macro-key-{key}"
                            )))
                            .accessibility_label(format!("Remove {key}"))
                            .px(surface::css(8.))
                            .py(surface::css(6.))
                            .border_1()
                            .rounded(surface::css(4.))
                            .border_color(Colors::input_border())
                            .hover(|style| style.border_color(Colors::danger()))
                            .child(key.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.keys.retain(|key| key != &key_to_remove);
                                cx.notify();
                            }))
                            .into_any_element()
                        })
                        .collect()
                })
                .into_any_element(),
            "" => div()
                .w_full()
                .min_h(surface::css(38.))
                .opacity(0.3)
                .child(
                    div()
                        .px(surface::css(8.))
                        .py(surface::css(6.))
                        .border_1()
                        .border_color(Colors::border())
                        .child("+"),
                )
                .into_any_element(),
            "text" => Textarea::new(&self.text)
                .appearance(false)
                .w_full()
                .h(surface::css(54.))
                .p(surface::css(5.))
                .bg(Colors::panel())
                .border_1()
                .border_color(Colors::input_border())
                .into_any_element(),
            _ => Input::new(&self.value)
                .appearance(false)
                .w_full()
                .h(surface::css(27.))
                .p(surface::css(5.))
                .bg(Colors::panel())
                .border_1()
                .border_color(Colors::input_border())
                .into_any_element(),
        }
    }

    fn section_icon(&self) -> AnyElement {
        if self.kind.is_empty() {
            return div()
                .w(surface::css(20.))
                .h(surface::css(20.))
                .opacity(0.3)
                .into_any_element();
        }
        img(SharedString::from(format!(
            "synapse/automation-quick-macro-{}.svg",
            self.kind
        )))
        .size(surface::css(20.))
        .into_any_element()
    }
}

impl Render for QuickMacroEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value_len = self.text.read(cx).value().encode_utf16().count();
        v_flex()
            .id("quick-macro-popup")
            .w(surface::css(500.))
            .h(surface::css(369.))
            .when(self.picker_error.is_some(), |view| {
                view.h(surface::css(409.))
            })
            .p(surface::css(20.))
            .bg(Colors::panel())
            .border_1()
            .border_color(Colors::primary())
            .font_family("Roboto")
            .text_color(Colors::foreground())
            .text_size(surface::css(14.))
            .relative()
            .child(
                gpui_kit::base::Button::new("quick-macro-close")
                    .accessibility_label(text("CLOSE"))
                    .absolute()
                    .right_0()
                    .top_0()
                    .size(surface::css(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|style| style.bg(Colors::close_hover()))
                    .child(img("synapse/automation-icon_close.svg").size(surface::css(20.)))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
            .child(
                h_flex().justify_between().mb(surface::css(24.)).child(
                    div()
                        .font_family("RazerF5")
                        .text_color(Colors::primary())
                        .text_center()
                        .flex_1()
                        .child(text("ADD_QUICK_MACRO").to_uppercase()),
                ),
            )
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(surface::css(8.))
                            .child(
                                div()
                                    .text_color(Colors::muted())
                                    .child(text("NAME").to_uppercase()),
                            )
                            .child(
                                Input::new(&self.name)
                                    .appearance(false)
                                    .w_full()
                                    .h(surface::css(27.))
                                    .p(surface::css(5.))
                                    .bg(Colors::panel())
                                    .border_1()
                                    .border_color(Colors::input_border()),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(surface::css(8.))
                            .child(
                                div()
                                    .text_color(Colors::muted())
                                    .child(text("TYPE").to_uppercase()),
                            )
                            .child(
                                Select::new(&self.type_select)
                                    .w_full()
                                    .h(surface::css(27.))
                                    .placeholder("Select a type of macro")
                                    .accessibility_label("Macro type"),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .justify_between()
                    .child(div().text_color(Colors::muted()).child(text("ACTION")))
                    .child(
                        gpui_kit::base::Button::new("quick-macro-clear")
                            .accessibility_label(text("CLEAR"))
                            .disabled(
                                self.keys.is_empty()
                                    && self.program_path.is_empty()
                                    && self.website.read(cx).value().is_empty()
                                    && (self.kind != "launch" || self.launch_kind == "PROGRAM")
                                    && self.value.read(cx).value().is_empty()
                                    && self.text.read(cx).value().is_empty(),
                            )
                            .text_color(Colors::foreground())
                            .hover(|style| style.text_color(Colors::primary()))
                            .child(text("CLEAR"))
                            .on_click(cx.listener(|this, _, window, cx| this.clear(window, cx))),
                    ),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(surface::css(10.))
                    .child(self.section_icon())
                    .child(self.field(cx)),
            )
            .when(self.kind == "text", |view| {
                view.child(
                    div()
                        .text_color(Colors::muted())
                        .text_size(surface::css(12.))
                        .child(format!("{value_len}/250")),
                )
            })
            .child(
                h_flex()
                    .mt_auto()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        command("quick-macro-cancel", text("CANCEL"), false, false)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        command("quick-macro-save", text("SAVE"), true, !self.ready(cx))
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}
