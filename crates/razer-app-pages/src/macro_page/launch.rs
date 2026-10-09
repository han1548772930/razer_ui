//! Current 58190.rt Type 10: local launch target drafts and native file selection.
use super::text_overlay::{TextAnchor, TextOverlay};
use super::*;
use razer_widgets::surface;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone)]
struct LaunchRow {
    baseline: ActionItem,
    mode: Option<bool>,
    program: String,
    website: String,
    armed: bool,
    trigger: Rc<Cell<Bounds<Pixels>>>,
    popup: Rc<Cell<Bounds<Pixels>>>,
    upward: Rc<Cell<Option<bool>>>,
}

fn mode(item: &ActionItem) -> Option<bool> {
    match item.state.as_str() {
        "program" => Some(false),
        "website" => Some(true),
        _ => None,
    }
}

impl LaunchRow {
    fn new(item: &ActionItem) -> Self {
        Self {
            baseline: item.clone(),
            mode: mode(item),
            program: item.value.clone(),
            website: item.secondary_value.clone(),
            armed: mode(item).is_some(),
            trigger: Default::default(),
            popup: Default::default(),
            upward: Default::default(),
        }
    }
    fn active_has_content(&self) -> bool {
        match self.mode {
            Some(false) => !self.program.is_empty(),
            Some(true) => !self.website.is_empty(),
            None => false,
        }
    }
    fn discard(&mut self, item: &ActionItem) {
        let mode_changed = self.mode != mode(item);
        let content_changed = self.program != item.value || self.website != item.secondary_value;
        self.mode = mode(item);
        self.program = item.value.clone();
        self.website = item.secondary_value.clone();
        self.baseline = item.clone();
        // k calls j (D(false)), then effects rerun only for changed values.
        // The later [p] effect arms any numeric RadioIndex, even with no path.
        self.armed =
            mode_changed && self.mode.is_some() || content_changed && self.active_has_content();
    }
}

#[derive(Default)]
pub struct LaunchUi {
    rows: RefCell<HashMap<(u64, usize), Rc<RefCell<LaunchRow>>>>,
    generation: u64,
    picker_pending: bool,
}

impl MacroPage {
    fn launch_row(&self, index: usize) -> Rc<RefCell<LaunchRow>> {
        let item = &self.actions()[index];
        let mut rows = self.launch_ui.rows.borrow_mut();
        let row = rows
            .entry((self.current.unwrap_or(0), index))
            .or_insert_with(|| Rc::new(RefCell::new(LaunchRow::new(item))));
        if row.borrow().baseline != *item {
            *row.borrow_mut() = LaunchRow::new(item);
        }
        row.clone()
    }

    pub fn open_launch_editor(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.launch_open == Some(index) {
            return;
        }
        self.finish_pending_edits(window, cx);
        self.cancel_launch_editor();
        self.choice_action = None;
        if !self
            .actions()
            .get(index)
            .is_some_and(|item| item.kind == ActionKind::Launch)
        {
            return;
        }
        let row = self.launch_row(index);
        row.borrow().upward.set(None);
        self.launch_open = Some(index);
        self.launch_ui.generation = self.launch_ui.generation.wrapping_add(1);
        self.launch_website.update(cx, |input, cx| {
            input.set_value(row.borrow().website.clone(), window, cx)
        });
        cx.notify();
    }

    pub fn cancel_launch_editor(&mut self) {
        if let Some(index) = self.launch_open.take() {
            if let Some(item) = self
                .actions()
                .get(index)
                .filter(|item| item.kind == ActionKind::Launch)
                && let Some(row) = self
                    .launch_ui
                    .rows
                    .borrow()
                    .get(&(self.current.unwrap_or(0), index))
            {
                row.borrow_mut().discard(item);
            }
            self.launch_ui.generation = self.launch_ui.generation.wrapping_add(1);
            self.launch_ui.picker_pending = false;
        }
    }

    pub fn launch_website_changed(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.launch_open else {
            return;
        };
        let row = self.launch_row(index);
        let mut row = row.borrow_mut();
        let value = self.launch_website.read(cx).value().to_string();
        if row.website != value {
            row.website = value;
            row.armed = row.active_has_content();
        }
        cx.notify();
    }

    fn choose_launch_mode(&mut self, website: bool, cx: &mut Context<Self>) {
        let Some(index) = self.launch_open else {
            return;
        };
        let row = self.launch_row(index);
        let mut row = row.borrow_mut();
        if row.mode != Some(website) {
            row.mode = Some(website);
            row.armed = true;
            self.launch_ui.generation = self.launch_ui.generation.wrapping_add(1);
            self.launch_ui.picker_pending = false;
        }
        cx.notify();
    }

    fn browse_launch_program(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.launch_open else {
            return;
        };
        if self.launch_ui.picker_pending || self.launch_row(index).borrow().mode != Some(false) {
            return;
        }
        self.launch_ui.generation = self.launch_ui.generation.wrapping_add(1);
        let generation = self.launch_ui.generation;
        let document = self.current;
        let baseline = self.actions()[index].clone();
        self.launch_ui.picker_pending = true;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select Launch App".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = picker.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.launch_ui.generation != generation
                    || this.current != document
                    || this.launch_open != Some(index)
                    || this.actions().get(index) != Some(&baseline)
                {
                    return;
                }
                this.launch_ui.picker_pending = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let row = this.launch_row(index);
                            let mut row = row.borrow_mut();
                            if row.mode == Some(false) {
                                row.program = path.to_string_lossy().into_owned();
                                row.armed = true;
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        window.push_notification("无法打开文件选择器，原路径已保留。", cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn save_launch_editor(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.launch_open else {
            return;
        };
        if self.actions_for != self.current
            || !self
                .actions()
                .get(index)
                .is_some_and(|item| item.kind == ActionKind::Launch)
        {
            return;
        }
        let row = self.launch_row(index);
        let mut row = row.borrow_mut();
        if !row.armed {
            return;
        }
        let state = match row.mode {
            Some(false) => "program",
            Some(true) => "website",
            None => "",
        };
        let item = &self.actions[index];
        if item.value != row.program || item.secondary_value != row.website || item.state != state {
            self.undo.push(self.actions.clone());
            self.actions[index].value = row.program.clone();
            self.actions[index].secondary_value = row.website.clone();
            self.actions[index].state = state.into();
            self.redo.clear();
        }
        row.baseline = self.actions[index].clone();
        row.armed = false;
        self.launch_open = None;
        self.launch_ui.generation = self.launch_ui.generation.wrapping_add(1);
        self.launch_ui.picker_pending = false;
        cx.notify();
    }

    pub fn launch_value_popup(
        &self,
        index: usize,
        item: ActionItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = self.launch_row(index);
        let state = row.borrow();
        let trigger_bounds = state.trigger.clone();
        let outside = state.popup.clone();
        let editor = self.text_ui.editor_bounds.clone();
        let opened = self.launch_open == Some(index);
        let selected = mode(&item);
        let label = match selected {
            Some(false) => format!(
                "{} \"{}\"",
                tr("TEXT_LAUNCH_PROGRAM"),
                state.program.rsplit('\\').next().unwrap_or_default()
            ),
            Some(true) => format!("{} \"{}\"", tr("TEXT_LAUNCH_WEBSITE"), state.website),
            None => tr("TEXT_PROGRAM_OR_WEBSITE"),
        };
        let trigger_group = SharedString::from(format!("macro-launch-label-{index}"));
        let trigger = BaseButton::new(("macro-launch-target", index))
            .group(trigger_group.clone())
            .max_w(css(350.))
            .p_0()
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(rgb(if self.launch_open == Some(index) {
                0x44d62c
            } else {
                0x707070
            }))
            .hover(|s| s.text_color(rgb(0x44d62c)))
            .child(if selected.is_some() {
                h_flex()
                    .text_color(rgb(0xffffff))
                    .child(
                        div()
                            .mr(css(10.))
                            .child(format!("{}:", tr("TEXT_ADD_MENU_LAUNCH"))),
                    )
                    .child(
                        div()
                            .truncate()
                            .group_hover(trigger_group, |style| style.text_color(rgb(0x44d62c)))
                            .child(label),
                    )
                    .into_any_element()
            } else {
                div().truncate().child(label).into_any_element()
            })
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_launch_editor(index, window, cx)),
            );
        let anchor = TextAnchor::Launch {
            trigger: state.trigger.clone(),
            editor: editor.clone(),
            row_offset: self.phase_row_offset(index),
            upward: state.upward.clone(),
        };
        drop(state);
        div()
            .id(("macro-launch-root", index))
            .on_prepaint(move |bounds, _, _| trigger_bounds.set(bounds))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .when(opened, |view| {
                view.on_mouse_down_out(cx.listener(
                    move |this, event: &MouseDownEvent, window, cx| {
                        if !outside.get().contains(&event.position)
                            || !editor.get().contains(&event.position)
                        {
                            this.cancel_launch_editor();
                            this.focus.focus(window, cx);
                            cx.notify();
                        }
                    },
                ))
            })
            .child(trigger)
            .when(opened, |view| {
                view.child(
                    deferred(TextOverlay {
                        content: self.launch_modal(index, window, cx),
                        anchor,
                    })
                    .priority(100),
                )
            })
            .into_any_element()
    }

    fn launch_modal(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = self.launch_row(index);
        let row = row.borrow();
        let popup_bounds = row.popup.clone();
        let program_active = row.mode == Some(false);
        let website_active = row.mode == Some(true);
        let close_id: ElementId = ("macro-launch-close", index).into();
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
        v_flex()
            .id(("macro-launch-modal", index))
            .occlude()
            .on_prepaint(move |bounds, _, _| popup_bounds.set(bounds))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .relative()
            .w(css(250.))
            .p(css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(css(5.))
            .font_family("Roboto")
            .text_size(css(14.))
            .line_height(css(17.))
            .shadow(vec![BoxShadow {
                color: rgba(0x000000b3).into(),
                offset: point(px(0.), px(0.)),
                blur_radius: window.rem_size() * (20. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .child(
                surface::track_pointer(BaseButton::new(close_id), &pointer, window)
                    .absolute()
                    .right_0()
                    .top_0()
                    .size(css(36.))
                    .p_0()
                    .bg(close_bg)
                    .child(img("synapse/macro/close.svg").size(css(20.)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.cancel_launch_editor();
                        this.focus.focus(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .line_height(css(19.))
                    .text_color(rgb(0x44d62c))
                    .mb(css(20.))
                    .child(tr("TEXT_ADD_MENU_LAUNCH").to_uppercase()),
            )
            .child(self.launch_radio(index, false, program_active, window, cx))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(css(29.))
                    .child(
                        BaseButton::new(("macro-launch-program-input", index))
                            .absolute()
                            .left(css(30.))
                            .top_0()
                            .w(css(142.))
                            .h(css(29.))
                            .p_0()
                            .pl(css(6.))
                            .pr(css(30.))
                            .justify_start()
                            .border_1()
                            .border_color(if program_active {
                                rgba(0x5d5d5dff)
                            } else {
                                rgba(0x5d5d5d4d)
                            })
                            .text_color(if program_active {
                                rgba(0xccccccff)
                            } else {
                                rgba(0xcccccc4d)
                            })
                            .text_size(css(14.))
                            .line_height(css(27.))
                            .disabled(!program_active || self.launch_ui.picker_pending)
                            .child(
                                div().truncate().child(
                                    row.program
                                        .rsplit('\\')
                                        .next()
                                        .unwrap_or_default()
                                        .to_owned(),
                                ),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.browse_launch_program(window, cx)
                            })),
                    )
                    .child(
                        img(if program_active {
                            "synapse/macro/launch-folder.svg"
                        } else {
                            "synapse/macro/drag-folder.svg"
                        })
                        .absolute()
                        .right(css(4.))
                        .top(css(5.))
                        .w(css(20.))
                        .h(css(16.)),
                    ),
            )
            .child(self.launch_radio(index, true, website_active, window, cx))
            .child(
                Input::new(&self.launch_website)
                    .id(("macro-launch-website-input", index))
                    .appearance(false)
                    .bordered(false)
                    .ml(css(30.))
                    .w(css(164.))
                    .h(css(27.))
                    .px(css(6.))
                    .py_0()
                    .mb(css(20.))
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(if website_active {
                        rgba(0x5d5d5dff)
                    } else {
                        rgba(0x5d5d5d4d)
                    })
                    .text_color(if website_active {
                        rgba(0xccccccff)
                    } else {
                        rgba(0xcccccc4d)
                    })
                    .text_size(css(14.))
                    .rounded_none()
                    .disabled(!website_active),
            )
            .child(
                h_flex()
                    .w_full()
                    .child(
                        super::text::text_button(
                            ("macro-launch-cancel", index).into(),
                            tr("TEXT_LAUNCH_CANCEL"),
                            false,
                            false,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.cancel_launch_editor();
                            this.focus.focus(window, cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        super::text::text_button(
                            ("macro-launch-save", index).into(),
                            tr("TEXT_LAUNCH_SAVE"),
                            true,
                            !row.armed,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.save_launch_editor(cx);
                            this.focus.focus(window, cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    fn launch_radio(
        &self,
        index: usize,
        website: bool,
        active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id: ElementId =
            SharedString::from(format!("macro-launch-radio-{index}-{website}")).into();
        let reveal = motion::transition(
            (id.clone(), "radio-dot"),
            if active { 1f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        BaseButton::new(id)
            .relative()
            .w_full()
            .my(css(10.))
            .p_0()
            .pl(css(30.))
            .h(css(17.))
            .justify_start()
            .text_size(css(14.))
            .line_height(css(17.))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .size(css(20.))
                    .border_1()
                    .border_color(rgb(0x737373))
                    .rounded_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(css(10. - 5. * reveal))
                    .top(css(10. - 5. * reveal))
                    .size(css(10. * reveal))
                    .rounded_full()
                    .bg(rgb(0x44d62c))
                    .opacity(reveal),
            )
            .child(tr(if website {
                "TEXT_LAUNCH_WEBSITE"
            } else {
                "TEXT_LAUNCH_PROGRAM"
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.choose_launch_mode(website, cx)))
            .into_any_element()
    }
}
