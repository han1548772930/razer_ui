//! Current 4264:se / 1958:N,S inspector. Working paint parameters are transient:
//! 9286:R / 1638:j merge them; only 9286:U applies them to device regions.
use super::studio_color::{StudioColor, StudioColorEvent};
use super::*;
use crate::ui::source_tooltip::{SourceTooltip, SourceTooltipKind};
use gpui_kit::component::slider::SliderState;

pub(super) struct StudioProperties {
    current: Option<(u64, String)>,
    tool: String,
    params: Value,
    blur: Entity<SliderState>,
    color: Option<Entity<StudioColor>>,
    color_subscription: Option<Subscription>,
    window: Option<AnyWindowHandle>,
    _subscriptions: Vec<Subscription>,
}

impl StudioProperties {
    pub(super) fn new(owner: &Entity<ChromaStudio>, cx: &mut Context<Self>) -> Self {
        let blur = cx.new(|_| Self::blur_state(5.));
        let subscriptions = vec![
            cx.observe(owner, |this, owner, cx| {
                let owner = owner.read(cx);
                let current = owner
                    .document
                    .layers
                    .iter()
                    .find(|layer| Some(layer.id) == owner.current && !layer.group)
                    .map(|layer| (layer.id, layer.name.clone()));
                let tool = owner.tool.clone();
                this.sync(current, tool, cx);
            }),
            cx.observe(&blur, |this, slider, cx| {
                // Base's accessibility actions use set_value without emitting
                // SliderEvent. Observe the retained value for all input paths.
                let value = slider.read(cx).value().start().round();
                let expected = this.params["blur"].as_f64().unwrap_or(5.) as f32;
                if value == expected {
                    return;
                }
                if this.enabled()
                    && this
                        .current
                        .as_ref()
                        .is_some_and(|(_, name)| name == "ambient")
                {
                    this.params["blur"] = Value::from(value as u32);
                    cx.notify();
                } else {
                    // The base accessibility callback is available even when
                    // disabled. A disabled source control cannot change state.
                    slider.update(cx, |slider, cx| {
                        *slider = Self::blur_state(expected);
                        cx.notify();
                    });
                }
            }),
        ];
        Self {
            current: None,
            tool: "select".into(),
            params: serde_json::json!({}),
            blur,
            color: None,
            color_subscription: None,
            window: None,
            _subscriptions: subscriptions,
        }
    }

    fn blur_state(value: f32) -> SliderState {
        SliderState::new()
            .min(1.)
            .max(9.)
            .step(1.)
            .default_value(value)
    }

    fn enabled(&self) -> bool {
        self.current.is_some() && matches!(self.tool.as_str(), "pen" | "bucket")
    }

    fn color_value(&self) -> Option<u32> {
        self.params["color"]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
    }

    pub(super) fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = window.window_handle();
        if self
            .window
            .is_some_and(|old| old.window_id() == handle.window_id())
            && self.color.is_some()
        {
            return;
        }
        // Input subscriptions are created for the current host only. This
        // retained session may have outlived its previous Chroma host window.
        let color = cx.new(|cx| StudioColor::new(self.color_value(), window, cx));
        color.update(cx, |color, cx| {
            color.set_enabled(self.enabled(), window, cx)
        });
        self.color_subscription = Some(cx.subscribe(&color, |this, _, event, cx| {
            if !this.enabled()
                || !this
                    .current
                    .as_ref()
                    .is_some_and(|(_, name)| name == "static")
            {
                return;
            }
            let StudioColorEvent::Changed(value) = event;
            if this.color_value() != *value {
                if let Some(value) = value {
                    this.params["color"] = Value::from(*value);
                } else if let Some(params) = this.params.as_object_mut() {
                    params.remove("color");
                }
                cx.notify();
            }
        }));
        self.color = Some(color);
        self.window = Some(handle);
        cx.notify();
    }

    fn refresh_color(&self, replace_value: bool, cx: &mut Context<Self>) {
        let (Some(color), Some(handle)) = (self.color.clone(), self.window) else {
            return;
        };
        let value = self.color_value();
        let enabled = self.enabled();
        // Plain parent observation is not tied to a stale window. Defer the
        // window-bound input synchronization out of the properties update.
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                color.update(cx, |color, cx| {
                    if replace_value {
                        color.set_value(value, window, cx);
                    }
                    color.set_enabled(enabled, window, cx);
                })
            });
        });
    }

    pub(super) fn reset(&mut self, cx: &mut Context<Self>) {
        if !self.enabled() {
            return;
        }
        if let Some(effect) = self
            .current
            .as_ref()
            .and_then(|(_, name)| source().effects.iter().find(|effect| &effect.name == name))
        {
            if let (Some(params), Some(defaults)) =
                (self.params.as_object_mut(), effect.paint_params.as_object())
            {
                params.extend(defaults.clone());
            }
            let blur = self.params["blur"].as_f64().unwrap_or(5.) as f32;
            self.blur.update(cx, |slider, cx| {
                *slider = Self::blur_state(blur);
                cx.notify();
            });
            self.refresh_color(true, cx);
            cx.notify();
        }
    }

    fn sync(&mut self, current: Option<(u64, String)>, tool: String, cx: &mut Context<Self>) {
        let selection_changed = current != self.current;
        let tool_changed = tool != self.tool;
        if !selection_changed && !tool_changed {
            return;
        }
        // 9870:d keeps pen↔bucket values; transitions involving select/move
        // merge defaults (1638:j zB), whereas 9286:C replaces on layer change.
        let reset = selection_changed
            || (tool_changed
                && (matches!(self.tool.as_str(), "select" | "move")
                    || matches!(tool.as_str(), "select" | "move")));
        self.current = current;
        self.tool = tool;
        if reset {
            let defaults = self
                .current
                .as_ref()
                .and_then(|(_, name)| source().effects.iter().find(|effect| &effect.name == name))
                .map(|effect| {
                    if matches!(self.tool.as_str(), "pen" | "bucket") {
                        effect.paint_params.clone()
                    } else {
                        effect.params.clone()
                    }
                })
                .unwrap_or_else(|| serde_json::json!({}));
            if selection_changed {
                self.params = defaults;
            } else if let (Some(params), Some(defaults)) =
                (self.params.as_object_mut(), defaults.as_object())
            {
                params.extend(defaults.clone());
            }
            let blur = self.params["blur"].as_f64().unwrap_or(5.) as f32;
            self.blur.update(cx, |slider, cx| {
                *slider = Self::blur_state(blur);
                cx.notify();
            });
        }
        self.refresh_color(reset, cx);
        cx.notify();
    }

    fn subtitle(&self, title: &'static str, help: &'static str) -> AnyElement {
        let tip = SourceTooltip::new(
            (ElementId::from("studio-property-help"), title),
            label(help),
            300.,
        )
        .kind(SourceTooltipKind::Studio)
        .trigger(move |_, _, _| {
            button(
                (ElementId::from("studio-property-help-button"), title),
                help,
            )
            .size(surface::css(14.))
            .rounded_full()
            .bg(Colors::help_background())
            .child(icon("tooltip_questionmark", 14.))
            .into_any_element()
        });
        div()
            .flex()
            .items_center()
            .min_h(surface::css(34.))
            .px(surface::css(10.))
            .py(surface::css(8.))
            .child(div().flex_1().child(label(title).to_uppercase()))
            .child(if self.enabled() {
                tip.into_any_element()
            } else {
                div()
                    .size(surface::css(14.))
                    .rounded_full()
                    .bg(Colors::help_background())
                    .child(icon("tooltip_questionmark", 14.))
                    .into_any_element()
            })
            .into_any_element()
    }

    fn region_box(region: &str, preset: bool) -> Div {
        let thickness = surface::css(if preset { 4. } else { 30. });
        let rect = div().absolute().bg(if preset {
            Colors::white()
        } else {
            Colors::text()
        });
        match region {
            "full" => rect.inset_0(),
            "left" => rect.left_0().top_0().h_full().w(thickness),
            "right" => rect.right_0().top_0().h_full().w(thickness),
            "top" => rect.left_0().top_0().w_full().h(thickness),
            "bottom" => rect.left_0().bottom_0().w_full().h(thickness),
            _ => rect.size_0(),
        }
    }

    fn ambient(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.enabled();
        let region = self.params["screen"].as_str().unwrap_or("");
        let mut presets = div().flex().flex_wrap();
        for name in ["full", "left", "top", "right", "bottom"] {
            presets = presets.child(super::studio_region_preset::RegionPreset::new(
                name,
                region == name,
                enabled,
                cx.listener(move |this, _, _, cx| {
                    if this.enabled() {
                        this.params["screen"] = Value::from(name);
                        cx.notify();
                    }
                }),
            ));
        }
        div()
            .flex()
            .flex_col()
            .child(self.subtitle("TEXT_REGION", "TEXT_AMBIENT_AWARENESS_REGION"))
            .child(
                section()
                    .child(
                        div()
                            .relative()
                            .h(surface::css(128.))
                            .mb(surface::css(10.))
                            .border_1()
                            .border_color(Colors::helper())
                            .bg(Colors::region_background())
                            .child(
                                div()
                                    .relative()
                                    .size_full()
                                    .child(Self::region_box(region, false)),
                            ),
                    )
                    // 1958:N starts the native region service. No device/native
                    // bridge exists locally, so this command cannot invent a region.
                    .child(
                        button("studio-edit-region", "TEXT_EDIT_REGION")
                            .disabled(true)
                            .styles(|style| {
                                style
                                    .disabled(|style| style.opacity(if enabled { 0.3 } else { 1. }))
                            })
                            .hover(|style| style.bg(Colors::panel()))
                            .w_full()
                            .h(surface::css(27.))
                            .mb(surface::css(10.))
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(Colors::text())
                            .text_size(surface::css(12.))
                            .child(label("TEXT_EDIT_REGION").to_uppercase()),
                    )
                    .child(div().mb(surface::css(6.)).child(label("TEXT_PRESETS")))
                    .child(presets),
            )
            .child(self.subtitle("PROPERTIES", "TEXT_AMBIENT_AWARENESS_PROPERTIES"))
            .child(
                section()
                    .child(div().mb(surface::css(6.)).child(label("TEXT_BLEND")))
                    .child(
                        super::studio_slider::StudioSlider::new(&self.blur, enabled)
                            .label(label("TEXT_BLEND"))
                            .on_key_change({
                                let owner = cx.entity();
                                move |value, _, cx| {
                                    owner.update(cx, |this, cx| {
                                        if this.enabled() {
                                            this.params["blur"] = Value::from(value.round() as u32);
                                            cx.notify();
                                        }
                                    })
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .mt(surface::css(3.))
                            .mb(surface::css(10.))
                            .child(label("TEXT_SHARP"))
                            .child(label("TEXT_BLUR")),
                    )
                    .child(
                        div()
                            .text_color(Colors::helper())
                            .child(label("TEXT_AMBIENT_SLIDER_HELPER")),
                    ),
            )
            .into_any_element()
    }
}

fn section() -> Div {
    div()
        .flex()
        .flex_col()
        .px(surface::css(10.))
        .pb(surface::css(20.))
        .border_b_1()
        .border_color(Colors::border())
}

impl Render for StudioProperties {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opacity = surface::fade_opacity(
            "studio-properties-opacity",
            if self.enabled() { 1. } else { 0.3 },
            100,
            window,
            cx,
        );
        let content = match self.current.as_ref().map(|(_, name)| name.as_str()) {
            Some("ambient") => self.ambient(cx),
            Some("static") => div()
                .flex()
                .flex_col()
                .child(self.subtitle("COLOR", "TEXT_STATIC_COLOR"))
                .child(section().children(self.color.clone()))
                .into_any_element(),
            _ => div().into_any_element(),
        };
        div()
            .id("studio-properties")
            .line_height(relative(1.22))
            .flex_1()
            .min_h_0()
            .scrollable_y()
            .border_t_1()
            .border_color(Colors::border())
            .child(div().opacity(opacity).child(content))
    }
}
