//! Current 58190 Za/$r/Ur/vr and 25572.G. Local recording preferences do not
//! register a system shortcut or imply a connected recording service.
use super::*;
use crate::{features::macro_library::MacroType, ui::surface};
use gpui_kit::component::select::SelectState;
use serde::Deserialize;
use std::{cell::Cell, rc::Rc, sync::OnceLock};

#[derive(Deserialize)]
struct Data {
    groups: Vec<Group>,
}
#[derive(Deserialize)]
struct Group {
    id: String,
    header: Header,
    #[serde(default)]
    content: Vec<Item>,
}
#[derive(Deserialize)]
struct Header {
    name: String,
    tip: String,
}
#[derive(Deserialize)]
struct Item {
    name: String,
}
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("record_options_data.json"))
            .expect("audited record settings")
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NumberField {
    Fixed,
    Min,
    Max,
}
pub(super) struct RecordUi {
    pub(super) open: bool,
    pub(super) shortcut: super::record_shortcut::ShortcutUi,
    pub(super) start: Entity<SelectState<Vec<String>>>,
    pub(super) fixed: Entity<InputState>,
    pub(super) min: Entity<InputState>,
    pub(super) max: Entity<InputState>,
    delay_time: f64,
    random: [f64; 2],
    mouse: usize,
    generation: [u64; 2],
    ignored_drafts: [Option<String>; 3],
    help: Option<usize>,
    help_bounds: Vec<Rc<Cell<Bounds<Pixels>>>>,
    pub(super) trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
}
impl RecordUi {
    pub(super) fn new(window: &mut Window, cx: &mut Context<MacroPage>) -> Self {
        Self {
            open: false,
            shortcut: super::record_shortcut::ShortcutUi::new(cx.focus_handle()),
            start: cx
                .new(|cx| SelectState::new(start_options(), Some(IndexPath::new(0)), window, cx)),
            fixed: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value("0")
                    .validate(|value, _| normalized_number(value, false).is_some())
            }),
            min: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value("0")
                    .validate(|value, _| normalized_number(value, true).is_some())
            }),
            max: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value("0")
                    .validate(|value, _| normalized_number(value, true).is_some())
            }),
            delay_time: 0.,
            random: [0., 0.],
            mouse: 0,
            generation: [0, 0],
            ignored_drafts: [None, None, None],
            help: None,
            help_bounds: (0..5).map(|_| Default::default()).collect(),
            trigger_bounds: Default::default(),
        }
    }
    pub(super) fn close(&mut self) {
        self.open = false;
        self.help = None;
        self.shortcut.stop();
    }
}
fn start_options() -> Vec<String> {
    (0..=5).map(|i| format!("{i}s")).collect()
}

// vr truncates the text before parseFloat. The decimal fixed-delay path does
// not limit the integer part. Reject non-finite values at the native boundary.
fn normalized_number(raw: &str, random: bool) -> Option<String> {
    if raw.is_empty() {
        return Some(String::new());
    }
    let n = source_number(raw)?;
    if !n.is_finite() {
        return None;
    }
    let (integer, fraction) = raw
        .split_once('.')
        .map_or((raw, None), |(i, f)| (i, Some(f)));
    let integer: String = if random || fraction.is_none() {
        integer.chars().take(3).collect()
    } else {
        integer.into()
    };
    let value = match fraction {
        Some(f) => format!("{integer}.{}", f.chars().take(3).collect::<String>()),
        None => integer,
    };
    if random && source_float(&value)? > 5. {
        return None;
    }
    Some(value)
}

// The source uses Number/isNaN for acceptance and parseFloat for submission.
// Keep those distinct (for example Number("0x5")=5, parseFloat("0x5")=0).
fn source_number(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if value.is_empty() {
        return Some(0.);
    }
    let radix = if value.starts_with("0x") || value.starts_with("0X") {
        Some(16)
    } else if value.starts_with("0b") || value.starts_with("0B") {
        Some(2)
    } else if value.starts_with("0o") || value.starts_with("0O") {
        Some(8)
    } else {
        None
    };
    let number = if let Some(radix) = radix {
        if value.len() == 2 {
            return None;
        }
        value[2..].chars().try_fold(0., |n, c| {
            c.to_digit(radix).map(|d| n * radix as f64 + d as f64)
        })?
    } else {
        value.parse::<f64>().ok()?
    };
    number.is_finite().then_some(number)
}
fn source_float(raw: &str) -> Option<f64> {
    let value = raw.trim_start();
    let bytes = value.as_bytes();
    let mut end = usize::from(bytes.first().is_some_and(|c| matches!(c, b'+' | b'-')));
    let start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut digits = end - start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        digits += end - start;
    }
    if digits == 0 {
        return None;
    }
    if bytes.get(end).is_some_and(|c| matches!(c, b'e' | b'E')) {
        let exponent = end;
        end += 1;
        if bytes.get(end).is_some_and(|c| matches!(c, b'+' | b'-')) {
            end += 1;
        }
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == start {
            end = exponent;
        }
    }
    value[..end].parse::<f64>().ok().filter(|n| n.is_finite())
}

impl MacroPage {
    pub(super) fn current_macro_type(&self) -> MacroType {
        self.entries
            .iter()
            .find(|e| Some(e.id) == self.current)
            .map(|e| e.macro_type)
            .unwrap_or_default()
    }
    pub(super) fn record_delay(&self) -> u8 {
        self.entries
            .iter()
            .find(|e| Some(e.id) == self.current)
            .map_or(0, |e| e.record_delay)
    }
    pub(super) fn toggle_record_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording_busy() || self.current.is_none() || self.tutorial != Tutorial::Complete {
            return;
        }
        if self.record_ui.open {
            self.record_ui.close();
        } else {
            self.finish_pending_edits(window, cx);
            self.clear_action_editors();
            self.reset_record_number_drafts(window, cx);
            self.record_ui.open = true;
            if self.current_macro_type() != MacroType::Standard {
                self.set_record_delay(0, window, cx);
                self.record_ui.mouse = 0;
            }
        }
        cx.notify();
    }
    fn set_record_type(&mut self, kind: MacroType, window: &mut Window, cx: &mut Context<Self>) {
        if !self.actions().is_empty() || kind == MacroType::Phased {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|e| Some(e.id) == self.current) {
            entry.macro_type = kind;
            if kind != MacroType::Standard {
                entry.record_delay = 0;
                self.record_ui.mouse = 0;
            }
            if kind == MacroType::Standard {
                self.reset_record_number_drafts(window, cx);
            }
            self.publish_library(cx);
            cx.notify();
        }
    }
    fn write_record_draft(
        &mut self,
        field: NumberField,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (slot, input) = match field {
            NumberField::Fixed => (0, self.record_ui.fixed.clone()),
            NumberField::Min => (1, self.record_ui.min.clone()),
            NumberField::Max => (2, self.record_ui.max.clone()),
        };
        if input.read(cx).value().as_ref() == value {
            return;
        }
        self.record_ui.ignored_drafts[slot] = Some(value.clone());
        input.update(cx, |s, cx| s.set_value(value, window, cx));
    }
    fn reset_record_number_drafts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.write_record_draft(
            NumberField::Fixed,
            self.record_ui.delay_time.to_string(),
            window,
            cx,
        );
        self.write_record_draft(
            NumberField::Min,
            self.record_ui.random[0].to_string(),
            window,
            cx,
        );
        self.write_record_draft(
            NumberField::Max,
            self.record_ui.random[1].to_string(),
            window,
            cx,
        );
    }
    fn set_record_delay(&mut self, value: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_randomized_range(false, window, cx);
        if let Some(entry) = self.entries.iter_mut().find(|e| Some(e.id) == self.current) {
            entry.record_delay = value;
            self.publish_library(cx);
            if value == 1 {
                self.record_ui.fixed.update(cx, |s, cx| s.focus(window, cx));
            }
            cx.notify();
        }
    }
    pub(super) fn record_number_changed(
        &mut self,
        field: NumberField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = match field {
            NumberField::Fixed => &self.record_ui.fixed,
            NumberField::Min => &self.record_ui.min,
            NumberField::Max => &self.record_ui.max,
        }
        .clone();
        let raw = input.read(cx).value().to_string();
        let slot = match field {
            NumberField::Fixed => 0,
            NumberField::Min => 1,
            NumberField::Max => 2,
        };
        if self.record_ui.ignored_drafts[slot].take().as_deref() == Some(raw.as_str()) {
            return;
        }
        let Some(value) = normalized_number(&raw, field != NumberField::Fixed) else {
            return;
        };
        if raw != value {
            self.write_record_draft(field, value.clone(), window, cx);
        }
        if value.is_empty() {
            return;
        }
        let Some(number) = source_float(&value) else {
            return;
        };
        let (slot, pair) = if field == NumberField::Fixed {
            (0, [number, 0.])
        } else {
            let min = if field == NumberField::Min {
                number.max(0.)
            } else {
                source_number(&self.record_ui.min.read(cx).value()).unwrap_or(0.)
            };
            let mut max = if field == NumberField::Max {
                number
            } else {
                source_number(&self.record_ui.max.read(cx).value()).unwrap_or(0.)
            };
            if (field == NumberField::Min && min >= max) || (field == NumberField::Max && max < min)
            {
                max = (min + 1.).min(5.);
                self.write_record_draft(NumberField::Max, max.to_string(), window, cx);
            }
            (1, [min, max])
        };
        self.record_ui.generation[slot] = self.record_ui.generation[slot].wrapping_add(1);
        let generation = self.record_ui.generation[slot];
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update(cx, |this, cx| {
                // vr keeps only the randomized debounce in a useRef. Fixed
                // input renders create separate delayed callbacks.
                if slot == 1 && this.record_ui.generation[slot] != generation {
                    return;
                }
                if slot == 0 {
                    this.record_ui.delay_time = pair[0];
                } else {
                    this.record_ui.random = pair;
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn macro_type_status(&self) -> String {
        match self.current_macro_type() {
            MacroType::Sequence => tr("TEXT_SEQUENCE_MACRO"),
            MacroType::Phased => tr("TEXT_PHASED_MACRO"),
            MacroType::Standard if self.record_delay() == 2 => format!(
                "{}s - {}s",
                self.record_ui.random[0], self.record_ui.random[1]
            ),
            MacroType::Standard => format!(
                "{:.3}s",
                self.actions()
                    .iter()
                    .filter(|a| a.kind == ActionKind::Delay || a.mouse_movement.is_some())
                    .map(|a| a
                        .value
                        .parse::<f64>()
                        .ok()
                        .filter(|n| n.is_finite())
                        .unwrap_or(0.))
                    .sum::<f64>()
            ),
        }
    }
    pub(super) fn recording_mode_label(&self) -> AnyElement {
        let label = match self.current_macro_type() {
            MacroType::Sequence => tr("TEXT_SEQUENCE_MACRO"),
            MacroType::Phased => tr("TEXT_PHASED_MACRO"),
            MacroType::Standard => match self.record_delay() {
                1 => tr("TEXT_SECOND_DELAY")
                    .replace("{{times}}", &self.record_ui.delay_time.to_string()),
                2 => tr("TEXT_RANDOMIZE_DELAY").replace(
                    "{{times}}",
                    &format!(
                        "{}s - {}s",
                        self.record_ui.random[0], self.record_ui.random[1]
                    ),
                ),
                3 => tr("TEXT_NO_DELAY"),
                _ => tr("TEXT_RECORD_DELAY"),
            },
        };
        v_flex()
            .id("macro-recording-mode")
            .text_size(css(12.))
            .text_right()
            .child(label)
            .when(self.record_ui.mouse > 0, |column| {
                column.child(tr(match self.record_ui.mouse {
                    1 => "TEXT_ABSOLUTE_POSITION_MOUSE",
                    2 => "TEXT_RELATIVE_POSITION_MOUSE",
                    _ => "TEXT_MOUSE_TRACKING",
                }))
            })
            .into_any_element()
    }
    pub(super) fn record_options(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let standard = self.current_macro_type() == MacroType::Standard;
        let height = f32::from(window.viewport_size().height / window.rem_size()) * 16.;
        div()
            .id("macro-record-settings")
            .test_support()
            // This deferred popup overlaps a list whose capture handler is
            // disabled while settings are open. It must own that hit area.
            .occlude()
            .aria_label("宏录制设置")
            .absolute()
            .right_0()
            .top(css(28.))
            .w(css(268.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .text_size(css(14.))
            .line_height(css(17.))
            .when(standard, |v| {
                v.h(css(581.))
                    .max_h(css((height - 42. - 185.).max(0.)))
                    .overflow_x_hidden()
            })
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                if !this
                    .record_ui
                    .trigger_bounds
                    .get()
                    .contains(&event.position)
                {
                    this.record_ui.close();
                    cx.notify();
                }
            }))
            .children(
                data()
                    .groups
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| standard || !matches!(g.id.as_str(), "delaySet" | "mmt"))
                    .map(|(index, group)| {
                        let last = if standard { index == 4 } else { index == 2 };
                        let mut header = h_flex().items_center().justify_between().child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_ellipsis()
                                .child(tr(&group.header.name).to_uppercase()),
                        );
                        if index == 0 {
                            header = header.child(
                                surface::select(&self.record_ui.start)
                                    .id("macro-record-start-delay")
                                    .accessibility_label(tr(&group.header.name))
                                    .items(start_options())
                                    .mx(css(10.))
                                    .w(css(50.)),
                            );
                        }
                        header =
                            header.child(self.record_help(index, &group.header.tip, window, cx));
                        let mut section = div()
                            .mx(css(20.))
                            .py(css(15.))
                            .when(!last, |v| v.border_b_1().border_color(rgb(0x5d5d5d)))
                            .child(header);
                        match index {
                            1 => {
                                section = section
                                    .child(self.record_shortcut_control(window, cx))
                                    .child(
                                        div()
                                            .id("macro-record-shortcut-unavailable")
                                            .test_support()
                                            .text_size(css(12.))
                                            .text_color(rgb(0xaaaaaa))
                                            .child("快捷键仅为本地设置；全局快捷键录制尚未连接，请使用录制/停止按钮"),
                                    )
                            }
                            2 => {
                                section = section.children(
                                    group.content.iter().take(2).enumerate().map(|(i, item)| {
                                        let kind = if i == 0 {
                                            MacroType::Standard
                                        } else {
                                            MacroType::Sequence
                                        };
                                        self.record_radio(
                                            ("macro-record-type", i),
                                            &tr(&item.name),
                                            self.current_macro_type() == kind,
                                            !self.actions().is_empty()
                                                && self.current_macro_type() != kind,
                                            window,
                                            cx,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.set_record_type(kind, window, cx)
                                            }),
                                        )
                                    }),
                                )
                            }
                            3 => {
                                section = section.children(group.content.iter().enumerate().map(
                                    |(i, item)| {
                                        let selected = self.record_delay() == i as u8;
                                        let label = tr(&item.name);
                                        let radio = self
                                            .record_radio(
                                                ("macro-record-delay", i),
                                                if i == 1 { "" } else { &label },
                                                selected,
                                                false,
                                                window,
                                                cx,
                                            )
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.set_record_delay(i as u8, window, cx)
                                            }));
                                        if i == 1 {
                                            h_flex()
                                                .child(radio)
                                                .child(record_input(
                                                    "macro-record-fixed",
                                                    &self.record_ui.fixed,
                                                    !selected,
                                                    window,
                                                    cx,
                                                ))
                                                .child(tr(&item.name))
                                                .into_any_element()
                                        } else if i == 2 {
                                            div()
                                                .child(radio)
                                                .child(
                                                    h_flex()
                                                        .pl(css(30.))
                                                        .w(css(219.))
                                                        .child(record_input(
                                                            "macro-record-min",
                                                            &self.record_ui.min,
                                                            !selected,
                                                            window,
                                                            cx,
                                                        ))
                                                        .child("-")
                                                        .child(div().ml(css(5.)).child(
                                                            record_input(
                                                                "macro-record-max",
                                                                &self.record_ui.max,
                                                                !selected,
                                                                window,
                                                                cx,
                                                            ),
                                                        ))
                                                        .child(tr("secs")),
                                                )
                                                .into_any_element()
                                        } else {
                                            radio.into_any_element()
                                        }
                                    },
                                ))
                            }
                            4 => {
                                section = section.children(group.content.iter().enumerate().map(
                                    |(i, item)| {
                                        self.record_radio(
                                            ("macro-record-mouse", i),
                                            &tr(&item.name),
                                            self.record_ui.mouse == i,
                                            i != 0,
                                            window,
                                            cx,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                if i != 0 {
                                                    return;
                                                }
                                                this.record_ui.mouse = i;
                                                cx.notify();
                                            }),
                                        )
                                    },
                                ))
                            }
                            _ => {}
                        }
                        if index == 4 {
                            section = section.child(
                                div()
                                    .id("macro-record-movement-unavailable")
                                    .test_support()
                                    .text_size(css(12.))
                                    .text_color(rgb(0xaaaaaa))
                                    .child("鼠标轨迹录制待接入监视器信息查询；当前可录制按键、鼠标按钮和滚轮"),
                            );
                        }
                        section
                    }),
            )
            .scrollable_y()
            .into_any_element()
    }
    fn record_radio(
        &self,
        id: (&'static str, usize),
        label: &str,
        selected: bool,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> BaseButton {
        let alpha = Presence::new((ElementId::from(id), "selected"), selected)
            .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
            .sample(window, cx)
            .progress;
        BaseButton::new(id)
            .selected(selected)
            .accessibility_label(format!(
                "{}；{}",
                if label.is_empty() {
                    "固定录制延迟".into()
                } else {
                    label.to_owned()
                },
                if selected { "已选择" } else { "未选择" }
            ))
            .when(!label.is_empty(), |v| v.w_full())
            .h(css(40.))
            .py(css(10.))
            .px_0()
            .justify_start()
            .disabled(disabled)
            .opacity(if disabled { 0.3 } else { 1. })
            .child(
                div()
                    .relative()
                    .size(css(20.))
                    .flex_shrink_0()
                    .mr(css(10.))
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(0x737373))
                    .child(
                        div()
                            .absolute()
                            .left(css(4. + 5. * (1. - alpha)))
                            .top(css(4. + 5. * (1. - alpha)))
                            .size(css(10. * alpha))
                            .rounded_full()
                            .bg(rgb(0x44d62c))
                            .opacity(alpha),
                    ),
            )
            .child(
                div()
                    .text_size(css(14.))
                    .line_height(css(17.))
                    .child(label.to_string()),
            )
    }
    fn record_help(
        &self,
        index: usize,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = self.record_ui.help == Some(index);
        let alpha = Presence::new(
            (ElementId::from(("macro-record-help", index)), "opacity"),
            hovered,
        )
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
        let target: Hsla = if hovered {
            rgba(0xffffff4d).into()
        } else {
            rgb(0x4a4a4a).into()
        };
        let background = motion::transition(
            (ElementId::from(("macro-record-help", index)), "background"),
            target,
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let anchor = self.record_ui.help_bounds[index].clone();
        div()
            .id(("macro-record-help-trigger", index))
            .relative()
            .size(css(14.))
            .flex_shrink_0()
            .on_prepaint({
                let anchor = anchor.clone();
                move |bounds, _, _| anchor.set(bounds)
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.record_ui.help = hovered.then_some(index);
                cx.notify();
            }))
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top(css(0.5))
                    .size(css(14.))
                    .rounded_full()
                    .bg(background)
                    .child(img("synapse/gr-help.svg").size_full()),
            )
            .when(hovered, |v| {
                v.child(
                    deferred(super::record_help::RecordHelp {
                        anchor,
                        upward: index == 4,
                        content: div()
                            .w(css(300.))
                            .px(css(10.))
                            .py(css(8.))
                            .bg(rgb(0))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .font_family("Roboto")
                            .text_size(css(14.))
                            .line_height(css(16.))
                            .text_color(rgb(0xcccccc))
                            .whitespace_normal()
                            .opacity(alpha)
                            .child(
                                tr(key)
                                    .replace("<br />", "\n")
                                    .replace("<br/>", "\n")
                                    .replace("<br>", "\n"),
                            )
                            .into_any_element(),
                    })
                    .with_priority(200),
                )
            })
            .into_any_element()
    }
}
fn record_input(
    id: &'static str,
    state: &Entity<InputState>,
    disabled: bool,
    window: &Window,
    cx: &App,
) -> AnyElement {
    div()
        .id((ElementId::from(id), "frame"))
        .test_support()
        .aria_label(format!(
            "{id}：{}；{}",
            state.read(cx).value(),
            if disabled {
                "不可编辑"
            } else {
                "可编辑"
            }
        ))
        .w(css(70.))
        .h(css(27.))
        .mr(css(5.))
        .border_1()
        .border_color(rgb(if state.read(cx).focus_handle(cx).is_focused(window) {
            0x44d62c
        } else {
            0x5d5d5d
        }))
        .hover(|s| s.border_color(rgb(0x44d62c)).cursor_pointer())
        .child(if disabled {
            // HTML's disabled input keeps the explicit #ccc color. Kit's
            // disabled Input halves its alpha, so paint the inert value here.
            div()
                .w(css(68.))
                .h(css(25.))
                .p(css(5.))
                .overflow_hidden()
                .text_size(css(14.))
                .line_height(css(17.))
                .text_color(rgb(0xcccccc))
                .child(state.read(cx).value().to_string())
                .into_any_element()
        } else {
            Input::new(state)
                .id(id)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .w(css(68.))
                .h(css(25.))
                .p(css(5.))
                .text_size(css(14.))
                .line_height(css(17.))
                .into_any_element()
        })
        .into_any_element()
}

impl MacroPage {
    pub(super) fn recording_options(
        &self,
        cx: &App,
    ) -> Result<super::recording_decode::Options, String> {
        if self.record_ui.mouse != 0 {
            return Err("鼠标轨迹录制尚未连接".into());
        }
        // Read accepted editor values too: the source 100ms debounce may
        // still be pending when Record is pressed immediately after input.
        let fixed = source_float(&self.record_ui.fixed.read(cx).value())
            .unwrap_or(self.record_ui.delay_time);
        let random = [
            source_float(&self.record_ui.min.read(cx).value()).unwrap_or(self.record_ui.random[0]),
            source_float(&self.record_ui.max.read(cx).value()).unwrap_or(self.record_ui.random[1]),
        ];
        let delay_mode = self.record_delay();
        if self.current_macro_type() == MacroType::Standard {
            if delay_mode == 1 && (!fixed.is_finite() || fixed < 0.) {
                return Err("固定录制延迟必须为非负数".into());
            }
            if delay_mode == 2
                && (!random
                    .iter()
                    .all(|value| value.is_finite() && (0. ..=5.).contains(value))
                    || random[0] > random[1])
            {
                return Err("随机录制延迟必须在 0–5 秒内，且最小值不能大于最大值".into());
            }
        }
        Ok(super::recording_decode::Options {
            macro_type: self.current_macro_type(),
            phase: (self.current_macro_type() == MacroType::Phased)
                .then(|| self.active_phase().unwrap_or(0)),
            delay_mode,
            fixed,
            random,
            next_pair: self.next_event_pair_id().ok_or("本地宏事件标识已耗尽")?,
        })
    }
}
