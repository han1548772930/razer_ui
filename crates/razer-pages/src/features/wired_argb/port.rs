//! One observed ARGB port owns its retained editors and local layout draft.
use super::*;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    select::{SelectEvent, SelectState},
};
use std::time::Duration;

pub(super) struct PortChanged {
    pub(super) id: u32,
    pub(super) draft: PortDraft,
}
struct LedControl {
    input: Entity<InputState>,
    fan: Entity<SelectState<Vec<Choice>>>,
    _subscriptions: Vec<Subscription>,
}
pub(super) struct PortEditor {
    spec: &'static Spec,
    fact: PortObservation,
    draft: PortDraft,
    mode: Entity<SelectState<Vec<Choice>>>,
    name: Entity<InputState>,
    rows: BTreeMap<u64, LedControl>,
    editing_name: bool,
    stepper_generation: u64,
    stepper_click_pending: bool,
    chroma_installed: Option<bool>,
    last_command: Option<String>,
    /// 源 `Gu position:"bottom-right"` 的 LED 数量提示由该元素自己的悬停切换挂载
    /// （`.tooltip-razer` 容器覆盖目标并自己监听鼠标），因此只需一个本地开关。
    hovered_info: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<PortChanged> for PortEditor {}
impl PortEditor {
    pub(super) fn new(
        spec: &'static Spec,
        fact: PortObservation,
        draft: PortDraft,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mode =
            cx.new(|cx| SelectState::new(mode_choices(spec, fact.maximum_leds), None, window, cx));
        let name = cx
            .new(|cx| InputState::new(window, cx).validate(|s, _| s.encode_utf16().count() <= 32));
        let subscriptions = vec![
            cx.subscribe_in(&mode, window, |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let strip = value == "strip";
                    if this.editable()
                        && (strip || this.fact.maximum_leds == 80)
                        && strip != this.draft.is_strip_mode
                    {
                        this.draft.is_strip_mode = strip;
                        this.draft.dismissed = false;
                        this.sync(window, cx);
                        this.changed(cx);
                    }
                }
            }),
            cx.subscribe_in(
                &name,
                window,
                |this: &mut Self, input, event, window, cx| {
                    if this.editing_name
                        && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    {
                        let value = input.read(cx).value().to_string();
                        this.editing_name = false;
                        if this.editable() && !value.is_empty() && value != this.draft.name {
                            this.draft.name = value;
                            this.changed(cx);
                        }
                        this.sync(window, cx);
                        cx.notify();
                    }
                },
            ),
        ];
        let mut this = Self {
            spec,
            fact,
            draft,
            mode,
            name,
            rows: BTreeMap::new(),
            editing_name: false,
            stepper_generation: 0,
            stepper_click_pending: false,
            chroma_installed: preview.then_some(false),
            last_command: None,
            hovered_info: false,
            _subscriptions: subscriptions,
        };
        this.sync(window, cx);
        this
    }
    pub(super) fn restore(
        &mut self,
        draft: PortDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = draft;
        self.editing_name = false;
        self.sync(window, cx);
        cx.notify();
    }
    pub(super) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_name = false;
        self.sync(window, cx);
        cx.notify();
    }
    fn editable(&self) -> bool {
        self.fact.active && self.fact.maximum_leds >= self.spec.minimum_leds
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(PortChanged {
            id: self.fact.id,
            draft: self.draft.clone(),
        });
        cx.notify();
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = if self.draft.is_strip_mode {
            "strip"
        } else {
            "fan"
        }
        .to_owned();
        self.mode
            .update(cx, |state, cx| state.set_selected_value(&mode, window, cx));
        if !self.editing_name {
            self.name.update(cx, |state, cx| {
                state.set_value(self.draft.name.clone(), window, cx)
            });
        }
        let all = self
            .draft
            .strip
            .iter()
            .chain(&self.draft.fan)
            .cloned()
            .collect::<Vec<_>>();
        self.rows.retain(|id, _| all.iter().any(|s| s.id == *id));
        for segment in all {
            let id = segment.id;
            if !self.rows.contains_key(&id) {
                let input = cx.new(|cx| {
                    InputState::new(window, cx).validate(|value, _| {
                        value.len() <= 4 && value.bytes().all(|b| b.is_ascii_digit())
                    })
                });
                let fan = cx.new(|cx| SelectState::new(fan_choices(self.spec), None, window, cx));
                let subscriptions = vec![
                    cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
                        if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                            if let Ok(value) = input.read(cx).value().parse::<u32>() {
                                this.edit_leds(id, value, window, cx);
                            } else {
                                this.sync(window, cx);
                            }
                        }
                    }),
                    cx.subscribe_in(&fan, window, move |this, _, event, window, cx| {
                        if let SelectEvent::Confirm(Some(value)) = event {
                            if let Ok(value) = value.parse::<u32>() {
                                this.edit_leds(id, value, window, cx);
                            }
                        }
                    }),
                ];
                self.rows.insert(
                    id,
                    LedControl {
                        input,
                        fan,
                        _subscriptions: subscriptions,
                    },
                );
            }
            let controls = &self.rows[&id];
            controls.input.update(cx, |input, cx| {
                input.set_value(segment.value.to_string(), window, cx)
            });
            controls.fan.update(cx, |state, cx| {
                state.set_selected_value(&segment.value.to_string(), window, cx)
            });
        }
    }
    fn edit_leds(&mut self, id: u64, value: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() || !self.draft.segments().iter().any(|s| s.id == id) {
            return;
        }
        if !self.draft.is_strip_mode && !self.spec.fan_counts.contains(&value) {
            return;
        }
        let before = self.draft.total();
        self.draft
            .set_leds(id, value, self.spec.minimum_leds, self.fact.maximum_leds);
        self.sync(window, cx);
        if before != self.draft.total() {
            self.changed(cx);
        }
    }
    fn step_leds(
        &mut self,
        id: u64,
        increase: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(value) = self
            .draft
            .segments()
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.value)
        else {
            return false;
        };
        let next = if increase {
            value.saturating_add(1)
        } else {
            value.saturating_sub(1)
        };
        self.edit_leds(id, next, window, cx);
        self.draft
            .segments()
            .iter()
            .find(|s| s.id == id)
            .is_some_and(|s| s.value != value)
    }
    fn begin_stepper_hold(
        &mut self,
        id: u64,
        increase: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stepper_generation = self.stepper_generation.wrapping_add(1);
        self.stepper_click_pending = true;
        self.step_leds(id, increase, window, cx);
        let generation = self.stepper_generation;
        let owner = cx.entity().downgrade();
        let window_handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                let Some(owner) = owner.upgrade() else { break };
                let keep_repeating = cx
                    .update_window(window_handle, |_, window, cx| {
                        owner.update(cx, |this, cx| {
                            if this.stepper_generation != generation {
                                return false;
                            }
                            this.step_leds(id, increase, window, cx)
                        })
                    })
                    .ok()
                    .unwrap_or(false);
                if !keep_repeating {
                    break;
                }
            }
        })
        .detach();
    }
    fn end_stepper_hold(&mut self) {
        self.stepper_generation = self.stepper_generation.wrapping_add(1);
    }
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable()
            || self.draft.is_strip_mode && self.draft.strip.len() >= 4
            || !self.draft.is_strip_mode && self.draft.total() >= self.fact.maximum_leds
        {
            return;
        }
        self.draft.add(self.spec.minimum_leds);
        self.sync(window, cx);
        self.changed(cx);
    }
    fn remove(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() || self.draft.segments().first().is_none_or(|s| s.id == id) {
            return;
        }
        self.draft.segments_mut().retain(|s| s.id != id);
        self.draft.dismissed = false;
        self.sync(window, cx);
        self.changed(cx);
    }
    /// 源 3871/778：`Gu position:"bottom-right"`，内容由
    /// `getTextItem(bO.vml, {ledCount: '<span style="color:#44d62c">N</span>'})` 生成——只有数字
    /// 是主题绿，其余是提示文本；提示容器自己覆盖目标并监听悬停，所以这里只需一个本地开关。
    fn detection(&self, cx: &Context<Self>) -> AnyElement {
        let template = self.spec.text("GLITTER_DETECTED_LED_COUNT");
        let count = self.fact.detected_leds.to_string();
        let (prefix, suffix) = match template.split_once("{{ledCount}}") {
            Some((prefix, suffix)) => (prefix.to_owned(), suffix.to_owned()),
            None => (template.clone(), String::new()),
        };
        let text = template.replace("{{ledCount}}", &count);
        div()
            .id("argb-detection-info-wrapper")
            .relative()
            .child(
                button::Button::new(("argb-detection-info", self.fact.id))
                    .child(img(self.spec.asset("detected")).size(surface::css(20.)))
                    .ghost()
                    .p_0()
                    .size(surface::css(20.))
                    .accessibility_label(text)
                    .xsmall()
                    .text_color(cx.theme().primary),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered_info = *hovered;
                cx.notify();
            }))
            .when(self.hovered_info, |row| {
                row.child(source_hover_tip_element(
                    "argb-detected-led-tip",
                    SourceTipPlacement::BottomRight,
                    h_flex()
                        .child(prefix)
                        .child(div().text_color(cx.theme().primary).child(count))
                        .child(suffix),
                ))
            })
            .into_any_element()
    }
    fn led_stepper(&self, id: u64, value: u32, cx: &Context<Self>) -> AnyElement {
        let controls = &self.rows[&id];
        let maximum = self.draft.maximum(id, self.fact.maximum_leds);
        h_flex()
            .h(surface::css(27.))
            .w(surface::css(62.))
            .border_1()
            .border_color(cx.theme().border)
            .child(
                Input::new(&controls.input)
                    .id(SharedString::from(format!(
                        "argb-led-{}-{id}",
                        self.fact.id
                    )))
                    .aria_label(format!(
                        "{}: {}",
                        self.draft.name,
                        self.spec.text("GLITTER_NO_OF_LED")
                    ))
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .w(surface::css(43.))
                    .h(surface::css(25.))
                    .px(surface::css(6.))
                    .text_size(surface::css(14.))
                    .disabled(!self.editable()),
            )
            .child(
                v_flex()
                    .w(surface::css(17.))
                    .children([true, false].map(|increase| {
                        gpui_kit::base::Button::new(SharedString::from(format!(
                            "argb-step-{}-{id}-{increase}",
                            self.fact.id
                        )))
                        .accessibility_label(format!(
                            "{} {}",
                            if increase { "+1" } else { "−1" },
                            self.spec.text("GLITTER_NO_OF_LED")
                        ))
                        .disabled(
                            !self.editable()
                                || if increase {
                                    value >= maximum
                                } else {
                                    value <= self.spec.minimum_leds
                                },
                        )
                        .w_full()
                        .h(surface::css(12.))
                        .text_size(surface::css(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|v| v.bg(Colors::stepper_hover()))
                        .active(|v| v.bg(Colors::stepper_active()))
                        .focus_visible(|v| v.bg(cx.theme().secondary))
                        .child(
                            img(self.spec.asset(if increase {
                                "stepper_up"
                            } else {
                                "stepper_down"
                            }))
                            .w(surface::css(8.))
                            .h(surface::css(4.)),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.begin_stepper_hold(id, increase, window, cx);
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| {
                                this.end_stepper_hold();
                            }),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| {
                                this.end_stepper_hold();
                            }),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if this.stepper_click_pending {
                                    this.stepper_click_pending = false;
                                } else {
                                    this.step_leds(id, increase, window, cx);
                                }
                            },
                        ))
                    })),
            )
            .into_any_element()
    }
    fn row(&self, ix: usize, segment: &state::Segment, cx: &Context<Self>) -> AnyElement {
        let id = segment.id;
        let strip = self.draft.is_strip_mode;
        let name = if strip { "TEXT_LED_STRIP" } else { "TEXT_FAN" };
        let mode = if ix == 0 {
            surface::select(&self.mode)
                .items(mode_choices(self.spec, self.fact.maximum_leds))
                .accessibility_label(self.spec.text("TEXT_DEVICE_TYPE"))
                .w(surface::css(150.))
                .h(surface::css(27.))
                .disabled(!self.editable())
                .into_any_element()
        } else {
            div()
                .w(surface::css(150.))
                .h(surface::css(27.))
                .flex()
                .items_center()
                .child(format!("{} {}", self.spec.text(name), ix + 1))
                .into_any_element()
        };
        let leds = if strip {
            self.led_stepper(id, segment.value, cx)
        } else {
            surface::select(&self.rows[&id].fan)
                .items(fan_choices(self.spec))
                .accessibility_label(self.spec.text("GLITTER_NO_OF_LED"))
                .w(surface::css(62.))
                .h(surface::css(27.))
                .disabled(!self.editable())
                .into_any_element()
        };
        h_flex()
            .items_start()
            .gap(surface::css(20.))
            .py(surface::css(5.))
            .child(
                v_flex()
                    .gap(surface::css(10.))
                    .when(ix == 0, |v| v.child(self.spec.text("TEXT_DEVICE_TYPE")))
                    .child(mode),
            )
            .child(
                v_flex()
                    .gap(surface::css(10.))
                    .when(ix == 0, |v| v.child(self.spec.text("GLITTER_NO_OF_LED")))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(leds)
                            .when(
                                !self.spec.mainboard() && self.draft.segments().len() == 1,
                                |v| v.child(self.detection(cx)),
                            )
                            .when(ix != 0, |v| {
                                v.child(
                                    button::Button::new(SharedString::from(format!(
                                        "argb-remove-{}-{id}",
                                        self.fact.id
                                    )))
                                    .child(img(self.spec.asset("remove")).size(surface::css(20.)))
                                    .ghost()
                                    .xsmall()
                                    .tooltip(format!(
                                        "{} {}",
                                        self.spec.text("TEXT_DEVICE_TYPE"),
                                        ix + 1
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.remove(id, window, cx)
                                    })),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
    fn chroma_message(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .gap(surface::css(10.))
            .mt(surface::css(10.))
            .p(surface::css(20.))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(surface::css(5.))
            .text_size(surface::css(13.))
            .child(self.spec.text("GLITTER_MESSAGE_CHROMA_STUDIO"))
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        button::Button::new(("argb-open-chroma", self.fact.id))
                            .outline()
                            .xsmall()
                            .label(self.spec.text("DASHBOARD_CHROMA_STUDIO"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.last_command = Some(local(
                                    "Chroma Studio 尚未连接。",
                                    "Chroma Studio is unavailable.",
                                ));
                                cx.notify();
                            })),
                    )
                    .child(
                        button::Button::new(("argb-dismiss-chroma", self.fact.id))
                            .ghost()
                            .xsmall()
                            .label(self.spec.text("DISMISS"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.draft.dismissed = true;
                                this.changed(cx);
                            })),
                    ),
            )
            .when_some(self.last_command.clone(), |v, message| {
                v.child(surface::note(message, cx))
            })
            .into_any_element()
    }
}
impl Render for PortEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.draft.total();
        let multiple = self.draft.segments().len() > 1;
        let image = if self.draft.is_strip_mode {
            format!("strip{}", self.draft.strip.len())
        } else {
            "fan".into()
        };
        let mut title = h_flex().h(surface::css(27.)).justify_between();
        if self.editing_name {
            title = title.child(
                Input::new(&self.name)
                    .aria_label(self.spec.text("RENAME"))
                    .h(surface::css(25.))
                    .w(surface::css(160.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.)),
            );
        } else {
            title = title.child(
                gpui_kit::base::Button::new(("argb-rename", self.fact.id))
                    .accessibility_label(format!(
                        "{} {}",
                        self.spec.text("RENAME"),
                        self.draft.name
                    ))
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_color(cx.theme().primary)
                    .text_size(surface::css(16.))
                    .font_family("RazerF5")
                    .focus_visible(|v| v.border_1().border_color(cx.theme().ring))
                    .child(self.draft.name.to_uppercase())
                    .child(img(self.spec.asset("rename")).size(surface::css(14.)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.editing_name = true;
                        this.name.focus_handle(cx).focus(window, cx);
                        cx.notify();
                    })),
            );
        }
        let info = v_flex()
            .flex_1()
            .min_w_0()
            .children(
                self.draft
                    .segments()
                    .iter()
                    .enumerate()
                    .map(|(ix, segment)| self.row(ix, segment, cx)),
            )
            .when(
                !self.draft.is_strip_mode || self.draft.strip.len() < 4,
                |v| {
                    v.child(
                        button::Button::new(("argb-add", self.fact.id))
                            .child(div().underline().child(self.spec.text(
                                if self.draft.is_strip_mode {
                                    "GLITTER_ADD_BEND"
                                } else {
                                    "GLITTER_ADD_FAN"
                                },
                            )))
                            .accessibility_label(self.spec.text(if self.draft.is_strip_mode {
                                "GLITTER_ADD_BEND"
                            } else {
                                "GLITTER_ADD_FAN"
                            }))
                            .ghost()
                            .p_0()
                            .h(surface::css(27.))
                            .text_size(surface::css(14.))
                            .custom(
                                button::ButtonCustomVariant::new(cx)
                                    .color(cx.theme().transparent)
                                    .foreground(Colors::secondary())
                                    .hover(cx.theme().transparent),
                            )
                            .disabled(
                                !self.editable()
                                    || !self.draft.is_strip_mode && total >= self.fact.maximum_leds,
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                    )
                },
            )
            .when(multiple, |v| {
                v.child(
                    h_flex()
                        .h(surface::css(27.))
                        .gap(surface::css(10.))
                        .child(
                            div()
                                .w(surface::css(150.))
                                .child(self.spec.text("GLITTER_TOTAL_LED_COUNT")),
                        )
                        .child(
                            div()
                                .w(surface::css(50.))
                                .px(surface::css(10.))
                                .child(total.to_string()),
                        )
                        .child(self.detection(cx)),
                )
            });
        v_flex()
            .relative()
            .w(surface::css(if self.spec.mainboard() {
                600.
            } else {
                460.
            }))
            .min_h(surface::css(199.))
            .pt(surface::css(26.))
            .pb(surface::css(30.))
            .px(surface::css(40.))
            .rounded(surface::css(5.))
            .bg(cx.theme().group_box)
            .text_size(surface::css(14.))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && this.editing_name {
                    this.editing_name = false;
                    this.sync(window, cx);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(title)
            // 源把 `.help`/`.tip` 直接放在 `.port-container.widget` 里
            // （`<div className="help"/><div className="tip">{getTextItem(OT.gt9)}</div>`），
            // `.widget .help{background-color:#4a4a4a;border-radius:50%;height:14px;
            // position:absolute;right:10px;top:10px;width:14px}`。共享的
            // `surface::help_control` 正是该控件与 `.widget .tip`；产品自己的
            // `tooltip_questionmark` 与共享图标字节相同（sha256 efe8667…）。
            .child(
                div()
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.))
                    .child(surface::help_control(
                        ("argb-port-help", self.fact.id),
                        self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE"),
                    )),
            )
            .child(
                h_flex()
                    .items_start()
                    .mt(surface::css(11.))
                    .child(info)
                    .child(
                        img(self.spec.asset(&image))
                            .size(surface::css(100.))
                            .flex_shrink_0(),
                    ),
            )
            .when(
                !self.draft.dismissed
                    && self.chroma_installed == Some(false)
                    && total != self.fact.detected_leds,
                |v| v.child(self.chroma_message(cx)),
            )
            .when(total > self.fact.maximum_leds, |v| {
                v.child(
                    h_flex()
                        .items_start()
                        .gap_2()
                        .mt_2()
                        .text_color(cx.theme().warning)
                        .child(img(self.spec.asset("warning")).size(surface::css(20.)))
                        .child(format!(
                            "{} {}",
                            self.spec
                                .text("GLITTER_MESSAGE_EXCEEDED_WARNING_1")
                                .replace("{{maxLed}}", &self.fact.maximum_leds.to_string()),
                            self.spec.text("GLITTER_MESSAGE_EXCEEDED_WARNING_2")
                        )),
                )
            })
    }
}
fn mode_choices(spec: &Spec, maximum: u32) -> Vec<Choice> {
    let mut choices = vec![Choice::new("strip", spec.text("TEXT_LED_STRIP"))];
    if maximum == 80 {
        choices.push(Choice::new("fan", spec.text("TEXT_FAN")));
    }
    choices
}
fn fan_choices(spec: &Spec) -> Vec<Choice> {
    spec.fan_counts
        .iter()
        .map(|count| Choice::new(count.to_string(), count.to_string()))
        .collect()
}
