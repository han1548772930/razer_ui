//! Current 1392/1442/3942 mounted demo and its profile-independent preference.
use super::audio_media::AudioMedia;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_i18n::t;
use razer_widgets::{surface, theme::AudioDemoColors as Colors};
use serde::Deserialize;
use std::{cell::Cell, rc::Rc, sync::OnceLock};

const FLOATING_WIDTH: f32 = 400.;
const FLOATING_HEIGHT: f32 = 225.;
const FLOATING_MARGIN: f32 = 20.;

fn clamp_floating(position: Point<f32>, viewport: Size<f32>) -> Point<f32> {
    point(
        position
            .x
            .min(viewport.width - FLOATING_WIDTH - FLOATING_MARGIN)
            .max(FLOATING_MARGIN),
        position
            .y
            .min(viewport.height - FLOATING_HEIGHT - FLOATING_MARGIN)
            .max(FLOATING_MARGIN),
    )
}

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
        .get_or_init(read_audio_demo_catalog)
        .iter()
        .find(|spec| spec.product_id == pid)
}
pub(super) fn supports(pid: u32) -> bool {
    specification(pid).is_some()
}

pub(super) struct AudioDemo {
    spec: &'static Spec,
    floating: bool,
    inline: bool,
    floating_open: bool,
    force_open: bool,
    position: Option<Point<f32>>,
    drag_offset: Option<Point<f32>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    media: Entity<AudioMedia>,
    _bounds_changed: Subscription,
}
impl AudioDemo {
    pub(super) fn for_product(pid: u32, window: &mut Window, cx: &mut App) -> Option<Entity<Self>> {
        let spec = specification(pid)?;
        Some(cx.new(|cx: &mut Context<Self>| {
            let media =
                AudioMedia::new(pid, spec.asset.clone(), spec.width, spec.height, window, cx);
            media.update(cx, |media, cx| {
                media.set_presentation(
                    match spec.object_fit {
                        PosterFit::Fill => ObjectFit::Fill,
                        PosterFit::Cover => ObjectFit::Cover,
                    },
                    spec.presentation.button_width,
                    spec.presentation.glyph_size,
                );
                media.preload(window, cx);
            });
            let bounds_changed = cx.observe_window_bounds(window, |this, window, cx| {
                if let Some(position) = this.position {
                    this.position = Some(clamp_floating(position, this.viewport(window)));
                }
                this.drag_offset = None;
                cx.notify();
            });
            Self {
                spec,
                floating: spec.floating_default,
                inline: false,
                floating_open: false,
                force_open: false,
                position: None,
                drag_offset: None,
                bounds: Rc::new(Cell::new(Bounds::default())),
                media,
                _bounds_changed: bounds_changed,
            }
        }))
    }
    pub(super) fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.floating_open = false;
        self.force_open = false;
        self.drag_offset = None;
        self.media.update(cx, |media, cx| media.stop(window, cx));
        cx.notify();
    }
    pub(super) fn is_floating(&self) -> bool {
        !self.inline && self.floating_open
    }
    pub(super) fn is_forced_open(&self) -> bool {
        self.force_open
    }
    pub(super) fn open_from_surround(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.force_open = true;
        self.floating_open = true;
        self.inline = false;
        self.position = None;
        self.media.update(cx, |media, cx| {
            media.leave_fullscreen(window, cx);
            media.set_size(FLOATING_WIDTH, FLOATING_HEIGHT, cx);
            media.play_from_offset(100, window, cx);
        });
        cx.notify();
    }

    pub(super) fn set_page(&mut self, inline: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.inline == inline {
            return;
        }
        let leaving_inline = self.inline && !inline;
        self.inline = inline;
        if inline {
            self.force_open = false;
        }
        self.drag_offset = None;
        self.position = None;
        let has_position = self.media.read(cx).has_position();
        let playing = self.media.read(cx).is_playing();
        self.floating_open = !inline
            && (self.force_open
                || self.floating && (has_position || self.spec.product_id != 1392 && playing));
        let show_inline = if self.spec.product_id == 1442 {
            playing
        } else {
            has_position
        };
        self.media.update(cx, |media, cx| {
            media.leave_fullscreen(window, cx);
            // The current 1442 inline cleanup explicitly clears the reducer
            // when floating is disabled; 1392 and 3942 preserve its time.
            if leaving_inline && !self.floating && self.spec.product_id == 1442 {
                media.stop(window, cx);
                return;
            }
            if inline {
                media.set_size(self.spec.width, self.spec.height, cx);
                media.remount_presentation(show_inline, window, cx);
                media.preload(window, cx);
            } else if self.floating_open {
                media.set_size(FLOATING_WIDTH, FLOATING_HEIGHT, cx);
                media.remount_presentation(true, window, cx);
            } else {
                media.pause(window, cx);
            }
        });
        cx.notify();
    }

    fn close_floating(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.floating_open = false;
        self.force_open = false;
        self.drag_offset = None;
        self.media.update(cx, |media, cx| {
            // Current 1442/3942 close handlers dispatch both time=0 and
            // isPlaying=false. The 1392 handler only closes the mounted player.
            if self.spec.product_id == 1392 {
                media.pause(window, cx);
            } else {
                media.stop(window, cx);
            }
        });
        cx.notify();
    }

    fn viewport(&self, window: &Window) -> Size<f32> {
        let scale = f32::from(window.rem_size()) / 16.;
        let bounds = self.bounds.get();
        size(
            f32::from(window.viewport_size().width) / scale,
            f32::from(window.viewport_size().height - bounds.origin.y) / scale,
        )
    }
    fn floating_player(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let viewport = self.viewport(window);
        let position = clamp_floating(
            self.position
                .unwrap_or(point(viewport.width, viewport.height)),
            viewport,
        );
        let bounds = self.bounds.clone();
        let dragging = self.drag_offset.is_some();
        div()
            .id("audio-demo-floating-parent")
            .absolute()
            .inset_0()
            .on_prepaint({
                let owner = cx.entity().downgrade();
                move |rect, window, cx| {
                    let previous = bounds.replace(rect);
                    if previous != rect {
                        let owner = owner.clone();
                        window.defer(cx, move |_, cx| {
                            let _ = owner.update(cx, |_, cx| cx.notify());
                        });
                    }
                }
            })
            .when(dragging, |parent| {
                parent
                    .occlude()
                    .cursor(CursorStyle::ClosedHand)
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                        if let Some(offset) = this.drag_offset {
                            let scale = f32::from(window.rem_size()) / 16.;
                            let origin = this.bounds.get().origin;
                            let position = point(
                                f32::from(event.position.x - origin.x) / scale - offset.x,
                                f32::from(event.position.y - origin.y) / scale - offset.y,
                            );
                            this.position = Some(clamp_floating(position, this.viewport(window)));
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.drag_offset = None;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.drag_offset = None;
                            cx.notify();
                        }),
                    )
                    .on_mouse_exit(cx.listener(|this, _, _, cx| {
                        this.drag_offset = None;
                        cx.notify();
                    }))
            })
            .child(
                div()
                    .absolute()
                    .when_some(self.position, |player, _| {
                        player
                            .left(surface::css(position.x))
                            .top(surface::css(position.y))
                    })
                    .when(self.position.is_none(), |player| {
                        player
                            .right(surface::css(FLOATING_MARGIN))
                            .bottom(surface::css(FLOATING_MARGIN))
                    })
                    .w(surface::css(FLOATING_WIDTH))
                    .h(surface::css(FLOATING_HEIGHT))
                    .child(self.media.clone())
                    .child(
                        razer_widgets::keyboard_navigation::Button::new("audio-demo-drag")
                            .group("audio-demo-drag")
                            .absolute()
                            .top_0()
                            .left_0()
                            .p(surface::css(5.))
                            .cursor(if dragging {
                                CursorStyle::ClosedHand
                            } else {
                                CursorStyle::OpenHand
                            })
                            .accessibility_label(t("REPOSITION"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    let scale = f32::from(window.rem_size()) / 16.;
                                    let origin = this.bounds.get().origin;
                                    this.position = Some(position);
                                    this.drag_offset = Some(point(
                                        f32::from(event.position.x - origin.x) / scale - position.x,
                                        f32::from(event.position.y - origin.y) / scale - position.y,
                                    ));
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(img("synapse/audio-demo-drag.svg"))
                            .child(
                                div()
                                    .absolute()
                                    .left(surface::css(0.))
                                    .top(surface::css(44.))
                                    .whitespace_nowrap()
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .px(surface::css(10.))
                                    .py(surface::css(8.))
                                    .text_size(surface::css(14.))
                                    .text_color(rgb(0xcccccc))
                                    .bg(rgb(0x111111))
                                    .invisible()
                                    .group_hover("audio-demo-drag", |style| style.visible())
                                    .child(t("REPOSITION")),
                            ),
                    )
                    .child(
                        razer_widgets::keyboard_navigation::Button::new("audio-demo-floating-close")
                            .absolute()
                            .right(surface::css(-26.))
                            .top(surface::css(-26.))
                            .p(surface::css(5.))
                            .accessibility_label("Close video")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_floating(window, cx)),
                            )
                            .child(img("synapse/audio-demo-close.svg")),
                    ),
            )
            .into_any_element()
    }
}
impl Render for AudioDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.inline {
            return if self.floating_open {
                self.floating_player(window, cx)
            } else {
                div().into_any_element()
            };
        }
        super::super::product_surface::body()
            .id("audio-demo-page")
            .flex()
            .flex_col()
            .text_color(cx.theme().foreground)
            .text_size(surface::css(14.))
            .child(div().text_center().child(t(&self.spec.title)))
            .child(
                div()
                    .id("audio-demo-player")
                    .relative()
                    .w(surface::css(self.spec.width))
                    .h(surface::css(self.spec.height))
                    .flex_shrink_0()
                    .mt(surface::css(20.))
                    .mx_auto()
                    .child(self.media.clone()),
            )
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
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_floating;
    use gpui_kit::{point, size};
    #[test]
    fn floating_position_keeps_source_twenty_pixel_margin_after_drag_and_resize() {
        assert_eq!(
            clamp_floating(point(2000., 1000.), size(1200., 700.)),
            point(780., 455.)
        );
        assert_eq!(
            clamp_floating(point(-50., -60.), size(1200., 700.)),
            point(20., 20.)
        );
        let shrunk = clamp_floating(point(780., 455.), size(600., 400.));
        assert_eq!(shrunk, point(180., 155.));
        assert_eq!(clamp_floating(shrunk, size(1200., 700.)), shrunk);
        assert_eq!(
            clamp_floating(point(0., 0.), size(300., 200.)),
            point(20., 20.)
        );
    }
}

// Generated catalog reader: razer-pages/audio_demo_data.
fn read_audio_demo_catalog() -> Vec<Spec> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Spec>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Spec> {
        let value = match r.tag()? {
            0 => Spec {
                product_id: r.child(decode_2)?,
                asset: r.child(decode_3)?,
                width: r.child(decode_4)?,
                height: r.child(decode_4)?,
                presentation: r.child(decode_5)?,
                object_fit: r.child(decode_6)?,
                title: r.child(decode_3)?,
                floating_label: r.child(decode_3)?,
                floating_default: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
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
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Presentation> {
        let value = match r.tag()? {
            0 => Presentation {
                button_width: r.child(decode_4)?,
                glyph_size: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PosterFit> {
        let value = match r.tag()? {
            0 => PosterFit::Fill,
            1 => PosterFit::Cover,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_AUDIO_DEMO_DATA,
        0,
        "baa3cb69adbebb9f13a552fc8209e5b128f85ad6d6d87a153a587fad725ccdc5",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod audio_demo_data_catalog_tests {
    #[test]
    fn packaged_catalog_audio_demo_data_decodes() {
        let _ = super::read_audio_demo_catalog();
    }
}
// End generated catalog reader: razer-pages/audio_demo_data.
