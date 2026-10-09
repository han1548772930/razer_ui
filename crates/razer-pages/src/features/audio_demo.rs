//! Current 1392/1442/3942 demo entry surface. Playback needs an audible native
//! player; a silent tutorial animation cannot implement this audio demo.
use gpui_kit::component::{
    ActiveTheme,
    slider::{Slider, SliderState},
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_i18n::t;
use razer_widgets::surface;
use razer_widgets::theme::AudioDemoColors as Colors;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    asset: String,
    width: f32,
    height: f32,
    presentation: Presentation,
    object_fit: PosterFit,
    title: String,
    floating_label: String,
    floating_default: bool,
}
#[derive(Deserialize)]
struct Presentation {
    button_width: f32,
    glyph_size: f32,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PosterFit {
    Fill,
    Cover,
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
    // Keep the poster action visible and report that boundary after a click.
    playback_requested: bool,
    // Source disables default controls and mounts only play/progress/fullscreen.
    // With no media transport the progress control is disabled, not a fake clock.
    progress: Entity<SliderState>,
}
impl AudioDemo {
    pub(super) fn for_product(pid: u32, cx: &mut App) -> Option<Entity<Self>> {
        let spec = specification(pid)?;
        Some(cx.new(|cx| {
            let progress = cx.new(|_| SliderState::new().max(100.).step(1.));
            Self {
                spec,
                floating: spec.floating_default,
                playback_requested: false,
                progress,
            }
        }))
    }
}
impl Render for AudioDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        super::super::product_surface::body()
            .id("audio-demo-page")
            .flex()
            .flex_col()
            .text_color(cx.theme().foreground)
            .text_size(surface::css(14.))
            .child(div().text_center().child(t(&self.spec.title)))
            .child(self.player(cx))
            .when(self.playback_requested, |view| {
                view.child(
                    div()
                        .id("audio-demo-playback-unavailable")
                        .w(surface::css(self.spec.width))
                        .mx_auto()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(15.))
                        .text_color(cx.theme().muted_foreground)
                        .child("音频演示播放服务未接入，播放、进度与全屏控件暂不可用"),
                )
            })
            .child(
                div()
                    .w(surface::css(self.spec.width))
                    .mx_auto()
                    .my(surface::css(20.))
                    .child(
                        surface::check_item_with_style(
                            "audio-demo-floating",
                            t(&self.spec.floating_label),
                            self.floating,
                            false,
                            surface::CheckItemStyle {
                                unchecked_background: Colors::unchecked_background(),
                                tick_bottom_origin: (0.8, 10.2),
                            },
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.floating = !this.floating;
                            cx.notify();
                        })),
                    ),
            )
    }
}

impl AudioDemo {
    fn player(&self, cx: &mut Context<Self>) -> AnyElement {
        let progress = self.progress.clone();
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
                    .object_fit(match self.spec.object_fit {
                        PosterFit::Cover => ObjectFit::Cover,
                        PosterFit::Fill => ObjectFit::Fill,
                    }),
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
                    div()
                        .id("audio-demo-control-bar")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(surface::css(40.))
                        .p_0()
                        .rounded(surface::css(5.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .bg(Colors::control_background())
                        .text_color(Colors::play_foreground())
                        .child(
                            Slider::new(&progress)
                                .absolute()
                                .left_0()
                                .bottom(surface::css(16.))
                                .w_full()
                                .h(surface::css(8.))
                                .disabled(true),
                        )
                        .child(
                            gpui_kit::base::Button::new("audio-demo-toggle")
                                .w(surface::css(self.spec.presentation.button_width))
                                .h_full()
                                .p_0()
                                .accessibility_label("播放演示视频")
                                .disabled(true)
                                .child(
                                    img("synapse/audio-demo-control-play.svg")
                                        .size(surface::css(self.spec.presentation.glyph_size)),
                                ),
                        )
                        .child(
                            gpui_kit::base::Button::new("audio-demo-fullscreen")
                                .w(surface::css(self.spec.presentation.button_width))
                                .h_full()
                                .p_0()
                                .accessibility_label("全屏")
                                .disabled(true)
                                .child(
                                    img("synapse/audio-demo-control-fullscreen.svg")
                                        .size(surface::css(self.spec.presentation.glyph_size)),
                                ),
                        ),
                )
            })
            .into_any_element()
    }
}
