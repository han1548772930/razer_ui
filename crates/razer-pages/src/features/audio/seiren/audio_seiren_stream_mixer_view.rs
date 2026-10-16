//! UI5235 xe/de/ue/H with current 684 CSS. Exact source dimensions scale with
//! rem and source channel-height clamp; Kit owns controls, focus and overlays.
use super::super::audio_seiren_eq::colors::Colors as C;
use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{
    Button as BaseButton, Checkbox as BaseCheckbox, Dialog, ElementExt as _, Positioner,
    Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack,
};
use gpui_kit::component::{
    IconName, Size, StyleSized,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    select::Caret,
};
use razer_widgets::{surface::css, synapse_select};

#[derive(Deserialize)]
struct Presentation {
    labels: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
}

fn presentation(pid: u32) -> &'static Presentation {
    static DATA: OnceLock<BTreeMap<String, Presentation>> = OnceLock::new();
    &DATA.get_or_init(|| read_audio_seiren_stream_mixer_catalog())[&pid.to_string()]
}
pub(super) fn text(pid: u32, key: &str) -> String {
    t(&presentation(pid).labels[key])
}
pub(super) fn asset(pid: u32, key: &str) -> SharedString {
    presentation(pid).assets[key].clone().into()
}
fn action_button(id: impl Into<ElementId>, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .h(css(27.))
        .min_w(css(90.))
        .px(css(10.))
        .rounded(css(3.))
        .text_size(css(12.))
        .bg(if primary { C::selected() } else { C::remainder() })
        .text_color(if primary { C::black() } else { C::white() })
        .hover(|style| style.opacity(0.9))
        .active(|style| style.opacity(0.8))
}
fn meter(level: f32, height: f32) -> Div {
    let level = level.clamp(0., 100.) / 100.;
    div()
        .relative()
        .w(css(10.))
        .h(css(height))
        .bg(C::black())
        .overflow_hidden()
        .children(
            [
                (0., 0.6667, C::stream_green()),
                (0.6667, 0.1666, C::stream_yellow()),
                (0.8333, 0.1667, C::stream_red()),
            ]
            .into_iter()
            .filter_map(move |(bottom, height, color)| {
                let height = (level - bottom).clamp(0., height);
                (height > 0.).then(|| {
                    div()
                        .absolute()
                        .bottom(relative(bottom))
                        .h(relative(height))
                        .w_full()
                        .bg(color)
                })
            }),
        )
}
fn native_meter(data: &Value, enum_name: &str) -> [f32; 2] {
    let Some(meter) = data["levelMeter"].as_array().and_then(|meters| {
        meters
            .iter()
            .find(|meter| meter["deviceIdEnum"] == enum_name)
    }) else {
        return [0.; 2];
    };
    let left = meter["levelMeter"][0].as_f64().unwrap_or(0.) as f32;
    [
        left,
        meter["levelMeter"][1]
            .as_f64()
            .map_or(left, |value| value as f32),
    ]
}
#[derive(Clone)]
struct ChannelDrag {
    owner: EntityId,
    name: String,
}
/// UI5235 drags the whole `.draggable-channel`. Keep a display snapshot so the
/// drag layer never mounts another copy of the source sliders/select entities.
struct ChannelDragPreview {
    pid: u32,
    channel: Value,
    levels: [f32; 2],
    height: f32,
    scale: f32,
    connected: bool,
    selected_device: String,
}
impl Render for ChannelDragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pid = self.pid;
        let channel = &self.channel;
        let name = channel["name"].as_str().unwrap_or("");
        let scale = self.scale;
        let linked = channel["isLinked"] == true;
        let configurable = channel["inputChannelType"] == "configurable";
        let mut column =
            v_flex()
                .relative()
                // GPUI anchors the preview to the initiating menu handle. Its
                // source position is x=5, y=10*s+(20*s-16)/2 within the column.
                .left(css(-5.))
                .top(css(8. - 20. * scale))
                .w(css(128.))
                .h(css(self.height))
                .flex_shrink_0()
                .items_center()
                .pt(css(10. * scale))
                .pb(css(15. * scale))
                .bg(C::knob())
                .rounded(css(5.))
                .text_color(C::text())
                .font_family("Roboto")
                .child(
                    h_flex()
                        .w(css(118.))
                        .h(css(20. * scale))
                        .mb(css(10. * scale))
                        .items_center()
                        .justify_between()
                        .child(svg().path(asset(pid, "menu")).size(css(16.)))
                        .child(svg().path(asset(pid, "close")).size(css(12.))),
                )
                .child(
                    div()
                        .h(css(16. * scale))
                        .mb(css(20. * scale))
                        .px(css(8.))
                        .text_size(css(14.))
                        .line_height(css(16. * scale))
                        .child(name.to_uppercase()),
                )
                .child(
                    h_flex()
                        .gap(css(18.))
                        .h(css(300. * scale))
                        .mb(css(6. * scale))
                        .children(["streamMixerInputVolume", "playbackMixInputVolume"].map(
                            |field| fader_snapshot(&channel[field], self.levels, 300. * scale),
                        )),
                )
                .child(
                    h_flex().gap(css(20.)).h(css(20. * scale)).children(
                        [
                            ("streamMixerInputVolume", "broadcast"),
                            ("playbackMixInputVolume", "headphone"),
                        ]
                        .into_iter()
                        .map(|(field, icon)| {
                            svg()
                                .path(asset(
                                    pid,
                                    &format!(
                                        "{icon}{}",
                                        if channel[field]["isEnabled"] == true {
                                            ""
                                        } else {
                                            "-disabled"
                                        }
                                    ),
                                ))
                                .size(css(20. * scale))
                        }),
                    ),
                )
                .child(
                    h_flex()
                        .h(css(12. * scale))
                        .mt(css(8. * scale))
                        .mb(css(10. * scale))
                        .items_center()
                        .gap(css(2.))
                        .child(svg().path(asset(pid, "link-left")).w(css(20.)).h(css(12.)))
                        .child(
                            svg()
                                .path(asset(pid, if linked { "link" } else { "unlink" }))
                                .size(css(16. * scale)),
                        )
                        .child(svg().path(asset(pid, "link-right")).w(css(20.)).h(css(12.))),
                );
        if configurable {
            let width = if self.connected { 118. } else { 100. };
            let mut footer = h_flex().w(css(118.)).h(css(27. * scale)).items_center();
            if !self.connected {
                footer = footer.child(svg().path(asset(pid, "device-warning")).size(css(16.)));
            }
            // The mounted source uses native <select>. This mirrors the
            // retained Kit Select trigger without installing interactive state
            // or registering hitboxes in the drag image.
            footer = footer.child(
                h_flex()
                    .relative()
                    .input_size(Size::XSmall)
                    .input_text_size(Size::XSmall)
                    .w(css(width))
                    .h(css(27. * scale))
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().input_background())
                    .text_color(cx.theme().foreground)
                    .text_size(css(11.))
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .gap_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(self.selected_device.clone()),
                            )
                            .child(
                                Caret::new(Size::XSmall).text_color(cx.theme().muted_foreground),
                            ),
                    ),
            );
            column = column.child(footer);
        } else {
            column = column.child(
                div()
                    .w(css(118.))
                    .h(css(27. * scale))
                    .text_size(css(11.))
                    .text_center()
                    .text_ellipsis()
                    .child(
                        channel["exterInputDevice"]
                            .as_str()
                            .filter(|name| !name.is_empty())
                            .unwrap_or(name)
                            .to_owned(),
                    ),
            );
        }
        column
    }
}

fn fader_snapshot(volume: &Value, levels: [f32; 2], height: f32) -> Div {
    let progress = volume["value"].as_f64().unwrap_or(0.).clamp(0., 100.) as f32 / 100.;
    div()
        .relative()
        .w(css(24.))
        .h(css(height))
        .flex_shrink_0()
        .child(
            h_flex()
                .gap(css(4.))
                .child(meter(levels[0], height))
                .child(meter(levels[1], height)),
        )
        .child(
            div()
                .absolute()
                .left(css(2.))
                .bottom(relative(progress))
                .mb(css(-5.))
                .w(css(20.))
                .h(css(10.))
                .bg(C::thumb())
                .border_1()
                .border_color(C::thumb_border())
                .rounded(css(2.))
                .opacity(if volume["isEnabled"] == true { 1. } else { 0.4 }),
        )
}

/// UI5235 positions the 600x339 panel from the add button's right edge and
/// vertical midpoint. Resolve after trigger prepaint so resize and horizontal
/// scrolling use current bounds; Base retains clipping, occlusion and paint.
struct AddOptions {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    content: Option<AnyElement>,
}
impl IntoElement for AddOptions {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for AddOptions {
    type RequestLayoutState = (Positioner, <Positioner as Element>::RequestLayoutState);
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .occlude()
            .child(self.content.take().expect("source add panel"));
        let (layout, state) = positioner.request_layout(None, None, window, cx);
        (layout, (positioner, state))
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scale = f32::from(window.rem_size()) / 16.;
        let position = add_panel_position(self.bounds.get(), window.viewport_size(), scale);
        state.0 = std::mem::replace(
            &mut state.0,
            Positioner::corner(Anchor::TopLeft, Point::default()),
        )
        .position(position);
        state
            .0
            .prepaint(None, None, bounds, &mut state.1, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        state
            .0
            .paint(None, None, bounds, &mut state.1, &mut (), window, cx);
    }
}
fn add_panel_position(
    trigger: Bounds<Pixels>,
    viewport: gpui_kit::Size<Pixels>,
    scale: f32,
) -> Point<Pixels> {
    let inset = 10. * scale;
    let left = f32::from(trigger.right()) - 600. * scale + 100. * scale;
    let top = f32::from(trigger.top()) + f32::from(trigger.size.height) / 2.;
    point(
        px(left
            .min(f32::from(viewport.width) - 600. * scale - inset)
            .max(inset)),
        px(top
            .min(f32::from(viewport.height) - 339. * scale - inset)
            .max(inset)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn stream_mixer_add_panel_uses_current_trigger_bounds_and_viewport_clamp() {
        let trigger = Bounds::new(point(px(900.), px(200.)), size(px(128.), px(300.)));
        assert_eq!(
            add_panel_position(trigger, size(px(1600.), px(1000.)), 1.),
            point(px(528.), px(350.))
        );
        assert_eq!(
            add_panel_position(trigger, size(px(1000.), px(600.)), 1.),
            point(px(390.), px(251.))
        );
        let scrolled = Bounds::new(point(px(-100.), px(-200.)), trigger.size);
        assert_eq!(
            add_panel_position(scrolled, size(px(1000.), px(600.)), 1.),
            point(px(10.), px(10.))
        );
    }
}

impl AudioProductWorkspace {
    pub(in crate::features) fn seiren_stream_mixer_page(
        &self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !matches!(key, "STREAM_MIXER_HEADER" | "TAB_STREAM_MIX")
            || !supported(self.spec.product_id)
        {
            return None;
        }
        let state = self.seiren_stream.as_ref()?;
        let pid = self.spec.product_id;
        let scale = f32::from(window.rem_size()) / 16.;
        let height =
            (f32::from(window.viewport_size().height) / scale * 0.85 - 280.).clamp(240., 381.);
        let channel_scale = height / 481.;
        let is_enabled = enabled(&state.data);
        let visible = channels(&state.data)
            .iter()
            .filter(|channel| channel["isVisible"] == true)
            .collect::<Vec<_>>();
        let has_add = channels(&state.data)
            .iter()
            .any(|channel| channel["inputChannelType"] == "fixed" && channel["isVisible"] != true)
            || state.data["recordDevices"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|device| {
                    !visible
                        .iter()
                        .any(|channel| channel["exterInputDevice"] == device["name"])
                });
        let mut input_rows = h_flex().gap(css(12.)).flex_shrink_0().items_stretch();
        for channel in visible {
            input_rows = input_rows.child(self.seiren_stream_input(
                channel,
                height,
                channel_scale,
                window,
                cx,
            ));
        }
        if has_add {
            input_rows = input_rows.child(self.seiren_stream_add(height, !is_enabled, window, cx));
        }
        let switch = surface::SynapseSwitch::new("seiren-stream-enable")
            .checked(is_enabled)
            .on_change(cx.listener(|this, _, window, cx| {
                let Some(state) = &mut this.seiren_stream else {
                    return;
                };
                if !enabled(&state.data) && state.data["activeStreamMixerStatus"] == true {
                    state.warning_open = true;
                    cx.notify();
                } else {
                    let next = !enabled(&state.data);
                    this.submit_seiren_stream_mixer(
                        "ON_SET_STREAM_MIXER_ENABLE",
                        json!({"isStreamMixerEnabled":next}),
                        window,
                        cx,
                    );
                }
            }));
        let body = v_flex()
            .id("seiren-stream-mixer-v2")
            .w_full()
            .font_family("Roboto")
            .text_color(C::text())
            .child(self.seiren_stream_presets(cx))
            .child(
                v_flex()
                    .id("seiren-stream-body")
                    .max_w(window.viewport_size().width - window.rem_size() * (124. / 16.))
                    .overflow_x_scroll()
                    .bg(C::surface())
                    .pl(css(60.))
                    .pb(css(30.))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(css(12.))
                            .mb(css(15.))
                            .child(
                                div()
                                    .text_size(css(18.))
                                    .text_color(C::selected())
                                    .child(text(pid, "Js1").to_uppercase()),
                            )
                            .child(switch)
                            .child(surface::help_control(
                                "seiren-stream-help",
                                text(pid, "idi"),
                            )),
                    )
                    .child(
                        h_flex()
                            .gap(css(16.))
                            .mb(css(10.))
                            .text_size(css(14.))
                            .line_height(css(16.))
                            .child(div().child(text(pid, "Z2r").to_uppercase()))
                            .child(div().ml(css(250.)).child(text(pid, "ueX").to_uppercase())),
                    )
                    .child(
                        h_flex()
                            .gap(css(16.))
                            .items_stretch()
                            .child(
                                h_flex()
                                    .gap(css(12.))
                                    .flex_shrink_0()
                                    .child(self.seiren_stream_output(
                                        true,
                                        height,
                                        channel_scale,
                                        cx,
                                    ))
                                    .child(self.seiren_stream_output(
                                        false,
                                        height,
                                        channel_scale,
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .w(css(1.))
                                            .h(css(height))
                                            .mx(css(12.))
                                            .bg(C::border()),
                                    ),
                            )
                            .child(input_rows.mr(css(24.))),
                    ),
            )
            .child(self.seiren_stream_routing(cx))
            .when(state.warning_open && !is_enabled, |view| {
                view.child(self.seiren_stream_warning(cx))
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if let Some(state) = &mut this.seiren_stream {
                        state.drag_channel = None;
                        state.drag_over = None;
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if let Some(state) = &mut this.seiren_stream {
                        state.drag_channel = None;
                        state.drag_over = None;
                    }
                    cx.notify();
                }),
            );
        Some(body.into_any_element())
    }

    fn seiren_stream_presets(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let pid = self.spec.product_id;
        let choices = presets(&state.data)
            .iter()
            .map(|preset| {
                MixerChoice::new(
                    preset["guid"].as_str().unwrap_or(""),
                    preset["name"].as_str().unwrap_or(""),
                )
            })
            .collect();
        let owner = cx.entity();
        let deleted = presets(&state.data).len() == 1;
        let command_owner = owner.clone();
        let menu = Button::new("seiren-stream-preset-menu")
            .ghost()
            .icon(IconName::Ellipsis)
            .w(css(28.))
            .h(css(27.))
            .dropdown_menu(move |mut menu, _, _| {
                for (command, label) in [
                    ("add", "ADD"),
                    ("rename", "RENAME"),
                    ("duplicate", "DUPLICATE"),
                    ("reset", "RESET_PROFILE"),
                    ("delete", "DELETE"),
                ] {
                    let owner = command_owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(t(label))
                            .disabled(command == "delete" && deleted)
                            .on_click(move |_, window, cx| {
                                owner.update(cx, |this, cx| {
                                    this.seiren_stream_mixer_preset_command(command, window, cx)
                                });
                            }),
                    );
                }
                menu
            });
        let mut profile = h_flex()
            .relative()
            .w(css(320.))
            .child(if state.renaming {
                Input::new(&state.rename)
                    .h(css(27.))
                    .w(css(288.))
                    .into_any_element()
            } else {
                synapse_select::select_pointer(&state.presets)
                    .items(choices)
                    .w(css(288.))
                    .h(css(27.))
                    .accessibility_label(text(pid, "QaP"))
                    .into_any_element()
            })
            .child(menu);
        if state.reset_open {
            profile = profile.child(self.seiren_stream_reset(cx));
        }
        h_flex()
            .w_full()
            .mb(css(20.))
            .child(
                v_flex()
                    .w(relative(0.5))
                    .items_end()
                    .pl(css(100.))
                    .child(
                        div()
                            .w(css(288.))
                            .mb(css(20.))
                            .text_size(css(14.))
                            .child(text(pid, "QaP").to_uppercase()),
                    )
                    .child(profile),
            )
            .child(
                v_flex()
                    .w(relative(0.5))
                    .pl(css(20.))
                    .child(
                        div()
                            .mb(css(10.))
                            .text_size(css(14.))
                            .child(text(pid, "JWp").to_uppercase()),
                    )
                    .child(self.seiren_stream_mixer_shortcut(cx)),
            )
            .into_any_element()
    }

    fn seiren_stream_reset(&self, cx: &mut Context<Self>) -> AnyElement {
        let pid = self.spec.product_id;
        deferred(
            v_flex()
                .id("seiren-stream-reset")
                .absolute()
                .right_0()
                .top(css(42.))
                .min_w(css(300.))
                .p(css(20.))
                .gap(css(10.))
                .bg(C::surface())
                .border_1()
                .border_color(C::destructive())
                .rounded(css(3.))
                .text_color(C::text())
                .text_size(css(14.))
                .occlude()
                .child(
                    div()
                        .text_center()
                        .text_color(C::destructive())
                        .font_weight(FontWeight::BOLD)
                        .child(text(pid, "DgD").to_uppercase()),
                )
                .child(div().text_center().child(text(pid, "iN2")))
                .child(
                    h_flex().gap(css(10.)).children(
                        [(true, "hJ0"), (false, "vMs")]
                            .into_iter()
                            .map(|(mapping, label)| {
                                action_button(
                                    SharedString::from(format!("seiren-stream-reset-{mapping}")),
                                    true,
                                )
                                .bg(C::destructive())
                                .text_color(C::surface())
                                .border_1()
                                .border_color(C::black().opacity(0.3))
                                .flex_1()
                                .child(text(pid, label))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        let Some(state) = &mut this.seiren_stream else {
                                            return;
                                        };
                                        state.reset_open = false;
                                        state.reset_timer = None;
                                        let guid = selected(&state.data).to_owned();
                                        if mapping {
                                            this.submit_seiren_stream_mixer(
                                        "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING",
                                        json!({"newShortcutMapping":{},"changePresetGUID":guid}),
                                        window,
                                        cx,
                                    );
                                        } else {
                                            this.submit_seiren_stream_mixer(
                                                "ON_RESET_STREAM_MIXER_PRESET",
                                                json!({"selectedPresetGuid":guid}),
                                                window,
                                                cx,
                                            );
                                        }
                                    },
                                ))
                            }),
                    ),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some(state) = &mut this.seiren_stream {
                            state.reset_open = false;
                            state.reset_timer = None;
                        }
                        cx.notify();
                    }),
                ),
        )
        .priority(11)
        .into_any_element()
    }

    fn seiren_stream_fader(
        &self,
        key: &str,
        enum_name: &str,
        volume: &Value,
        height: f32,
        disabled: bool,
        _cx: &Context<Self>,
    ) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let slider = &state.faders[key];
        let muted = volume["isEnabled"] != true;
        let levels = native_meter(&state.data, enum_name);
        let progress = volume["value"].as_f64().unwrap_or(0.).clamp(0., 100.) as f32 / 100.;
        div()
            .relative()
            .w(css(24.))
            .h(css(height))
            .flex_shrink_0()
            .child(
                h_flex()
                    .gap(css(4.))
                    .child(meter(levels[0], height))
                    .child(meter(levels[1], height)),
            )
            .child(
                BaseSlider::new(slider)
                    .vertical()
                    .disabled(disabled || muted)
                    .size_full()
                    .absolute()
                    .inset_0()
                    .opacity(if muted { 0.4 } else { 1. })
                    .child(
                        SliderTrack::new(slider)
                            .axis(Axis::Vertical)
                            .disabled(disabled || muted)
                            .size_full()
                            .child(
                                SliderIndicator::new(slider).size_full().relative().child(
                                    SliderThumb::new(slider)
                                        .axis(Axis::Vertical)
                                        .disabled(disabled || muted)
                                        .absolute()
                                        .left(css(2.))
                                        .bottom(relative(progress))
                                        .mb(css(-5.))
                                        .w(css(20.))
                                        .h(css(10.))
                                        .bg(C::thumb())
                                        .border_1()
                                        .border_color(C::thumb_border())
                                        .rounded(css(2.))
                                        .hover(|style| style.bg(C::thumb_hover()))
                                        .active(|style| style.bg(C::thumb_pressed())),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn seiren_stream_output(
        &self,
        stream: bool,
        height: f32,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let pid = self.spec.product_id;
        let data = &state.data;
        let disabled = !enabled(data);
        let (field, label, enum_name, icon) = if stream {
            ("streamMixVolume", "C_0", "DeviceId_StreamMix", "broadcast")
        } else {
            (
                "playbackMixVolume",
                "$N7",
                "DeviceId_PlaybackMix",
                "headphone",
            )
        };
        let muted = data[field]["isEnabled"] != true;
        let listening = (data["isStreamMixMonitor"] == true) == stream;
        let title = text(pid, label);
        let device = if stream {
            recipe(pid).device_name.as_str()
        } else {
            data["playbackMixDevice"].as_str().unwrap_or("")
        }
        .to_owned();
        let footer = if stream {
            div()
                .w(css(118.))
                .h(css(27. * scale))
                .text_size(css(12.))
                .line_height(css(14.))
                .text_center()
                .text_ellipsis()
                .child(if device.is_empty() { "-".into() } else { device })
                .into_any_element()
        } else {
            Select::new(&state.playback)
                .xsmall()
                .h(css(27. * scale))
                .w(css(118.))
                .text_size(css(11.))
                .placeholder("--")
                .disabled(disabled || data["playbackDevices"].as_array().is_none_or(Vec::is_empty))
                .into_any_element()
        };
        v_flex()
            .w(css(128.))
            .h(css(height))
            .flex_shrink_0()
            .items_center()
            .pt(css(10. * scale))
            .pb(css(15. * scale))
            .bg(C::knob())
            .rounded(css(5.))
            .opacity(if disabled { 0.5 } else { 1. })
            .child(
                BaseButton::new(SharedString::from(format!(
                    "seiren-stream-output-mute-{stream}"
                )))
                .disabled(disabled)
                .w(css(27.))
                .h(css(20. * scale))
                .mb(css(10. * scale))
                .p_0()
                .accessibility_label(title.clone())
                .child(
                    svg()
                        .path(asset(
                            pid,
                            &format!("{icon}{}", if muted { "-disabled" } else { "" }),
                        ))
                        .size_full(),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.toggle_seiren_stream_mixer_mute("output", field, window, cx)
                })),
            )
            .child(
                div()
                    .h(css(16. * scale))
                    .mb(css(20. * scale))
                    .px(css(8.))
                    .text_size(css(14.))
                    .line_height(css(16. * scale))
                    .child(title.to_uppercase()),
            )
            .child(self.seiren_stream_fader(
                &format!("output/{field}"),
                enum_name,
                &data[field],
                300. * scale,
                disabled,
                cx,
            ))
            .child(
                BaseButton::new(SharedString::from(format!("seiren-stream-listen-{stream}")))
                    .disabled(disabled)
                    .size(css(20. * scale))
                    .mt(css(10. * scale))
                    .mb(css(35. * scale))
                    .p_0()
                    .accessibility_label(t("LISTEN"))
                    .child(
                        svg()
                            .path(asset(
                                pid,
                                if listening { "listen" } else { "listen-disabled" },
                            ))
                            .size_full(),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        let Some(state) = &this.seiren_stream else {
                            return;
                        };
                        let next = state.data["isStreamMixMonitor"] != true;
                        this.submit_seiren_stream_mixer(
                            "ON_SET_STREAM_MIX_MONITOR",
                            json!({"isStreamMixMonitor":next}),
                            window,
                            cx,
                        );
                    })),
            )
            .child(footer)
            .into_any_element()
    }

    fn seiren_stream_input(
        &self,
        channel: &Value,
        height: f32,
        scale: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let pid = self.spec.product_id;
        let disabled = !enabled(&state.data);
        let name = channel["name"].as_str().unwrap_or("").to_owned();
        let enum_name = recipe(pid)
            .name_to_device_id
            .get(&name)
            .map_or("", String::as_str);
        let linked = channel["isLinked"] == true;
        let configurable = channel["inputChannelType"] == "configurable";
        let drag_name = name.clone();
        let drag_owner = cx.entity();
        let owner_id = cx.entity_id();
        let drop_name = name.clone();
        let over_name = name.clone();
        let remove_name = name.clone();
        let link_name = name.clone();
        let device = channel["exterInputDevice"].as_str().unwrap_or("");
        let connected = device.is_empty()
            || state.data["recordDevices"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| item["name"] == device);
        let preview = ChannelDragPreview {
            pid,
            channel: channel.clone(),
            levels: native_meter(&state.data, enum_name),
            height,
            scale,
            connected,
            selected_device: state
                .external
                .get(&name)
                .and_then(|select| select.read(cx).selected_value())
                .cloned()
                .unwrap_or_default(),
        };
        let policy = Transition::new(Duration::from_millis(200)).easing(Easing::Ease);
        let translate = motion::transition(
            SharedString::from(format!("seiren-stream-channel-{name}-translate")),
            if state.drag_over.as_deref() == Some(name.as_str()) {
                10.
            } else {
                0.
            },
            policy.clone(),
            window,
            cx,
        );
        let opacity = motion::transition(
            SharedString::from(format!("seiren-stream-channel-{name}-opacity")),
            if disabled || state.drag_channel.as_deref() == Some(name.as_str()) {
                0.5
            } else {
                1.
            },
            policy,
            window,
            cx,
        );
        let mut body = v_flex()
            .id(SharedString::from(format!("seiren-stream-channel-{name}")))
            .relative()
            .left(css(translate))
            .w(css(128.))
            .h(css(height))
            .items_center()
            .flex_shrink_0()
            .pt(css(10. * scale))
            .pb(css(15. * scale))
            .bg(C::knob())
            .rounded(css(5.))
            .opacity(opacity)
            .child(
                h_flex()
                    .w(css(118.))
                    .h(css(20. * scale))
                    .mb(css(10. * scale))
                    .justify_between()
                    .items_center()
                    .child(
                        BaseButton::new(SharedString::from(format!("seiren-stream-drag-{name}")))
                            .disabled(disabled)
                            .size(css(16.))
                            .p_0()
                            .accessibility_label(format!("Reorder {name}"))
                            .child(svg().path(asset(pid, "menu")).size_full())
                            .on_drag(
                                ChannelDrag {
                                    owner: owner_id,
                                    name: drag_name,
                                },
                                move |drag, _, _, cx| {
                                    drag_owner.update(cx, |this, cx| {
                                        if let Some(state) = &mut this.seiren_stream {
                                            state.drag_channel = Some(drag.name.clone());
                                        }
                                        cx.notify();
                                    });
                                    cx.new(|_| ChannelDragPreview {
                                        pid: preview.pid,
                                        channel: preview.channel.clone(),
                                        levels: preview.levels,
                                        height: preview.height,
                                        scale: preview.scale,
                                        connected: preview.connected,
                                        selected_device: preview.selected_device.clone(),
                                    })
                                },
                            ),
                    )
                    .child(
                        BaseButton::new(SharedString::from(format!("seiren-stream-remove-{name}")))
                            .disabled(disabled)
                            .size(css(12.))
                            .p_0()
                            .accessibility_label(format!("Close {name}"))
                            .child(svg().path(asset(pid, "close")).size_full())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_seiren_stream_mixer_channel(&remove_name, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .h(css(16. * scale))
                    .mb(css(20. * scale))
                    .px(css(8.))
                    .text_size(css(14.))
                    .line_height(css(16. * scale))
                    .child(name.to_uppercase()),
            )
            .child(
                h_flex()
                    .gap(css(18.))
                    .h(css(300. * scale))
                    .mb(css(6. * scale))
                    .child(self.seiren_stream_fader(
                        &format!("{name}/streamMixerInputVolume"),
                        enum_name,
                        &channel["streamMixerInputVolume"],
                        300. * scale,
                        disabled,
                        cx,
                    ))
                    .child(self.seiren_stream_fader(
                        &format!("{name}/playbackMixInputVolume"),
                        enum_name,
                        &channel["playbackMixInputVolume"],
                        300. * scale,
                        disabled,
                        cx,
                    )),
            )
            .child(
                h_flex().gap(css(20.)).h(css(20. * scale)).children(
                    [
                        ("streamMixerInputVolume", "broadcast"),
                        ("playbackMixInputVolume", "headphone"),
                    ]
                    .into_iter()
                    .map(|(field, icon)| {
                        let name = name.clone();
                        let muted = channel[field]["isEnabled"] != true;
                        BaseButton::new(SharedString::from(format!(
                            "seiren-stream-mute-{name}-{field}"
                        )))
                        .disabled(disabled)
                        .size(css(20. * scale))
                        .p_0()
                        .accessibility_label(format!(
                            "{name} {}",
                            if icon == "broadcast" {
                                t("STREAM_MIX_HEADER")
                            } else {
                                t("PLAYBACK_MIX")
                            }
                        ))
                        .child(
                            svg()
                                .path(asset(
                                    pid,
                                    &format!("{icon}{}", if muted { "-disabled" } else { "" }),
                                ))
                                .size_full(),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.toggle_seiren_stream_mixer_mute(&name, field, window, cx)
                            },
                        ))
                    }),
                ),
            )
            .child(
                h_flex()
                    .h(css(12. * scale))
                    .mt(css(8. * scale))
                    .mb(css(10. * scale))
                    .items_center()
                    .gap(css(2.))
                    .child(svg().path(asset(pid, "link-left")).w(css(20.)).h(css(12.)))
                    .child(
                        BaseButton::new(SharedString::from(format!("seiren-stream-link-{name}")))
                            .group(SharedString::from(format!("seiren-stream-link-{name}")))
                            .relative()
                            .disabled(disabled)
                            .size(css(16. * scale))
                            .p_0()
                            .accessibility_label(text(pid, if linked { "_lP" } else { "psq" }))
                            .child(
                                svg()
                                    .path(asset(pid, if linked { "link" } else { "unlink" }))
                                    .size_full()
                                    .group_hover(
                                        SharedString::from(format!("seiren-stream-link-{name}")),
                                        |style| style.opacity(0.),
                                    ),
                            )
                            .child(
                                svg()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .path(asset(
                                        pid,
                                        if linked { "link-hover" } else { "unlink-hover" },
                                    ))
                                    .size_full()
                                    .opacity(0.)
                                    .group_hover(
                                        SharedString::from(format!("seiren-stream-link-{name}")),
                                        |style| style.opacity(1.),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_seiren_stream_mixer_input(
                                    &link_name,
                                    "isLinked",
                                    json!(!linked),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(svg().path(asset(pid, "link-right")).w(css(20.)).h(css(12.))),
            )
            .on_drop(cx.listener(move |this, drag: &ChannelDrag, window, cx| {
                if drag.owner == owner_id {
                    this.reorder_seiren_stream_mixer(&drag.name, &drop_name, window, cx);
                }
            }))
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<ChannelDrag>, _, cx| {
                    let drag = event.drag(cx);
                    if drag.owner != owner_id {
                        return;
                    }
                    let Some(state) = &mut this.seiren_stream else {
                        return;
                    };
                    let over = !disabled
                        && drag.name != over_name
                        && event.bounds.contains(&event.event.position);
                    let next = if over {
                        Some(over_name.clone())
                    } else if state.drag_over.as_deref() == Some(over_name.as_str()) {
                        None
                    } else {
                        return;
                    };
                    if state.drag_over != next {
                        state.drag_over = next;
                        cx.notify();
                    }
                }),
            );
        if configurable {
            let device = channel["exterInputDevice"].as_str().unwrap_or("");
            let found = state.data["recordDevices"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| item["name"] == device)
                || device.is_empty();
            let mut footer = h_flex().w(css(118.)).h(css(27. * scale)).items_center();
            if !found {
                footer = footer.child(
                    div()
                        .id(SharedString::from(format!(
                            "seiren-stream-device-warning-{name}"
                        )))
                        .size(css(16.))
                        .child(svg().path(asset(pid, "device-warning")).size(css(16.)))
                        .tooltip({
                            let tip = text(pid, "YHN");
                            move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(tip.clone())
                                    .build(window, cx)
                            }
                        }),
                );
            }
            if let Some(select) = state.external.get(&name) {
                footer = footer.child(
                    Select::new(select)
                        .xsmall()
                        .h(css(27. * scale))
                        .w(css(if found { 118. } else { 100. }))
                        .text_size(css(11.))
                        .placeholder("")
                        .disabled(disabled),
                );
            }
            body = body.child(footer);
        } else {
            body = body.child(
                div()
                    .w(css(118.))
                    .h(css(27. * scale))
                    .text_size(css(11.))
                    .text_center()
                    .text_ellipsis()
                    .child(
                        channel["exterInputDevice"]
                            .as_str()
                            .filter(|name| !name.is_empty())
                            .unwrap_or(&name)
                            .to_owned(),
                    ),
            );
        }
        body.into_any_element()
    }

    fn seiren_stream_add(
        &self,
        height: f32,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let pid = self.spec.product_id;
        let bounds = state.add_bounds.clone();
        let popover = window.use_keyed_state(
            (ElementId::from("seiren-stream-add-popup"), "lifecycle"),
            cx,
            |_, cx| gpui_kit::base::PopoverState::new(false, cx),
        );
        popover.update(cx, |popover, cx| {
            popover.sync_open(state.add_open, window, cx)
        });
        let focus = popover.focus_handle(cx);
        div()
            .id("seiren-stream-add-popup")
            .relative()
            .w(css(128.))
            .h(css(height))
            .on_prepaint({
                let bounds = bounds.clone();
                move |current, _, _| bounds.set(current)
            })
            .child(
                BaseButton::new("seiren-stream-add")
                    .disabled(disabled)
                    .w(css(128.))
                    .h(css(height))
                    .border_2()
                    .border_color(C::grid())
                    .rounded(css(5.))
                    .opacity(if disabled { 0.3 } else { 1. })
                    .hover(|style| style.border_color(C::track()))
                    .accessibility_label(text(pid, "aB7"))
                    .child(img(asset(pid, "add")).size(css(24.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(state) = &mut this.seiren_stream {
                            state.add_open = !state.add_open;
                        }
                        cx.notify();
                    })),
            )
            .when(state.add_open, |trigger| {
                let content = div()
                    .id("seiren-stream-add-surface")
                    .role(Role::Dialog)
                    .tab_group()
                    .track_focus(&focus)
                    .key_context("Popover")
                    .on_action(
                        cx.listener(|this, _: &gpui_kit::base::actions::Cancel, _, cx| {
                            if let Some(state) = &mut this.seiren_stream {
                                state.add_open = false;
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseUpEvent, _, cx| {
                            if let Some(state) = &mut this.seiren_stream {
                                if !state.add_bounds.get().contains(&event.position) {
                                    state.add_open = false;
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .child(self.seiren_stream_add_content(cx));
                trigger.child(
                    deferred(AddOptions {
                        bounds,
                        content: Some(content.into_any_element()),
                    })
                    .with_priority(100),
                )
            })
            .into_any_element()
    }
    fn seiren_stream_add_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let pid = self.spec.product_id;
        let fixed = channels(&state.data)
            .iter()
            .filter(|channel| {
                channel["inputChannelType"] == "fixed" && channel["isVisible"] != true
            })
            .collect::<Vec<_>>();
        let external = state.data["recordDevices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|device| {
                !channels(&state.data).iter().any(|channel| {
                    channel["isVisible"] == true && channel["exterInputDevice"] == device["name"]
                })
            })
            .collect::<Vec<_>>();
        let mut list = v_flex()
            .id("seiren-stream-add-scroll")
            .max_h(css(260.))
            .overflow_y_scroll()
            .gap(css(10.));
        if !fixed.is_empty() {
            list = list.child(
                h_flex()
                    .flex_wrap()
                    .gap(css(10.))
                    .children(fixed.iter().map(|channel| {
                        let mut channel = (*channel).clone();
                        channel["isVisible"] = json!(true);
                        channel["streamMixerInputVolume"] = json!({"isEnabled":true,"value":50});
                        channel["playbackMixInputVolume"] = json!({"isEnabled":true,"value":50});
                        BaseButton::new(SharedString::from(format!(
                            "seiren-stream-add-{}",
                            channel["name"].as_str().unwrap_or("")
                        )))
                        .h(css(30.))
                        .px(css(12.))
                        .border_1()
                        .border_color(C::border())
                        .rounded(css(3.))
                        .text_size(css(14.))
                        .hover(|style| style.border_color(C::selected()))
                        .child(channel["name"].as_str().unwrap_or("").to_owned())
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.add_seiren_stream_mixer_channel(channel.clone(), window, cx)
                            },
                        ))
                    })),
            );
        }
        if !fixed.is_empty() && !external.is_empty() {
            list = list.child(div().h(css(1.)).w_full().bg(C::grid()).my(css(10.)));
        }
        if !external.is_empty() {
            list = list
                .child(
                    div()
                        .text_size(css(14.))
                        .child(text(pid, "snF").to_uppercase()),
                )
                .child(
                    h_flex().flex_wrap().gap(css(10.)).children(
                        external
                            .into_iter()
                            .filter_map(|device| device["name"].as_str())
                            .map(|name| {
                                let name = name.to_owned();
                                BaseButton::new(SharedString::from(format!(
                                    "seiren-stream-add-external-{name}"
                                )))
                                .h(css(30.))
                                .max_w(css(500.))
                                .px(css(12.))
                                .border_1()
                                .border_color(C::border())
                                .rounded(css(3.))
                                .text_size(css(14.))
                                .hover(|style| style.border_color(C::selected()))
                                .child(format!("+ {name}"))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.add_seiren_stream_mixer_external(&name, window, cx)
                                    },
                                ))
                            }),
                    ),
                );
        }
        v_flex()
            .w(css(600.))
            .h(css(339.))
            .py(css(30.))
            .px(css(40.))
            .bg(C::surface())
            .rounded(css(5.))
            .text_color(C::text())
            .font_family("Roboto")
            .child(
                h_flex()
                    .gap(css(10.))
                    .mb(css(20.))
                    .child(
                        div()
                            .text_size(css(18.))
                            .text_color(C::selected())
                            .child(text(pid, "aB7").to_uppercase()),
                    )
                    .child(surface::help_control(
                        "seiren-stream-add-help",
                        text(pid, "qup"),
                    )),
            )
            .child(list)
            .into_any_element()
    }

    fn seiren_stream_routing(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.seiren_stream.as_ref().expect("source mixer");
        let pid = self.spec.product_id;
        let disabled = !enabled(&state.data);
        let visible = channels(&state.data)
            .iter()
            .filter(|channel| channel["isVisible"] == true)
            .collect::<Vec<_>>();
        let cell = || {
            div()
                .w(css(70.))
                .min_h(css(26.))
                .p(css(6.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(css(12.))
                .line_height(css(14.))
        };
        let mut grid = v_flex()
            .id("seiren-stream-routing-grid")
            .gap(css(3.))
            .bg(C::surface())
            .rounded(css(5.));
        grid = grid.child(h_flex().gap(css(8.)).child(div().w(css(100.))).children(
            visible.iter().map(|channel| {
                cell()
                    .text_center()
                    .child(channel["name"].as_str().unwrap_or("").to_uppercase())
            }),
        ));
        for (field, label) in [
            ("streamMixerInputVolume", "C_0"),
            ("playbackMixInputVolume", "$N7"),
        ] {
            grid = grid.child(
                h_flex()
                    .gap(css(8.))
                    .child(
                        div()
                            .w(css(100.))
                            .p(css(6.))
                            .text_size(css(12.))
                            .line_height(css(14.))
                            .child(text(pid, label).to_uppercase()),
                    )
                    .children(visible.iter().map(|channel| {
                        let name = channel["name"].as_str().unwrap_or("").to_owned();
                        let checked = channel[field]["isEnabled"] == true;
                        let mut volume = channel[field].clone();
                        volume["isEnabled"] = json!(!checked);
                        let owner = cx.entity();
                        cell().child(
                            BaseCheckbox::new(SharedString::from(format!(
                                "seiren-stream-route-{name}-{field}"
                            )))
                            .checked(checked)
                            .disabled(disabled)
                            .accessibility_label(format!("{name} {}", text(pid, label)))
                            .size(css(18.))
                            .rounded(css(3.))
                            .border_1()
                            .border_color(if checked { C::selected() } else { C::route_border() })
                            .bg(if checked { C::selected() } else { C::surface() })
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|style| style.bg(C::selected().opacity(0.3)))
                            .when(checked, |control| {
                                control.child(img(asset(pid, "check")).size(css(10.)))
                            })
                            .on_change(move |_, _, window, cx| {
                                owner.update(cx, |this, cx| {
                                    this.set_seiren_stream_mixer_input(
                                        &name,
                                        field,
                                        volume.clone(),
                                        window,
                                        cx,
                                    )
                                });
                            }),
                        )
                    })),
            );
        }
        v_flex()
            .w_full()
            .mb(css(20.))
            .px(css(20.))
            .pb(css(20.))
            .opacity(if disabled { 0.5 } else { 1. })
            .child(
                h_flex()
                    .gap(css(10.))
                    .my(css(15.))
                    .child(
                        div()
                            .text_size(css(18.))
                            .text_color(C::selected())
                            .child(text(pid, "N_W").to_uppercase()),
                    )
                    .child(surface::help_control(
                        "seiren-stream-routing-help",
                        text(pid, "fR7"),
                    )),
            )
            .child(grid)
            .into_any_element()
    }
    fn seiren_stream_warning(&self, cx: &mut Context<Self>) -> AnyElement {
        let pid = self.spec.product_id;
        Dialog::new(cx)
            .close_on_escape(false)
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_cancel(|_, _, _| false)
            .backdrop(div().size_full().bg(C::overlay()))
            .popup(
                v_flex()
                    // Current warning-alert uses top:50%, translateY(-100%).
                    // Base centers the 164px popup; move it by half its height.
                    .relative()
                    .top(css(-82.))
                    .w(css(620.))
                    .h(css(164.))
                    .py(css(20.))
                    .px(css(30.))
                    .bg(C::surface())
                    .border_1()
                    .border_color(C::warning())
                    .rounded(css(5.))
                    .text_color(C::text())
                    .text_size(css(14.))
                    .child(
                        h_flex()
                            .justify_center()
                            .items_center()
                            .w_full()
                            .gap(css(10.))
                            .text_size(css(16.))
                            .line_height(css(16.8))
                            .text_color(C::warning())
                            .child(svg().path(asset(pid, "warning")).size(css(25.)))
                            .child(text(pid, "Ec2").to_uppercase()),
                    )
                    .child(
                        div()
                            .mt(css(20.))
                            .text_center()
                            .line_height(css(16.8))
                            .child(text(pid, "nlF")),
                    )
                    .child(
                        h_flex()
                            .justify_center()
                            .gap(css(20.))
                            .mt_auto()
                            .child(
                                action_button("seiren-stream-warning-cancel", false)
                                    .child(text(pid, "bOp"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(state) = &mut this.seiren_stream {
                                            state.warning_open = false;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                action_button("seiren-stream-warning-enable", true)
                                    .child(text(pid, "iRy"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if let Some(state) = &mut this.seiren_stream {
                                            state.warning_open = false;
                                        }
                                        this.submit_seiren_stream_mixer(
                                            "ON_SET_STREAM_MIXER_ENABLE",
                                            json!({"isStreamMixerEnabled":true}),
                                            window,
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/audio_seiren_stream_mixer_data.
fn read_audio_seiren_stream_mixer_catalog() -> BTreeMap<String, Presentation> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Presentation>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Presentation> {
        let value = match r.tag()? {
            0 => Presentation {
                labels: r.child(decode_3)?,
                assets: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_AUDIO_SEIREN_STREAM_MIXER_DATA,
        0,
        "0ea352da1f899bc8bbee0b91c313be422613a20bc12e49ada0445afd0d9126de",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod audio_seiren_stream_mixer_data_catalog_tests {
    #[test]
    fn packaged_catalog_audio_seiren_stream_mixer_data_decodes() {
        let _ = super::read_audio_seiren_stream_mixer_catalog();
    }
}
// End generated catalog reader: razer-pages/audio_seiren_stream_mixer_data.
