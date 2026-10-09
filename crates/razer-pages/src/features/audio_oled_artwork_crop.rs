//! 1383 rh/nh local crop surface. Never reports the vendor worker's completion.
use super::*;
use media_decode::{CROP_HEIGHT, CROP_TOP, CropCanvas, HEIGHT, WIDTH};
use std::sync::Arc;

pub(super) struct CropState {
    index: usize,
    source: String,
    image: Arc<RenderImage>,
    canvas: CropCanvas,
    initial_canvas: CropCanvas,
    drag_position: Option<Point<Pixels>>,
    zoom_level: u8,
    slider: Entity<SliderState>,
    _subscription: Subscription,
}

impl ArtworkEditor {
    pub(super) fn import_custom(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.import_generation = self.import_generation.wrapping_add(1);
        let generation = self.import_generation;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        let mode = self.mode;
        let renderer = cx.svg_renderer();
        let parent = cx.weak_entity();
        cx.spawn_in(window, async move |_, cx| {
            let path = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let Some(path) = path else {
                return;
            };
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !(mode == 0 && extension == "gif"
                || mode == 1 && matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "bmp"))
            {
                return;
            }
            let Ok(imported) = cx
                .background_spawn(
                    async move { media_decode::load_preset(&path, &extension, renderer) },
                )
                .await
            else {
                return;
            };
            let _ = parent.update_in(cx, |this, window, cx| {
                if this.import_generation != generation
                    || !this.dialog.is_open()
                    || this.draft["list"][index]["enabled"] != true
                {
                    return;
                }
                let (source, _, image) = imported.into_parts();
                let dimensions = image.size(0);
                let canvas = CropCanvas::new(dimensions.width.0 as f32, dimensions.height.0 as f32);
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(1.)
                        .max(10.)
                        .step(1.)
                        .default_value(1.)
                });
                let subscription = cx.subscribe_in(&slider, window, |this, _, event, _, cx| {
                    let SliderEvent::Change(value) = event else {
                        return;
                    };
                    let Some(crop) = &mut this.crop else {
                        return;
                    };
                    let next = value.start().round().clamp(1., 10.) as u8;
                    if next != crop.zoom_level {
                        let delta = if next < crop.zoom_level {
                            (next as f32 - 11.) / 10.
                        } else {
                            next as f32 / 10.
                        };
                        crop.canvas.zoom_by(delta);
                        crop.zoom_level = next;
                        cx.notify();
                    }
                });
                this.crop = Some(CropState {
                    index,
                    source,
                    image,
                    canvas,
                    initial_canvas: canvas,
                    drag_position: None,
                    zoom_level: 1,
                    slider,
                    _subscription: subscription,
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn cancel_crop(&mut self, cx: &mut Context<Self>) {
        self.crop = None;
        cx.notify();
    }

    pub(super) fn apply_crop(&mut self, cx: &mut Context<Self>) {
        let Some(crop) = self.crop.take() else {
            return;
        };
        let item = &mut self.draft["list"][crop.index];
        if item["enabled"] == true {
            item["src"] = json!(crop.source);
            item["custom"] = json!(true);
            // The local draft retains original decoded data + crop placement.
            // This is not the GIF worker's encoded gifData/size response.
            item["local_crop"] = json!(crop.canvas);
            item["size"] = json!(0);
        }
        cx.notify();
    }

    fn crop_zoom(&mut self, direction: i8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(crop) = &mut self.crop else {
            return;
        };
        let next = (crop.zoom_level as i8 + direction).clamp(1, 10) as u8;
        if next != crop.zoom_level {
            crop.zoom_level = next;
            crop.canvas.zoom_by(if direction > 0 { 0.1 } else { -0.1 });
            crop.slider
                .update(cx, |slider, cx| slider.set_value(next as f32, window, cx));
            cx.notify();
        }
    }

    fn reset_crop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(crop) = &mut self.crop else {
            return;
        };
        crop.canvas = crop.initial_canvas;
        crop.zoom_level = 1;
        crop.slider
            .update(cx, |slider, cx| slider.set_value(1., window, cx));
        cx.notify();
    }

    pub(super) fn render_crop(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let crop = self.crop.as_ref().expect("open crop");
        let canvas = crop.canvas;
        let owner = cx.weak_entity();
        let view = div()
            .id("1383-artwork-crop-canvas")
            .relative()
            .w(surface::css(WIDTH))
            .h(surface::css(HEIGHT))
            .cursor_move()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if let Some(crop) = &mut this.crop {
                        crop.drag_position = Some(event.position);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if let Some(crop) = &mut this.crop {
                    if let Some(previous) = crop.drag_position {
                        if event.dragging() {
                            let scale = f32::from(surface::css(1.).to_pixels(window.rem_size()));
                            crop.canvas.move_by(
                                f32::from(event.position.x - previous.x) / scale,
                                f32::from(event.position.y - previous.y) / scale,
                            );
                            crop.drag_position = Some(event.position);
                        } else {
                            crop.drag_position = None;
                        }
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(crop) = &mut this.crop {
                        crop.drag_position = None;
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(crop) = &mut this.crop {
                        crop.drag_position = None;
                    }
                }),
            )
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .child(
                        img(crop.image.clone())
                            .absolute()
                            .left(surface::css(canvas.left()))
                            .top(surface::css(canvas.top()))
                            .w(surface::css(canvas.width()))
                            .h(surface::css(canvas.height()))
                            .object_fit(ObjectFit::Fill)
                            .grayscale(true),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(surface::css(CROP_TOP))
                            .bg(ArtworkColors::black().opacity(0.5)),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .h(surface::css(CROP_TOP))
                            .bg(ArtworkColors::black().opacity(0.5)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(surface::css(CROP_TOP))
                    .w(surface::css(WIDTH))
                    .h(surface::css(CROP_HEIGHT))
                    .border_1()
                    .border_dashed()
                    .border_color(ArtworkColors::selected())
                    .bg(ArtworkColors::white().opacity(0.1)),
            )
            .child(
                gpui_kit::canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        let Some(entity) = owner.upgrade() else {
                            return;
                        };
                        if entity
                            .read(cx)
                            .crop
                            .as_ref()
                            .is_none_or(|crop| crop.drag_position.is_none())
                        {
                            return;
                        }
                        let motion = entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if !phase.capture() {
                                return;
                            }
                            motion.update(cx, |this, cx| {
                                if let Some(crop) = &mut this.crop {
                                    if let Some(previous) = crop.drag_position {
                                        if event.dragging() {
                                            let scale = f32::from(
                                                surface::css(1.).to_pixels(window.rem_size()),
                                            );
                                            crop.canvas.move_by(
                                                f32::from(event.position.x - previous.x) / scale,
                                                f32::from(event.position.y - previous.y) / scale,
                                            );
                                            crop.drag_position = Some(event.position);
                                        } else {
                                            crop.drag_position = None;
                                        }
                                        cx.notify();
                                    }
                                }
                            });
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase.capture() && event.button == MouseButton::Left {
                                entity.update(cx, |this, cx| {
                                    if let Some(crop) = &mut this.crop {
                                        crop.drag_position = None;
                                        cx.notify();
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
        let controls = h_flex()
            .justify_center()
            .child(
                BaseButton::new("1383-artwork-zoom-out")
                    .p_0()
                    .child(img("synapse/audio-oled-artwork-zoom-out.svg").size(surface::css(24.)))
                    .on_click(cx.listener(|this, _, window, cx| this.crop_zoom(-1, window, cx))),
            )
            .child(
                div()
                    .w(surface::css(150.))
                    .mt(surface::css(4.))
                    .mx(surface::css(10.))
                    .child(razer_widgets::source_slider::SourceSlider::new(
                        &crop.slider,
                        (crop.zoom_level as f32 - 1.) / 9.,
                    )),
            )
            .child(
                BaseButton::new("1383-artwork-zoom-in")
                    .p_0()
                    .child(img("synapse/audio-oled-artwork-zoom-in.svg").size(surface::css(24.)))
                    .on_click(cx.listener(|this, _, window, cx| this.crop_zoom(1, window, cx))),
            );
        v_flex()
            .id("1383-artwork-crop-body")
            .flex_1()
            .min_h_0()
            .pt(surface::css(20.))
            .px(surface::css(25.))
            .pb(surface::css(97.))
            .scrollable_y()
            .child(
                div()
                    .text_center()
                    .child(artwork_label("CHf").to_uppercase()),
            )
            .child(" ")
            .child(h_flex().justify_center().child(view))
            .child(" ")
            .child(controls)
            .child(
                h_flex().justify_center().child(
                    dialog::action("1383-artwork-reset-crop", t("RESET"), false)
                        .on_click(cx.listener(|this, _, window, cx| this.reset_crop(window, cx))),
                ),
            )
            .child(" ")
            .when(self.mode == 0, |view| {
                view.child(
                    div()
                        .text_center()
                        .text_color(ArtworkColors::muted())
                        .child(artwork_label("SKm")),
                )
            })
            .child(" ")
            .into_any_element()
    }
}
