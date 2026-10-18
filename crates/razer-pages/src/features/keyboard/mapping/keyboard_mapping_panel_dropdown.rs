//! keysDrop's retained Dropdown27734, independently extracted for each product.
use super::*;
use std::time::Duration;

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    choices: Vec<Value>,
    arrow: String,
    close: String,
    inclusive_viewport_edge: bool,
}
fn source(pid: u32) -> &'static Product {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_panel_dropdown_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("actual keyboard keysDrop")
}
pub(super) fn close_icon(pid: u32) -> AnyElement {
    img(source(pid).close.clone())
        .size(surface::css(20.))
        .into_any_element()
}
#[derive(Default)]
pub(super) struct State {
    anchor: Rc<Cell<Bounds<Pixels>>>,
    options: Rc<Cell<Bounds<Pixels>>>,
    above: bool,
    scroll: ScrollHandle,
    activation: Option<Subscription>,
}
pub(super) fn control(
    workspace: &KeyboardProductWorkspace,
    cx: &Context<KeyboardProductWorkspace>,
) -> AnyElement {
    let state = &workspace.analog_gamepad.mapping;
    Control {
        product_id: workspace.spec.product_id,
        selected: usize::from(state.filter_customized),
        open: state.filter_menu,
        above: state.panel_dropdown.above,
        anchor: state.panel_dropdown.anchor.clone(),
        options: state.panel_dropdown.options.clone(),
        scroll: state.panel_dropdown.scroll.clone(),
        owner: cx.entity().downgrade(),
    }
    .into_any_element()
}
#[derive(IntoElement)]
struct Control {
    product_id: u32,
    selected: usize,
    open: bool,
    above: bool,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    options: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
    owner: WeakEntity<KeyboardProductWorkspace>,
}
impl RenderOnce for Control {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::base::motion::{self, Easing, Transition};
        let metadata = source(self.product_id);
        let rotation = motion::transition(
            "keyboard-panel-dropdown-rotation",
            if self.open { std::f32::consts::PI } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let max_height = motion::transition(
            "keyboard-panel-dropdown-height",
            if self.open { 180. } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let measured_anchor = self.anchor.clone();
        let owner_toggle = self.owner.clone();
        let owner_outside = self.owner.clone();
        let option_bounds = self.options.clone();
        let anchor_outside = self.anchor.clone();
        let mut width = surface::css(180.).to_pixels(window.rem_size());
        for choice in &metadata.choices {
            let mut measure = div()
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .whitespace_nowrap()
                .p(surface::css(4.))
                .border_1()
                .child(t(&text(&choice["content"])))
                .into_any_element();
            width = width.max(
                measure
                    .layout_as_root(
                        size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
                        window,
                        cx,
                    )
                    .width,
            );
        }
        let owner_select = self.owner.clone();
        let measured_options = self.options.clone();
        let options = v_flex()
            .id("keyboard-panel-filter-options")
            .w(width)
            .max_h(surface::css(max_height))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(rgb(0x000000))
            .when(self.open, |options| {
                options.border_1().border_color(rgb(0x515151))
            })
            .on_prepaint(move |rect, _, _| measured_options.set(rect))
            .children(
                self.open
                    .then_some(())
                    .into_iter()
                    .flat_map(|_| metadata.choices.iter().enumerate())
                    .map(|(index, choice)| {
                        let owner = owner_select.clone();
                        BaseButton::new(SharedString::from(format!(
                            "keyboard-panel-filter-option-{index}"
                        )))
                        .w_full()
                        .h(surface::css(25.))
                        .min_h(surface::css(25.))
                        .p(surface::css(4.))
                        .text_left()
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .text_color(rgb(if self.selected == index {
                            0x44d62c
                        } else {
                            0xcccccc
                        }))
                        .hover(|option| option.bg(rgba(0xffffff1a)))
                        .active(|option| option.text_color(rgb(0x44d62c)))
                        .child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(t(&text(&choice["content"]))),
                        )
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.analog_gamepad.mapping.filter_menu = false;
                                this.analog_gamepad.mapping.filter_customized = index == 1;
                                cx.notify();
                            });
                        })
                    }),
            )
            .into_any_element();
        div()
            .relative()
            .w(surface::css(180.))
            .h(surface::css(27.))
            .mr(surface::css(10.))
            .child(
                BaseButton::new("keyboard-panel-filter-trigger")
                    .relative()
                    .w_full()
                    .h(surface::css(27.))
                    .min_h(surface::css(27.))
                    .px(surface::css(5.))
                    .py(surface::css(4.))
                    .bg(rgba(0x00000000))
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .border_1()
                    .border_color(rgb(if self.open { 0x44d62c } else { 0x515151 }))
                    .hover(|trigger| trigger.border_color(rgb(0x44d62c)))
                    .text_color(rgb(0xcccccc))
                    .text_left()
                    .on_prepaint(move |rect, _, _| measured_anchor.set(rect))
                    .child(
                        div()
                            .max_w(surface::css(156.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(t(&text(&metadata.choices[self.selected]["content"]))),
                    )
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(surface::css(29.))
                            .h(surface::css(25.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                razer_assets::resource_svg()
                                    .path(metadata.arrow.clone())
                                    .text_color(rgb(0x999999))
                                    .size(surface::css(10.))
                                    .with_transformation(Transformation::rotate(radians(rotation))),
                            ),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = owner_toggle.update(cx, |this, cx| {
                            if this
                                .analog_gamepad
                                .mapping
                                .panel_dropdown
                                .activation
                                .is_none()
                            {
                                this.analog_gamepad.mapping.panel_dropdown.activation = Some(
                                    cx.observe_window_activation(window, |this, window, cx| {
                                        if !window.is_window_active()
                                            && this.analog_gamepad.mapping.filter_menu
                                        {
                                            this.analog_gamepad.mapping.filter_menu = false;
                                            cx.notify();
                                        }
                                    }),
                                );
                            }
                            let state = &mut this.analog_gamepad.mapping;
                            state.filter_menu = !state.filter_menu;
                            if state.filter_menu {
                                let bottom = state.panel_dropdown.anchor.get().bottom()
                                    + surface::css(2.).to_pixels(window.rem_size());
                                let height = surface::css(
                                    (metadata.choices.len() as f32 * 25. + 2.).min(180.),
                                )
                                .to_pixels(window.rem_size());
                                state.panel_dropdown.above = if metadata.inclusive_viewport_edge {
                                    bottom + height >= window.viewport_size().height
                                } else {
                                    bottom + height > window.viewport_size().height
                                };
                                state.panel_dropdown.scroll.set_offset(point(
                                    px(0.),
                                    -surface::css(
                                        25. * usize::from(state.filter_customized) as f32,
                                    )
                                    .to_pixels(window.rem_size()),
                                ));
                            }
                            cx.notify();
                        });
                    })
                    .on_mouse_up_out(MouseButton::Left, move |event, _, cx| {
                        if !option_bounds.get().contains(&event.position)
                            && !anchor_outside.get().contains(&event.position)
                        {
                            let _ = owner_outside.update(cx, |this, cx| {
                                if this.analog_gamepad.mapping.filter_menu {
                                    this.analog_gamepad.mapping.filter_menu = false;
                                    cx.notify();
                                }
                            });
                        }
                    }),
            )
            .child(
                razer_assets::deferred_resource(OptionsPosition {
                    child: options,
                    anchor: self.anchor,
                    above: self.above,
                })
                .with_priority(if keypad_panel::supported(self.product_id) {
                    13
                } else {
                    152
                }),
            )
    }
}
struct OptionsPosition {
    child: AnyElement,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    above: bool,
}
impl IntoElement for OptionsPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for OptionsPosition {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let anchor = self.anchor.get();
        let height = window.layout_bounds(*child).size.height;
        let top = if self.above {
            anchor.bottom() - surface::css(28.).to_pixels(window.rem_size()) - height
        } else {
            anchor.top() + surface::css(28.).to_pixels(window.rem_size())
        };
        let origin = point(anchor.left(), top);
        window.with_element_offset(origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_panel_dropdown_data.
fn read_keyboard_mapping_panel_dropdown_catalog() -> Vec<Product> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Product>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Product> {
        let value = match r.tag()? {
            0 => Product {
                product_id: r.child(decode_2)?,
                choices: r.child(decode_3)?,
                arrow: r.child(decode_5)?,
                close: r.child(decode_5)?,
                inclusive_viewport_edge: r.child(decode_6)?,
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
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_PANEL_DROPDOWN_DATA,
        0,
        "d53b4fe7a109477b4c059953f1e736d26f8171abbee368c9e07db314f64569ef",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_panel_dropdown_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_panel_dropdown_data_decodes() {
        let _ = super::read_keyboard_mapping_panel_dropdown_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_panel_dropdown_data.
