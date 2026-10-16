//! Current tm renderer's product resources, wheel/SmartSwitch hover layers and
//! SmartSwitch canvas and the source-mounted configurable wheel button.
use super::sound::{Product, message, product};
use super::*;
use razer_widgets::theme::AudioEnhancementColors as Colors;

impl AudioProductWorkspace {
    fn sound_wheel_canvas(&self, row: &Product) -> AnyElement {
        // Current six tm.drawLines/drawLine: one right-hand button at (205,165).
        let config = &row.configs.renderer_geometry;
        let (x, y, length, dot, ring) = (
            config.x_right as f32,
            config.y_start as f32,
            config.line_length as f32,
            config.dot_radius as f32,
            config.circle_radius as f32,
        );
        let selected = self.sound.mapping.open || self.sound.customizer_hover == Some(0);
        let color = if selected {
            Colors::selected()
        } else {
            Colors::border()
        };
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / 500.;
                let pt = |x: f32, y: f32| {
                    point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                };
                let endpoint = pt(205. + 3. * dot, 165. + dot);
                let mut line = PathBuilder::stroke(px(scale));
                line.move_to(pt(x, y));
                line.line_to(pt(x - length, y));
                line.line_to(endpoint);
                if let Ok(path) = line.build() {
                    window.paint_path(path, color);
                }
                for center in [pt(x, y), endpoint] {
                    window.paint_quad(quad(
                        Bounds::new(
                            center - point(px(dot * scale), px(dot * scale)),
                            size(px(2. * dot * scale), px(2. * dot * scale)),
                        ),
                        px(dot * scale),
                        color,
                        px(0.),
                        color,
                        BorderStyle::Solid,
                    ));
                }
                if selected {
                    window.paint_quad(quad(
                        Bounds::new(
                            endpoint - point(px(ring * scale), px(ring * scale)),
                            size(px(2. * ring * scale), px(2. * ring * scale)),
                        ),
                        px(ring * scale),
                        rgba(0),
                        px(scale),
                        color,
                        BorderStyle::Solid,
                    ));
                }
            },
        )
        .absolute()
        .left(surface::css(213.))
        .bottom(surface::css(-23.))
        .w(surface::css(500.))
        .h(surface::css(config.cvs_h as f32))
        .into_any_element()
    }
    pub(super) fn refresh_sound_images(&mut self) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let edition = self.draft["device"]["editionId"].as_u64().unwrap_or(0);
        if self.sound.image_edition == Some(edition) {
            return;
        }
        self.sound.image_edition = Some(edition);
        if let Some(images) = row.media.editions.get(&edition.to_string()) {
            // Source Promise.all installs all three together. Its rejection
            // retains previous successful resources, never another edition.
            if ["prd", "wheel-cutout", "smart-switch-button"]
                .iter()
                .all(|key| images.contains_key(*key))
            {
                self.sound.image_assets = Some(images.clone());
            }
        }
    }
    fn sound_smart_switch_canvas(&self, row: &Product) -> AnyElement {
        let config = &row.configs.renderer_geometry;
        let height = config.cvs_h as f32;
        let x = config.x_right as f32;
        let length = config.line_length as f32;
        let dot = config.dot_radius as f32;
        let ring = config.circle_radius as f32;
        let hovered = self.sound.customizer_hover == Some(1);
        let endpoint = if hovered { (220., 145.) } else { (230., 138.) };
        let color = (if hovered {
            &config.active_color
        } else {
            &config.dot_color
        })
        .trim_start_matches('#');
        let color = rgb(u32::from_str_radix(color, 16).expect("audited renderer hex"));
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / 500.;
                let pt = |x: f32, y: f32| {
                    point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                };
                let mut path = PathBuilder::stroke(px(scale));
                path.move_to(pt(x, 120.));
                path.line_to(pt(x - length, 120.));
                path.line_to(pt(endpoint.0, endpoint.1));
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
                for center in [pt(x, 120.), pt(endpoint.0, endpoint.1)] {
                    window.paint_quad(quad(
                        Bounds::new(
                            center - point(px(dot * scale), px(dot * scale)),
                            size(px(dot * 2. * scale), px(dot * 2. * scale)),
                        ),
                        px(dot * scale),
                        color,
                        px(0.),
                        color,
                        BorderStyle::Solid,
                    ));
                }
                if hovered {
                    let center = pt(endpoint.0, endpoint.1);
                    window.paint_quad(quad(
                        Bounds::new(
                            center - point(px(ring * scale), px(ring * scale)),
                            size(px(ring * 2. * scale), px(ring * 2. * scale)),
                        ),
                        px(ring * scale),
                        rgba(0x00000000),
                        px(scale),
                        color,
                        BorderStyle::Solid,
                    ));
                }
            },
        )
        .absolute()
        .left(surface::css(213.))
        .bottom(surface::css(-23.))
        .w(surface::css(500.))
        .h(surface::css(height))
        .into_any_element()
    }
    pub(super) fn current_headset_image(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        let assets = self.sound.image_assets.as_ref();
        let button = self.sound_mapping_button(row);
        let mapping_label = message(row, button["assignmentValue"].as_str().unwrap_or_default());
        let mapped = super::sound::js_truthy(&button["isMapped"]);
        let disabled_assignment = button["assignment"] == "DISABLE";
        let anchor = self.sound.mapping.anchor.clone();
        let block = div()
            .relative()
            .w(surface::css(770.))
            .h(surface::css(250.))
            .mx_auto()
            .when_some(
                assets.and_then(|images| images.get("prd").map(String::as_str)),
                |body, image| {
                    body.child(
                        img(image.to_string())
                            .absolute()
                            .left(surface::css(235.))
                            .w(surface::css(325.))
                            .h(surface::css(250.))
                            .object_fit(ObjectFit::Contain),
                    )
                },
            )
            .when(
                self.sound.customizer_hover == Some(0) || self.sound.mapping.open,
                |body| {
                    body.when_some(
                        assets.and_then(|images| images.get("wheel-cutout").map(String::as_str)),
                        |body, image| {
                            body.child(
                                img(image.to_string())
                                    .absolute()
                                    .left(relative(0.306))
                                    .top_0()
                                    .w(surface::css(325.))
                                    .h(surface::css(250.)),
                            )
                        },
                    )
                },
            )
            .when(self.sound.customizer_hover == Some(1), |body| {
                body.when_some(
                    assets.and_then(|images| images.get("smart-switch-button").map(String::as_str)),
                    |body, image| {
                        body.child(
                            img(image.to_string())
                                .absolute()
                                .left(relative(0.306))
                                .top_0()
                                .w(surface::css(325.))
                                .h(surface::css(250.)),
                        )
                    },
                )
            })
            .child(self.sound_wheel_canvas(row))
            .child(self.sound_smart_switch_canvas(row))
            .child(
                gpui_kit::base::Button::new("headset-wheel-mapping")
                    .absolute()
                    .left(surface::css(496.))
                    .bottom(surface::css(12.))
                    .h(surface::css(30.))
                    .px(surface::css(10.))
                    .pt(surface::css(5.))
                    .rounded(surface::css(3.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(if disabled_assignment {
                        Colors::disabled_assignment()
                    } else if mapped {
                        Colors::selected()
                    } else {
                        Colors::text()
                    })
                    .when(self.sound.customizer_hover == Some(0), |button| {
                        button.bg(Colors::mapping_hover())
                    })
                    .when(self.sound.mapping.open, |button| {
                        button.bg(Colors::background())
                    })
                    .hover(|style| style.bg(Colors::mapping_hover()))
                    .active(|style| style.bg(Colors::background()))
                    .child(
                        canvas(move |bounds, _, _| anchor.set(bounds), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .child(mapping_label)
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.sound.customizer_hover = hovered.then_some(0);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_sound_mapping(window, cx)),
                    ),
            )
            .when_some(self.source_owner.clone(), |body, owner| {
                body.child(
                    gpui_kit::base::Button::new("headset-view-smart-switch")
                        .absolute()
                        .left(surface::css(495.))
                        .bottom(surface::css(92.))
                        .px(surface::css(10.))
                        .py(surface::css(7.))
                        .rounded(surface::css(3.))
                        .text_color(Colors::secondary())
                        .hover(|style| style.bg(rgb(0x383838)).text_color(Colors::text()))
                        .child(
                            h_flex()
                                .gap(surface::css(5.))
                                .child(
                                    img(row
                                        .media
                                        .css
                                        .get("icon-info")
                                        .cloned()
                                        .unwrap_or_default())
                                    .size(surface::css(16.)),
                                )
                                .child(message(row, "VIEW_SMART_SWITCH_SHORTCUTS")),
                        )
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            this.sound.customizer_hover = hovered.then_some(1);
                            cx.notify();
                        }))
                        .on_click(move |_, _, cx| {
                            owner.update(cx, |owner, cx| {
                                owner.show(
                                    super::super::source_help_owner::HelpOwnerOverlay::SmartSwitch,
                                    cx,
                                )
                            })
                        }),
                )
            })
            .child(
                div()
                    .id("current-sound-wheel-cover")
                    .absolute()
                    .right(surface::css(297.))
                    .bottom(surface::css(12.))
                    .size(surface::css(100.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.sound.customizer_hover = hovered.then_some(0);
                        cx.notify();
                    })),
            );
        div()
            .h(surface::css(260.))
            .w_full()
            .child(block)
            .into_any_element()
    }
}
