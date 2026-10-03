//! Source 653 `JM` / 777 `Ah` palette and their `BM` / `Oh` custom picker.
//! The palette is shared product data; a custom-color editor owns an uncommitted draft.
use crate::{
    preferences::CustomColors,
    ui::{surface, theme::PaletteColors},
};
use gpui_kit::base::{
    Button as BaseButton, ColorPicker as ColorPickerRoot, Popover, Slider as BaseSlider,
    SliderIndicator, SliderThumb, SliderTrack,
};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    color_picker::{ColorPickerEvent, ColorPickerState},
    input::{Input, InputEvent, InputState},
    slider::{SliderEvent, SliderState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{cell::Cell, rc::Rc};

// These are the original mM / Eh values, including the asymmetric #ffc182.
const PRESETS: [u32; 40] = [
    0xffffff, 0xffc0c0, 0xffe0c0, 0xffffc0, 0xc0ffc0, 0xc0ffff, 0xc0c0ff, 0xffc0ff, 0xe0e0e0,
    0xff8080, 0xffc182, 0xffff80, 0x80ff80, 0x80ffff, 0x8080ff, 0xff80ff, 0xc0c0c0, 0xff0000,
    0xff8000, 0xffff00, 0x00ff00, 0x00ffff, 0x0000ff, 0xff00ff, 0x808080, 0xc00000, 0xc06000,
    0xc0c000, 0x00c000, 0x00c0c0, 0x0000c0, 0xc000c0, 0x404040, 0x800000, 0x804000, 0x808000,
    0x008000, 0x008080, 0x000080, 0x800080,
];

#[derive(IntoElement)]
pub(super) struct LightingColorPicker {
    state: Entity<ColorPickerState>,
    label: SharedString,
    disabled: bool,
    allow_none: bool,
    dial_trigger: bool,
}
impl LightingColorPicker {
    pub(super) fn new(state: &Entity<ColorPickerState>, label: impl Into<SharedString>) -> Self {
        Self {
            state: state.clone(),
            label: label.into(),
            disabled: false,
            allow_none: true,
            dial_trigger: false,
        }
    }
    pub(super) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub(super) fn allow_none(mut self, allow: bool) -> Self {
        self.allow_none = allow;
        self
    }
    pub(super) fn dial_trigger(mut self, dial: bool) -> Self {
        self.dial_trigger = dial;
        self
    }
}

struct LightingColorView {
    props: LightingColorPicker,
    trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
    plane_bounds: Rc<Cell<Bounds<Pixels>>>,
    upward: bool,
    editing: Option<usize>,
    selected_slot: Option<usize>,
    last_value: Option<[u8; 3]>,
    context_slot: Option<usize>,
    draft: [u8; 3],
    hue: f32,
    saturation: f32,
    value: f32,
    fields: [Entity<InputState>; 4],
    written_fields: [String; 4],
    brightness: Entity<SliderState>,
    plane_focus: FocusHandle,
    brightness_focus: FocusHandle,
    editor_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl RenderOnce for LightingColorPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.clone();
        let label = self.label.clone();
        let view =
            window.use_keyed_state(("lighting-color", state.entity_id()), cx, |window, cx| {
                CustomColors::ensure(cx);
                let fields = std::array::from_fn(|index| {
                    cx.new(|cx| {
                        InputState::new(window, cx).validate(move |text, _| {
                            text.len() <= if index == 0 { 6 } else { 3 }
                                && text.bytes().all(|b| {
                                    if index == 0 {
                                        b.is_ascii_hexdigit()
                                    } else {
                                        b.is_ascii_digit()
                                    }
                                })
                        })
                    })
                });
                let brightness = cx.new(|_| {
                    SliderState::new()
                        .min(0.)
                        .max(1.)
                        .step(0.001)
                        .default_value(1.)
                });
                let mut subscriptions = vec![
                    cx.observe(&state, |this: &mut LightingColorView, state, cx| {
                        let value = state.read(cx).value().map(color_bytes);
                        if value != this.last_value {
                            this.selected_slot = None;
                            this.last_value = value;
                        }
                        if !state.read(cx).is_open() {
                            this.editing = None;
                            this.context_slot = None;
                        }
                        cx.notify();
                    }),
                    cx.observe_global::<CustomColors>(|this, cx| {
                        let colors = cx.global::<CustomColors>().colors();
                        if this
                            .selected_slot
                            .is_some_and(|slot| colors[slot] != this.last_value)
                        {
                            this.selected_slot = colors
                                .iter()
                                .position(|color| color.is_some() && *color == this.last_value);
                        }
                        cx.notify();
                    }),
                    cx.subscribe_in(
                        &brightness,
                        window,
                        |this: &mut LightingColorView, slider, _: &SliderEvent, window, cx| {
                            this.value = slider.read(cx).value().start();
                            this.update_from_hsv(window, cx);
                        },
                    ),
                ];
                for (index, field) in fields.iter().enumerate() {
                    subscriptions.push(cx.subscribe_in(
                        field,
                        window,
                        move |this: &mut LightingColorView, field, event, window, cx| match event {
                            InputEvent::Change => {
                                let text = field.read(cx).value().to_string();
                                if text != this.written_fields[index] && this.editing.is_some() {
                                    this.change_field(index, &text, window, cx);
                                }
                            }
                            InputEvent::Blur => this.write_fields(None, window, cx),
                            InputEvent::PressEnter { .. } => {
                                this.write_fields(None, window, cx);
                                this.editor_focus.focus(window, cx);
                            }
                            _ => {}
                        },
                    ));
                }
                LightingColorView {
                    props: LightingColorPicker::new(&state, label),
                    trigger_bounds: Rc::new(Cell::new(Bounds::default())),
                    plane_bounds: Rc::new(Cell::new(Bounds::default())),
                    upward: false,
                    editing: None,
                    selected_slot: None,
                    last_value: state.read(cx).value().map(color_bytes),
                    context_slot: None,
                    draft: [68, 214, 44],
                    hue: 0.,
                    saturation: 1.,
                    value: 1.,
                    fields,
                    written_fields: std::array::from_fn(|_| String::new()),
                    brightness,
                    plane_focus: cx.focus_handle(),
                    brightness_focus: cx.focus_handle(),
                    editor_focus: cx.focus_handle(),
                    _subscriptions: subscriptions,
                }
            });
        view.update(cx, |view, _| view.props = self);
        view
    }
}

fn color_bytes(color: Hsla) -> [u8; 3] {
    let c = Rgba::from(color);
    [c.r, c.g, c.b].map(|v| (v * 255.).round().clamp(0., 255.) as u8)
}
fn color_value(color: [u8; 3]) -> Hsla {
    rgb((u32::from(color[0]) << 16) | (u32::from(color[1]) << 8) | u32::from(color[2])).into()
}
fn hex_value(color: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

impl LightingColorView {
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open && !self.props.disabled {
            let scale = window.rem_size() / px(16.);
            // JM checks the 250px palette even when it will subsequently show the taller editor.
            self.upward = self.trigger_bounds.get().bottom() + px(252. * scale)
                >= window.viewport_size().height;
        } else {
            self.editing = None;
            self.context_slot = None;
        }
        self.props.state.update(cx, |state, cx| {
            state.set_open(open && !self.props.disabled, cx)
        });
        cx.notify();
    }
    fn select(
        &mut self,
        color: Option<[u8; 3]>,
        slot: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.last_value = color;
        self.selected_slot = slot;
        self.context_slot = None;
        self.props.state.update(cx, |state, cx| {
            if let Some(color) = color {
                state.select_color(color_value(color), window, cx);
            } else {
                state.clear_value(window, cx);
                state.set_open(false, cx);
                cx.emit(ColorPickerEvent::Change(None));
            }
        });
        self.props.state.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    fn open_editor(&mut self, requested: usize, window: &mut Window, cx: &mut Context<Self>) {
        let slots = cx.global::<CustomColors>().colors();
        let slot = slots
            .iter()
            .position(Option::is_none)
            .filter(|first| *first < requested)
            .unwrap_or(requested);
        self.editing = Some(slot);
        self.context_slot = None;
        self.draft = slots[slot].unwrap_or([68, 214, 44]);
        self.read_hsv(window, cx);
        self.write_fields(None, window, cx);
        self.plane_focus.focus(window, cx);
        cx.notify();
    }
    fn cancel_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        self.props.state.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(slot) = self.editing.take() else {
            return;
        };
        let mut colors = cx.global::<CustomColors>().colors();
        colors[slot] = Some(self.draft);
        CustomColors::replace(colors, cx);
        self.select(Some(self.draft), Some(slot), window, cx);
        // The source's saveColor closes the editor and reopens the palette.
        self.props
            .state
            .update(cx, |state, cx| state.set_open(true, cx));
        cx.notify();
    }
    fn delete_slot(&mut self, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut colors = cx.global::<CustomColors>().colors();
        colors.copy_within(slot + 1.., slot);
        colors[15] = None;
        CustomColors::replace(colors, cx);
        self.context_slot = None;
        if self.selected_slot == Some(slot) {
            self.select(self.last_value, None, window, cx);
        } else if self.selected_slot.is_some_and(|selected| selected > slot) {
            self.selected_slot = self.selected_slot.map(|selected| selected - 1);
        }
        cx.notify();
    }
    fn read_hsv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let [r, g, b] = self.draft.map(|v| f32::from(v) / 255.);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        self.hue = if delta == 0. {
            0.
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.) / 6.
        } else if max == g {
            ((b - r) / delta + 2.) / 6.
        } else {
            ((r - g) / delta + 4.) / 6.
        };
        self.saturation = if max == 0. { 0. } else { delta / max };
        self.value = max;
        self.brightness
            .update(cx, |slider, cx| slider.set_value(max, window, cx));
    }
    fn update_from_hsv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft = color_bytes(hsv_color(self.hue, self.saturation, self.value));
        self.write_fields(None, window, cx);
        cx.notify();
    }
    fn change_field(
        &mut self,
        index: usize,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index == 0 {
            // Source filterHexValue repeats the entire 3-digit string, rather than each digit.
            let expanded = match text.len() {
                1 => text.repeat(6),
                2 => text.repeat(3),
                3 => text.repeat(2),
                4 => format!("{text}ff"),
                5 => format!("{text}f"),
                6 => text.to_string(),
                _ => "000000".into(),
            };
            let value = u32::from_str_radix(&expanded, 16).unwrap_or(0);
            self.draft = [(value >> 16) as u8, (value >> 8) as u8, value as u8];
        } else {
            for (channel, field) in self.fields[1..].iter().enumerate() {
                self.draft[channel] = field.read(cx).value().parse::<u8>().unwrap_or(0);
            }
        }
        self.written_fields[index] = text.to_string();
        self.read_hsv(window, cx);
        self.write_fields(Some(index), window, cx);
        cx.notify();
    }
    fn write_fields(
        &mut self,
        preserve: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let values = [
            hex_value(self.draft).to_uppercase(),
            self.draft[0].to_string(),
            self.draft[1].to_string(),
            self.draft[2].to_string(),
        ];
        for (index, value) in values.into_iter().enumerate() {
            if preserve == Some(index) {
                continue;
            }
            self.written_fields[index] = value.clone();
            if self.fields[index].read(cx).value().as_str() != value {
                self.fields[index].update(cx, |input, cx| input.set_value(value, window, cx));
            }
        }
    }
    fn pick_plane(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = self.plane_bounds.get();
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        self.hue = ((position.x - bounds.left()) / bounds.size.width).clamp(0., 1.);
        self.saturation = (1. - (position.y - bounds.top()) / bounds.size.height).clamp(0., 1.);
        self.update_from_hsv(window, cx);
    }
}

fn hsv_color(hue: f32, saturation: f32, value: f32) -> Hsla {
    let chroma = value * saturation;
    let sector = hue * 6.;
    let x = chroma * (1. - (sector % 2. - 1.).abs());
    let [r, g, b] = match sector as usize % 6 {
        0 => [chroma, x, 0.],
        1 => [x, chroma, 0.],
        2 => [0., chroma, x],
        3 => [0., x, chroma],
        4 => [x, 0., chroma],
        _ => [chroma, 0., x],
    };
    let offset = value - chroma;
    Rgba {
        r: r + offset,
        g: g + offset,
        b: b + offset,
        a: 1.,
    }
    .into()
}

#[derive(Clone)]
struct ColorPlaneDrag(EntityId);
impl Render for ColorPlaneDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl LightingColorView {
    fn swatch(
        &self,
        id: SharedString,
        color: Hsla,
        selected: bool,
        empty: bool,
        cx: &App,
    ) -> BaseButton {
        let palette = PaletteColors;
        BaseButton::new(id)
            .accessibility_label(if empty {
                "添加自定义颜色".into()
            } else {
                format!("#{}", hex_value(color_bytes(color)))
            })
            .w(surface::css(20.))
            .flex()
            .items_center()
            .justify_center()
            .h(surface::css(20.))
            .p_0()
            .rounded(cx.theme().font_size * (3. / 16.))
            .border_1()
            .border_color(if empty {
                palette.picker_border()
            } else {
                palette.swatch_border()
            })
            .bg(color)
            .active(|s| s.bg(color.opacity(0.7)))
            .focus_visible(|s| s.border_color(cx.theme().primary))
            .when(!empty, |button| {
                button.hover(|style| {
                    style
                        .border_color(palette.selected_dot())
                        .shadow(vec![BoxShadow {
                            inset: false,
                            color: palette.white(),
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(0.),
                            spread_radius: surface::css(2.).to_pixels(px(16.)),
                        }])
                })
            })
            .when(empty, |button| {
                button.hover(|style| style.border_color(cx.theme().primary))
            })
            .when(selected, |button| {
                button
                    .border_color(palette.selected_dot())
                    .shadow(vec![BoxShadow {
                        inset: false,
                        color: palette.white(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: surface::css(2.).to_pixels(px(16.)),
                    }])
                    .child(
                        div()
                            .size(surface::css(10.))
                            .rounded_full()
                            .border_2()
                            .border_color(palette.white())
                            .shadow(vec![BoxShadow {
                                inset: false,
                                color: palette.selected_dot(),
                                offset: point(px(0.), px(0.)),
                                blur_radius: px(2.),
                                spread_radius: px(1.),
                            }]),
                    )
            })
            .when(empty, |button| {
                button.child(img("synapse/plus.svg").size(surface::css(8.)))
            })
    }
    fn render_palette(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = PaletteColors;
        let current = self.props.state.read(cx).value().map(color_bytes);
        let colors = cx.global::<CustomColors>().colors();
        let has_preset =
            current.is_some_and(|c| PRESETS.iter().any(|v| color_bytes(rgb(*v).into()) == c));
        v_flex()
            .relative()
            .w(surface::css(250.))
            .h(surface::css(250.))
            .pt(surface::css(5.))
            .pb(surface::css(10.))
            .border_1()
            .border_color(palette.border())
            .bg(palette.surface())
            .opacity(0.99)
            .child(
                h_flex()
                    .flex_wrap()
                    .h(surface::css(150.))
                    .flex_shrink_0()
                    .justify_center()
                    .children(PRESETS.into_iter().map(|value| {
                        let color: Hsla = rgb(value).into();
                        let bytes = color_bytes(color);
                        div().m(surface::css(5.)).child(
                            self.swatch(
                                format!("preset-{value:06x}").into(),
                                color,
                                self.selected_slot.is_none() && current == Some(bytes),
                                false,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.select(Some(bytes), None, window, cx)
                                },
                            )),
                        )
                    })),
            )
            .child(
                div()
                    .mt(surface::css(10.))
                    .h(surface::css(20.))
                    .flex_shrink_0()
                    .pl(surface::css(10.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(20.))
                    .text_color(cx.theme().foreground)
                    .child(crate::i18n::t("COLOR")),
            )
            .when(self.props.allow_none, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .right(surface::css(9.))
                        .top(surface::css(160.))
                        .child(
                            self.swatch(
                                "palette-no-color".into(),
                                palette.white(),
                                current.is_none(),
                                false,
                                cx,
                            )
                            .accessibility_label("无颜色")
                            .child(img("synapse/palette-none.svg").size(surface::css(18.)))
                            .on_click(cx.listener(
                                |this, _, window, cx| this.select(None, None, window, cx),
                            )),
                        ),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .h(surface::css(60.))
                    .flex_shrink_0()
                    .justify_center()
                    .children(colors.into_iter().enumerate().map(|(slot, color)| {
                        let selected = self.selected_slot == Some(slot)
                            || (self.selected_slot.is_none()
                                && !has_preset
                                && color.is_some()
                                && current == color);
                        let trigger = self
                            .swatch(
                                format!("custom-slot-{slot}").into(),
                                color.map(color_value).unwrap_or(palette.surface()),
                                selected,
                                color.is_none(),
                                cx,
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(color) = cx.global::<CustomColors>().colors()[slot] {
                                    this.select(Some(color), Some(slot), window, cx);
                                } else {
                                    this.open_editor(slot, window, cx);
                                }
                            }));
                        div().m(surface::css(5.)).child(
                            Popover::new(("custom-menu", slot))
                                .mouse_button(MouseButton::Right)
                                .open(self.context_slot == Some(slot))
                                .offset(surface::css(-11.).to_pixels(cx.theme().font_size))
                                .on_open_change(cx.listener(move |this, open, _, cx| {
                                    this.context_slot = (*open && color.is_some()).then_some(slot);
                                    cx.notify();
                                }))
                                .trigger(trigger)
                                .when(self.context_slot == Some(slot), |menu| {
                                    let content = v_flex()
                                        .ml(surface::css(9.))
                                        .w(surface::css(90.))
                                        .h(surface::css(58.))
                                        .overflow_hidden()
                                        .border_1()
                                        .border_color(palette.picker_border())
                                        .bg(palette.surface())
                                        .child(
                                            Button::new("edit-color")
                                                .label(crate::i18n::t("EDIT"))
                                                .w_full()
                                                .h(surface::css(29.))
                                                .px(surface::css(10.))
                                                .py_0()
                                                .rounded(px(0.))
                                                .justify_start()
                                                .custom(
                                                    ButtonCustomVariant::new(cx)
                                                        .color(palette.surface())
                                                        .hover(cx.theme().foreground.opacity(0.1)),
                                                )
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.open_editor(slot, window, cx)
                                                    },
                                                )),
                                        )
                                        .child(
                                            Button::new("delete-color")
                                                .label(crate::i18n::t("DELETE"))
                                                .w_full()
                                                .h(surface::css(29.))
                                                .px(surface::css(10.))
                                                .py_0()
                                                .rounded(px(0.))
                                                .justify_start()
                                                .custom(
                                                    ButtonCustomVariant::new(cx)
                                                        .color(palette.surface())
                                                        .hover(cx.theme().foreground.opacity(0.1)),
                                                )
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.delete_slot(slot, window, cx)
                                                    },
                                                )),
                                        );
                                    menu.content(move |_, _, _| content)
                                }),
                        )
                    })),
            )
            .into_any_element()
    }

    fn render_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = PaletteColors;
        let plane_bounds = self.plane_bounds.clone();
        let entity_id = cx.entity_id();
        let hue = self.hue;
        let saturation = self.saturation;
        let brightness = self.brightness.clone();
        v_flex()
            .id("custom-color-editor")
            .track_focus(&self.editor_focus)
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::actions::Cancel, window, cx| {
                    cx.stop_propagation();
                    this.cancel_editor(window, cx);
                }),
            )
            .w(surface::css(220.))
            .h(surface::css(269.))
            .p(surface::css(9.))
            .border_1()
            .border_color(palette.picker_border())
            .bg(palette.surface())
            .child(
                div()
                    .id("color-plane")
                    .track_focus(&self.plane_focus.clone().tab_stop(true))
                    .role(Role::Slider)
                    .aria_label("色相与饱和度；左右调整色相，上下调整饱和度")
                    .relative()
                    .flex_shrink_0()
                    .w(surface::css(200.))
                    .h(surface::css(120.))
                    .border_1()
                    .border_color(palette.picker_border())
                    .overflow_hidden()
                    .child(h_flex().absolute().inset_0().children((0..6).map(|sector| {
                        div().flex_1().h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(hsv_color(sector as f32 / 6., 1., 1.), 0.),
                            linear_color_stop(hsv_color((sector + 1) as f32 / 6., 1., 1.), 1.),
                        ))
                    })))
                    .child(div().absolute().inset_0().bg(linear_gradient(
                        180.,
                        linear_color_stop(palette.white().opacity(0.), 0.),
                        linear_color_stop(palette.white(), 1.),
                    )))
                    .child(
                        div()
                            .absolute()
                            .left(relative(hue))
                            .top(relative(1. - saturation))
                            .ml(surface::css(-7.))
                            .mt(surface::css(-7.))
                            .size(surface::css(14.))
                            .rounded_full()
                            .border_2()
                            .border_color(palette.white())
                            .shadow(vec![BoxShadow {
                                inset: false,
                                color: palette.selected_dot(),
                                offset: point(px(0.), px(0.)),
                                blur_radius: px(2.),
                                spread_radius: px(0.),
                            }]),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| plane_bounds.set(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.plane_focus.focus(window, cx);
                            this.pick_plane(event.position, window, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_drag(ColorPlaneDrag(entity_id), |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    })
                    .on_drag_move(cx.listener(
                        move |this, event: &DragMoveEvent<ColorPlaneDrag>, window, cx| {
                            if event.drag(cx).0 == entity_id {
                                this.pick_plane(event.event.position, window, cx);
                            }
                        },
                    ))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "left" => this.hue = (this.hue - 1. / 198.).max(0.),
                            "right" => this.hue = (this.hue + 1. / 198.).min(1.),
                            "up" => this.saturation = (this.saturation + 1. / 118.).min(1.),
                            "down" => this.saturation = (this.saturation - 1. / 118.).max(0.),
                            _ => return,
                        }
                        cx.stop_propagation();
                        this.update_from_hsv(window, cx);
                    })),
            )
            .child(
                h_flex()
                    .relative()
                    .mt(surface::css(10.))
                    .h(surface::css(24.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .size(surface::css(24.))
                            .border_1()
                            .border_color(palette.white())
                            .bg(color_value(self.draft)),
                    )
                    .child(
                        div()
                            .id("brightness-focus")
                            .track_focus(&self.brightness_focus.clone().tab_stop(true))
                            .ml(surface::css(10.))
                            .w(surface::css(130.))
                            .h(surface::css(24.))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                let delta = match event.keystroke.key.as_str() {
                                    "left" | "down" => -1. / 130.,
                                    "right" | "up" => 1. / 130.,
                                    _ => return,
                                };
                                this.value = (this.value + delta).clamp(0., 1.);
                                this.brightness.update(cx, |state, cx| {
                                    state.set_value(this.value, window, cx)
                                });
                                this.update_from_hsv(window, cx);
                                cx.stop_propagation();
                            }))
                            .child(
                                BaseSlider::new(&brightness)
                                    .relative()
                                    .w_full()
                                    .h_full()
                                    .child(
                                        SliderTrack::new(&brightness)
                                            .relative()
                                            .mt(surface::css(4.))
                                            .h(surface::css(16.))
                                            .w_full()
                                            .bg(linear_gradient(
                                                90.,
                                                linear_color_stop(hsv_color(0., 0., 0.), 0.),
                                                linear_color_stop(
                                                    hsv_color(hue, saturation, 1.),
                                                    1.,
                                                ),
                                            ))
                                            .border_1()
                                            .border_color(palette.swatch_border())
                                            .child(
                                                SliderIndicator::new(&brightness)
                                                    .absolute()
                                                    .inset_0(),
                                            )
                                            .child(
                                                SliderThumb::new(&brightness)
                                                    .absolute()
                                                    .left(relative(self.value))
                                                    .ml(surface::css(-4.))
                                                    .mt(surface::css(-4.))
                                                    .w(surface::css(8.))
                                                    .h(surface::css(20.))
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .bottom_0()
                                                            .w_full()
                                                            .h(surface::css(16.))
                                                            .border_1()
                                                            .border_color(palette.white()),
                                                    )
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top_0()
                                                            .w_full()
                                                            .h(surface::css(4.))
                                                            .text_size(surface::css(8.))
                                                            .line_height(surface::css(4.))
                                                            .text_color(palette.white())
                                                            .child("▼"),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("screen-color")
                            .accessibility_label("从屏幕选取颜色")
                            .tooltip("尚未连接系统拾色服务")
                            .disabled(true)
                            .ghost()
                            .p_0()
                            .size(surface::css(24.))
                            .ml(surface::css(12.))
                            .child(img("synapse/color-picker.svg").size(surface::css(16.))),
                    ),
            )
            .child(
                h_flex()
                    .mt(surface::css(10.))
                    .h(surface::css(48.))
                    .flex_shrink_0()
                    .children(
                        self.fields
                            .iter()
                            .zip(["HEX", "R", "G", "B"])
                            .enumerate()
                            .map(|(index, (field, label))| {
                                v_flex()
                                    .w(surface::css(if index == 0 { 70. } else { 45. }))
                                    .h_full()
                                    .child(
                                        Input::new(field)
                                            .appearance(false)
                                            .w(surface::css(if index == 0 { 65. } else { 40. }))
                                            .h(surface::css(26.))
                                            .px_0()
                                            .py_0()
                                            .text_size(surface::css(14.))
                                            .text_center()
                                            .rounded(px(0.))
                                            .border_1()
                                            .border_color(palette.picker_border())
                                            .bg(palette.surface()),
                                    )
                                    .child(
                                        div()
                                            .mt_auto()
                                            .mr(surface::css(5.))
                                            .text_center()
                                            .text_size(surface::css(14.))
                                            .line_height(surface::css(19.))
                                            .child(label),
                                    )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .mt(surface::css(10.))
                    .justify_around()
                    .child(
                        Button::new("cancel-color")
                            .label(crate::i18n::t("CANCEL"))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(palette.secondary())
                                    .foreground(palette.white())
                                    .hover(palette.secondary().opacity(0.8))
                                    .active(palette.secondary().opacity(0.6)),
                            )
                            .h(surface::css(27.))
                            .px_0()
                            .py_0()
                            .text_size(surface::css(12.))
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .w(surface::css(90.))
                            .min_w(surface::css(90.))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.cancel_editor(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("save-color")
                            .label(crate::i18n::t("SAVE"))
                            .primary()
                            .h(surface::css(27.))
                            .px_0()
                            .py_0()
                            .text_size(surface::css(12.))
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .w(surface::css(90.))
                            .min_w(surface::css(90.))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_editor(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for LightingColorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = PaletteColors;
        let state = self.props.state.clone();
        let current = state.read(cx).value();
        let open = state.read(cx).is_open() && !self.props.disabled;
        let focus = state.read(cx).focus_handle(cx);
        let bounds = self.trigger_bounds.clone();
        let root_view = cx.entity().downgrade();
        let scale = window.rem_size() / px(16.);
        let gap = if self.upward {
            1.
        } else if self.editing.is_some() {
            2.
        } else {
            0.
        };
        ColorPickerRoot::new("lighting-color-root")
            .track_focus(&focus)
            .accessibility_label(self.props.label.clone())
            .disabled(self.props.disabled)
            .open(open)
            .on_open_change(move |open, window, cx| {
                let _ = root_view.update(cx, |this, cx| this.set_open(open, window, cx));
            })
            .child(
                Popover::new("lighting-palette")
                    .open(open)
                    .anchor(if self.upward {
                        Anchor::BottomLeft
                    } else {
                        Anchor::TopLeft
                    })
                    .offset(px(gap * scale))
                    .on_open_change(
                        cx.listener(|this, open, window, cx| this.set_open(*open, window, cx)),
                    )
                    .trigger(
                        BaseButton::new("color-trigger")
                            .flex()
                            .items_center()
                            .justify_between()
                            .styles(|s| s.disabled(|s| s.opacity(0.3)))
                            .tab_stop(false)
                            .accessibility_label(self.props.label.clone())
                            .disabled(self.props.disabled)
                            .relative()
                            .w(surface::css(53.))
                            .h(surface::css(27.))
                            .px(surface::css(5.))
                            .py_0()
                            .rounded(px(0.))
                            .border_1()
                            .border_color(if open {
                                cx.theme().primary
                            } else if self.props.dial_trigger {
                                palette.surface().opacity(0.)
                            } else {
                                cx.theme().input
                            })
                            .bg(if self.props.dial_trigger {
                                palette.surface().opacity(0.)
                            } else {
                                palette.surface()
                            })
                            .when(!self.props.disabled, |button| {
                                button.hover(|style| style.border_color(cx.theme().primary))
                            })
                            .focus_visible(|style| style.border_color(cx.theme().primary))
                            .when(self.props.dial_trigger && !self.props.disabled, |button| {
                                button.group_hover("dial-row", |style| {
                                    style.bg(hsv_color(0., 0., 0.))
                                })
                            })
                            .child(
                                div()
                                    .size(surface::css(20.))
                                    .flex_shrink_0()
                                    .rounded(cx.theme().font_size * (3. / 16.))
                                    .border_1()
                                    .border_color(palette.swatch_border())
                                    .bg(current.unwrap_or(palette.white()))
                                    .when(current.is_none(), |s| {
                                        s.child(
                                            img("synapse/palette-none.svg").size(surface::css(18.)),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .w(surface::css(10.))
                                    .h(surface::css(5.))
                                    .when(self.props.dial_trigger && !open, |icon| {
                                        icon.opacity(0.)
                                            .group_hover("dial-row", |style| style.opacity(1.))
                                    })
                                    .child(
                                        Icon::default()
                                            .path("synapse/expand.svg")
                                            .w(surface::css(10.))
                                            .h(surface::css(5.))
                                            .transform(Transformation::rotate(radians(
                                                gpui_kit::base::motion::transition(
                                                    ElementId::from((
                                                        "palette-arrow",
                                                        cx.entity_id(),
                                                    )),
                                                    if open { std::f32::consts::PI } else { 0. },
                                                    gpui_kit::base::motion::Transition::new(
                                                        std::time::Duration::from_millis(300),
                                                    )
                                                    .easing(gpui_kit::base::motion::Easing::Ease),
                                                    window,
                                                    cx,
                                                ),
                                            ))),
                                    ),
                            )
                            .child(
                                canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                                    .absolute()
                                    .inset_0(),
                            ),
                    )
                    .when(open, |popover| {
                        let content = if self.editing.is_some() {
                            self.render_editor(cx)
                        } else {
                            self.render_palette(cx)
                        };
                        let editing = self.editing.is_some();
                        popover.content(move |_, window, cx| {
                            // The secondary picker uses display:none/block;
                            // only the palette has the .1s opacity transition.
                            let opacity = if editing {
                                1.
                            } else {
                                gpui_kit::base::motion::Presence::new("palette-fade", true)
                                    .transition(
                                        gpui_kit::base::motion::Transition::new(
                                            std::time::Duration::from_millis(100),
                                        )
                                        .easing(gpui_kit::base::motion::Easing::Ease),
                                    )
                                    .sample(window, cx)
                                    .progress
                            };
                            div().opacity(opacity).child(content)
                        })
                    }),
            )
    }
}
