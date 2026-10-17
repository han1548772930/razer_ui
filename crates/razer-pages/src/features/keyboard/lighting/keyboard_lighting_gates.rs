//! Current mounted Lighting roots and their factory/BLE/WDL guards.
//! See keyboard-lighting-gates-2026-10-11.json for source hashes and slices.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::theme::KeyboardRgbEffectColors as Colors;

impl KeyboardProductWorkspace {
    pub(super) fn lighting_columns_disabled(&self) -> bool {
        match self.spec.product_id {
            642 | 678 | 679 | 688 | 719 | 720 | 721 | 728 | 740 | 741 | 742 | 746 => {
                self.factory_default_profile
            }
            756 => {
                self.calibration_v2.normal_mode
                    && (self.factory_default_profile || self.observed_bluetooth() == Some(true))
            }
            _ => false,
        }
    }

    fn lighting_factory_warning(&self) -> bool {
        self.factory_default_profile
            && match self.spec.product_id {
                642 | 678 | 679 | 688 | 719 | 720 | 721 | 740 | 741 | 742 | 746 => true,
                728 => self
                    .lighting_observation
                    .as_ref()
                    .and_then(|observation| observation.is_windows_mode)
                    .unwrap_or(true),
                756 => self.calibration_v2.normal_mode && self.observed_bluetooth() != Some(true),
                _ => false,
            }
    }

    fn lighting_wdl_mounts(&self) -> usize {
        // Current Lighting roots (nav `lgc` entry and the chromaApp iframe root)
        // mount the WDL popup either unconditionally or behind
        // `DeviceInfo.isWDLSupported`. 2596 mounts both a guarded and an
        // unconditional copy; preserve the original stacking behavior.
        // 721/739/740/741/742/746/756 only have the guarded caller and declare
        // `isWDLSupported:!1`, so they mount none.
        match self.spec.product_id {
            2596 => 2,
            // Unconditional caller.
            574 | 579 | 599 | 605 | 614 | 617 | 619 | 620 | 647 | 655 | 659 | 664 | 673 | 675
            | 677 | 717 | 752 | 2595 => 1,
            // Guarded caller with `isWDLSupported:!0`.
            590 | 600 | 602 | 642 | 658 | 661 | 678 | 679 | 688 | 689 | 691 | 697 | 708 | 716
            | 719 | 720 | 724 | 727 | 728 => 1,
            _ => 0,
        }
    }

    fn lighting_wdl_visible(&self) -> bool {
        self.lighting_wdl_mounts() != 0
            && self.observed_bluetooth() != Some(true)
            && self
                .lighting_observation
                .as_ref()
                .is_some_and(|observation| observation.is_dynamic_lighting_enabled)
    }

    pub(super) fn lighting_controls_disabled(&self) -> bool {
        self.lighting_columns_disabled() || self.lighting_wdl_visible()
    }

    pub(super) fn lighting_path_disabled(&self, path: &str) -> bool {
        if path.starts_with("/brightness/") || path.starts_with("/switchOffLighting/") {
            self.lighting_controls_disabled()
                || (path.starts_with("/switchOffLighting/")
                    && !self.boolean("/brightness/isEnabled"))
                || (path == "/switchOffLighting/idleMinutes"
                    && !self.boolean("/switchOffLighting/isIdleEnabled"))
        } else {
            false
        }
    }

    pub(super) fn lighting_page_gates(
        &self,
        columns: AnyElement,
        iframe: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let unit = window.rem_size() / 16.;
        div()
            .relative()
            .min_h_full()
            .child(columns)
            .when(self.lighting_factory_warning(), |page| {
                page.child(calibration::factory_warning(window))
            })
            .when(
                self.spec.product_id == 756
                    && self.calibration_v2.normal_mode
                    && self.observed_bluetooth() == Some(true),
                |page| page.child(ble_warning(window)),
            )
            .when(self.lighting_wdl_visible(), |page| {
                page.children((0..self.lighting_wdl_mounts()).map(|index| {
                    div()
                        .id(SharedString::from(format!("keyboard-wdl-backdrop-{index}")))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .bg(Colors::backdrop())
                        .opacity(if self.lighting_columns_disabled() { 0.3 } else { 1. })
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            v_flex()
                                .w(surface::css(400.))
                                .p(surface::css(20.))
                                .items_center()
                                .rounded(surface::css(3.))
                                .bg(Colors::background())
                                .border_1()
                                .border_color(Colors::warning())
                                .shadow(vec![BoxShadow {
                                    color: Colors::black(),
                                    offset: point(px(0.), unit * 6.),
                                    blur_radius: unit * 10.,
                                    spread_radius: px(0.),
                                    inset: false,
                                }])
                                .text_color(Colors::text())
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .text_center()
                                .child(
                                    img("synapse/keyboard-lighting-warning.svg")
                                        .size(surface::css(20.)),
                                )
                                .child(
                                    gpui_kit::base::TextView::html(
                                        SharedString::from(format!(
                                            "keyboard-wdl-description-{index}"
                                        )),
                                        t("POPUP_WARNING_DYNAMIC_LIGHTING_AUDIO_VISUALIZER"),
                                    )
                                    .selectable(false)
                                    .scrollable(false),
                                )
                                .child(
                                    BaseButton::new(SharedString::from(format!(
                                        "keyboard-wdl-settings-{index}"
                                    )))
                                    .disabled(self.lighting_columns_disabled())
                                    .underline()
                                    .hover(|button| button.text_color(Colors::accent()))
                                    .child(t(if iframe {
                                        "MANAGE_CHROMA_APP_SETTINGS"
                                    } else {
                                        "MANAGE_SYNAPSE_SETTINGS"
                                    }))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.lighting_wdl_visible()
                                            && !this.lighting_columns_disabled()
                                        {
                                            this.request_lighting_command(
                                                KeyboardLightingCommand::OpenSettings {
                                                    chroma: iframe,
                                                },
                                                cx,
                                            );
                                        }
                                    })),
                                ),
                        )
                }))
            })
            .into_any_element()
    }
}

#[derive(IntoElement)]
pub(super) struct BleWarning;
impl RenderOnce for BleWarning {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        ble_warning(window)
    }
}

fn ble_warning(window: &Window) -> AnyElement {
    let unit = window.rem_size() / 16.;
    let icon = f32::from(unit * 14.);
    let html = t("WIRED_OR_HYPERSPEED_REQUIRED_DESC").replace(
        "{{dots}}",
        &format!("<img src=\"factory-profile-dots\" width=\"{icon}\" height=\"{icon}\" />"),
    );
    razer_assets::deferred_resource(
        anchored()
            .anchor(Anchor::LeftCenter)
            .position(point(window.viewport_size().width / 2., unit * 250.))
            .child(
                v_flex()
                    .relative()
                    .left(surface::css(-200.))
                    .w(surface::css(400.))
                    .px(surface::css(24.))
                    .py(surface::css(16.))
                    .rounded(surface::css(3.))
                    .bg(Colors::background())
                    .border_1()
                    .border_color(Colors::warning())
                    .shadow(vec![BoxShadow {
                        color: Colors::warning_shadow(),
                        offset: point(px(0.), unit * 6.),
                        blur_radius: unit * 10.,
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .text_center()
                    .text_color(Colors::text())
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(
                        h_flex()
                            .justify_center()
                            .mb(surface::css(10.))
                            .gap(surface::css(6.))
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .line_height(surface::css(16.))
                            .text_color(Colors::warning())
                            .child(img("synapse/keyboard-ble-warning.svg").size(surface::css(20.)))
                            .child(t("WIRED_OR_HYPERSPEED_REQUIRED").to_uppercase()),
                    )
                    .child(
                        gpui_kit::base::TextView::html("keyboard-lighting-ble-description", html)
                            .image_source(|_| "synapse/profile-more.svg".into())
                            .selectable(false)
                            .scrollable(false),
                    ),
            ),
    )
    .with_priority(1)
    .into_any_element()
}
