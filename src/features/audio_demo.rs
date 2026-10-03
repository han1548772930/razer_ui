//! Current 1392/1442/3942 demo entry surface. Playback needs an audible native
//! player; a silent tutorial animation cannot implement this audio demo.
use crate::{
    i18n::t,
    ui::{surface, theme::AudioDemoColors as Colors},
};
use gpui_kit::component::{ActiveTheme, checkbox::Checkbox, tooltip::Tooltip, v_flex};
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
}
impl AudioDemo {
    pub(super) fn for_product(pid: u32, cx: &mut App) -> Option<Entity<Self>> {
        let spec = specification(pid)?;
        Some(cx.new(|_| Self {
            spec,
            floating: spec.floating_default,
        }))
    }
}
impl Render for AudioDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("audio-demo-page")
            .min_w(surface::css(self.spec.width))
            .w_full()
            .p(surface::css(20.))
            .text_color(cx.theme().foreground)
            .text_size(surface::css(14.))
            .child(div().text_center().child(t(&self.spec.title)))
            .child(
                div()
                    .id("audio-demo-player")
                    .test_support()
                    .relative()
                    .w(surface::css(self.spec.width))
                    .h(surface::css(self.spec.height))
                    .flex_shrink_0()
                    .mt(surface::css(20.))
                    .mx_auto()
                    .tooltip(|window, cx| Tooltip::new("演示视频播放暂不可用").build(window, cx))
                    .child(
                        gpui_kit::base::Button::new("audio-demo-play")
                            .size_full()
                            .p_0()
                            .disabled(true)
                            // This is the exact aria-label on the current source
                            // poster button; PLAY is not a source translation key.
                            .accessibility_label("Play demo video")
                            .child(
                                img(SharedString::from(self.spec.asset.clone()))
                                    .size_full()
                                    .object_fit(ObjectFit::Fill),
                            )
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
                    ),
            )
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
