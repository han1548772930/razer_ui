//! Current 2636 mounted useOm panel, radio order and 20px palette. The add
//! editor is VC/YC's 220x269 editor rather than the shared 40-color palette.
use super::lighting_state::EffectId;
use super::*;
use crate::features::lighting_color::{CustomColorEditor, CustomColorEditorEvent};
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Popover, Radio};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{ButtonCustomVariant, ButtonVariants};
use razer_widgets::{
    source_slider::SourceSlider,
    theme::{GamepadTriggerColors as Colors, PaletteColors},
};
use std::time::Duration;

pub(super) struct LightingUi {
    editor: Entity<CustomColorEditor>,
    editor_open: bool,
    context_color: Option<u64>,
    swatch_bounds: Rc<Cell<Bounds<Pixels>>>,
    _subscription: Subscription,
}
impl LightingUi {
    pub(super) fn new(window: &mut Window, cx: &mut Context<GamepadProductWorkspace>) -> Self {
        let editor = cx.new(|cx| CustomColorEditor::new(window, cx));
        let subscription = cx.subscribe_in(
            &editor,
            window,
            |this, _, event: &CustomColorEditorEvent, _, cx| {
                if !this.lighting_ui.editor_open {
                    return;
                }
                this.lighting_ui.editor_open = false;
                if let CustomColorEditorEvent::Saved(color) = event {
                    if let Some(settings) = &mut this.lighting_runtime.settings {
                        match settings.append(*color) {
                            Ok(()) => this.submit_controller_effect(cx),
                            Err(error) => eprintln!("[controller-lighting] {error:#}"),
                        }
                    }
                }
                cx.notify();
            },
        );
        Self {
            editor,
            editor_open: false,
            context_color: None,
            swatch_bounds: Rc::new(Cell::new(Bounds::default())),
            _subscription: subscription,
        }
    }
    pub(super) fn cancel(&mut self, cx: &mut App) {
        self.editor_open = false;
        self.context_color = None;
        self.editor.update(cx, |editor, cx| editor.cancel(cx));
    }
}

impl GamepadProductWorkspace {
    pub(super) fn controller_preset_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(settings) = &self.lighting_runtime.settings else {
            return div().into_any_element();
        };
        surface::panel_with_help(
            t("EFFECTS"),
            surface::help_control("controller-effects-help", t("EFFECTS_TOOLTIP_RADIO_OPTION")),
            cx,
        )
        .child(
            v_flex().children(
                [
                    (EffectId::Static, "STATIC"),
                    (EffectId::Spectrum, "SPECTRUM_CYCLING"),
                    (EffectId::Off, "NONE"),
                ]
                .map(|(effect, label)| self.controller_effect_radio(effect, label, cx)),
            ),
        )
        .child(match settings.effect_id {
            EffectId::Spectrum => v_flex()
                .child(div().my(surface::css(10.)).child(t("EFFECT_SPEED")))
                .child(h_flex().gap(surface::css(10.)).children(
                    [(3, "SLOW"), (2, "MEDIUM"), (1, "AGGRO")].map(|(rate, label)| {
                        BaseButton::new(("controller-spectrum-rate", rate as u64))
                            .accessibility_label(t(label))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(t(label))
                            .bg(Colors::meter_background())
                            .text_color(Colors::text())
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(if settings.rate == rate {
                                Colors::accent()
                            } else {
                                PaletteColors.picker_border()
                            })
                            .hover(|style| style.border_color(Colors::accent()))
                            .w(surface::css(72.))
                            .h(surface::css(27.))
                            .p_0()
                            .text_size(surface::css(14.))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(settings) = &mut this.lighting_runtime.settings {
                                    settings.rate = rate;
                                    this.submit_controller_effect(cx);
                                }
                            }))
                    }),
                ))
                .into_any_element(),
            EffectId::Static => self.controller_palette(cx),
            EffectId::Off => div().into_any_element(),
        })
        .into_any_element()
    }
    fn controller_effect_radio(
        &self,
        effect: EffectId,
        label: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let checked = self
            .lighting_runtime
            .settings
            .as_ref()
            .is_some_and(|settings| settings.effect_id == effect);
        let owner = cx.entity().downgrade();
        Radio::new(("controller-lighting-effect", u8::from(effect) as u64))
            .checked(checked)
            .accessibility_label(t(label))
            .flex()
            .items_center()
            .mb(surface::css(10.))
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .text_color(Colors::text())
            .child(
                div()
                    .size(surface::css(20.))
                    .rounded_full()
                    .border_1()
                    .border_color(Colors::radio_border())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10.))
                            .rounded_full()
                            .bg(Colors::accent())
                            .opacity(if checked { 1. } else { 0. }),
                    ),
            )
            .child(div().ml(surface::css(10.)).child(t(label)))
            .on_change(move |_, _, _, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.lighting_ui.cancel(cx);
                    if let Some(settings) = &mut this.lighting_runtime.settings {
                        settings.effect_id = effect;
                        this.submit_controller_effect(cx);
                    }
                });
            })
    }
    fn controller_palette(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(settings) = &self.lighting_runtime.settings else {
            return div().into_any_element();
        };
        let palette = PaletteColors;
        let bounds = self.lighting_ui.swatch_bounds.clone();
        let editor = self.lighting_ui.editor.clone();
        let editor_focus = editor.read(cx).focus_handle(cx);
        v_flex()
            .relative()
            .child(div().my(surface::css(10.)).child(t("Color")))
            .child(
                h_flex()
                    .relative()
                    .flex_wrap()
                    .w(surface::css(300.))
                    .my(surface::css(10.))
                    .gap(surface::css(10.))
                    .children(settings.color_list.iter().map(|color| {
                        let identity = color.identity;
                        let selected = color.index == settings.index;
                        let rgb = color.value.0;
                        let trigger = BaseButton::new(("controller-static-color", identity))
                            .accessibility_label(String::from(color.value))
                            .relative()
                            .size(surface::css(20.))
                            .mr(surface::css(5.))
                            .mb(surface::css(5.))
                            .rounded(surface::css(4.))
                            .bg(gpui_kit::rgb(
                                (u32::from(rgb[0]) << 16)
                                    | (u32::from(rgb[1]) << 8)
                                    | u32::from(rgb[2]),
                            ))
                            .border_1()
                            .border_color(if selected {
                                palette.swatch_border()
                            } else {
                                palette.surface().opacity(0.)
                            })
                            .hover(|s| {
                                s.border_color(palette.swatch_border())
                                    .shadow(vec![BoxShadow {
                                        color: palette.white(),
                                        offset: point(px(0.), px(0.)),
                                        blur_radius: px(0.),
                                        spread_radius: surface::css(2.)
                                            .to_pixels(cx.theme().font_size),
                                        inset: false,
                                    }])
                            })
                            .when(selected, |button| {
                                button
                                    .shadow(vec![BoxShadow {
                                        color: palette.white(),
                                        offset: point(px(0.), px(0.)),
                                        blur_radius: px(0.),
                                        spread_radius: surface::css(2.)
                                            .to_pixels(cx.theme().font_size),
                                        inset: false,
                                    }])
                                    .child(
                                        div()
                                            .absolute()
                                            .left(surface::css(3.75))
                                            .top(surface::css(3.75))
                                            .size(surface::css(11.))
                                            .border_2()
                                            .border_color(palette.white())
                                            .rounded(surface::css(5.)),
                                    )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(settings) = &mut this.lighting_runtime.settings {
                                    settings.select(identity);
                                    this.submit_controller_effect(cx);
                                }
                            }));
                        if color.index < 8 {
                            return trigger.into_any_element();
                        }
                        Popover::new(("controller-color-delete", identity))
                            .mouse_button(MouseButton::Right)
                            .offset(surface::css(-10.).to_pixels(cx.theme().font_size))
                            .open(self.lighting_ui.context_color == Some(identity))
                            .on_open_change(cx.listener(move |this, open, _, cx| {
                                this.lighting_ui.context_color = open.then_some(identity);
                                cx.notify();
                            }))
                            .trigger(trigger)
                            .when(
                                self.lighting_ui.context_color == Some(identity),
                                |popover| {
                                    let content = div()
                                        .ml(surface::css(10.))
                                        .w(surface::css(90.))
                                        .h(surface::css(31.5))
                                        .border_1()
                                        .border_color(palette.picker_border())
                                        .bg(palette.surface())
                                        .child(
                                            Button::new("controller-delete-color")
                                                .label(t("DELETE"))
                                                .w_full()
                                                .h(surface::css(29.))
                                                .px(surface::css(10.))
                                                .py_0()
                                                .rounded(px(0.))
                                                .justify_start()
                                                .custom(
                                                    ButtonCustomVariant::new(cx)
                                                        .color(palette.surface())
                                                        .hover(palette.white().opacity(0.1)),
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.lighting_ui.context_color = None;
                                                    if let Some(settings) =
                                                        &mut this.lighting_runtime.settings
                                                    {
                                                        match settings.delete(identity) {
                                                            Ok(()) => {
                                                                this.submit_controller_effect(cx)
                                                            }
                                                            Err(error) => eprintln!(
                                                                "[controller-lighting] {error:#}"
                                                            ),
                                                        }
                                                    }
                                                    cx.notify();
                                                })),
                                        );
                                    popover.content(move |_, _, _| content)
                                },
                            )
                            .into_any_element()
                    }))
                    .when(settings.color_list.len() < 16, |row| {
                        row.child(
                            BaseButton::new("controller-add-color")
                                .accessibility_label(t("ADD"))
                                .size(surface::css(20.))
                                .mr(surface::css(5.))
                                .mb(surface::css(5.))
                                .rounded(surface::css(4.))
                                .bg(palette.surface())
                                .border_1()
                                .border_color(palette.surface().opacity(0.))
                                .hover(|style| style.border_color(Colors::accent()))
                                .child(img("synapse/plus.svg").size(surface::css(8.)))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.lighting_ui.context_color = None;
                                    this.lighting_ui.editor_open = true;
                                    this.lighting_ui
                                        .editor
                                        .update(cx, |editor, cx| editor.begin(window, cx));
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                            .absolute()
                            .inset_0(),
                    ),
            )
            // VC only changes top/bottom when openDirection itself changes.
            // This mounted caller's constant "up" retains the CSS top:29px.
            .child(
                Popover::new("controller-add-editor")
                    .open(self.lighting_ui.editor_open)
                    .offset(surface::css(29.).to_pixels(cx.theme().font_size))
                    .track_focus(&editor_focus)
                    .overlay_closable(false)
                    .on_open_change(cx.listener(|this, open, _, cx| {
                        if !open {
                            this.lighting_ui.cancel(cx);
                            cx.notify();
                        }
                    }))
                    .trigger(
                        BaseButton::new("controller-editor-anchor")
                            .tab_stop(false)
                            .h_0()
                            .w_0(),
                    )
                    .when(self.lighting_ui.editor_open, |popover| {
                        let content = div().child(editor).on_mouse_down_out(cx.listener(
                            |this, event: &MouseDownEvent, _, cx| {
                                if !this
                                    .lighting_ui
                                    .swatch_bounds
                                    .get()
                                    .contains(&event.position)
                                {
                                    this.lighting_ui.cancel(cx);
                                    cx.notify();
                                }
                            },
                        ));
                        popover.content(move |_, _, _| content)
                    }),
            )
            .into_any_element()
    }
    pub(super) fn controller_brightness_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = self.checked("/profile/brightness/isEnabled");
        let available = !self
            .lighting_runtime
            .settings
            .as_ref()
            .is_some_and(|state| state.effect_id == EffectId::Off);
        let slider = &self.sliders["/profile/brightness/value"];
        let value = self.number("/profile/brightness/value") as f32;
        let opacity = motion::transition(
            ElementId::from(("controller-brightness-enabled", cx.entity_id())),
            if available { 1_f32 } else { 0.3 },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        surface::panel_with_title_switch(
            t("BRIGHTNESS_HEADER"),
            surface::SynapseSwitch::new("gamepad-brightness")
                .accessibility_label(t("BRIGHTNESS_HEADER"))
                .checked(enabled)
                .disabled(!available)
                .on_change(cx.listener(move |this, next: &bool, _, cx| {
                    if available {
                        this.write("/profile/brightness/isEnabled", json!(*next), cx);
                    }
                })),
            surface::help_control("gamepad-brightness-help", t("BRIGHTNESS_TOOLTIP")),
            cx,
        )
        .opacity(opacity)
        .child(
            SourceSlider::new(slider, value / 100.)
                .enabled(enabled && available)
                .tip(Some(value.to_string()))
                .range_labels("0", "100", Colors::text()),
        )
        .into_any_element()
    }
}
