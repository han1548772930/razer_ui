use super::*;
use controls::source_checkbox;

fn switch_callback(
    cx: &Context<AlexaPage>,
    change: impl Fn(&mut AlexaPage, bool, &mut Context<AlexaPage>) + 'static,
) -> impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    move |value, _, _, cx| {
        _ = view.update(cx, |this, cx| change(this, value, cx));
    }
}
fn checkbox_callback(
    cx: &Context<AlexaPage>,
    change: impl Fn(&mut AlexaPage, bool) + 'static,
) -> impl Fn(CheckboxState, &ClickEvent, &mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    move |value, _, _, cx| {
        _ = view.update(cx, |this, cx| {
            if this.enabled {
                change(this, value == CheckboxState::Checked);
                cx.notify();
            }
        });
    }
}

impl AlexaPage {
    pub fn skills(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .max_w(css(980.))
            .mx_auto()
            .child(
                h_flex()
                    .mb(css(10.))
                    .gap(css(10.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(css(24.))
                            .font_weight(FontWeight::THIN)
                            .text_color(cx.theme().primary)
                            .child(text("SYNAPSE_SKILL").to_uppercase()),
                    )
                    .child(
                        source_switch(
                            "alexa-synapse-skills",
                            text("SYNAPSE_SKILL"),
                            self.synapse_skills,
                            window,
                            cx,
                        )
                        .mt(css(-1.))
                        .on_change(switch_callback(
                            cx,
                            |this, value, cx| {
                                this.set_synapse_skills(value, cx);
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .mb(css(10.))
                    .text_color(AlexaColors::skill_text())
                    .child(text("PAGE_SKILL_DESC")),
            )
            .children(SKILLS.iter().map(|skill| {
                let open = self.expanded == Some(skill.id);
                let pointer = surface::pointer_state(skill.id, window, cx);
                let (hovered, pressed) = pointer.read(cx).sample();
                let more_color = gpui_kit::base::motion::transition(
                    (skill.id, "more-color"),
                    if hovered || pressed {
                        cx.theme().primary
                    } else {
                        razer_widgets::theme::SettingsButtonColors::background()
                    },
                    Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                    window,
                    cx,
                );
                let more_alpha = gpui_kit::base::motion::transition(
                    (skill.id, "more-opacity"),
                    if pressed { 0.7_f32 } else { 1. },
                    Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
                    window,
                    cx,
                );
                let reveal = Presence::new(("alexa-skill", skill.id), open)
                    .transition(Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut))
                    .sample(window, cx)
                    .progress;
                let header = h_flex()
                    .w_full()
                    .pt(css(20.))
                    .pb(css(19.))
                    .px(css(20.))
                    .bg(cx.theme().group_box)
                    .child(img(skill.icon).size(css(41.)).flex_shrink_0().mr(css(10.)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(css(16.)).child(text(skill.title)))
                            .child(
                                div()
                                    .mt(css(2.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(text(skill.description)),
                            ),
                    )
                    .when(!skill.content.is_empty(), |row| {
                        row.child(
                            div()
                                .id((ElementId::from(skill.id), "more"))
                                .ml(css(10.))
                                .flex_shrink_0()
                                .underline()
                                .text_color(more_color)
                                .opacity(more_alpha)
                                .child(text(if open { "TEXT_CLOSE" } else { "MORE_SKILL" })),
                        )
                    });
                let header = if skill.content.is_empty() {
                    header.into_any_element()
                } else {
                    surface::track_pointer(Button::new(skill.id), &pointer, window)
                        .group(skill.id)
                        .w_full()
                        .p_0()
                        .aria_expanded(open)
                        .accessibility_label(text(skill.title))
                        .focus_visible(|style| style.border_1().border_color(cx.theme().ring))
                        .child(header)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.expanded = if this.expanded == Some(skill.id) {
                                None
                            } else {
                                Some(skill.id)
                            };
                            cx.notify();
                        }))
                        .into_any_element()
                };
                Collapsible::new()
                    .open(open)
                    .reveal((ElementId::from("alexa-skill-content"), skill.id), reveal)
                    .mb(css(2.))
                    .child(header)
                    .content(
                        v_flex()
                            .p(css(20.))
                            .gap(css(8.))
                            .bg(razer_widgets::theme::MainPageColors.mobile_action())
                            .children(skill.content.iter().map(|key| {
                                div()
                                    .relative()
                                    .pl(css(20.))
                                    .child(
                                        div()
                                            .absolute()
                                            .left(css(6.))
                                            .top(css(7.))
                                            .size(css(5.))
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .bg(cx.theme().foreground),
                                    )
                                    .child(text(key))
                            })),
                    )
            }))
            .into_any_element()
    }

    pub fn settings(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let disabled = !self.enabled;
        let alpha = gpui_kit::base::motion::transition(
            "alexa-settings-disabled",
            if disabled { 0.3_f32 } else { 1. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
            window,
            cx,
        );
        let group = || v_flex().mb(css(20.)).opacity(alpha);
        v_flex()
            .relative()
            .w_full()
            .max_w(css(600.))
            .mx_auto()
            .mb(css(25.))
            .py(css(30.))
            .px(css(40.))
            .bg(cx.theme().group_box)
            .rounded(css(5.))
            .child(
                h_flex()
                    .gap(css(10.))
                    .mb(css(20.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(css(18.))
                            // CSS asks for 200, but declares 100/400/600/700 faces;
                            // CSS matching selects the lower Thin face first.
                            .font_weight(FontWeight::THIN)
                            .text_color(cx.theme().primary)
                            .child("ALEXA"),
                    )
                    .child(
                        source_switch("alexa-enabled", "Alexa".into(), self.enabled, window, cx)
                            .on_change(switch_callback(cx, |this, value, cx| {
                                this.enabled = value;
                                this.recording = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(css(10.))
                    .right(css(10.))
                    .child(controls::settings_help()),
            )
            .child(
                group()
                    .child(
                        source_checkbox(
                            "alexa-sounds",
                            text("TEXT_SETTING_1"),
                            self.sounds,
                            disabled,
                            window,
                            cx,
                        )
                        .on_change(checkbox_callback(cx, |this, value| {
                            this.sounds = value;
                        })),
                    )
                    .child(
                        source_checkbox(
                            "alexa-cards",
                            text("TEXT_SETTING_2"),
                            self.cards,
                            disabled,
                            window,
                            cx,
                        )
                        .mb_0()
                        .on_change(checkbox_callback(cx, |this, value| {
                            this.cards = value;
                        })),
                    ),
            )
            .child(
                group()
                    .child(
                        div()
                            .mb(css(10.))
                            .child(text("TEXT_SETTING_3").to_uppercase()),
                    )
                    .child(
                        source_checkbox(
                            "alexa-wake-word",
                            text("TEXT_SETTING_4"),
                            self.wake_word,
                            disabled,
                            window,
                            cx,
                        )
                        .on_change(checkbox_callback(cx, |this, value| {
                            this.wake_word = value;
                        })),
                    )
                    .child(
                        source_checkbox(
                            "alexa-shortcut-enabled",
                            text("TEXT_SETTING_5"),
                            self.shortcut_enabled,
                            disabled,
                            window,
                            cx,
                        )
                        .on_change(checkbox_callback(cx, |this, value| {
                            this.shortcut_enabled = value;
                            this.recording = false;
                        })),
                    )
                    .when(self.shortcut_enabled, |view| {
                        view.child(
                            Button::new("alexa-shortcut")
                                .track_focus(&self.shortcut_focus)
                                .disabled(disabled)
                                .w(css(230.))
                                .h(css(27.))
                                .ml(css(30.))
                                .px(css(6.))
                                .py(css(2.))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().transparent)
                                .text_color(cx.theme().foreground)
                                .hover(|style| style.border_color(cx.theme().primary))
                                .when(self.shortcut_focus.is_focused(window), |field| {
                                    field.border_color(cx.theme().primary)
                                })
                                .focus_visible(|style| style.border_color(cx.theme().primary))
                                .accessibility_label("Alexa 快捷键（仅本地预览）")
                                .child(if self.recording {
                                    "按下组合键…".into()
                                } else {
                                    self.shortcut.clone()
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.enabled {
                                        this.recording = true;
                                        this.shortcut_focus.focus(window, cx);
                                        cx.notify();
                                    }
                                }))
                                .on_key_down(cx.listener(Self::capture_shortcut)),
                        )
                    }),
            )
            .child(
                group()
                    .child(
                        div()
                            .mb(css(10.))
                            .child(text("TEXT_SETTING_6").to_uppercase()),
                    )
                    .child(
                        surface::select_alexa(&self.inputs)
                            .id("alexa-input-device")
                            .items(input_choices(&self.media_inputs))
                            // yE's trigger uses literal Default; the menu alone localizes it.
                            .selected_text(
                                self.media_inputs
                                    .selected()
                                    .map_or("Default", MediaInputDevice::label)
                                    .to_owned(),
                            )
                            .w(css(230.))
                            .disabled(disabled)
                            // Source yE dims the containing .item only once.
                            .opacity(1.)
                            .accessibility_label(text("TEXT_SETTING_6")),
                    ),
            )
            .child(
                group()
                    .child(
                        div()
                            .mb(css(10.))
                            .child(text("TEXT_SETTING_7").to_uppercase()),
                    )
                    .child(
                        surface::select_alexa(&self.languages)
                            .id("alexa-speech-language")
                            .items(language_choices())
                            .w(css(230.))
                            .disabled(disabled)
                            .opacity(1.)
                            .accessibility_label(text("TEXT_SETTING_7")),
                    ),
            )
            .child(
                h_flex()
                    .child(text("TEXT_NOT_USER").replace("{{name}}", "示例用户"))
                    .child(
                        text_button("alexa-logout", text("TEXT_LOG_OUT"), cx)
                            .text_size(css(16.))
                            .ml(css(10.))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_modal(ModalKind::Logout, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }

    fn capture_shortcut(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.recording || !self.enabled || !self.shortcut_enabled {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            self.recording = false;
            cx.notify();
            return;
        }
        if matches!(key, "control" | "shift" | "alt" | "super" | "cmd" | "win") {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        let mut parts = Vec::new();
        if modifiers.control {
            parts.push("Ctrl".into());
        }
        if modifiers.shift {
            parts.push("Shift".into());
        }
        if modifiers.alt {
            parts.push("Alt".into());
        }
        // Original keyboard recorder ignores Win and supplies Ctrl+Shift if no
        // supported modifier was captured. It does not register a global hotkey here.
        if parts.is_empty() {
            parts.extend(["Ctrl".into(), "Shift".into()]);
        }
        parts.push(key.to_uppercase());
        self.shortcut = parts.join(" + ");
        self.recording = false;
        cx.notify();
    }

    pub fn help(&self, cx: &App) -> AnyElement {
        let locale = i18n::locale();
        let guide =
            format!("https://dl.razerzone.com/master-guides/RazerSynapse3/ALEXA-{locale}.pdf");
        v_flex().w_full().max_w(css(600.)).min_w(css(550.)).mx_auto().py(css(30.)).px(css(40.))
            .bg(cx.theme().group_box).rounded(css(5.))
            .child(div().font_family("RazerF5").text_size(css(16.)).text_color(cx.theme().primary).mb(css(20.)).child(text("SUPPORT").to_uppercase()))
            .children([
                ("alexa-master-guide", "VIEW_MASTER_GUIDE", guide),
                ("alexa-online-help", "VIEW_ALEXA_GUIDE", "https://mysupport.razer.com/app/answers/detail/a_id/3806/~/razer-amazon-alexa-faqs".into()),
            ].into_iter().map(|(id, key, url)| Link::new(id).href(url).open_with(|url, _, _, cx| cx.open_url(url))
                .mt(css(10.)).flex().items_center().gap(css(10.)).underline().text_color(cx.theme().foreground)
                .accessibility_label(text(key)).hover(|style| style.text_color(cx.theme().primary))
                .child(text(key)).child(img("synapse/release-notes-external.svg").size(css(20.)))))
            .into_any_element()
    }
}
