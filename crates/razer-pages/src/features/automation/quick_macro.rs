use super::*;
use gpui_kit::base::motion;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    radio::Radio,
};
use std::time::Duration;

#[derive(Clone, Copy)]
struct MacroType {
    value: &'static str,
    label: &'static str,
}

impl MacroType {
    fn content(self, selected: bool) -> AnyElement {
        h_flex()
            .min_w_0()
            .gap(surface::css(6.))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_color(if selected {
                Colors::primary()
            } else {
                Colors::foreground()
            })
            .child(
                img(SharedString::from(format!(
                    "synapse/automation-quick-macro-{}.svg",
                    self.value
                )))
                .size(surface::css(20.))
                .flex_shrink_0(),
            )
            .child(div().truncate().child(self.label))
            .into_any_element()
    }
}

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
    type_menu_open: bool,
    type_hovered: bool,
    type_focus: FocusHandle,
    type_option_focus: [FocusHandle; 4],
    type_trigger_bounds: Bounds<Pixels>,
    kind: String,
    launch_kind: String,
    keys: Vec<String>,
    capture: FocusHandle,
    capture_input: Entity<InputState>,
    capturing: bool,
    capture_modifiers: Modifiers,
    hovered_key: Option<String>,
    pressed_key: Option<String>,
    subscriptions: Vec<Subscription>,
}

impl QuickMacroEditor {
    pub(super) fn new(names: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let next_name = Self::next_name_for(&names);
        let name = cx.new(|cx| InputState::new(window, cx).default_value(next_name));
        let value = cx.new(|cx| InputState::new(window, cx).placeholder("Insert Command"));
        let website = cx.new(|cx| InputState::new(window, cx));
        let text = cx.new(|cx| TextareaState::new(window, cx).placeholder("Enter Text"));
        let capture_input = cx.new(|cx| InputState::new(window, cx).placeholder("Start typing"));
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
            type_menu_open: false,
            type_hovered: false,
            type_focus: cx.focus_handle(),
            type_option_focus: std::array::from_fn(|_| cx.focus_handle()),
            type_trigger_bounds: Bounds::default(),
            kind: String::new(),
            launch_kind: "PROGRAM".into(),
            keys: vec![],
            capture: cx.focus_handle(),
            capture_input,
            capturing: false,
            capture_modifiers: Modifiers::default(),
            hovered_key: None,
            pressed_key: None,
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
        this
    }

    fn type_choices() -> [MacroType; 4] {
        [
            MacroType {
                value: "keyboard",
                label: "Keyboard function",
            },
            MacroType {
                value: "launch",
                label: "Launch",
            },
            MacroType {
                value: "runCommand",
                label: "Run command",
            },
            MacroType {
                value: "text",
                label: "Text function",
            },
        ]
    }

    fn type_selector(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let choices = Self::type_choices();
        let selected = choices.iter().copied().find(|item| item.value == self.kind);
        let placeholder = text("SELECT_MACRO_TYPE");
        let title = selected.map_or(placeholder.as_str(), |item| item.label);
        let policy =
            || motion::Transition::new(Duration::from_millis(300)).easing(motion::Easing::Ease);
        let border: Hsla = if self.type_menu_open || self.type_hovered {
            Colors::primary()
        } else {
            rgb(0x515151).into()
        };
        let border = motion::transition("quick-macro-type-border", border, policy(), window, cx);
        let angle = motion::transition(
            "quick-macro-type-chevron",
            if self.type_menu_open {
                std::f32::consts::PI
            } else {
                0.
            },
            policy(),
            window,
            cx,
        );
        // CSS fit-content is bounded below by the whole trigger. The source
        // row's 20px SVG + 6px gap + 12px side padding determines max-content.
        let content_width = choices
            .iter()
            .map(|item| surface::label_width(item.label, 14., window) + 20. + 6. + 24. + 2.)
            .fold(0., f32::max);
        let menu_width = self
            .type_trigger_bounds
            .size
            .width
            .max(surface::css(content_width).to_pixels(window.rem_size()));
        let trigger_owner = cx.entity().downgrade();
        let mut selector = div()
            .id("quick-macro-type-select")
            .relative()
            .w_full()
            .h(surface::css(27.))
            .child(
                gpui_kit::base::Button::new("quick-macro-type-control")
                    .track_focus(&self.type_focus)
                    .accessibility_label(format!("{}: {title}", text("TYPE")))
                    .aria_expanded(self.type_menu_open)
                    .relative()
                    .w_full()
                    .h(surface::css(27.))
                    .px(surface::css(6.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(border)
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(Colors::foreground())
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.type_hovered = *hovered;
                        cx.notify();
                    }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.type_menu_open = !this.type_menu_open;
                        cx.notify();
                    }))
                    .child(selected.map_or_else(
                        || {
                            div()
                                .text_color(rgb(0x666666))
                                .truncate()
                                .child(placeholder)
                                .into_any_element()
                        },
                        |item| item.content(false),
                    ))
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(surface::css(29.))
                            .h(surface::css(25.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                svg()
                                    .path("synapse/automation-icon_expand.svg")
                                    .size(surface::css(10.))
                                    .text_color(Colors::muted())
                                    .with_transformation(Transformation::rotate(radians(angle))),
                            ),
                    )
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                let _ = trigger_owner.update(cx, |this, cx| {
                                    let resized = this.type_trigger_bounds.size != bounds.size;
                                    this.type_trigger_bounds = bounds;
                                    if resized && this.type_menu_open {
                                        cx.notify();
                                    }
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    ),
            );
        if self.type_menu_open {
            // aH mounts this only while k is true. Its CSS declares height and
            // max-height transitions, but no closed DOM node or delayed open
            // style exists; retain the immediate mount/unmount rather than
            // introducing a new popup entrance/exit animation.
            let menu = v_flex()
                .id("quick-macro-type-menu")
                .absolute()
                .left_0()
                .top(surface::css(28.))
                .w(menu_width)
                .max_h(surface::css(180.))
                .bg(Colors::black())
                .border_1()
                .border_color(rgb(0x515151))
                .overflow_x_hidden()
                .overflow_y_scroll()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if !this.type_trigger_bounds.contains(&event.position) {
                        this.type_menu_open = false;
                        // A disappearing focused option must not strand GPUI
                        // keyboard dispatch. The clicked field may take focus
                        // later in the same event, as normal native buttons do.
                        if this
                            .type_option_focus
                            .iter()
                            .any(|focus| focus.is_focused(window))
                        {
                            this.type_focus.focus(window, cx);
                        }
                        cx.notify();
                    }
                }))
                .children(choices.into_iter().enumerate().map(|(index, item)| {
                    gpui_kit::base::Button::new(("quick-macro-type-option", index))
                        .track_focus(&self.type_option_focus[index])
                        .accessibility_label(item.label)
                        .aria_selected(self.kind == item.value)
                        .w_full()
                        .min_h(surface::css(30.))
                        .flex_shrink_0()
                        .px(surface::css(12.))
                        .py(surface::css(8.))
                        .flex()
                        .items_center()
                        .gap(surface::css(10.))
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .hover(|style| style.bg(Colors::close_hover()))
                        .child(item.content(self.kind == item.value))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            // Source resets the action even when reselecting
                            // the already-selected type; it has no no-op guard.
                            this.set_kind(item.value, window, cx);
                            this.type_focus.focus(window, cx);
                        }))
                }));
            selector = selector.child(deferred(menu).priority(3));
        }
        selector.into_any_element()
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

    fn capture_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.capturing {
            if event.keystroke.key == "escape" {
                window.close_dialog(cx);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        // aH installs document listeners only during X. Escape is a captured
        // key while X is true; it must not activate the enclosing dialog.
        window.prevent_default();
        cx.stop_propagation();
        let key = match event.keystroke.key.as_str() {
            "meta" => "Windows".into(),
            "control" => "Ctrl".into(),
            "alt" => "Alt".into(),
            "shift" => "Shift".into(),
            " " | "space" => "Space".into(),
            "arrowup" | "up" => "Arrow Up".into(),
            "arrowdown" | "down" => "Arrow Down".into(),
            "arrowleft" | "left" => "Arrow Left".into(),
            "arrowright" | "right" => "Arrow Right".into(),
            value if value.len() == 1 => value.to_uppercase(),
            value => {
                let mut chars = value.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            }
        };
        self.add_key(key, cx);
        // The source temporary input unmounts after the first key. Keep its
        // listener scope focused so the remaining chord and keyup still arrive.
        self.capture.focus(window, cx);
    }

    fn add_key(&mut self, key: String, cx: &mut Context<Self>) {
        if self.keys.len() < 10 && !self.keys.iter().any(|value| value == &key) {
            self.keys.push(key);
            cx.notify();
        }
    }

    fn modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.capturing {
            return;
        }
        let before = self.capture_modifiers;
        self.capture_modifiers = event.modifiers;
        // GPUI Windows reports modifier edges separately from KeyDown/KeyUp.
        // Any release ends the same session as the source document keyup.
        let edges = [
            (before.control, event.control, "Ctrl"),
            (before.alt, event.alt, "Alt"),
            (before.shift, event.shift, "Shift"),
            (before.platform, event.platform, "Windows"),
        ];
        if edges.iter().any(|(old, new, _)| *old && !*new) {
            self.capturing = false;
            cx.notify();
            return;
        }
        for (old, new, name) in edges {
            if !old && new {
                self.add_key(name.into(), cx);
                self.capture.focus(window, cx);
            }
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
        self.type_menu_open = false;
        self.picker_error = None;
        self.program_path.clear();
        self.launch_kind = "PROGRAM".into();
        self.keys.clear();
        self.capturing = false;
        self.hovered_key = None;
        self.pressed_key = None;
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

    fn keyboard_field(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.kind.is_empty();
        let mut field = h_flex()
            .w_full()
            .items_center()
            .flex_wrap()
            .gap(surface::css(5.))
            .when(disabled, |field| field.opacity(0.3));
        if self.keys.is_empty() && !self.capturing {
            field = field.child(
                gpui_kit::base::Button::new("quick-macro-capture-start")
                    .accessibility_label("Start typing")
                    .disabled(disabled)
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .min_w(surface::css(34.))
                    .min_h(surface::css(34.))
                    .border_1()
                    .rounded(surface::css(4.))
                    .border_color(Colors::muted())
                    .hover(|style| style.border_color(Colors::primary()))
                    .child("+")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.capturing = true;
                        this.capture_modifiers = window.modifiers();
                        this.capture_input
                            .read(cx)
                            .focus_handle(cx)
                            .focus(window, cx);
                        cx.notify();
                    })),
            );
        } else if self.keys.is_empty() {
            field = field.child(
                Input::new(&self.capture_input)
                    .appearance(false)
                    .w_full()
                    .h(surface::css(28.))
                    .p_0(),
            );
        }
        for (index, key) in self.keys.iter().enumerate() {
            if index > 0 {
                field = field.child(div().text_size(surface::css(14.)).child("+"));
            }
            let hovered = self.hovered_key.as_ref() == Some(key);
            let pressed = self.pressed_key.as_ref() == Some(key);
            let policy =
                || motion::Transition::new(Duration::from_millis(200)).easing(motion::Easing::Ease);
            let border_target: Hsla = if pressed {
                rgb(0x4e4e4e)
            } else if hovered {
                rgb(0xff4a4a)
            } else {
                rgb(0x5d5d5d)
            }
            .into();
            let border = motion::transition(
                (
                    SharedString::from(format!("quick-macro-pill-{key}")),
                    "border",
                ),
                border_target,
                policy(),
                window,
                cx,
            );
            let delete_opacity = motion::transition(
                (
                    SharedString::from(format!("quick-macro-pill-{key}")),
                    "opacity",
                ),
                if hovered { 1.0_f32 } else { 0.0_f32 },
                policy(),
                window,
                cx,
            );
            let delete_target: Hsla = if pressed {
                rgb(0x4e4e4e)
            } else {
                rgb(0xfd4949)
            }
            .into();
            let delete_color = motion::transition(
                (
                    SharedString::from(format!("quick-macro-pill-{key}")),
                    "delete-color",
                ),
                delete_target,
                policy(),
                window,
                cx,
            );
            let hover_key = key.clone();
            let press_key = key.clone();
            let remove_key = key.clone();
            field = field.child(
                gpui_kit::base::Button::new(SharedString::from(format!("quick-macro-key-{key}")))
                    .accessibility_label(format!("Remove {key}"))
                    .relative()
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .min_w(surface::css(34.))
                    .min_h(surface::css(34.))
                    .text_size(surface::css(13.))
                    .line_height(surface::css(16.))
                    .border_1()
                    .rounded(surface::css(4.))
                    .border_color(border)
                    .child(div().opacity(1. - delete_opacity).child(key.clone()))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(delete_opacity)
                            .child(
                                svg()
                                    .path("synapse/automation-quick-macro-delete.svg")
                                    .size(surface::css(24.))
                                    .text_color(delete_color),
                            ),
                    )
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered_key = hovered.then(|| hover_key.clone());
                        if !hovered {
                            this.pressed_key = None;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pressed_key = Some(press_key.clone());
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pressed_key = None;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pressed_key = None;
                            cx.notify();
                        }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.keys.retain(|key| key != &remove_key);
                        this.hovered_key = None;
                        this.pressed_key = None;
                        cx.notify();
                    })),
            );
        }
        field.into_any_element()
    }

    fn field(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.kind.as_str() {
            "launch" => self.launch_field(cx),
            "keyboard" | "" => self.keyboard_field(window, cx),
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
        let kind = if self.kind.is_empty() {
            "keyboard"
        } else {
            &self.kind
        };
        img(SharedString::from(format!(
            "synapse/automation-quick-macro-{kind}.svg"
        )))
        .size(surface::css(20.))
        .flex_shrink_0()
        .when(!matches!(self.kind.as_str(), "text" | "launch"), |icon| {
            icon.mt(surface::css(6.))
        })
        .when(self.kind.is_empty(), |icon| icon.opacity(0.3))
        .into_any_element()
    }
}

impl Render for QuickMacroEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value_len = self.text.read(cx).value().encode_utf16().count();
        v_flex()
            .id("quick-macro-popup")
            .track_focus(&self.capture)
            .capture_key_down(
                cx.listener(|this, event, window, cx| this.capture_key(event, window, cx)),
            )
            .on_key_up(cx.listener(|this, _: &KeyUpEvent, _, cx| {
                if this.capturing {
                    this.capturing = false;
                    cx.notify();
                }
            }))
            .on_modifiers_changed(
                cx.listener(|this, event, window, cx| this.modifiers_changed(event, window, cx)),
            )
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
                            .child(self.type_selector(window, cx)),
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
                    .child(self.field(window, cx)),
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
