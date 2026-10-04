//! main business an/un/Ct, and 25572.iV. See macro-metadata-source.json.
use super::chrome::Menu;
use super::*;

#[derive(Clone)]
struct EntryDrag {
    page: EntityId,
    id: u64,
    name: String,
    kind: EntryKind,
}
struct DragPreview {
    entry: EntryDrag,
    offset: Point<Pixels>,
}
impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let delta = window.rem_size() * (10. / 16.);
        // an's custom HTML drag image follows pageX/pageY minus 10 CSS px.
        h_flex()
            .w(css(280.))
            .h(css(40.))
            .px(css(20.))
            .py(css(10.))
            .ml(self.offset.x - delta)
            .mt(self.offset.y - delta)
            .bg(rgb(0x44d62c))
            .rounded(css(5.))
            .font_family("Roboto")
            .text_color(rgb(0x212121))
            .text_size(css(14.))
            .font_weight(FontWeight::BOLD)
            .child(
                img(if self.entry.kind == EntryKind::Folder {
                    "synapse/macro/drag-folder.svg"
                } else {
                    "synapse/macro/drag-file.svg"
                })
                .size(css(20.))
                .mr(css(10.)),
            )
            // This text is hard-coded in the current source drag handler.
            .child(format!("Move {}", self.entry.name))
    }
}

pub(super) fn capitalize(text: String) -> String {
    let mut start = true;
    let mut result = String::new();
    for ch in text.chars() {
        if start {
            result.extend(ch.to_uppercase());
        } else {
            result.push(ch);
        }
        start = ch.is_whitespace();
    }
    result
}

pub(super) fn menu_action(
    id: impl Into<ElementId>,
    key: &str,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let id = id.into();
    let hover = window.use_keyed_state((id.clone(), "hover"), cx, |_, _| false);
    let alpha = motion::transition(
        (id.clone(), "action-background"),
        if *hover.read(cx) && !disabled {
            26. / 255.
        } else {
            0.
        },
        Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
        window,
        cx,
    );
    BaseButton::new(id)
        .w_full()
        .h(css(27.))
        .px(css(6.))
        .py(css(5.))
        .justify_start()
        .text_size(css(14.))
        .line_height(css(17.))
        .bg(rgb(0xffffff).opacity(alpha))
        .disabled(disabled)
        .when(disabled, |b| b.opacity(0.3))
        .child(capitalize(tr(key)))
        .on_hover(move |value, _, cx| {
            hover.update(cx, |hover, cx| {
                *hover = *value;
                cx.notify();
            })
        })
}

impl MacroPage {
    pub(super) fn tree(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let page = cx.entity_id();
        v_flex()
            .id("macro-tree")
            .max_h(window.viewport_size().height * 0.5)
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                if !this.tree_scrolling {
                    this.tree_scrolling = true;
                    this.tree_menu = None;
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, _, _, cx| {
                if this.tree_scrolling {
                    this.tree_scrolling = false;
                    cx.notify();
                }
            }))
            .scrollable_y()
            .children(self.visible_entries(cx).into_iter().map(|(entry, depth)| {
                let id = entry.id;
                let folder = entry.kind == EntryKind::Folder;
                let group = SharedString::from(format!("macro-tree-entry-{id}"));
                let dragging = EntryDrag {
                    page,
                    id,
                    name: entry.name.clone(),
                    kind: entry.kind,
                };
                let hovered = self.tree_hover == Some(id);
                let menu_visible = !self.tree_scrolling && (hovered || self.tree_menu == Some(id));
                let icon = if folder {
                    if entry.open {
                        "synapse/macro/folder-open.svg"
                    } else {
                        "synapse/macro/folder.svg"
                    }
                } else {
                    "synapse/macro/file.svg"
                };
                let owner = cx.entity().downgrade();
                let mut row = h_flex()
                    .id(("macro-tree-row", id))
                    .group(group.clone())
                    .relative()
                    .h(css(32.))
                    .w_full()
                    .pl(css(30.))
                    .border_1()
                    .border_color(rgba(0))
                    .text_size(css(14.))
                    .line_height(css(30.))
                    .hover(move |s| {
                        s.bg(rgb(0x222222))
                            .when(!folder, |s| s.text_color(rgb(0x44d62c)))
                    })
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        let next = if *hovered {
                            Some(id)
                        } else {
                            this.tree_hover.filter(|current| *current != id)
                        };
                        if this.tree_hover != next {
                            this.tree_hover = next;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            this.tree_menu = if this.tree_menu == Some(id) {
                                None
                            } else {
                                Some(id)
                            };
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_drag(dragging, move |drag, offset, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.tree_menu = None;
                            cx.notify();
                        });
                        cx.new(|_| DragPreview {
                            entry: drag.clone(),
                            offset,
                        })
                    })
                    .drag_over::<EntryDrag>(move |style, drag, _, _| {
                        if drag.page == page {
                            style.border_color(rgb(0x44d62c))
                        } else {
                            style
                        }
                    })
                    .on_drop(cx.listener(move |this, drag: &EntryDrag, _, cx| {
                        if drag.page == cx.entity_id() {
                            this.move_entry(drag.id, id, cx);
                        }
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(css(5.))
                            .top(css(5.))
                            .size(css(20.))
                            .child(img(icon).size_full().when(!folder, |img| {
                                img.group_hover(group.clone(), |s| s.opacity(0.))
                            }))
                            .when(!folder, |v| {
                                v.child(
                                    img("synapse/macro/file-active.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .opacity(0.)
                                        .group_hover(group.clone(), |s| s.opacity(1.)),
                                )
                            }),
                    )
                    .child(
                        BaseButton::new(("macro-entry", id))
                            .absolute()
                            .inset_0()
                            .size_full()
                            .min_w_0()
                            .h_full()
                            .p_0()
                            .pl(css(30.))
                            .pr(css(30.))
                            .justify_start()
                            .child(div().min_w_0().truncate().child(entry.name.clone()))
                            .on_click(cx.listener(move |this, _, _, cx| this.select_entry(id, cx))),
                    );
                if self.rename == Some(id) && self.rename_in_tree {
                    row = row.child(
                        div()
                            .absolute()
                            .left(css(25.))
                            .top(css(2.))
                            .min_w(css(250.))
                            .child(self.rename_field(cx)),
                    );
                } else if menu_visible {
                    let more_group = SharedString::from(format!("macro-tree-more-{id}"));
                    let trigger = BaseButton::new(("macro-tree-more", id))
                        .group(more_group.clone())
                        .size(css(24.))
                        .p_0()
                        .accessibility_label(entry.name)
                        .child(
                            div()
                                .relative()
                                .size_full()
                                .child(img("synapse/profile-more.svg").size_full())
                                .child(
                                    img("synapse/macro/tree-more-hover.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .opacity(0.)
                                        .group_hover(more_group.clone(), |s| s.opacity(1.)),
                                )
                                .child(
                                    img("synapse/macro/tree-more-active.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .opacity(if self.tree_menu == Some(id) { 1. } else { 0. })
                                        .group_active(more_group, |s| s.opacity(1.)),
                                ),
                        );
                    row = row.child(div().absolute().right(css(6.)).top(css(3.)).child(
                        self.popup(
                            ("macro-tree-popup", id),
                            trigger,
                            Menu::Tree(id),
                            window,
                            cx,
                        ),
                    ));
                }
                div().w_full().pl(css(depth as f32 * 20.)).child(row)
            }))
            .into_any_element()
    }

    pub(super) fn tree_menu(
        &self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let keys = [
            "TEXT_PROFILE_BAR_S3_DROPDOWN_RENAME",
            "TEXT_PROFILE_BAR_S3_DROPDOWN_DUPLICATE",
            "TEXT_PROFILE_BAR_S3_DROPDOWN_DELETE",
        ];
        // .structureFolder .profile-act.show overrides fixed180px with auto.
        let mut font = window.text_style().font();
        font.family = "Roboto".into();
        font.weight = FontWeight::NORMAL;
        let width = keys
            .iter()
            .map(|key| {
                let text = capitalize(tr(key));
                let run = TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: rgb(0xcccccc).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window
                    .text_system()
                    .shape_line(text.into(), window.rem_size() * (14. / 16.), &[run], None)
                    .width
            })
            .fold(px(0.), |a, b| a.max(b))
            + window.rem_size() * (14. / 16.);
        let opacity = Presence::new((ElementId::from(("macro-tree-menu", id)), "opacity"), true)
            .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        v_flex()
            .id(("macro-tree-context-menu", id))
            .occlude()
            .w(width)
            .bg(rgb(0))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .opacity(opacity)
            .child(
                menu_action(("macro-tree-rename", id), keys[0], false, window, cx).on_click(
                    cx.listener(move |this, _, window, cx| this.start_rename(id, true, window, cx)),
                ),
            )
            .child(
                menu_action(("macro-tree-duplicate", id), keys[1], false, window, cx).on_click(
                    cx.listener(move |this, _, _, cx| this.duplicate_entry(id, false, true, cx)),
                ),
            )
            .child(div().h(css(1.)).mx(css(6.)).my(css(4.)).bg(rgb(0x5d5d5d)))
            .child(
                menu_action(("macro-tree-delete", id), keys[2], false, window, cx)
                    .on_click(cx.listener(move |this, _, _, cx| this.request_delete(id, cx))),
            )
            .into_any_element()
    }
}
