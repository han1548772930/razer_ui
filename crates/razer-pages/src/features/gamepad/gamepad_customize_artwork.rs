//! Mounted current controller Customize artwork, callouts and column ordering.
//! DeviceInfo and ProductImage own numeric-layout/prd and edition retry rules.
//! The workspace keeps hover and mapping selection; artwork never writes state.
use super::*;
use razer_widgets::theme::GamepadCustomizeColors as Colors;

#[derive(Deserialize)]
struct Atlas {
    products: Vec<Product>,
}

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    geometry: Geometry,
    lines: Vec<Callout>,
    left_buttons: Vec<String>,
    right_buttons: Vec<String>,
    images: Vec<Image>,
    mkeys: Vec<Image>,
    mkeys_mode: String,
    normalize_to_prd: bool,
    two_canvas_layers: bool,
}

#[derive(Deserialize)]
struct Geometry {
    wrapper_width: f32,
    wrapper_max_width: f32,
    height: f32,
    image_left: f32,
    image_height: f32,
    canvas_left: f32,
    canvas_width: f32,
    mkeys_left: f32,
    mkeys_top: f32,
    mkeys_width: f32,
    mkeys_height: f32,
    left_column_right: f32,
    right_column_left: f32,
    top_buttons_y: f32,
    left_bottom_y: f32,
    right_bottom_y: f32,
    button_height: f32,
    button_gap: f32,
    column_padding: f32,
    button_padding: f32,
    dot_radius: f32,
    left_line_x: f32,
    right_line_x: f32,
}

#[derive(Clone, Deserialize)]
struct Callout {
    input_id: String,
    side: String,
    start_y: f32,
    x: f32,
    y: f32,
    bend: f32,
    layer_behavior: String,
}

#[derive(Deserialize)]
struct Image {
    edition_id: u32,
    layout_id: Option<u32>,
    #[serde(default = "one")]
    resolution: u32,
    asset: String,
    width: f32,
    height: f32,
}

fn one() -> u32 {
    1
}

fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(|| crate::data::gamepad_artwork_current_data!())
}

fn product(pid: u32) -> &'static Product {
    atlas()
        .products
        .iter()
        .find(|product| product.product_id == pid)
        .expect("audited controller Customize product")
}

impl Product {
    fn image(&self, edition: u32, layout: u32, scale: f32) -> Option<&Image> {
        let layout = (!self.normalize_to_prd).then_some(layout);
        let density = if scale > 1. { 3 } else { 1 };
        let candidates = |edition| {
            self.images
                .iter()
                .filter(move |image| image.edition_id == edition && image.layout_id == layout)
        };
        candidates(edition)
            .find(|image| image.resolution == density)
            .or_else(|| candidates(0).find(|image| image.resolution == density))
            .or_else(|| candidates(edition).find(|image| image.resolution == 1))
            .or_else(|| candidates(0).find(|image| image.resolution == 1))
    }

    fn mkeys(&self, edition: u32) -> Option<&Image> {
        let find = |edition| self.mkeys.iter().find(|image| image.edition_id == edition);
        match self.mkeys_mode.as_str() {
            "zero_or_nonzero" => find(if edition == 0 { 0 } else { 128 }),
            "edition_zero" => find(0),
            "requested_or_zero" => find(edition).or_else(|| find(0)),
            _ => None,
        }
    }
}

impl GamepadProductWorkspace {
    pub(super) fn thumbstick_product_image(&self, window: &Window) -> AnyElement {
        let product = product(self.spec.product_id);
        let Some(image) = product.image(self.edition_id, self.layout_id, window.scale_factor())
        else {
            return div()
                .w(surface::css(372.))
                .h(surface::css(251.))
                .into_any_element();
        };
        img(SharedString::from(image.asset.clone()))
            .w(surface::css(372.))
            .h(surface::css(372. * image.height / image.width))
            .object_fit(ObjectFit::Contain)
            .into_any_element()
    }
}

fn above(callout: &Callout, active: bool) -> bool {
    callout.layer_behavior == "alwaysOnTop" || (callout.layer_behavior == "hoverOnly" && active)
}

fn line_layer(
    product: &'static Product,
    controls: &[Value],
    selected: Option<&str>,
    hovered: Option<&str>,
    upper: bool,
) -> AnyElement {
    let lines: Vec<_> = product
        .lines
        .iter()
        .filter_map(|callout| {
            let button = controls
                .iter()
                .find(|button| button["inputID"].as_str() == Some(&callout.input_id))?;
            let active = selected == Some(callout.input_id.as_str())
                || hovered == Some(callout.input_id.as_str());
            if product.two_canvas_layers && above(callout, active) != upper {
                return None;
            }
            let color = if button["isEnabled"] == false || button["disabled"] == true {
                Colors::disabled_line()
            } else if active {
                Colors::accent()
            } else {
                Colors::line()
            };
            Some((callout.clone(), color, active))
        })
        .collect();
    let geometry = &product.geometry;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = bounds.size.width / geometry.canvas_width;
            let at = |x: f32, y: f32| bounds.origin + point(scale * x, scale * y);
            for (callout, color, active) in &lines {
                let left = callout.side == "left";
                let start_x = if left {
                    geometry.left_line_x
                } else {
                    geometry.right_line_x
                };
                let end_x = callout.x + 3. * geometry.dot_radius;
                let end_y = callout.y + geometry.dot_radius;
                let mut line = PathBuilder::stroke(scale);
                line.move_to(at(start_x, callout.start_y));
                line.line_to(at(
                    start_x + if left { callout.bend } else { -callout.bend },
                    callout.start_y,
                ));
                line.line_to(at(end_x, end_y));
                if let Ok(path) = line.build() {
                    window.paint_path(path, *color);
                }
                for (x, y, stroke) in [(start_x, callout.start_y, false), (end_x, end_y, false)]
                    .into_iter()
                    .chain(active.then_some((end_x, end_y, true)))
                {
                    let radius = geometry.dot_radius;
                    let mut dot = if stroke {
                        PathBuilder::stroke(scale)
                    } else {
                        PathBuilder::fill()
                    };
                    dot.move_to(at(x + radius, y));
                    dot.arc_to(
                        point(scale * radius, scale * radius),
                        px(0.),
                        false,
                        true,
                        at(x - radius, y),
                    );
                    dot.arc_to(
                        point(scale * radius, scale * radius),
                        px(0.),
                        false,
                        true,
                        at(x + radius, y),
                    );
                    dot.close();
                    if let Ok(path) = dot.build() {
                        window.paint_path(path, *color);
                    }
                }
            }
        },
    )
    .absolute()
    .left(surface::css(geometry.canvas_left))
    .top_0()
    .w(surface::css(geometry.canvas_width))
    .h(surface::css(geometry.height))
    .into_any_element()
}

impl GamepadProductWorkspace {
    pub(super) fn customize_artwork(&self, cx: &Context<Self>) -> AnyElement {
        let source = product(self.spec.product_id);
        let geometry = &source.geometry;
        let mut artwork = div()
            .relative()
            .w(surface::css(geometry.wrapper_width))
            .h(surface::css(geometry.height))
            .mx_auto();
        if source.two_canvas_layers {
            artwork = artwork.child(line_layer(
                source,
                &self.spec.button_controls,
                self.selected_button.as_deref(),
                self.customize_hover.as_deref(),
                false,
            ));
        }
        // RenderOnce receives the actual display scale; image selection never
        // normalizes the observed layout beyond this product's source category.
        artwork = artwork.child(DeviceImage {
            product: source,
            edition: self.edition_id,
            layout: self.layout_id,
        });
        if let Some(image) = source.mkeys(self.edition_id) {
            artwork = artwork.child(
                img(image.asset.clone())
                    .absolute()
                    .left(surface::css(geometry.mkeys_left))
                    .top(surface::css(geometry.mkeys_top))
                    .w(surface::css(geometry.mkeys_width))
                    .h(surface::css(geometry.mkeys_height)),
            );
        }
        artwork = artwork
            .child(self.customize_button_column(source, true, cx))
            .child(self.customize_button_column(source, false, cx))
            .child(line_layer(
                source,
                &self.spec.button_controls,
                self.selected_button.as_deref(),
                self.customize_hover.as_deref(),
                true,
            ));
        div()
            .relative()
            .w_full()
            .min_w(surface::css(geometry.wrapper_width))
            .max_w(surface::css(geometry.wrapper_max_width))
            .h(surface::css(geometry.height))
            .mx_auto()
            .flex_shrink_0()
            .child(surface::dot_background(cx))
            .child(artwork)
            .into_any_element()
    }

    fn customize_button_column(
        &self,
        source: &'static Product,
        left: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let geometry = &source.geometry;
        let ids = if left {
            &source.left_buttons
        } else {
            &source.right_buttons
        };
        let mut column = div()
            .absolute()
            .top_0()
            .h_full()
            .max_w(surface::css(235.))
            .when(left, |column| {
                column
                    .right(surface::css(geometry.left_column_right))
                    .pr(surface::css(geometry.column_padding))
            })
            .when(!left, |column| {
                column
                    .left(surface::css(geometry.right_column_left))
                    .pl(surface::css(geometry.column_padding))
            });
        for (index, input) in ids.iter().enumerate() {
            let Some(button) = self
                .spec
                .button_controls
                .iter()
                .find(|button| button["inputID"].as_str() == Some(input))
            else {
                continue;
            };
            let top = if index == 0 {
                geometry.top_buttons_y
            } else {
                (if left {
                    geometry.left_bottom_y
                } else {
                    geometry.right_bottom_y
                }) + (index - 1) as f32 * (geometry.button_height + geometry.button_gap)
            };
            let current = self.draft["profile"]["mappings"]
                .as_array()
                .and_then(|mappings| {
                    mappings
                        .iter()
                        .find(|mapping| mapping["inputID"].as_str() == Some(input))
                });
            let disabled_assignment =
                current.is_some_and(|mapping| mapping["outputType"] == "disableGroup");
            let assignment = current
                .and_then(|mapping| mapping["controllerGroup"]["controllerAssignment"].as_str())
                .or_else(|| button["assignmentValue"].as_str())
                .unwrap_or("");
            let assignment = if disabled_assignment { "DISABLE" } else { assignment };
            let label = t(assignment);
            let selected = self.selected_button.as_deref() == Some(input);
            let disabled = button["isEnabled"] == false || button["disabled"] == true;
            let color = if disabled {
                Colors::disabled_text()
            } else if disabled_assignment {
                Colors::disabled_assignment()
            } else if current.is_some() {
                Colors::accent()
            } else {
                Colors::text()
            };
            let click_input = input.clone();
            let hover_input = input.clone();
            let control = BaseButton::new(SharedString::from(format!("gamepad-input-{input}")))
                .accessibility_label(format!(
                    "{}: {label}",
                    button["counter"].as_str().unwrap_or(input)
                ))
                .selected(selected)
                .disabled(disabled)
                .h(surface::css(geometry.button_height))
                .px(surface::css(geometry.button_padding))
                .py_0()
                .max_w(surface::css(235. - geometry.column_padding - 10.))
                .rounded(surface::css(3.))
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(color)
                .bg(if selected {
                    Colors::active()
                } else {
                    Colors::transparent()
                })
                .flex()
                .items_center()
                .overflow_hidden()
                .hover(|style| {
                    style.bg(if selected {
                        Colors::active()
                    } else {
                        Colors::hover()
                    })
                })
                .active(|style| style.bg(Colors::active()))
                .focus_visible(|style| style.border_1().border_color(Colors::accent()))
                .child(div().text_ellipsis().child(label))
                .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                    let next = entered.then(|| hover_input.clone());
                    if this.customize_hover != next {
                        this.customize_hover = next;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.selected_button.as_ref() != Some(&click_input) {
                        this.selected_button = Some(click_input.clone());
                        cx.notify();
                    }
                }));
            column = column.child(
                div()
                    .absolute()
                    .top(surface::css(top))
                    .when(left, |view| {
                        view.right(surface::css(geometry.column_padding + 10.))
                    })
                    .when(!left, |view| {
                        view.left(surface::css(geometry.column_padding + 10.))
                    })
                    .child(control),
            );
        }
        column.into_any_element()
    }
}

#[derive(IntoElement)]
struct DeviceImage {
    product: &'static Product,
    edition: u32,
    layout: u32,
}
impl RenderOnce for DeviceImage {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let geometry = &self.product.geometry;
        div().when_some(
            self.product
                .image(self.edition, self.layout, window.scale_factor()),
            |view, image| {
                view.child(
                    img(image.asset.clone())
                        .absolute()
                        .left(surface::css(geometry.image_left))
                        .top(surface::css((geometry.height - geometry.image_height) / 2.))
                        .h(surface::css(geometry.image_height))
                        .w(surface::css(
                            geometry.image_height * image.width / image.height,
                        )),
                )
            },
        )
    }
}

#[cfg(test)]
#[path = "gamepad_customize_artwork_tests.rs"]
mod tests;
