//! Current Studio 1698 color editor and 9170 RGB/HSV/HEX helpers.
//! Independent of the device-product color picker; emits only completed edits.
use super::theme::Colors;
use crate::ui::surface;
use gpui_kit::base::{Button, Slider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
    slider::{SliderEvent, SliderState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{cell::Cell, rc::Rc};

const WIDTH: f32 = 228.;
const HEIGHT: f32 = 136.;
const THRESHOLD: f32 = 191. / 255.;
const PRESETS: [Option<u32>; 8] = [
    None,
    Some(0xff0000),
    Some(0xff8000),
    Some(0xffff00),
    Some(0x00ff00),
    Some(0x00ffff),
    Some(0x0000ff),
    Some(0xff00ff),
];

struct Palette;
impl Palette {
    fn input_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    fn black() -> Hsla {
        rgb(0x000000).into()
    }
    fn swatch_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    fn checker_gray() -> Hsla {
        rgb(0xdedede).into()
    }
}

#[derive(Clone, Debug)]
pub(super) enum StudioColorEvent {
    Changed(Option<u32>),
}
impl EventEmitter<StudioColorEvent> for StudioColor {}

/// Current 4809/1638 custom picker array, scoped to this application's session.
/// Kept separate from device-product palettes and persisted profile files.
#[derive(Default)]
struct StudioCustomColors {
    colors: Vec<(u64, u32)>,
    next_id: u64,
}
impl Global for StudioCustomColors {}

pub(super) struct StudioColor {
    value: Option<u32>,
    committed: Option<u32>,
    base: Option<[u8; 3]>,
    marker: Option<(f32, f32)>,
    brightness: Entity<SliderState>,
    brightness_level: f32,
    brightness_pointer: bool,
    fields: [Entity<InputState>; 4],
    focus: FocusHandle,
    brightness_focus: FocusHandle,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    focused_field: Option<usize>,
    dirty: bool,
    dragging: bool,
    enabled: bool,
    _subscriptions: Vec<Subscription>,
}

fn unpack(value: u32) -> [u8; 3] {
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}
fn pack(value: [u8; 3]) -> u32 {
    (u32::from(value[0]) << 16) | (u32::from(value[1]) << 8) | u32::from(value[2])
}
// 9170:i and 9170:s, including round-to-byte at the conversion boundary.
fn hsv(rgb: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb.map(|c| f32::from(c) / 255.);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0. {
        0.
    } else if max == r {
        (g - b) / d + if g < b { 6. } else { 0. }
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    } / 6.;
    (h, if max == 0. { 0. } else { d / max }, max)
}
fn from_hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let sector = (h * 6.).floor();
    let f = h * 6. - sector;
    let p = v * (1. - s);
    let q = v * (1. - f * s);
    let t = v * (1. - (1. - f) * s);
    let channels = match sector as u32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    };
    channels.map(|c| (c * 255.).round().clamp(0., 255.) as u8)
}
// 9170:a: CSS shorthand expansion differs from ordinary integer parsing.
fn expand_hex(text: &str) -> String {
    match text.len() {
        1..=3 => text.repeat(6 / text.len()),
        4..=6 => format!("{text}{}", "f".repeat(6 - text.len())),
        _ => "000000".into(),
    }
}

impl StudioColor {
    pub(super) fn new(value: Option<u32>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if !cx.has_global::<StudioCustomColors>() {
            cx.set_global(StudioCustomColors::default());
        }
        let fields = std::array::from_fn(|_| cx.new(|cx| InputState::new(window, cx)));
        let brightness = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(100.)
        });
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.observe_global::<StudioCustomColors>(|_, cx| cx.notify()));
        for (index, input) in fields.iter().enumerate() {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event, window, cx| match event {
                    InputEvent::Focus => {
                        this.focused_field = Some(index);
                        input.update(cx, |input, cx| input.select_all(window, cx));
                        cx.notify();
                    }
                    InputEvent::Change if this.enabled => this.edit_input(index, window, cx),
                    InputEvent::Blur => {
                        this.focused_field = None;
                        if this.enabled && this.dirty {
                            this.write_fields(None, window, cx);
                            this.commit(cx);
                        }
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => window.blur(cx),
                    _ => {}
                },
            ));
        }
        subscriptions.push(cx.subscribe_in(
            &brightness,
            window,
            |this, slider, event, window, cx| {
                if !this.enabled {
                    return;
                }
                let level = slider.read(cx).value().end();
                if level != this.brightness_level {
                    this.brightness_level = level;
                    this.change_brightness(level, window, cx);
                }
                match event {
                    SliderEvent::Change(_) => {}
                    SliderEvent::Release(_) => {
                        this.brightness_pointer = false;
                        this.commit(cx);
                    }
                }
            },
        ));
        // Base accessibility actions set_value without emitting SliderEvent.
        // The tracked level excludes programmatic sync; pointer capture keeps
        // notify-before-Change ordering from committing an unfinished drag.
        subscriptions.push(
            cx.observe_in(&brightness, window, |this, slider, window, cx| {
                let level = slider.read(cx).value().end();
                if level == this.brightness_level {
                    return;
                }
                if !this.enabled {
                    slider.update(cx, |state, cx| {
                        state.set_value(this.brightness_level, window, cx)
                    });
                    return;
                }
                this.brightness_level = level;
                this.change_brightness(level, window, cx);
                if !this.brightness_pointer {
                    this.commit(cx);
                }
            }),
        );
        let mut this = Self {
            value: None,
            committed: None,
            base: None,
            marker: None,
            brightness,
            brightness_level: 100.,
            brightness_pointer: false,
            fields,
            focus: cx.focus_handle(),
            brightness_focus: cx.focus_handle(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            focused_field: None,
            dirty: false,
            dragging: false,
            enabled: true,
            _subscriptions: subscriptions,
        };
        this.set_value(value, window, cx);
        this
    }

    pub(super) fn value(&self) -> Option<u32> {
        self.value
    }

    /// Owner synchronization never emits a Changed event or retains an old draft.
    pub(super) fn set_value(
        &mut self,
        value: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dragging = false;
        self.brightness_pointer = false;
        self.dirty = false;
        self.committed = value;
        self.receive_color(value, window, cx);
        self.write_fields(None, window, cx);
        cx.notify();
    }

    pub(super) fn set_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        if !enabled {
            self.set_value(self.committed, window, cx);
        }
        cx.notify();
    }

    fn receive_color(&mut self, value: Option<u32>, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value;
        let Some(value) = value else {
            self.base = None;
            self.marker = None;
            return;
        };
        let channels = unpack(value);
        let (h, s, v) = hsv(channels);
        let mut base = channels;
        let mut brightness = 100.;
        if v < THRESHOLD {
            base = from_hsv(h, s, THRESHOLD);
            if base[0] == base[1] && base[1] == base[2] {
                base = [255; 3];
            }
            brightness = (v / THRESHOLD * 100.).round();
        }
        self.base = Some(base);
        self.marker = Some(((h * WIDTH).floor(), HEIGHT - (s * HEIGHT).floor()));
        self.brightness_level = brightness;
        self.brightness
            .update(cx, |state, cx| state.set_value(brightness, window, cx));
    }

    fn write_fields(&self, except: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let texts = if let Some(value) = self.value {
            let [r, g, b] = unpack(value);
            [
                format!("{value:06x}"),
                r.to_string(),
                g.to_string(),
                b.to_string(),
            ]
        } else {
            std::array::from_fn(|_| String::new())
        };
        for (index, text) in texts.into_iter().enumerate() {
            if except != Some(index) && self.fields[index].read(cx).value().as_ref() != text {
                self.fields[index].update(cx, |input, cx| input.set_value(text, window, cx));
            }
        }
    }

    fn edit_input(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.fields[index].read(cx).value().to_string();
        let mut text: String = raw
            .chars()
            .filter(|c| {
                if index == 0 {
                    c.is_ascii_hexdigit()
                } else {
                    c.is_ascii_digit()
                }
            })
            .collect();
        if index == 0 {
            text.truncate(6);
        } else if !text.is_empty() {
            // Native number inputs ignore maxLength. ge strips digits, then
            // leading zeros, then clamps; large pastes must saturate, not wrap.
            text = text
                .bytes()
                .fold(0_u16, |value, digit| {
                    (value * 10 + u16::from(digit - b'0')).min(255)
                })
                .to_string();
        }
        if text != raw {
            self.fields[index].update(cx, |input, cx| input.set_value(text.clone(), window, cx));
        }
        let value = if index == 0 {
            u32::from_str_radix(&expand_hex(&text), 16).unwrap_or(0)
        } else {
            pack(std::array::from_fn(|channel| {
                self.fields[channel + 1]
                    .read(cx)
                    .value()
                    .parse::<u8>()
                    .unwrap_or(0)
            }))
        };
        self.receive_color(Some(value), window, cx);
        self.dirty = true;
        self.write_fields(Some(index), window, cx);
        cx.notify();
    }

    fn change_brightness(&mut self, brightness: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = self.base else {
            return;
        };
        let (h, s, v) = hsv(base);
        self.value = Some(pack(if brightness >= 100. {
            base
        } else {
            from_hsv(h, s, v * brightness / 100.)
        }));
        self.dirty = true;
        self.write_fields(None, window, cx);
        cx.notify();
    }

    fn pick(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let bounds = self.bounds.get();
        let x = (f32::from(position.x - bounds.left()) / f32::from(bounds.size.width) * WIDTH)
            .clamp(0., WIDTH - 1.);
        let y = (f32::from(position.y - bounds.top()) / f32::from(bounds.size.height) * HEIGHT)
            .clamp(0., HEIGHT - 1.);
        self.marker = Some((x, y));
        // Canvas getImageData samples a pixel center in the RGB hue/white field.
        self.base = Some(from_hsv(
            (x.floor() + 0.5) / WIDTH,
            1. - (y.floor() + 0.5) / HEIGHT,
            1.,
        ));
        self.change_brightness(self.brightness.read(cx).value().end(), window, cx);
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if self.enabled && self.dirty {
            self.dirty = false;
            if self.committed != self.value {
                self.committed = self.value;
                cx.emit(StudioColorEvent::Changed(self.value));
            }
        }
        cx.notify();
    }

    fn plane(&self, cx: &mut Context<Self>) -> AnyElement {
        let bounds = self.bounds.clone();
        let owner = cx.weak_entity();
        let dragging = self.dragging;
        let mut plane = div()
            .id("studio-color-plane")
            .relative()
            .w(surface::css(WIDTH))
            .h(surface::css(HEIGHT))
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .children((0..6).map(|sector| {
                        div().flex_1().h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(rgb(pack(from_hsv(sector as f32 / 6., 1., 1.))), 0.),
                            linear_color_stop(
                                rgb(pack(from_hsv((sector + 1) as f32 / 6., 1., 1.))),
                                1.,
                            ),
                        ))
                    })),
            )
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(Colors::white().opacity(0.), 0.),
                linear_color_stop(Colors::white(), 1.),
            )));
        if let Some((x, y)) = self.marker {
            plane = plane.child(
                div()
                    .absolute()
                    .left(surface::css(x - 7.))
                    .top(surface::css(y - 7.))
                    .size(surface::css(14.))
                    .rounded_full()
                    .border_2()
                    .border_color(Colors::white())
                    .bg(rgb(self.base.map(pack).unwrap_or(0)))
                    .shadow(vec![BoxShadow {
                        inset: false,
                        color: Palette::black(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(2.),
                        spread_radius: px(0.),
                    }]),
            );
        }
        plane = plane.child(
            canvas(
                move |rect, _, _| bounds.set(rect),
                move |_, _, window, _| {
                    if !dragging {
                        return;
                    }
                    let moving = owner.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            let _ = moving.update(cx, |this, cx| {
                                if this.dragging {
                                    if event.dragging() {
                                        this.pick(event.position, window, cx);
                                    } else {
                                        this.dragging = false;
                                        this.commit(cx);
                                    }
                                }
                            });
                        }
                    });
                    let release = owner.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            let _ = release.update(cx, |this, cx| {
                                if this.dragging {
                                    this.dragging = false;
                                    this.commit(cx);
                                }
                            });
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        );
        plane
            .when(self.enabled, |plane| {
                plane
                    .track_focus(&self.focus)
                    .tab_index(0)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.focus.focus(window, cx);
                            this.dragging = true;
                            this.pick(event.position, window, cx);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.dragging = false;
                            this.commit(cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.pick(event.position, window, cx);
                            this.commit(cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.pick(event.position, window, cx);
                            this.commit(cx);
                        }),
                    )
            })
            .into_any_element()
    }

    fn brightness(&self, cx: &mut Context<Self>) -> AnyElement {
        let progress = self.brightness.read(cx).value().end() / 100.;
        let visible = self.base.is_some();
        let track = div()
            .absolute()
            .inset_0()
            .rounded(surface::css(3.))
            .when_some(self.base, |view, base| {
                view.bg(linear_gradient(
                    90.,
                    linear_color_stop(Palette::black(), 0.),
                    linear_color_stop(rgb(pack(base)), 1.),
                ))
            })
            .when(!visible, |view| view.child(checkered()))
            .border_1()
            .border_color(Palette::swatch_border());
        let thumb = SliderThumb::new(&self.brightness)
            .disabled(!self.enabled)
            .absolute()
            .left(relative(progress))
            .ml(surface::css(-3.))
            .w(surface::css(6.))
            .h(surface::css(16.))
            .border_1()
            .border_color(Colors::white())
            .shadow(vec![BoxShadow {
                inset: false,
                color: Palette::black(),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
            }])
            .child(
                div()
                    .absolute()
                    .top(surface::css(-7.))
                    .left(surface::css(-2.))
                    .w(surface::css(8.))
                    .h(surface::css(4.))
                    .child(
                        svg()
                            .path("synapse/chroma-studio-brightness-pointer.svg")
                            .size_full()
                            .text_color(Colors::white()),
                    ),
            );
        div()
            .id("studio-color-brightness")
            .relative()
            .flex_1()
            .h(surface::css(16.))
            .when(self.enabled, |view| {
                view.track_focus(&self.brightness_focus)
                    .tab_index(0)
                    .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, _| {
                        if event.button == MouseButton::Left {
                            this.brightness_pointer = true;
                        }
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let old = this.brightness.read(cx).value().end();
                        let next = match event.keystroke.key.as_str() {
                            "left" | "down" => old - 1.,
                            "right" | "up" => old + 1.,
                            "home" => 0.,
                            "end" => 100.,
                            _ => return,
                        }
                        .clamp(0., 100.);
                        this.brightness_level = next;
                        this.brightness
                            .update(cx, |slider, cx| slider.set_value(next, window, cx));
                        this.change_brightness(next, window, cx);
                        this.commit(cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }))
            })
            .child(
                Slider::new(&self.brightness)
                    .disabled(!self.enabled)
                    .relative()
                    .size_full()
                    .child(track)
                    .child(
                        SliderTrack::new(&self.brightness)
                            .disabled(!self.enabled)
                            .absolute()
                            .inset_0()
                            .child(
                                SliderIndicator::new(&self.brightness)
                                    .absolute()
                                    .left(surface::css(3.))
                                    .right(surface::css(3.))
                                    .h_full()
                                    .when(visible, |view| view.child(thumb)),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn checkered() -> impl IntoElement {
    // The source SVG tile is 8x8; repeat instead of stretching a single square.
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let unit = f32::from(window.rem_size()) / 16. * 4.;
            let columns = (f32::from(bounds.size.width) / unit).ceil() as usize;
            let rows = (f32::from(bounds.size.height) / unit).ceil() as usize;
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                for y in 0..rows {
                    for x in 0..columns {
                        window.paint_quad(fill(
                            Bounds::new(
                                bounds.origin + point(px(x as f32 * unit), px(y as f32 * unit)),
                                size(px(unit), px(unit)),
                            ),
                            if (x + y) % 2 == 0 {
                                Palette::checker_gray()
                            } else {
                                Colors::white()
                            },
                        ));
                    }
                }
            });
        },
    )
    .size_full()
}

impl Render for StudioColor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fields = div()
            .flex()
            .justify_between()
            .children(self.fields.iter().enumerate().map(|(index, field)| {
                let label = ["HEX", "R", "G", "B"][index];
                div()
                    .flex()
                    .flex_col()
                    .w(surface::css(if index == 0 { 77. } else { 46. }))
                    .when(index < 3, |view| view.mr(surface::css(5.)))
                    .child(
                        div()
                            .id((ElementId::from("studio-color-input"), label))
                            .h(surface::css(27.))
                            .border_1()
                            .border_color(if self.focused_field == Some(index) {
                                Colors::selected()
                            } else {
                                Palette::input_border()
                            })
                            .when(self.enabled, |view| {
                                view.hover(|style| style.border_color(Colors::selected()))
                            })
                            .capture_key_down(cx.listener(
                                move |this, event: &KeyDownEvent, window, cx| {
                                    if event.keystroke.key == "escape" {
                                        window.blur(cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    } else if this.enabled && index != 0 {
                                        let delta = match event.keystroke.key.as_str() {
                                            "up" => 1,
                                            "down" => -1,
                                            "." | "+" | "-" | "e" => {
                                                window.prevent_default();
                                                cx.stop_propagation();
                                                return;
                                            }
                                            _ => return,
                                        };
                                        let old = this.fields[index]
                                            .read(cx)
                                            .value()
                                            .parse::<i16>()
                                            .unwrap_or(0);
                                        let next = (old + delta).clamp(0, 255).to_string();
                                        this.fields[index].update(cx, |input, cx| {
                                            input.set_value(next, window, cx)
                                        });
                                        this.edit_input(index, window, cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                            .on_key_down(|_, _, cx| cx.stop_propagation())
                            .child(
                                Input::new(field)
                                    .appearance(false)
                                    .bordered(false)
                                    .disabled(!self.enabled)
                                    .w_full()
                                    .h_full()
                                    .px_0()
                                    .py(surface::css(2.))
                                    .text_size(surface::css(14.))
                                    .text_center()
                                    .rounded(px(0.))
                                    .text_color(Colors::text()),
                            ),
                    )
                    .child(
                        div()
                            .mt(surface::css(5.))
                            .text_center()
                            .text_size(surface::css(14.))
                            .child(label),
                    )
            }));
        let custom = cx.global::<StudioCustomColors>().colors.clone();
        let colors: Vec<_> = PRESETS
            .into_iter()
            .enumerate()
            .map(|(index, color)| (ElementId::from(("studio-color-preset", index)), color, None))
            .chain(custom.iter().map(|&(id, color)| {
                (
                    ElementId::from(("studio-custom-color", id)),
                    Some(color),
                    Some(id),
                )
            }))
            .collect();
        let mut presets =
            div()
                .grid()
                .grid_cols(8)
                .m(surface::css(-5.))
                .children(colors.into_iter().map(|(id, color, custom_id)| {
                    let swatch = Button::new(id)
                        .accessibility_label(
                            color
                                .map(|v| format!("#{v:06X}"))
                                .unwrap_or_else(|| "No color".into()),
                        )
                        .disabled(!self.enabled)
                        .size(surface::css(20.))
                        .rounded(surface::css(3.))
                        .overflow_hidden()
                        .border_1()
                        .border_color(Palette::swatch_border())
                        .when_some(color, |view, color| view.bg(rgb(color)))
                        .when(color.is_none(), |view| view.child(checkered()))
                        .when(self.enabled, |view| {
                            view.hover(|style| {
                                style.border_color(Palette::black()).shadow(vec![BoxShadow {
                                    inset: false,
                                    color: Colors::white(),
                                    offset: point(px(0.), px(0.)),
                                    blur_radius: px(0.),
                                    spread_radius: px(2.),
                                }])
                            })
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if !this.enabled {
                                return;
                            }
                            this.receive_color(color, window, cx);
                            this.write_fields(None, window, cx);
                            this.dirty = true;
                            this.commit(cx);
                        }));
                    let swatch = if let Some(id) = custom_id.filter(|_| self.enabled) {
                        let owner = cx.weak_entity();
                        swatch
                            .context_menu(move |menu, window, _| {
                                let owner = owner.clone();
                                menu.min_w(surface::css(90.).to_pixels(window.rem_size()))
                                    .item(PopupMenuItem::new(super::label("DELETE")).on_click(
                                        move |_, _, cx| {
                                            if owner
                                                .upgrade()
                                                .is_some_and(|owner| owner.read(cx).enabled)
                                            {
                                                cx.update_global::<StudioCustomColors, _>(
                                                    |state, _| {
                                                        state.colors.retain(|&(candidate, _)| {
                                                            candidate != id
                                                        });
                                                    },
                                                );
                                            }
                                        },
                                    ))
                            })
                            .into_any_element()
                    } else {
                        swatch.into_any_element()
                    };
                    div().p(surface::css(5.)).child(swatch)
                }));
        if custom.len() < 8 {
            presets = presets.child(
                div().p(surface::css(5.)).child(
                    Button::new("studio-custom-color-add")
                        .accessibility_label("Add New Custom Color")
                        .disabled(!self.enabled || self.value.is_none())
                        .size(surface::css(20.))
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(Palette::input_border())
                        .when(self.enabled && self.value.is_some(), |view| {
                            view.hover(|style| style.border_color(Colors::selected()))
                        })
                        .when(self.enabled && self.value.is_none(), |view| {
                            view.opacity(0.3)
                        })
                        .child(
                            svg()
                                .path("synapse/chroma-studio-add-gray.svg")
                                .size(surface::css(14.))
                                .text_color(Colors::helper()),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.enabled {
                                return;
                            }
                            let Some(color) = this.value() else {
                                return;
                            };
                            cx.update_global::<StudioCustomColors, _>(|state, _| {
                                if state.colors.len() < 8 {
                                    let id = state.next_id;
                                    state.next_id += 1;
                                    state.colors.push((id, color));
                                }
                            });
                        })),
                ),
            );
        }
        div()
            .id("studio-color")
            .flex()
            .flex_col()
            .w_full()
            .text_color(Colors::text())
            .child(
                div()
                    .w_full()
                    .h(surface::css(138.))
                    .border_1()
                    .border_color(Palette::input_border())
                    .mb(surface::css(10.))
                    .child(self.plane(cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .mb(surface::css(22.))
                    .child(
                        div()
                            .size(surface::css(24.))
                            .flex_shrink_0()
                            .mr(surface::css(11.))
                            .border_1()
                            .border_color(Colors::white())
                            .when_some(self.value, |view, color| view.bg(rgb(color)))
                            .when(self.value.is_none(), |view| view.child(checkered())),
                    )
                    .child(self.brightness(cx))
                    .child(
                        Button::new("studio-eyedropper")
                            .accessibility_label(super::label("TEXT_PICK_COLOR"))
                            .disabled(true)
                            .ml(surface::css(13.))
                            .size(surface::css(20.))
                            .opacity(if self.enabled { 0.3 } else { 1. })
                            .child(
                                svg()
                                    .path("synapse/chroma-studio-eyedropper-white.svg")
                                    .size_full()
                                    .text_color(Colors::text()),
                            ),
                    ),
            )
            .child(fields)
            .child(
                div()
                    .h(surface::css(1.))
                    .my(surface::css(10.))
                    .bg(Palette::input_border()),
            )
            .child(presets)
    }
}
