//! Current 58190.dn text modal. Draft text is committed only by its own Save.
use super::text_overlay::{TextAnchor, TextOverlay};
use super::*;
use crate::ui::surface;
use serde::Deserialize;
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::OnceLock, time::Instant};

pub(super) fn tip_opacity(id: ElementId, hovered: bool, window: &mut Window, cx: &mut App) -> f32 {
    motion::transition(
        (id, "tip-opacity"),
        if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    )
}

pub(super) fn tip_surface(opacity: f32) -> Div {
    div()
        .flex()
        .px(css(10.))
        .py(css(8.))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .bg(rgb(0))
        .text_color(rgb(0xcccccc))
        .font_family("Roboto")
        .text_size(css(14.))
        .line_height(css(16.))
        .whitespace_nowrap()
        .opacity(opacity)
}

pub(super) fn text_button(
    id: ElementId,
    label: String,
    save: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let pointer = surface::pointer_state(id.clone(), window, cx);
    let (hovered, pressed) = pointer.read(cx).sample();
    let opacity = surface::fade_opacity(
        id.clone(),
        if disabled {
            0.3
        } else if pressed {
            0.6
        } else if hovered {
            0.8
        } else {
            1.
        },
        300,
        window,
        cx,
    );
    surface::track_pointer(BaseButton::new(id), &pointer, window)
        .disabled(disabled)
        .min_w(css(100.))
        .h(css(27.))
        .flex_shrink_0()
        .px(css(10.))
        .pt(css(6.))
        .pb(css(7.))
        .mr(css(10.))
        .border_1()
        .border_color(rgb(0))
        .rounded(css(3.))
        .font_family("Roboto")
        .text_size(css(12.))
        .line_height(css(14.))
        .bg(rgb(if save { 0x44d62c } else { 0x707070 }))
        .text_color(rgb(if save { 0 } else { 0xffffff }))
        .opacity(opacity)
        .child(label.to_uppercase())
}

#[derive(Deserialize)]
pub(super) struct TextSource {
    pub(super) groups: Vec<TextEmojiGroup>,
    pub(super) tabs: Vec<TextEmojiTab>,
    pub(super) search: Vec<TextEmojiSearch>,
    pub(super) variants: std::collections::BTreeMap<String, Vec<String>>,
    pub(super) excluded_windows_11: Vec<String>,
}
#[derive(Deserialize)]
pub(super) struct TextEmojiGroup {
    pub(super) label: String,
    pub(super) name: String,
    pub(super) values: Vec<String>,
}
#[derive(Deserialize)]
pub(super) struct TextEmojiTab {
    pub(super) emo: String,
    pub(super) name: String,
    pub(super) pos: usize,
}
#[derive(Deserialize)]
pub(super) struct TextEmojiSearch {
    pub(super) emo: String,
    pub(super) name: String,
}
pub(super) fn text_source() -> &'static TextSource {
    static DATA: OnceLock<TextSource> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("text_data.json")).expect("current Macro text data")
    })
}

pub(super) struct TextRowState {
    pub(super) armed: bool,
    pub(super) mounted: Instant,
    pub(super) query: String,
    pub(super) applied_query: String,
    pub(super) category: usize,
    pub(super) scroll: ScrollHandle,
    pub(super) trigger: Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub(super) modal: Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub(super) emoji: Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub(super) placement: super::text_overlay::TextPlacement,
}

pub(super) struct TextUi {
    pub(super) search: Entity<InputState>,
    pub(super) previous: String,
    pub(super) ignored_query: Option<String>,
    pub(super) emoji_open: bool,
    pub(super) hovered_emoji: Option<(usize, usize)>,
    pub(super) windows_version: Option<String>,
    pub(super) editor_bounds: Rc<std::cell::Cell<Bounds<Pixels>>>,
    rows: RefCell<HashMap<(u64, usize), Rc<RefCell<TextRowState>>>>,
}

impl TextUi {
    pub(super) fn new(search: Entity<InputState>) -> Self {
        Self {
            search,
            previous: String::new(),
            ignored_query: None,
            emoji_open: false,
            hovered_emoji: None,
            windows_version: None,
            editor_bounds: Default::default(),
            rows: Default::default(),
        }
    }
}

impl MacroPage {
    pub(super) fn text_value_popup(
        &self,
        index: usize,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = self.text_row(index, cx);
        let trigger = row.borrow().trigger.clone();
        let outside = row.clone();
        let opened = self.editing_action == Some(index);
        let mut root = div()
            .id(("macro-text-root", index))
            .on_prepaint(move |bounds, _, _| trigger.set(bounds))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                BaseButton::new(("macro-text-value", index))
                    .max_w(css(350.))
                    .p_0()
                    .text_size(css(14.))
                    .line_height(css(17.))
                    .text_color(rgb(if opened {
                        0x44d62c
                    } else if self.actions()[index].value.is_empty() {
                        0x707070
                    } else {
                        0xcccccc
                    }))
                    .hover(|s| s.text_color(rgb(0x44d62c)))
                    .child(div().truncate().child(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_text_editor(index, window, cx);
                        cx.stop_propagation();
                    })),
            )
            .when(opened, |view| {
                view.on_mouse_down_out(cx.listener(
                    move |this, event: &MouseDownEvent, window, cx| {
                        let row = outside.borrow();
                        if !row.modal.get().contains(&event.position)
                            && !row.emoji.get().contains(&event.position)
                        {
                            drop(row);
                            this.cancel_text_editor(window, cx);
                        }
                    },
                ))
            });
        // on is mounted with dn, including its 30ms initial data-loading delay.
        if cx
            .background_executor()
            .now()
            .duration_since(row.borrow().mounted)
            < Duration::from_millis(30)
        {
            window.request_animation_frame();
        }
        if opened {
            root = root.child(
                deferred(TextOverlay {
                    content: self.text_modal(index, window, cx),
                    anchor: TextAnchor::Modal {
                        row,
                        editor: self.text_ui.editor_bounds.clone(),
                        viewport: self.source_viewport.clone(),
                    },
                })
                .priority(100),
            );
        }
        root.into_any_element()
    }

    fn text_modal(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let row = self.text_row(index, cx);
        let modal_bounds = row.borrow().modal.clone();
        let count = self.text_editor.read(cx).value().encode_utf16().count();
        let opened = self.text_ui.emoji_open;
        if !opened {
            row.borrow().emoji.set(Bounds::default());
        }
        let close_id: ElementId = ("macro-text-close", index).into();
        let pointer = surface::pointer_state(close_id.clone(), window, cx);
        let (hovered, pressed) = pointer.read(cx).sample();
        let close_bg = surface::fade_color(
            close_id.clone(),
            Hsla::from(if pressed {
                rgba(0x0000001a)
            } else if hovered {
                rgba(0xffffff1a)
            } else {
                rgba(0)
            }),
            200,
            window,
            cx,
        );
        let emoji_id: ElementId = ("macro-text-emoji-toggle", index).into();
        let emoji_pointer = surface::pointer_state(emoji_id.clone(), window, cx);
        let (emoji_hovered, _) = emoji_pointer.read(cx).sample();
        let tooltip_alpha = tip_opacity(emoji_id.clone(), emoji_hovered, window, cx);
        let emoji_alpha = motion::transition(
            (ElementId::from(("macro-text-emoji", index)), "opacity"),
            if opened { 1f32 } else { 0. },
            Transition::new(Duration::from_millis(500)).easing(Easing::Linear),
            window,
            cx,
        );
        let emoji = surface::track_pointer(BaseButton::new(emoji_id), &emoji_pointer, window)
            .relative()
            .size(css(20.))
            .p_0()
            .child(
                img(if opened {
                    "synapse/macro/binding-close.svg"
                } else if emoji_hovered {
                    "synapse/shortcuts-emoji-active.svg"
                } else {
                    "synapse/shortcuts-emoji.svg"
                })
                .size_full(),
            )
            .when(emoji_hovered, |button| {
                button.child(
                    deferred(
                        tip_surface(tooltip_alpha)
                            .absolute()
                            .left_0()
                            .bottom(css(-40.))
                            .child(tr("TEXT_EMOJI")),
                    )
                    .priority(103),
                )
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.text_ui.emoji_open = !this.text_ui.emoji_open;
                this.text_ui.hovered_emoji = None;
                cx.notify();
            }));
        let character_id: ElementId = ("macro-text-character", index).into();
        let character_pointer = surface::pointer_state(character_id.clone(), window, cx);
        let (character_hovered, _) = character_pointer.read(cx).sample();
        v_flex()
            .id(("macro-text-modal", index))
            .relative()
            .w(css(250.))
            .p(css(20.))
            .font_family("Roboto")
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(rgb(0xcccccc))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(css(5.))
            .shadow(vec![BoxShadow {
                color: rgba(0x000000b3).into(),
                offset: point(px(0.), px(0.)),
                blur_radius: window.rem_size() * (20. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .occlude()
            .on_prepaint(move |bounds, _, _| modal_bounds.set(bounds))
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                surface::track_pointer(BaseButton::new(close_id), &pointer, window)
                    .absolute()
                    .right_0()
                    .top_0()
                    .size(css(36.))
                    .p_0()
                    .bg(close_bg)
                    .child(img("synapse/macro/close.svg").size(css(20.)))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.cancel_text_editor(window, cx)),
                    ),
            )
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .line_height(css(19.))
                    .text_color(rgb(0x44d62c))
                    .mb(css(20.))
                    .child(tr("TEXT_TEXT_FUNCTION").to_uppercase()),
            )
            .child(
                Textarea::new(&self.text_editor)
                    .appearance(false)
                    .bordered(false)
                    .w(css(210.))
                    .min_w(css(210.))
                    .h(css(96.))
                    .p(css(5.))
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .rounded_none()
                    .text_size(css(14.))
                    .line_height(css(17.)),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .py(css(2.))
                    .child(
                        h_flex().w(css(50.)).justify_between().child(emoji).child(
                            surface::track_pointer(
                                BaseButton::new(character_id),
                                &character_pointer,
                                window,
                            )
                            .size(css(20.))
                            .p_0()
                            .child(
                                img(if character_hovered {
                                    "synapse/shortcuts-character-active.svg"
                                } else {
                                    "synapse/shortcuts-character.svg"
                                })
                                .size_full(),
                            )
                            .on_click(|_, window, cx| {
                                if let Err(error) = crate::backend::system::open_character_map() {
                                    window.push_notification(
                                        format!("无法打开字符映射表：{error}"),
                                        cx,
                                    );
                                }
                            }),
                        ),
                    )
                    .child(
                        div()
                            .text_right()
                            .text_size(css(14.))
                            .line_height(css(17.))
                            .text_color(rgb(0x707070))
                            .mb(css(20.))
                            .child(format!("{count}/250")),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .child(
                        text_button(
                            ("macro-text-cancel", index).into(),
                            tr("TEXT_TEXT_FUNCTION_CANCEL"),
                            false,
                            false,
                            window,
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.cancel_text_editor(window, cx)),
                        ),
                    )
                    .child(
                        text_button(
                            ("macro-text-save", index).into(),
                            tr("TEXT_TEXT_FUNCTION_SAVE"),
                            true,
                            !row.borrow().armed,
                            window,
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.save_text_editor(window, cx)),
                        ),
                    ),
            )
            .when(opened, |view| {
                view.child(self.text_emoji_popup(index, emoji_alpha, window, cx))
            })
            .into_any_element()
    }

    pub(super) fn text_row(&self, index: usize, cx: &App) -> Rc<RefCell<TextRowState>> {
        self.text_ui
            .rows
            .borrow_mut()
            .entry((self.current.unwrap_or(0), index))
            .or_insert_with(|| {
                Rc::new(RefCell::new(TextRowState {
                    armed: self
                        .actions()
                        .get(index)
                        .is_some_and(|item| !item.value.is_empty()),
                    mounted: cx.background_executor().now(),
                    query: String::new(),
                    applied_query: String::new(),
                    category: 0,
                    scroll: ScrollHandle::new(),
                    trigger: Default::default(),
                    modal: Default::default(),
                    emoji: Default::default(),
                    placement: Default::default(),
                }))
            })
            .clone()
    }

    pub(super) fn open_text_editor(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_open = self.editing_action == Some(index);
        self.finish_pending_edits(window, cx);
        if was_open {
            self.cancel_text_editor(window, cx);
            return;
        }
        self.choice_action = None;
        self.cancel_launch_editor();
        let Some(item) = self
            .actions()
            .get(index)
            .filter(|item| item.kind == ActionKind::Text)
        else {
            return;
        };
        let value = item.value.clone();
        self.editing_action = Some(index);
        let row = self.text_row(index, cx);
        // The trigger's document.body.click() calls O -> N, disarming Save.
        // Only a later nonempty change to d arms it again; empty never disarms.
        row.borrow_mut().armed = false;
        row.borrow_mut().placement = Default::default();
        let query = row.borrow().query.clone();
        self.text_ui.ignored_query = Some(query.clone());
        self.text_ui
            .search
            .update(cx, |input, cx| input.set_value(query, window, cx));
        self.text_ui.previous = value.clone();
        self.text_ui.emoji_open = false;
        self.text_ui.hovered_emoji = None;
        self.text_editor
            .update(cx, |input, cx| input.set_value(value, window, cx));
        cx.notify();
    }

    pub(super) fn text_input_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.text_editor.read(cx).value().to_string();
        let value = if let Some((range, limited)) =
            crate::features::shortcuts::limit_shortcut_text(&self.text_ui.previous, &value)
        {
            self.text_editor.update(cx, |input, cx| {
                let caret = range.start;
                input.set_selected_range(range, cx);
                input.replace("", window, cx);
                input.set_selected_range(caret..caret, cx);
            });
            limited
        } else {
            value
        };
        if value != self.text_ui.previous
            && let Some(index) = self.editing_action.filter(|index| {
                self.actions()
                    .get(*index)
                    .is_some_and(|item| item.kind == ActionKind::Text)
            })
        {
            self.text_row(index, cx).borrow_mut().armed |= !value.is_empty();
        }
        self.text_ui.previous = value;
        cx.notify();
    }

    pub(super) fn text_search_changed(&mut self, cx: &mut Context<Self>) {
        let query = self.text_ui.search.read(cx).value().to_string();
        if self.text_ui.ignored_query.take().as_ref() == Some(&query) {
            return;
        }
        if let Some(index) = self.editing_action {
            let row = self.text_row(index, cx);
            let mut row = row.borrow_mut();
            row.query = query.clone();
            row.applied_query = query;
        }
        self.text_ui.hovered_emoji = None;
        cx.notify();
    }

    pub(super) fn cancel_text_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.editing_action {
            self.text_row(index, cx).borrow_mut().armed = false;
        }
        self.editing_action = None;
        self.text_ui.emoji_open = false;
        self.text_ui.hovered_emoji = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn save_text_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.editing_action else {
            return;
        };
        if self.actions_for != self.current || !self.text_row(index, cx).borrow().armed {
            return;
        }
        let Some(item) = self
            .actions
            .get(index)
            .filter(|item| item.kind == ActionKind::Text)
        else {
            return;
        };
        let value = self.text_editor.read(cx).value().to_string();
        if item.value != value {
            self.undo.push(self.actions.clone());
            self.actions[index].value = value;
            self.redo.clear();
        }
        self.cancel_text_editor(window, cx);
    }

    pub(super) fn append_text_emoji(
        &mut self,
        emoji: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = self.text_editor.read(cx).value().to_string();
        if value.encode_utf16().count() + emoji.encode_utf16().count() > 250 {
            return;
        }
        // dn appends to d; unlike Dashboard MapText it never replaces a selection.
        self.text_editor
            .update(cx, |input, cx| input.set_value(value + emoji, window, cx));
        self.text_input_changed(window, cx);
    }
}
