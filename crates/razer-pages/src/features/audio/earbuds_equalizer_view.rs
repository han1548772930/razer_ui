//! The mounted compact EQ card. The profile owns presets; the page owns hover state.
use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use razer_device::earbuds_equalizer::{Mode, Preset};
use razer_widgets::{
    keyboard_navigation::Button as SourceButton, theme::AudioEqualizerColors as C,
};
use std::time::Duration;

#[derive(Default)]
pub(super) struct State {
    help_hovered: bool,
    requires_hovered: bool,
    disabled_cursor: Option<Point<Pixels>>,
}

impl AudioProductWorkspace {
    pub(super) fn earbuds_equalizer_card(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.earbuds_equalizer_disabled();
        let selected = self.earbuds_equalizer_setting().ok();
        let changed = selected.is_some_and(|setting| {
            self.earbuds.eq_defaults.iter().any(|default| {
                default.mode == setting.mode
                    && default.preset == setting.preset
                    && default.bands != setting.bands
            })
        });
        // Ip tests the Standard selection even while an incoming profile selects Esports.
        let standard_custom =
            if let Some(setting) = selected.filter(|setting| setting.mode == Mode::Standard) {
                setting.preset == Preset::Custom
            } else {
                self.draft
                    .pointer("/profile/audioEqualizer/presets/selectedPreset/standard")
                    .and_then(Value::as_str)
                    == Some("custom")
            };
        let key = self.spec.equalizers.first().map(|eq| eq.key.clone());
        let mut presets = h_flex()
            .id("earbuds-eq-presets")
            .test_support()
            .gap(surface::css(10.))
            .ml(surface::css(10.))
            .mt(surface::css(20.));
        for entry in &self.earbuds.eq_presets {
            let preset = entry.preset;
            let esports = entry.mode == Mode::Esports;
            let selected = selected.is_some_and(|setting| setting.preset == preset);
            let label = t(preset.key().to_uppercase().as_str()).to_uppercase();
            let icon = (!esports).then(|| preset_icon(preset)).flatten();
            let button_key = key.clone();
            presets = presets.child(
                SourceButton::new(SharedString::from(format!(
                    "earbuds-eq-preset-{}",
                    preset.key()
                )))
                .accessibility_label(label.clone())
                .selected(selected)
                .disabled(disabled)
                .flex()
                .w(surface::css(if esports { 200. } else { 252. }))
                .min_w_0()
                .h(surface::css(36.))
                .items_center()
                .justify_center()
                .border_1()
                .border_color(if selected { C::selected() } else { C::border() })
                .rounded(surface::css(3.))
                .text_size(surface::css(12.))
                .when(!disabled && !esports, |button| {
                    button.hover(|style| style.border_color(C::selected()))
                })
                .when_some(icon, |button, icon| {
                    button.child(img(icon).size(surface::css(20.)).mr(surface::css(10.)))
                })
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(key) = &button_key {
                        this.select_earbuds_eq_preset(key, preset, window, cx);
                    }
                })),
            );
        }
        if standard_custom {
            let reset_key = key.clone();
            presets = presets.child(
                SourceButton::new("earbuds-eq-reset")
                    .accessibility_label(t("RESET_LOWERCASE"))
                    .disabled(disabled || !changed)
                    .flex()
                    .flex_1()
                    .items_center()
                    .ml(surface::css(17.))
                    .mr(surface::css(13.))
                    .text_size(surface::css(14.))
                    .underline()
                    .when(!changed, |button| button.opacity(0.3))
                    .when(!disabled && changed, |button| {
                        button.hover(|style| style.text_color(C::selected()))
                    })
                    .child(
                        razer_assets::resource_svg()
                            .path("synapse/earbuds-eq-reset.svg")
                            .size(surface::css(16.))
                            .mr(surface::css(4.)),
                    )
                    .child(t("RESET_LOWERCASE"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(key) = &reset_key {
                            this.reset_equalizer(key, window, cx);
                        }
                    })),
            );
        }
        let title = h_flex()
            .h(surface::css(20.))
            .ml(surface::css(10.))
            .justify_between()
            .child(
                div()
                    .text_size(surface::css(14.))
                    .child(t("SELECT_AUDIO_PRESET")),
            )
            .when(changed, |row| {
                row.child(self.earbuds_eq_required(window, cx))
            });
        div()
            .id("earbuds-eq-panel")
            .test_support()
            .relative()
            .flex_none()
            .w(surface::css(1220.))
            .h(surface::css(544.))
            .p(surface::css(30.))
            .mt(surface::css(20.))
            .mb(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(C::surface())
            .font_family("Roboto")
            .text_color(C::text())
            .when(disabled, |card| card.opacity(0.5))
            .child(
                div()
                    .ml(surface::css(10.))
                    .mb(surface::css(20.))
                    .h(surface::css(20.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(24.))
                    .text_color(C::selected())
                    .child(t("AUDIO_EQUALIZER").to_uppercase()),
            )
            .child(title)
            .child(presets)
            .child(self.earbuds_eq_chart(window, cx))
            .child(self.earbuds_eq_help(disabled, window, cx))
            .children(self.earbuds_equalizer_error().map(|error| {
                div()
                    .id("earbuds-eq-submission-error")
                    .test_support()
                    .absolute()
                    .left(surface::css(30.))
                    .right(surface::css(30.))
                    .bottom(surface::css(10.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(16.))
                    .text_color(cx.theme().danger)
                    .child(error)
            }))
            .when(disabled, |card| {
                card.child(
                    div()
                        .id("earbuds-eq-disabled")
                        .test_support()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if !*hovered {
                                this.earbuds.eq_ui.disabled_cursor = None;
                            }
                            cx.notify();
                        }))
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.earbuds.eq_ui.disabled_cursor = Some(event.position);
                            cx.notify();
                        })),
                )
            })
            .when(disabled, |card| {
                card.children(self.earbuds.eq_ui.disabled_cursor.map(|cursor| {
                    razer_assets::deferred_resource(CursorTip {
                        cursor,
                        source_corner: false,
                        content: div()
                            .id("earbuds-eq-disabled-tip")
                            .test_support()
                            .max_w(surface::css(320.))
                            .px(surface::css(10.))
                            .py(surface::css(8.))
                            .border_1()
                            .border_color(C::border())
                            .bg(C::tip_surface())
                            .text_color(C::text())
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .child(
                                gpui_kit::base::Tooltip::new("earbuds-eq-disabled-content")
                                    .child(t("LIMITED_FUNCTIONALITY_EARBUD")),
                            )
                            .into_any_element(),
                    })
                    .with_priority(999999)
                }))
            })
            .into_any_element()
    }

    fn earbuds_eq_help(
        &self,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = !disabled && self.earbuds.eq_ui.help_hovered;
        let background = surface::fade_color(
            "earbuds-eq-help",
            if hovered { C::help_hover() } else { C::help() },
            300,
            window,
            cx,
        );
        let message = if self.spec.product_id == 2664 {
            "EQUALIZER_TOOLTIP_1"
        } else {
            "EQUALIZER_TOOLTIP_2"
        };
        SourceButton::new("earbuds-eq-help")
            .accessibility_label(t(message))
            .disabled(disabled)
            .absolute()
            .right(surface::css(10.))
            .top(surface::css(10.))
            .size(surface::css(14.))
            .rounded(surface::css(7.))
            .bg(background)
            .child(img("synapse/earbuds-eq-help.svg").size_full())
            .child(
                tip("earbuds-eq-help-tip", message, hovered, 10., window, cx)
                    .right(surface::css(5.))
                    .top(surface::css(20.)),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.earbuds.eq_ui.help_hovered = *hovered;
                cx.notify();
            }))
            .into_any_element()
    }

    fn earbuds_eq_required(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let hovered = self.earbuds.eq_ui.requires_hovered && !self.earbuds_equalizer_disabled();
        div()
            .relative()
            .child(
                div()
                    .id("earbuds-eq-requires")
                    .test_support()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(surface::css(20.))
                    .pl(surface::css(2.))
                    .pr(surface::css(9.))
                    .py(surface::css(2.))
                    .mr(surface::css(112.))
                    .rounded(surface::css(30.))
                    .bg(C::tip_surface())
                    .text_color(C::required_text())
                    .text_size(surface::css(12.))
                    .child(
                        img("synapse/earbuds-eq-requires.svg")
                            .size(surface::css(16.))
                            .mr(surface::css(5.)),
                    )
                    .child(t("REQUIRES_RAZER_SYNAPSE").to_uppercase())
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.earbuds.eq_ui.requires_hovered = *hovered;
                        cx.notify();
                    })),
            )
            .child(
                tip(
                    "earbuds-eq-requires-tip",
                    "REQUIRE_RAZER_SYNAPSE_TIPS",
                    hovered,
                    8.,
                    window,
                    cx,
                )
                .left_0()
                .top(surface::css(30.)),
            )
            .into_any_element()
    }
}

fn preset_icon(preset: Preset) -> Option<&'static str> {
    match preset {
        Preset::Game => Some("synapse/controller-fill-grey-icon.svg"),
        Preset::Music => Some("synapse/earbuds-eq-music.svg"),
        Preset::Movie => Some("synapse/earbuds-eq-movie.svg"),
        Preset::Custom => Some("synapse/earbuds-eq-custom.svg"),
        Preset::Voice => None,
    }
}

fn tip(
    id: &'static str,
    message: &'static str,
    visible: bool,
    bottom_padding: f32,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + Styled {
    let opacity = motion::transition(
        (id, "opacity"),
        if visible { 1_f32 } else { 0_f32 },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    );
    div()
        .id(id)
        .test_support()
        .absolute()
        .w(surface::css(300.))
        .pt(surface::css(8.))
        .px(surface::css(10.))
        .pb(surface::css(bottom_padding))
        .border_1()
        .border_color(C::border())
        .bg(C::tip_surface())
        .text_color(C::text())
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_left()
        .opacity(opacity)
        .when(!visible, |tip| tip.invisible())
        .child(gpui_kit::base::Tooltip::new((ElementId::from(id), "content")).child(t(message)))
}

// Ou's fixed cursor portal reserves 320x40 for edge decisions, then centers
// the measured text box. It does not use the ordinary anchor/flip policy.
pub(super) struct CursorTip {
    cursor: Point<Pixels>,
    content: AnyElement,
    source_corner: bool,
}

impl CursorTip {
    pub(super) fn new(cursor: Point<Pixels>, content: impl IntoElement) -> Self {
        Self {
            cursor,
            content: content.into_any_element(),
            source_corner: false,
        }
    }
    /// MicrophoneBasic uses pageX/pageY+25 without the shared edge-clamping hook.
    pub(super) fn at_source_corner(cursor: Point<Pixels>, content: impl IntoElement) -> Self {
        Self { cursor, content: content.into_any_element(), source_corner: true }
    }
}

impl IntoElement for CursorTip {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for CursorTip {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let unit = window.rem_size() / 16.;
        let viewport = window.viewport_size();
        let center = self
            .cursor
            .x
            .max(unit * 168.)
            .min(viewport.width - unit * 168.);
        let top = if self.cursor.y + unit * 64. < viewport.height - unit * 8. {
            self.cursor.y + unit * 24.
        } else {
            (self.cursor.y - unit * 64.).max(unit * 8.)
        };
        let position = if self.source_corner {
            point(self.cursor.x, self.cursor.y + unit * 25.)
        } else { point(center - bounds.size.width / 2., top) };
        window.with_element_offset(position - bounds.origin, |window| {
            self.content.prepaint(window, cx);
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
