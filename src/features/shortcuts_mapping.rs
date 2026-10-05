// Current ie → ee/M + dt/Map* tree. The source's list and mapping container
// are siblings; the .custom-global-shortcuts .key-config selector does not match.
use gpui_kit::base::{
    Button as ShortcutButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::radio::Radio;

fn shortcut_category_asset(category: &str, selected: bool) -> SharedString {
    let stem = match category {
        "SWITCH_DEVICE_PROFILE" => "profile",
        "SWITCH_DEVICE_SENSITIVITY" => "sensitivity",
        "SWITCH_LIGHTING" => "lighting",
        "MACRO" => "macro",
        "TEXT_FUNCTION" => "text",
        "LAUNCH_PROGRAM" => "launch",
        "MULTIMEDIA" => "multimedia",
        _ => "windows",
    };
    format!(
        "synapse/mapping-{stem}{}.{}",
        if selected { "-active" } else { "" },
        if stem == "lighting" { "png" } else { "svg" }
    )
    .into()
}

impl Shortcuts {
    fn mapping_function_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let draft = self.draft.as_ref().expect("mapping draft");
        match self.mapping_category.as_str() {
            "TEXT_FUNCTION" => self.text_function(window, cx),
            "LAUNCH_PROGRAM" => {
                let program = draft.value.output.kind() == "program";
                v_flex()
                    .child(
                        Radio::new("shortcut-launch-program")
                            .label(i18n::t("PROGRAM"))
                            .checked(program)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(draft) = &mut this.draft {
                                    draft.value.output = ShortcutOutput::Program {
                                        target: this.launch_program.clone(),
                                    };
                                }
                                this.sync(window, cx);
                                this.changed(cx);
                            })),
                    )
                    .child(
                        h_flex()
                            .relative()
                            .ml(surface::css(30.))
                            .mt(surface::css(17.))
                            .mb(surface::css(20.))
                            .w(surface::css(170.))
                            .h(surface::css(25.))
                            .when(!program, |s| s.opacity(0.3))
                            .child(
                                div()
                                    .w(surface::css(146.))
                                    .h_full()
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .pl(surface::css(5.))
                                    .pr(surface::css(25.))
                                    .truncate()
                                    .child(self.launch_program.clone()),
                            )
                            .child(
                                ShortcutButton::new("shortcut-browse")
                                    .accessibility_label(i18n::t("PROGRAM"))
                                    .absolute()
                                    .left(surface::css(121.))
                                    .top(surface::css(5.))
                                    .size(surface::css(20.))
                                    .disabled(!program)
                                    .child(img("synapse/automation-icon_folder.svg").size_full())
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.browse(window, cx)),
                                    ),
                            ),
                    )
                    .child(
                        Radio::new("shortcut-launch-website")
                            .label(i18n::t("WEBSITE"))
                            .checked(!program)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(draft) = &mut this.draft {
                                    draft.value.output = ShortcutOutput::Website {
                                        target: this.launch_website.clone(),
                                    };
                                }
                                this.sync(window, cx);
                                this.changed(cx);
                            })),
                    )
                    .child(
                        div()
                            .ml(surface::css(30.))
                            .mb(surface::css(10.))
                            .w(surface::css(170.))
                            .when(program, |s| s.opacity(0.3))
                            .child(
                                Input::new(&self.target)
                                    .id("shortcut-target")
                                    .appearance(false)
                                    .bordered(false)
                                    .h(surface::css(25.))
                                    .w_full()
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .rounded_none()
                                    .p(surface::css(1.))
                                    .pl(surface::css(5.))
                                    .bg(rgb(0x111111))
                                    .text_color(rgb(0xcccccc))
                                    .text_size(surface::css(14.))
                                    .disabled(program),
                            ),
                    )
                    .into_any_element()
            }
            "MULTIMEDIA" | "WINDOWS_SHORTCUT" => div()
                .mb(surface::css(20.))
                .child(
                    surface::select(&self.action)
                        .id("shortcut-action")
                        .items(action_choices(draft.value.output.kind()))
                        .w_full(),
                )
                .into_any_element(),
            "MACRO" => self.macro_function(cx),
            "SWITCH_LIGHTING" => v_flex()
                .child(
                    surface::select(&self.empty_catalog)
                        .id("shortcut-chroma-profile")
                        .w_full(),
                )
                .child(
                    div()
                        .mb(surface::css(20.))
                        .child(i18n::t("CONFIGURE_CHROMA_STUDIO_MSG")),
                )
                .child(
                    ShortcutButton::new("shortcut-configure-chroma-studio")
                        .text_left()
                        .mb(surface::css(20.))
                        .underline()
                        .child(i18n::t("CONFIGURE_CHROMA_STUDIO"))
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(ShortcutsOpenModule::ChromaStudio)),
                        ),
                )
                .into_any_element(),
            "SWITCH_DEVICE_PROFILE" => {
                v_flex()
                    .mb(surface::css(20.))
                    .child(
                        surface::select(&self.empty_device)
                            .id("shortcut-profile-device")
                            .placeholder(i18n::t("DEVICE_NOT_CONNECTED_MSG"))
                            .w_full(),
                    )
                    .child(v_flex().opacity(0.3).children(
                        source_shortcuts().profile_actions.iter().enumerate().map(
                            |(index, action)| {
                                Radio::new(SharedString::from(format!(
                                    "shortcut-profile-{}",
                                    action.id
                                )))
                                .label(i18n::t(&action.content))
                                .checked(index == 0)
                                .disabled(true)
                                .mb(surface::css(10.))
                            },
                        ),
                    ))
                    .into_any_element()
            }
            _ => v_flex()
                .mb(surface::css(20.))
                .child(
                    surface::select(&self.empty_device)
                        .id("shortcut-sensitivity-device")
                        .placeholder(i18n::t("DEVICE_NOT_CONNECTED_MSG"))
                        .w_full(),
                )
                .child(
                    v_flex()
                        .opacity(0.3)
                        .child(
                            div()
                                .w_full()
                                .h(surface::css(27.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .px(surface::css(5.))
                                .child(i18n::t(&source_shortcuts().sensitivity_actions[0].content)),
                        )
                        .child(
                            h_flex()
                                .mt(surface::css(10.))
                                .mb(surface::css(20.))
                                .gap(surface::css(10.))
                                .child("DPI")
                                .child(
                                    div()
                                        .w(surface::css(54.))
                                        .border_1()
                                        .border_color(rgb(0x5d5d5d))
                                        .child("800"),
                                ),
                        ),
                )
                .into_any_element(),
        }
    }

    fn editor(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let rail_width = motion::transition(
            "shortcuts-mapping-rail-width",
            if self.mapping_expanded {
                250_f32
            } else {
                40_f32
            },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let body = v_flex()
            .w(surface::css(250.))
            .ml(surface::css(40.))
            .p(surface::css(20.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(rgb(0x44d62c))
                    .mb(surface::css(20.))
                    .child(i18n::t(&self.mapping_category).to_uppercase()),
            )
            .child(self.mapping_function_body(window, cx))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(
                        Button::new("shortcut-cancel")
                            .label(i18n::t("CANCEL"))
                            .w(surface::css(100.))
                            .h(surface::css(27.))
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .py_0()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.discard_editor(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("shortcut-apply")
                            .label(i18n::t("SAVE"))
                            .primary()
                            .w(surface::css(100.))
                            .h(surface::css(27.))
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .py_0()
                            .disabled(!self.valid())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.prepare_save(window, cx);
                            })),
                    ),
            );
        let rail = v_flex()
            .id("shortcut-mapping-categories")
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(surface::css(250.))
            .max_w(surface::css(rail_width))
            .bg(rgb(0x222222))
            .overflow_hidden()
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.mapping_expanded = *hovered;
                cx.notify();
            }))
            .children(source_shortcuts().functions.iter().map(|category| {
                let selected = &self.mapping_category == category;
                let category = category.clone();
                ShortcutButton::new(SharedString::from(format!("shortcut-category-{category}")))
                    .accessibility_label(i18n::t(&category))
                    .h(surface::css(40.))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(surface::css(20.))
                    .px(surface::css(10.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_color(if selected {
                        rgb(0x44d62c)
                    } else {
                        rgb(0xcccccc)
                    })
                    .when(selected, |s| s.bg(rgb(0x111111)))
                    .hover(|s| s.bg(rgb(0x393939)))
                    .active(|s| s.bg(rgb(0x111111)))
                    .child(
                        img(shortcut_category_asset(&category, selected))
                            .size(surface::css(20.))
                            .flex_shrink_0(),
                    )
                    .child(i18n::t(&category).to_uppercase())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_category(category.clone(), window, cx)
                    }))
            }));
        let panel = v_flex()
            .id("shortcut-editor")
            .test_support()
            .track_focus(&self.focus)
            .tab_group()
            .w(surface::css(292.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(5.))
            .shadow(vec![BoxShadow {
                color: rgba(0x000000b3).into(),
                offset: point(px(0.), px(0.)),
                blur_radius: window.rem_size() * (20. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    if this.emoji_open {
                        this.close_emoji(window, cx);
                    } else {
                        this.close_editor(window, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .child(
                h_flex()
                    .relative()
                    .h(surface::css(36.))
                    .bg(rgb(0x222222))
                    .border_b_1()
                    .border_color(rgb(0x5d5d5d))
                    .rounded_t(surface::css(4.))
                    .justify_center()
                    // Global shortcut BUTTON has no counter/default caption here.
                    .child(
                        surface::keymap_close_button("shortcut-close", "Close", window, cx)
                            .absolute()
                            .top_0()
                            .right_0()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_editor(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .min_h(surface::css(486.))
                    .child(body)
                    .child(rail),
            );
        // Fe's se is a sibling of the custom widget. The final .body-wrapper
        // rules override inline top132 with top142!important and min-height486.
        let width = window.viewport_size().width / window.rem_size() * 16.;
        let left = width / 2.
            + if width <= 824. {
                0.
            } else if width <= 1220. {
                110.
            } else {
                296.
            };
        deferred(ShortcutMappingPosition {
            origin: point(
                window.rem_size() * (left / 16.),
                window.rem_size() * (106. / 16.),
            ),
            child: panel.into_any_element(),
        })
        .priority(103)
        .into_any_element()
    }
}

/// `.key-config` is an absolute source element, not a popup that flips or
/// clamps to the viewport. Keep top142 even below the source's 486px minimum.
struct ShortcutMappingPosition {
    origin: Point<Pixels>,
    child: AnyElement,
}
impl IntoElement for ShortcutMappingPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ShortcutMappingPosition {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_offset(self.origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
