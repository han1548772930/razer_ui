//! Current 1392/1442/3942 demo entry surface. Playback needs an audible native
//! player; a silent tutorial animation cannot implement this audio demo.
use crate::{
    i18n::t,
    ui::{surface, theme::AudioDemoColors as Colors},
};
use gpui_kit::component::{
    ActiveTheme,
    checkbox::Checkbox,
    slider::{Slider, SliderEvent, SliderState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    asset: String,
    width: f32,
    height: f32,
    title: String,
    floating_label: String,
    floating_default: bool,
}
fn specification(pid: u32) -> Option<&'static Spec> {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_demo_data.json"))
                .expect("audited audio demo descriptors")
        })
        .iter()
        .find(|spec| spec.product_id == pid)
}
pub(super) fn supports(pid: u32) -> bool {
    specification(pid).is_some()
}

pub(super) struct AudioDemo {
    spec: &'static Spec,
    // floatingVideoReducer is outside the audio profile in all three sources.
    // Keep this preference across tabs/profile changes without marking a draft.
    floating: bool,
    // The native audio/video player service is not part of this workspace.
    // Keep the poster action visible and report that boundary after an
    // explicit click; the control surface below is local preview state only.
    playback_requested: bool,
    // These entities mirror the current video-react control surface. They are
    // deliberately local: without the native media transport they only retain
    // a preview position/volume and never claim that audio is playing.
    progress: Entity<SliderState>,
    volume: Entity<SliderState>,
    position: f32,
    volume_value: f32,
    playing: bool,
    muted: bool,
    fullscreen: bool,
    _subscriptions: Vec<Subscription>,
}
impl AudioDemo {
    pub(super) fn for_product(pid: u32, cx: &mut App) -> Option<Entity<Self>> {
        let spec = specification(pid)?;
        Some(cx.new(|cx| {
            let progress = cx.new(|_| SliderState::new().max(100.).step(1.));
            let volume = cx.new(|_| SliderState::new().max(100.).step(1.).default_value(100.));
            let progress_subscription = cx.subscribe(
                &progress,
                |this: &mut AudioDemo, _, event: &SliderEvent, cx| {
                    if let SliderEvent::Change(value) = event {
                        this.position = value.end();
                        cx.notify();
                    }
                },
            );
            let volume_subscription = cx.subscribe(
                &volume,
                |this: &mut AudioDemo, _, event: &SliderEvent, cx| {
                    if let SliderEvent::Change(value) = event {
                        this.volume_value = value.end();
                        this.muted = value.end() <= 0.;
                        cx.notify();
                    }
                },
            );
            Self {
                spec,
                floating: spec.floating_default,
                playback_requested: false,
                progress,
                volume,
                position: 0.,
                volume_value: 100.,
                playing: false,
                muted: false,
                fullscreen: false,
                _subscriptions: vec![progress_subscription, volume_subscription],
            }
        }))
    }
}
impl Render for AudioDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let position = self.position;
        let volume = self.volume_value;
        v_flex()
            .id("audio-demo-page")
            .min_w(surface::css(self.spec.width))
            .w_full()
            .p(surface::css(20.))
            .text_color(cx.theme().foreground)
            .text_size(surface::css(14.))
            .child(div().text_center().child(t(&self.spec.title)))
            .child(self.player(cx, position, volume))
            .when(self.playback_requested, |view| {
                view.child(
                    div()
                        .id("audio-demo-playback-unavailable")
                        .w(surface::css(self.spec.width))
                        .mx_auto()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(15.))
                        .text_color(cx.theme().muted_foreground)
                        .child("音频演示播放服务未接入；下方仅保留本地控件外观"),
                )
            })
            .child(
                div()
                    .w(surface::css(self.spec.width))
                    .mx_auto()
                    .my(surface::css(20.))
                    .child(
                        Checkbox::new("audio-demo-floating")
                            .label(t(&self.spec.floating_label))
                            .checked(self.floating)
                            .on_click(cx.listener(|this, value, _, cx| {
                                this.floating = *value;
                                cx.notify();
                            })),
                    ),
            )
    }
}

impl AudioDemo {
    fn player(&self, cx: &mut Context<Self>, position: f32, volume: f32) -> AnyElement {
        let progress = self.progress.clone();
        let volume_state = self.volume.clone();
        let disabled = !self.playback_requested;
        let play_label = if self.playing {
            "暂停演示视频"
        } else {
            "播放演示视频"
        };
        div()
            .id("audio-demo-player")
            .test_support()
            .relative()
            .w(surface::css(self.spec.width))
            .h(surface::css(self.spec.height))
            .flex_shrink_0()
            .mt(surface::css(20.))
            .mx_auto()
            .tooltip(|window, cx| Tooltip::new("演示视频播放服务未接入").build(window, cx))
            .child(
                img(SharedString::from(self.spec.asset.clone()))
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .when(!self.playback_requested, |player| {
                player.child(
                    gpui_kit::base::Button::new("audio-demo-play")
                        .absolute()
                        .inset_0()
                        .p_0()
                        .accessibility_label("Play demo video")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.playback_requested = true;
                            this.playing = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .pb(surface::css(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .size(surface::css(100.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_3()
                                        .border_color(Colors::play_foreground())
                                        .rounded_full()
                                        .bg(Colors::play_background())
                                        .child(
                                            img("synapse/audio-demo-play.svg")
                                                .size(surface::css(48.))
                                                .ml(surface::css(2.)),
                                        ),
                                ),
                        ),
                )
            })
            .when(self.playback_requested, |player| {
                player.child(
                    v_flex()
                        .id("audio-demo-control-bar")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(surface::css(54.))
                        .px(surface::css(12.))
                        .py(surface::css(8.))
                        .gap(surface::css(4.))
                        .bg(rgba(0x000000cc))
                        .text_color(rgb(0xffffff))
                        .child(
                            Slider::new(&progress)
                                .w_full()
                                .h(surface::css(8.))
                                .disabled(disabled),
                        )
                        .child(
                            div()
                                .flex()
                                .h(surface::css(26.))
                                .items_center()
                                .gap(surface::css(8.))
                                .child(
                                    gpui_kit::base::Button::new("audio-demo-toggle")
                                        .size(surface::css(24.))
                                        .p_0()
                                        .accessibility_label(play_label)
                                        .disabled(true)
                                        .child(if self.playing { "Ⅱ" } else { "▶" }),
                                )
                                .child(div().text_size(surface::css(11.)).child(format!(
                                    "预览 {:03}% · 音量 {:03}%",
                                    position.round() as u32,
                                    volume.round() as u32
                                )))
                                .child(
                                    gpui_kit::base::Button::new("audio-demo-mute")
                                        .size(surface::css(24.))
                                        .p_0()
                                        .accessibility_label(if self.muted {
                                            "取消静音"
                                        } else {
                                            "静音"
                                        })
                                        .disabled(true)
                                        .child(if self.muted { "🔇" } else { "🔊" }),
                                )
                                .child(
                                    Slider::new(&volume_state)
                                        .w(surface::css(90.))
                                        .h(surface::css(8.))
                                        .disabled(disabled),
                                )
                                .child(div().ml_auto().text_size(surface::css(11.)).child(
                                    if self.fullscreen {
                                        "退出全屏"
                                    } else {
                                        "全屏"
                                    },
                                )),
                        ),
                )
            })
            .into_any_element()
    }
}
