use super::{controls::Control, settings::EqKind, workspace::DeviceWorkspace};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface;
use razer_widgets::surface::SynapseSwitch as Switch;
impl DeviceWorkspace {
    pub(super) fn eq_page(&self, kind: EqKind, cx: &mut Context<Self>) -> AnyElement {
        let eq = self.settings().eq(kind);
        let prefix = if kind == EqKind::Audio {
            "audio"
        } else {
            "mic"
        };
        let presets = h_flex()
            .gap_3()
            .flex_wrap()
            .mb(surface::css(20.))
            .when(kind == EqKind::Mic, |this| this.w(surface::css(870.)))
            .children(kind.presets().iter().map(|preset| {
                let preset = *preset;
                Button::new(SharedString::from(format!("{prefix}-preset-{preset}")))
                    .label(preset_label(preset))
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(if eq.preset == preset {
                                cx.theme().secondary
                            } else {
                                cx.theme().group_box
                            })
                            .foreground(cx.theme().foreground)
                            .hover(cx.theme().group_box)
                            .active(cx.theme().secondary),
                    )
                    .h(surface::css(27.))
                    .min_w(surface::css(90.))
                    .px(surface::css(16.))
                    .py_0()
                    .text_size(surface::css(12.))
                    .border_1()
                    .border_color(if eq.preset == preset {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .when(kind == EqKind::Mic, |this| this.flex_1())
                    .selected(eq.preset == preset)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.edit(w, cx, |s| s.eq_mut(kind).select(kind, preset))
                    }))
            }));
        let chart = h_flex()
            .w(surface::css(if kind == EqKind::Mic { 860. } else { 530. }))
            .items_start()
            .mb(surface::css(20.))
            .children(eq.bands.iter().map(|band| {
                let decibel = band.decibel;
                let control = match kind {
                    EqKind::Audio => Control::Audio(band.frequency),
                    EqKind::Mic => Control::Mic(band.frequency),
                };
                v_flex()
                    .id(SharedString::from(format!(
                        "{prefix}-band-{}",
                        band.frequency
                    )))
                    .flex_shrink_0()
                    .w(surface::css(if kind == EqKind::Mic { 78. } else { 47. }))
                    .items_center()
                    .gap(surface::css(10.))
                    .child(
                        div()
                            .id("eq-slider")
                            .test_support()
                            .group("eq-slider")
                            .relative()
                            .h(surface::css(300.))
                            .w(surface::css(16.))
                            .child(eq_slider(&self.controls.sliders[&control], cx))
                            .child(
                                div()
                                    .id("eq-value")
                                    .test_support()
                                    .absolute()
                                    .left(surface::css(-28.))
                                    .top(surface::css(eq_bubble_top(decibel)))
                                    .w(surface::css(26.))
                                    .h(surface::css(20.))
                                    .rounded(surface::css(3.))
                                    .bg(cx.theme().primary)
                                    .text_color(cx.theme().button_primary_foreground)
                                    .text_size(surface::css(12.))
                                    .line_height(surface::css(20.))
                                    .text_center()
                                    .opacity(0.)
                                    .group_hover("eq-slider", |s| s.opacity(1.))
                                    .child(eq_bubble_label(decibel)),
                            ),
                    )
                    .child(div().text_xs().child(frequency_label(band.frequency)))
            }))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h(surface::css(300.))
                    .mr(surface::css(40.))
                    .items_end()
                    .justify_between()
                    .text_size(surface::css(12.))
                    .child("+5dB")
                    .child("0dB")
                    .child("-5dB"),
            )
            .child(
                div()
                    .w(surface::css(20.))
                    .h(surface::css(300.))
                    .flex()
                    .items_center()
                    .child(
                        surface::asset_button(
                            if kind == EqKind::Audio {
                                "audio-eq-reset"
                            } else {
                                "mic-eq-reset"
                            },
                            "synapse/eq-reset.svg",
                            "重置均衡器",
                            cx,
                        )
                        .size(surface::css(20.))
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.edit(w, cx, |s| s.eq_mut(kind).reset(kind))
                        })),
                    ),
            );
        let mut content = surface::panel(
            if kind == EqKind::Audio {
                "均衡器"
            } else {
                "麦克风均衡器"
            },
            cx,
        )
        .gap_0()
        .child(div().mt(surface::css(20.)).child(presets).child(chart));
        if kind == EqKind::Mic {
            content = content
                .w(surface::css(940.))
                .min_w(surface::css(940.))
                .max_w(surface::css(940.))
                .min_h(surface::css(473.))
                .mx_auto();
        }
        content
            .id(SharedString::from(format!("{prefix}-eq-panel")))
            .test_support()
            .into_any_element()
    }
    pub(super) fn sound_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let sound = self.settings();
        v_flex()
            .gap_5()
            .child(surface::product_banner(
                self.pid(),
                self.device().edition_id,
                self.device().layout_id,
                cx,
            ))
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        v_flex()
                            .gap_5()
                            .child(
                                surface::panel_with_control(
                                    "音量",
                                    Switch::new("playback-enabled")
                                        .accessibility_label("声音")
                                        .checked(sound.playback_enabled)
                                        .on_change(cx.listener(|this, value, w, cx| {
                                            this.edit(w, cx, |s| s.playback_enabled = *value)
                                        })),
                                    cx,
                                )
                                .child(self.slider(
                                    Control::Volume,
                                    "音量",
                                    !sound.playback_enabled,
                                    cx,
                                ))
                                .child(
                                    super::device_pages::system_button(
                                        "volume-mixer",
                                        "Windows 音量混合器",
                                        razer_platform::system::Properties::Volume,
                                    ),
                                ),
                            )
                            .child(surface::panel("声音属性", cx).child(
                                super::device_pages::system_button(
                                    "sound-properties",
                                    "打开 Windows 声音属性",
                                    razer_platform::system::Properties::Sound,
                                ),
                            )),
                    ))
                    .child(surface::page_column(self.eq_page(EqKind::Audio, cx))),
            )
            .into_any_element()
    }
}
fn eq_slider(state: &Entity<gpui_kit::component::slider::SliderState>, cx: &App) -> AnyElement {
    use gpui_kit::base::{Slider, SliderIndicator, SliderThumb, SliderTrack};
    let position = state.read(cx).percentage().end;
    // cM's 300px input includes a 16px thumb: the thumb center travels 284px.
    // Its track stays dim on both sides; only the zero marker is bright green.
    Slider::new(state)
        .vertical()
        .h_full()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            SliderTrack::new(state)
                .axis(Axis::Vertical)
                .h_full()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    SliderIndicator::new(state)
                        .relative()
                        .h(surface::css(284.))
                        .w(surface::css(6.))
                        .rounded(surface::css(2.5))
                        .bg(cx.theme().primary.opacity(0.3))
                        .child(
                            div()
                                .absolute()
                                .top(relative(0.5))
                                .mt(surface::css(-2.))
                                .left(surface::css(1.))
                                .size(surface::css(4.))
                                .rounded_full()
                                .bg(cx.theme().primary),
                        )
                        .child(
                            SliderThumb::new(state)
                                .axis(Axis::Vertical)
                                .absolute()
                                .bottom(relative(position))
                                .mb(surface::css(-8.))
                                .left(surface::css(-5.))
                                .size(surface::css(16.))
                                .rounded_full()
                                .border_1()
                                .border_color(cx.theme().primary)
                                .bg(cx.theme().primary)
                                .hover(|s| s.bg(cx.theme().button))
                                .active(|s| s.bg(cx.theme().list_hover)),
                        ),
                ),
        )
        .into_any_element()
}
// cM: span top 302, wrapper top -20 - 2.84 * percentage (range -5..5).
fn eq_bubble_top(decibel: i8) -> f32 {
    282. - 284. * (decibel.clamp(-5, 5) as f32 + 5.) / 10.
}
fn eq_bubble_label(decibel: i8) -> String {
    if decibel > 0 {
        format!("+{decibel}")
    } else {
        decibel.to_string()
    }
}
fn frequency_label(f: u32) -> String {
    if f >= 1000 {
        format!("{}kHz", f as f32 / 1000.)
    } else {
        format!("{f}Hz")
    }
}
fn preset_label(id: &str) -> String {
    razer_i18n::t_or(
        &format!("EQ_{}", id.to_uppercase()),
        match id {
            "default" => "默认",
            "amplified" => "增强",
            "vocals" => "人声",
            "bassboost" => "低音增强",
            "enhancedclarity" => "清晰度增强",
            "boost" => "增强",
            "broadcast" => "广播",
            "conference" => "会议",
            "custom" => "自定义",
            _ => id,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{DeviceWorkspace, eq_bubble_label};
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, px, size};

    #[gpui_kit::test]
    fn eq_value_bubble_follows_source_rail_geometry(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut workspace = None;
        let handle = cx.open_window(size(px(1280.), px(960.)), |window, cx| {
            let mut device = razer_model::demo::demo_keyboard();
            device.product_id = 777;
            device.category = razer_model::model::DeviceCategory::Headset;
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            view.update(cx, |view, cx| {
                view.set_page(crate::nav::Tab::Sound, window, cx)
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        for (value, offset, label) in [(-5, 282., "-5"), (0, 140., "0"), (5, -2., "+5")] {
            cx.update_window(handle.into(), |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.edit(window, cx, |s| s.audio.edit(31, value))
                });
                window.render_frame(cx);
                window.within("audio-band-31").hover("eq-slider", cx);
                let rail = window.within("audio-band-31").find("eq-slider").bounds();
                let bubble = window.within("audio-band-31").find("eq-value");
                assert!(bubble.visible());
                assert_eq!(bubble.bounds().size, size(px(26.), px(20.)));
                assert_eq!(bubble.bounds().left(), rail.left() - px(28.));
                assert_eq!(bubble.bounds().top(), rail.top() + px(offset));
                assert_eq!(eq_bubble_label(value), label);
                window.hover("device-navigation", cx);
                assert!(!window.within("audio-band-31").find("eq-value").visible());
            })
            .unwrap();
        }
    }
}
