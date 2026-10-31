//! Current 7003.As/Ss controls and 5572.U recording preferences.
use super::editors::{source_float, source_number};
use super::*;
use gpui_kit::component::select::SelectState;
use razer_model::macro_library::{
    MovementMode,
    recording::{DelayMode, Preferences, Seconds},
};
use razer_pages::features::macro_library::MacroType;
use razer_widgets::surface;
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
    DATA.get_or_init(|| read_record_options_catalog())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NumberField {
    Fixed,
    Min,
    Max,
}
pub struct RecordUi {
    pub open: bool,
    pub shortcut: super::record_shortcut::ShortcutUi,
    pub start: Entity<SelectState<Vec<String>>>,
    pub fixed: Entity<InputState>,
    pub min: Entity<InputState>,
    pub max: Entity<InputState>,
    profile: Preferences,
    generation: [u64; 2],
    ignored_drafts: [Option<String>; 3],
    number_tasks: [Option<Task<()>>; 2],
    help: Option<usize>,
    pub(super) help_focus: Vec<FocusHandle>,
    help_bounds: Vec<Rc<Cell<Bounds<Pixels>>>>,
    pub trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
}
impl RecordUi {
    pub fn new(window: &mut Window, cx: &mut Context<MacroPage>) -> Self {
        let profile = Preferences::default();
        Self {
            open: false,
            shortcut: super::record_shortcut::ShortcutUi::new(cx.focus_handle()),
            start: cx
                .new(|cx| SelectState::new(start_options(), Some(IndexPath::new(0)), window, cx)),
            fixed: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(profile.fixed.value().to_string())
                    .validate(|value, _| normalized_number(value, false).is_some())
            }),
            min: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(profile.random[0].value().to_string())
                    .validate(|value, _| normalized_number(value, true).is_some())
            }),
            max: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(profile.random[1].value().to_string())
                    .validate(|value, _| normalized_number(value, true).is_some())
            }),
            profile,
            generation: [0, 0],
            ignored_drafts: [None, None, None],
            number_tasks: [None, None],
            help: None,
            help_focus: (0..5).map(|_| cx.focus_handle().tab_stop(true)).collect(),
            help_bounds: (0..5).map(|_| Default::default()).collect(),
            trigger_bounds: Default::default(),
        }
    }
    pub fn close(&mut self) {
        self.open = false;
        self.help = None;
        self.shortcut.stop();
    }

    pub(super) fn set_help(&mut self, index: usize, visible: bool) {
        self.help = (visible && self.open).then_some(index);
    }

    fn delay_time(&self) -> f64 {
        self.profile.fixed.value()
    }

    fn random(&self) -> [f64; 2] {
        self.profile.random.map(Seconds::value)
    }

    fn mouse(&self) -> usize {
        usize::from(self.profile.movement.setting())
    }
}
fn start_options() -> Vec<String> {
    (0..=5).map(|i| format!("{i}s")).collect()
}

// As keeps the current input text distinct from its accepted delay. Both
// integer and fractional segments are truncated before parsing.
fn normalized_number(raw: &str, random: bool) -> Option<String> {
    if raw.is_empty() {
        return Some(String::new());
    }
    if random {
        if !source_number(raw)?.is_finite() {
            return None;
        }
    } else if raw.chars().filter(|c| *c == '.').count() > 1
        || raw.chars().any(|c| !c.is_ascii_digit() && c != '.')
    {
        return None;
    }
    let (integer, fraction) = raw
        .split_once('.')
        .map_or((raw, None), |(i, f)| (i, Some(f)));
    let integer: String = integer.chars().take(3).collect();
    let value = match fraction {
        Some(f) => format!("{integer}.{}", f.chars().take(3).collect::<String>()),
        None => integer,
    };
    if random && source_float(&value)? > 5. {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod number_tests {
    use super::normalized_number;

    #[::core::prelude::v1::test]
    fn fixed_text_accepts_partial_decimals_and_truncates_each_segment() {
        for (input, expected) in [
            ("", ""),
            (".", "."),
            ("4.", "4."),
            (".1256", ".125"),
            ("1234.5678", "123.567"),
        ] {
            assert_eq!(normalized_number(input, false).as_deref(), Some(expected));
        }
        for input in ["-1", "+1", "1e2", " 1", "1.2.3", "Infinity", "NaN"] {
            assert!(normalized_number(input, false).is_none());
        }
    }

    #[::core::prelude::v1::test]
    fn randomized_text_keeps_source_number_and_parse_float_acceptance() {
        for (input, expected) in [
            ("", ""),
            ("-1", "-1"),
            ("0x1", "0x1"),
            ("0.1234", "0.123"),
            ("5.", "5."),
        ] {
            assert_eq!(normalized_number(input, true).as_deref(), Some(expected));
        }
        for input in [".", "5.001", "6", "not a number"] {
            assert!(normalized_number(input, true).is_none());
        }
    }
}

impl MacroPage {
    pub(super) fn record_preferences(&self) -> Preferences {
        self.record_ui.profile.clone()
    }

    pub(super) fn adopt_record_preferences(
        &mut self,
        profile: Option<Preferences>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let profile = profile.unwrap_or_else(|| {
            // Migration of local files that did not yet store recordProfile.
            // Co supplies numeric defaults; existing document metadata stays.
            let mut profile = Preferences::default();
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| Some(entry.id) == self.current)
            {
                profile.macro_type = entry.macro_type;
                if let Ok(mode) = DelayMode::from_setting(entry.record_delay) {
                    profile.delay_mode = mode;
                }
                if let Ok(execution) = entry.editor_execution() {
                    profile.movement = execution.engine().movement;
                }
            }
            profile
        });
        if let Err(error) = profile.validate() {
            self.recording.error = Some(error);
            return;
        }
        let fixed_changed = self.record_ui.profile.fixed != profile.fixed;
        self.record_ui.profile = profile;
        if fixed_changed {
            self.write_record_draft(
                NumberField::Fixed,
                self.record_ui.delay_time().to_string(),
                window,
                cx,
            );
        }
        self.write_record_draft(
            NumberField::Min,
            self.record_ui.random()[0].to_string(),
            window,
            cx,
        );
        self.write_record_draft(
            NumberField::Max,
            self.record_ui.random()[1].to_string(),
            window,
            cx,
        );
        let selected = usize::from(self.record_ui.profile.start_delay);
        self.record_ui.start.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(selected)), window, cx);
        });
    }

    pub(super) fn set_record_start_delay(&mut self, value: &str, cx: &mut Context<Self>) {
        let Some(delay) = value
            .strip_suffix('s')
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|delay| *delay <= 5)
        else {
            return;
        };
        if self.record_ui.profile.start_delay != delay {
            self.record_ui.profile.start_delay = delay;
            self.publish_library(cx);
        }
    }

    pub fn current_macro_type(&self) -> MacroType {
        self.entries
            .iter()
            .find(|e| Some(e.id) == self.current)
            .map(|e| e.macro_type)
            .unwrap_or_default()
    }
    pub fn record_delay(&self) -> u8 {
        self.entries
            .iter()
            .find(|e| Some(e.id) == self.current)
            .map_or(0, |e| e.record_delay)
    }
    pub fn toggle_record_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                self.record_ui.profile.movement = MovementMode::None;
                self.set_record_delay(0, window, cx);
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
            self.record_ui.profile.macro_type = kind;
            if kind != MacroType::Standard {
                entry.record_delay = 0;
                self.record_ui.profile.delay_mode = DelayMode::Recorded;
                self.record_ui.profile.movement = MovementMode::None;
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
            self.record_ui.delay_time().to_string(),
            window,
            cx,
        );
        self.write_record_draft(
            NumberField::Min,
            self.record_ui.random()[0].to_string(),
            window,
            cx,
        );
        self.write_record_draft(
            NumberField::Max,
            self.record_ui.random()[1].to_string(),
            window,
            cx,
        );
    }
    fn set_record_delay(&mut self, value: u8, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(mode) = DelayMode::from_setting(value) else {
            return;
        };
        self.finish_randomized_range(false, window, cx);
        if let Some(entry) = self.entries.iter_mut().find(|e| Some(e.id) == self.current) {
            entry.record_delay = value;
            self.record_ui.profile.delay_mode = mode;
            self.publish_library(cx);
            if value == 1 {
                self.record_ui.fixed.update(cx, |s, cx| s.focus(window, cx));
            }
            cx.notify();
        }
    }
    pub fn record_number_changed(
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
        if field != NumberField::Fixed || value.ends_with('.') {
            cx.notify();
            return;
        }
        self.schedule_record_number(0, [number, 0.], window, cx);
    }

    pub(super) fn record_number_blurred(
        &mut self,
        field: NumberField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if field == NumberField::Fixed {
            return;
        }
        let mut min = source_float(&self.record_ui.min.read(cx).value()).unwrap_or(0.);
        let mut max = source_float(&self.record_ui.max.read(cx).value()).unwrap_or(0.);
        if field == NumberField::Min {
            min = min.max(0.);
            if min >= max {
                max = (min + 1.).min(5.);
                self.write_record_draft(NumberField::Max, max.to_string(), window, cx);
            }
            self.write_record_draft(NumberField::Min, min.to_string(), window, cx);
        } else {
            if max < min {
                max = (min + 1.).min(5.);
            }
            self.write_record_draft(NumberField::Max, max.to_string(), window, cx);
        }
        self.schedule_record_number(1, [min, max], window, cx);
    }

    fn schedule_record_number(
        &mut self,
        slot: usize,
        pair: [f64; 2],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.record_ui.generation[slot] = self.record_ui.generation[slot].wrapping_add(1);
        let generation = self.record_ui.generation[slot];
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.record_ui.generation[slot] != generation {
                    return;
                }
                if slot == 0 {
                    match Seconds::new(pair[0]) {
                        Ok(value) => {
                            if this.record_ui.profile.fixed != value {
                                this.record_ui.profile.fixed = value;
                                this.write_record_draft(
                                    NumberField::Fixed,
                                    pair[0].to_string(),
                                    window,
                                    cx,
                                );
                            }
                        }
                        Err(error) => {
                            this.recording.error = Some(error);
                            cx.notify();
                            return;
                        }
                    }
                } else {
                    match (Seconds::new(pair[0]), Seconds::new(pair[1])) {
                        (Ok(min), Ok(max)) => {
                            this.record_ui.profile.random = [min, max];
                            this.write_record_draft(
                                NumberField::Min,
                                pair[0].to_string(),
                                window,
                                cx,
                            );
                            this.write_record_draft(
                                NumberField::Max,
                                pair[1].to_string(),
                                window,
                                cx,
                            );
                        }
                        (Err(error), _) | (_, Err(error)) => {
                            this.recording.error = Some(error);
                            cx.notify();
                            return;
                        }
                    }
                }
                this.publish_library(cx);
                this.update_recording_preview_options(cx);
                cx.notify();
            });
        });
        self.record_ui.number_tasks[slot] = Some(task);
        cx.notify();
    }
    pub fn macro_type_status(&self) -> String {
        match self.current_macro_type() {
            MacroType::Sequence => tr("TEXT_SEQUENCE_MACRO"),
            MacroType::Phased => tr("TEXT_PHASED_MACRO"),
            MacroType::Standard if self.record_delay() == 2 => format!(
                "{}s - {}s",
                self.record_ui.random()[0],
                self.record_ui.random()[1]
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
    pub fn recording_mode_label(&self) -> AnyElement {
        let label = match self.current_macro_type() {
            MacroType::Sequence => tr("TEXT_SEQUENCE_MACRO"),
            MacroType::Phased => tr("TEXT_PHASED_MACRO"),
            MacroType::Standard => match self.record_delay() {
                1 => tr("TEXT_SECOND_DELAY")
                    .replace("{{times}}", &self.record_ui.delay_time().to_string()),
                2 => tr("TEXT_RANDOMIZE_DELAY").replace(
                    "{{times}}",
                    &format!(
                        "{}s - {}s",
                        self.record_ui.random()[0],
                        self.record_ui.random()[1]
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
            .when(self.record_ui.mouse() > 0, |column| {
                column.child(tr(match self.record_ui.mouse() {
                    1 => "TEXT_ABSOLUTE_POSITION_MOUSE",
                    2 => "TEXT_RELATIVE_POSITION_MOUSE",
                    _ => "TEXT_MOUSE_TRACKING",
                }))
            })
            .into_any_element()
    }
    pub fn record_options(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
                            super::tooltip::heading(
                                index,
                                tr(&group.header.name).to_uppercase(),
                                window,
                                cx,
                            ),
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
                            1 => section = section.child(self.record_shortcut_control(window, cx)),
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
                                                        .child(tr("SECS")),
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
                                            self.record_ui.mouse() == i,
                                            false,
                                            window,
                                            cx,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                let Ok(mode) = MovementMode::from_setting(i as u8)
                                                else {
                                                    return;
                                                };
                                                this.record_ui.profile.movement = mode;
                                                this.publish_library(cx);
                                                cx.notify();
                                            }),
                                        )
                                    },
                                ))
                            }
                            _ => {}
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
        BaseButton::new(("macro-record-help-trigger", index))
            .track_focus(&self.record_ui.help_focus[index])
            .tab_stop(true)
            .p_0()
            .relative()
            .size(css(14.))
            .flex_shrink_0()
            .on_prepaint({
                let anchor = anchor.clone();
                move |bounds, _, _| anchor.set(bounds)
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.record_ui.set_help(index, *hovered);
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
                    razer_assets::deferred_resource(super::record_help::RecordHelp {
                        anchor,
                        upward: index == 4,
                        content: div()
                            .id(("macro-record-help-content", index))
                            .test_support()
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
            if disabled { "不可编辑" } else { "可编辑" }
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
            razer_widgets::keyboard_navigation::focus_input(
                Input::new(state)
                    .id(id)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .w(css(68.))
                    .h(css(25.))
                    .p(css(5.))
                    .text_size(css(14.))
                    .line_height(css(17.)),
                state,
                cx,
            )
            .into_any_element()
        })
        .into_any_element()
}

impl MacroPage {
    pub(super) fn recording_movement_setting(&self) -> u8 {
        self.record_ui.profile.movement.setting()
    }

    pub fn recording_options(&self, cx: &App) -> Result<super::recording_decode::Options, String> {
        let mut options = self.recording_commit_options(cx)?;
        let entry = self
            .entries
            .iter()
            .find(|entry| Some(entry.id) == self.current)
            .ok_or("当前宏文档不存在，无法处理录制结果")?;
        let execution = self.library_snapshot.execution_for_actions(
            entry,
            &self.actions,
            self.execution.as_ref(),
            options.movement_mode,
        )?;
        if !execution.engine().ordinary_events()?.is_empty() {
            options.movement_mode = execution.engine().movement.setting();
        }
        Ok(options)
    }

    pub(super) fn recording_commit_options(
        &self,
        _cx: &App,
    ) -> Result<super::recording_decode::Options, String> {
        if self.current.is_none()
            || self.actions_for != self.current
            || !self
                .entries
                .iter()
                .any(|entry| Some(entry.id) == self.current && entry.kind == EntryKind::Macro)
        {
            return Err("当前宏文档不存在，无法处理录制结果".into());
        }
        // Nr reads accepted Redux values; a pending numeric debounce remains
        // an input draft until its own dispatch has occurred.
        let fixed = self.record_ui.delay_time();
        let random = self.record_ui.random();
        let delay_mode = self.record_delay();
        if self.current_macro_type() == MacroType::Standard {
            if delay_mode == 1 && (!fixed.is_finite() || fixed < 0.) {
                return Err("固定录制延迟必须为非负数".into());
            }
            if delay_mode == 2
                && (!random
                    .iter()
                    .all(|value| value.is_finite() && (0. ..=5.).contains(value)))
            {
                return Err("随机录制延迟必须在 0–5 秒内".into());
            }
        }
        Ok(super::recording_decode::Options {
            macro_type: self.current_macro_type(),
            phase: if self.current_macro_type() == MacroType::Phased {
                self.active_phase()
            } else {
                None
            },
            delay_mode,
            movement_mode: self.recording_movement_setting(),
            fixed,
            random,
            next_pair: self.next_event_pair_id().ok_or("本地宏事件标识已耗尽")?,
            shortcut: self.record_ui.shortcut.recording_keys()?,
            finalization_macro_type: self.record_ui.profile.macro_type,
            finalization_delay_mode: self.record_ui.profile.delay_mode.setting(),
        })
    }
}

// Generated catalog reader: razer-app-pages/record_options_data.
fn read_record_options_catalog() -> Data {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                groups: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Group>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Group> {
        let value = match r.tag()? {
            0 => Group {
                id: r.child(decode_3)?,
                header: r.child(decode_4)?,
                content: Default::default(),
            },
            1 => Group {
                id: r.child(decode_3)?,
                header: r.child(decode_4)?,
                content: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Header> {
        let value = match r.tag()? {
            0 => Header {
                name: r.child(decode_3)?,
                tip: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Item>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Item> {
        let value = match r.tag()? {
            0 => Item {
                name: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_RECORD_OPTIONS_DATA,
        0,
        "a1c4babfa2b74965f8d34d129a0466dfcd96884bf738457a48f52776263dca53",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod record_options_data_catalog_tests {
    #[test]
    fn packaged_catalog_record_options_data_decodes() {
        let _ = super::read_record_options_catalog();
    }
}
// End generated catalog reader: razer-app-pages/record_options_data.
