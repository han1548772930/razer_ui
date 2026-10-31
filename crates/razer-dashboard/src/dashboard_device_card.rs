//! Mounted trees from current Dashboard 22534/z/E/Z and the final 55 CSS.
use super::{
    dashboard_device as device_state,
    dashboard_grid::{Action, DashboardState},
};
use gpui_kit::base::{
    ElementExt as _,
    motion::{self, Easing, Presence, Transition},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_model::model::SetupStatus;
use razer_widgets::surface::css;
use std::{cell::Cell, rc::Rc, time::Duration};

/// Current Dashboard's product-image slot, in source CSS pixels.
pub const PRODUCT_CARD_IMAGE_CSS_SIZE: [f32; 2] = [250., 140.];

/// App.tt applies these overrides in order to a real runtimeData record.
/// Missing observations retain the normal product request, not a guessed
/// attached module. String sidepad values preserve the source's numeric IDs.
fn artwork_parameters(device: &Device) -> (u32, u32, Option<u32>, bool) {
    let fields = &device.dashboard;
    let edition = device.edition_id;
    let mut filename_edition = edition;
    let mut layout = Some(device.layout_id);
    let mut wrist = false;
    if let Some(sidepad) = fields
        .sidepad_layout
        .as_ref()
        .filter(|v| device_state::js_truthy(v))
    {
        if sidepad.is_boolean() {
            wrist = true;
        } else {
            layout = sidepad
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .or_else(|| {
                    sidepad.as_str().and_then(|text| {
                        text.parse::<u32>().ok().filter(|id| id.to_string() == text)
                    })
                });
        }
    }
    if fields.wristrest_connected == Some(true) {
        layout = Some(device.layout_id);
        wrist = true;
    }
    if fields.is_mat_plugged == Some(true) && fields.is_mouse_connected != Some(true) {
        filename_edition = 0;
        layout = Some(1);
        wrist = false;
    }
    (edition, filename_edition, layout, wrist)
}

/// Current Dashboard selection, with an explicitly supplied local-catalog
/// image only for user-created previews whose PluginImages request is absent.
pub fn product_card_image(
    device: &Device,
    scale_factor: f32,
    local_preview_image: Option<&'static str>,
) -> Option<&'static str> {
    let (edition, filename_edition, layout, wrist) = artwork_parameters(device);
    razer_assets::dashboard_card_image(
        device.product_id,
        edition,
        filename_edition,
        layout,
        wrist,
        scale_factor,
    )
    .or_else(|| {
        (device.dashboard.local_snapshot
            && device.dashboard.connection_observation.is_none()
            && device.serial_number.starts_with("PREVIEW-"))
        .then_some(local_preview_image)
        .flatten()
    })
}

#[cfg(test)]
mod artwork_tests {
    use super::{artwork_parameters, product_card_image};
    use razer_model::model::DeviceConnectionObservation;
    use razer_model::product_preview::registered_preview;
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn runtime_artwork_overrides_follow_the_complete_source_order() {
        let mut device = registered_preview(182).unwrap();
        device.edition_id = 0;
        device.layout_id = 2;
        assert_eq!(artwork_parameters(&device), (0, 0, Some(2), false));
        device.dashboard.sidepad_layout = Some(json!(6));
        assert_eq!(artwork_parameters(&device), (0, 0, Some(6), false));
        device.dashboard.sidepad_layout = Some(json!(true));
        assert_eq!(artwork_parameters(&device), (0, 0, Some(2), true));
        device.dashboard.sidepad_layout = Some(json!(6));
        device.dashboard.wristrest_connected = Some(true);
        assert_eq!(artwork_parameters(&device), (0, 0, Some(2), true));
        device.dashboard.is_mat_plugged = Some(true);
        device.dashboard.is_mouse_connected = Some(false);
        assert_eq!(artwork_parameters(&device), (0, 0, Some(1), false));
        device.edition_id = 128;
        assert_eq!(artwork_parameters(&device), (128, 0, Some(1), false));
        device.dashboard.is_mouse_connected = Some(true);
        assert_eq!(artwork_parameters(&device), (128, 128, Some(2), true));
    }

    #[::core::prelude::v1::test]
    fn unobserved_artwork_fields_and_truthy_invalid_layout_do_not_invent_a_module() {
        let mut device = registered_preview(182).unwrap();
        device.edition_id = 0;
        device.layout_id = 1;
        device.dashboard.sidepad_layout = Some(json!("6"));
        assert_eq!(artwork_parameters(&device), (0, 0, Some(6), false));
        device.dashboard.sidepad_layout = Some(json!("06"));
        assert_eq!(artwork_parameters(&device), (0, 0, None, false));
        device.dashboard.sidepad_layout = Some(json!("missing-layout"));
        assert_eq!(artwork_parameters(&device), (0, 0, None, false));
        device.dashboard.sidepad_layout = Some(json!(false));
        assert_eq!(artwork_parameters(&device), (0, 0, Some(1), false));
        device.dashboard.sidepad_layout = Some(json!(0));
        assert_eq!(artwork_parameters(&device), (0, 0, Some(1), false));
        device.dashboard.wristrest_connected = Some(true);
        device.begin_local_session();
        assert_eq!(artwork_parameters(&device), (0, 0, Some(1), false));
    }

    #[::core::prelude::v1::test]
    fn preview_artwork_does_not_replace_source_dashboard_images_or_live_device_failures() {
        let device = registered_preview(190).unwrap();
        let source = product_card_image(&device, 1., None).unwrap();
        assert_eq!(
            product_card_image(&device, 1., Some("preview-only")),
            Some(source)
        );

        // Blade's actual layout-zero request and the Dashboard error retry
        // are unavailable. The local catalog can still resolve ProductImage.
        let mut device = registered_preview(563).unwrap();
        device.layout_id = 0;
        assert_eq!(product_card_image(&device, 1., None), None);
        assert_eq!(
            product_card_image(&device, 1., Some("preview-only")),
            Some("preview-only")
        );
        device.dashboard.connection_observation = Some(DeviceConnectionObservation::UsbPresent);
        assert_eq!(product_card_image(&device, 1., Some("preview-only")), None);
        device.dashboard.connection_observation = None;
        device.serial_number = "real-device-local-snapshot".into();
        assert_eq!(product_card_image(&device, 1., Some("preview-only")), None);
    }
}

pub struct Actions {
    pub retry: Action,
    pub firmware: Action,
    pub settings: Action,
}

#[derive(IntoElement)]
pub struct DeviceCard {
    id: String,
    device: Device,
    state: Entity<DashboardState>,
    actions: Actions,
    /// Independent local page capability; never changes the device snapshot.
    open_without_installation: bool,
    local_preview_image: Option<&'static str>,
}

impl DeviceCard {
    pub fn new(
        id: impl Into<String>,
        device: Device,
        state: Entity<DashboardState>,
        actions: Actions,
    ) -> Self {
        Self {
            id: id.into(),
            device,
            state,
            actions,
            open_without_installation: false,
            local_preview_image: None,
        }
    }

    pub fn open_without_installation(mut self, enabled: bool) -> Self {
        self.open_without_installation = enabled;
        self
    }

    pub fn local_preview_image(mut self, image: Option<&'static str>) -> Self {
        self.local_preview_image = image;
        self
    }
}

/// Nested mousedown only changes the short-drag release action, as in xi/xe.
/// Keyboard activation remains owned by the native button.
fn override_button<T: StatefulInteractiveElement>(
    button: T,
    card: &str,
    state: &Entity<DashboardState>,
    action: Action,
) -> T {
    let card = card.to_owned();
    let state = state.clone();
    let mouse_action = action.clone();
    button
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            state.update(cx, |state, _| {
                state.override_action(card.clone(), mouse_action.clone())
            });
        })
        .on_click(move |event, window, cx| {
            if !matches!(event, ClickEvent::Mouse(_)) {
                action(window, cx);
            }
            cx.stop_propagation();
        })
}

impl RenderOnce for DeviceCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let device = &self.device;
        let fields = &device.dashboard;
        let ready = device.setup_status == SetupStatus::Ready;
        let installation_gate = !self.open_without_installation;
        let raw_off = device_state::power_off(device);
        let observed_power = device_state::observed_power(&self.id, device, window, cx);
        let off = observed_power.off();
        // Current 7861.38e19423 BoxItemDevice.reInstall has distinct local
        // background-manager recovery and installer dispatch branches. The
        // latter's resource chain is still unresolved; do not classify local
        // installation as Internet-only merely from the action name.
        let retry = (installation_gate
            && matches!(
                device.setup_status,
                SetupStatus::InstallCanceled | SetupStatus::Error
            ))
            || fields.no_alive_sign == Some(true);
        let source_spinner = device_state::show_spinner(device.setup_status, raw_off);
        let spinner = source_spinner && !retry && installation_gate;
        let image_disabled = !spinner
            && if !off || fields.is_playstation == Some(true) || !ready {
                observed_power.standby_or_off()
            } else {
                true
            };
        let xbox = fields.is_xbox == Some(true);
        let ps = fields.is_playstation == Some(true);
        let category = device_state::category(device);
        let min_firmware = device_state::min_firmware(device);
        let firmware_info = fields
            .firmware_update_info
            .as_ref()
            .is_some_and(device_state::js_truthy);
        let firmware = ready
            && (fields.firmware_needs_upgrade == Some(true)
                || (firmware_info && fields.upgrade_mode.as_deref() != Some("SDK")));
        let wdl = fields.is_dynamic_lighting == Some(true)
            && fields.is_wdl_supported == Some(true)
            && ready
            && !device.use_ble;
        let selected = device
            .sub_devices
            .as_ref()
            .and_then(|items| {
                items.iter().position(|item| {
                    item.get("isShownOnUI").and_then(|v| v.as_bool()) == Some(true)
                })
            })
            .map(|index| index + 1)
            .filter(|index| *index > 1);
        let count = device
            .sub_devices
            .as_ref()
            .filter(|items| items.len() >= 2)
            .map(Vec::len)
            .or_else(|| {
                fields
                    .count
                    .filter(|count| *count > 0)
                    .map(|count| count as usize)
            });
        let mut artwork = Vec::new();
        if min_firmware {
            artwork.push((
                "synapse/dashboard-card/arcade-controller-fw-update-disable.svg",
                250.,
                !ready && installation_gate,
            ));
        }
        if !xbox && !ps && !min_firmware {
            if let Some(asset) =
                product_card_image(device, window.scale_factor(), self.local_preview_image)
            {
                artwork.push((
                    asset,
                    PRODUCT_CARD_IMAGE_CSS_SIZE[0],
                    !ready && installation_gate,
                ));
            }
        }
        if xbox && category == "Controller" {
            artwork.push((
                "synapse/dashboard-card/controller-disable.svg",
                145.,
                !ready,
            ));
        }
        if ps {
            artwork.push((
                if fields.sub_category.as_deref() == Some("ARCADE_CONTROLLER") {
                    "synapse/dashboard-card/arcade-controller-disable.svg"
                } else {
                    "synapse/dashboard-card/ps-controller-disable.png"
                },
                145.,
                false,
            ));
        }
        if xbox && category == "Headset" {
            artwork.push(("synapse/dashboard-card/headset-disable.svg", 250., !ready));
        }
        if xbox
            && category.eq_ignore_ascii_case("AUDIO")
            && fields
                .sub_category
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("EARBUDS"))
        {
            artwork.push(("synapse/dashboard-card/earbuds-disable.svg", 250., !ready));
        }
        let mut image = div()
            .relative()
            .w(css(PRODUCT_CARD_IMAGE_CSS_SIZE[0]))
            .h(css(PRODUCT_CARD_IMAGE_CSS_SIZE[1]))
            .flex_shrink_0()
            .when(image_disabled, |view| view.opacity(0.3))
            .children(artwork.into_iter().map(|(asset, width, blurred)| {
                img(asset)
                    .absolute()
                    .top_0()
                    .left(css((PRODUCT_CARD_IMAGE_CSS_SIZE[0] - width) / 2.))
                    .w(css(width))
                    .h(css(PRODUCT_CARD_IMAGE_CSS_SIZE[1]))
                    .object_fit(ObjectFit::Contain)
                    .when(blurred, |view| view.opacity(0.3))
            }));
        let mut retry_layer = None;
        if retry {
            // Local service capability is independent of connection.isOnline.
            // Preserve the source command/drag owner and report unavailable
            // adapters in the shell, without an Internet-dependent placeholder.
            let button = razer_widgets::module_button::module_action(
                SharedString::from(format!("{}-retry", self.id)),
                i18n::t("RETRY"),
                true,
                false,
                cx,
            )
            .min_w(css(100.))
            .px(css(3.))
            .py_0()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer();
            let button = override_button(button, &self.id, &self.state, self.actions.retry.clone());
            let retry_content = div()
                .relative()
                .flex()
                .flex_col()
                .items_center()
                .mt(css(62.5))
                .child(button);
            if image_disabled {
                image = image.child(retry_content);
            } else {
                retry_layer = Some(
                    div()
                        .absolute()
                        .left(css(20.))
                        .top(css(10.))
                        .w(css(250.))
                        .child(retry_content),
                );
            }
        }
        if spinner {
            image = image.child(
                div()
                    .absolute()
                    .left(css(111.))
                    .top(css(56.))
                    .size(css(27.))
                    .child(SourceSpinner),
            );
        }
        let mut content = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .pt(css(10.))
            .px(css(20.))
            .pb(css(9.))
            .when_some(selected, |view, number| {
                view.child(
                    div()
                        .absolute()
                        .top(css(10.))
                        .right(css(10.))
                        .size(css(20.))
                        .rounded(css(50.))
                        .bg(rgb(0xbbbbbb))
                        .text_color(rgb(0x111111))
                        .text_size(css(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(number.to_string()),
                )
            })
            .when_some(count, |view, count| {
                view.child(
                    div()
                        .absolute()
                        .top(css(10.))
                        .right(css(10.))
                        .size(css(24.))
                        .rounded(css(15.))
                        .bg(rgb(0x707070))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(count.to_string()),
                )
            })
            .child(image);

        content = content.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .when(device_state::disabled(device, &observed_power), |view| {
                    view.opacity(0.3)
                })
                .child(
                    div()
                        .w_full()
                        .px(css(10.))
                        .min_h(css(17.))
                        .text_size(css(14.))
                        .line_height(css(16.))
                        .text_center()
                        .whitespace_normal()
                        .when(xbox || ps, |view| view.opacity(0.3))
                        .child(device_state::name(device).to_uppercase()),
                )
                .child(
                    div()
                        .px(css(10.))
                        .text_size(css(12.))
                        .line_height(css(14.))
                        .text_color(rgb(0x707070))
                        .text_center()
                        .child(device_state::edition(device).to_uppercase()),
                )
                .when(!off, |view| {
                    let status = device_state::status(device);
                    view.child(
                        div()
                            .id(SharedString::from(format!("{}-status", self.id)))
                            .test_support()
                            .role(Role::Status)
                            .aria_label(status.clone())
                            .w(css(230.))
                            .mx_auto()
                            .text_size(css(12.))
                            .line_height(css(14.))
                            .text_color(rgb(
                                if matches!(
                                    device.setup_status,
                                    SetupStatus::InstallCanceled | SetupStatus::Error
                                ) {
                                    0xfd4949
                                } else {
                                    0x44d62c
                                },
                            ))
                            .text_center()
                            .text_ellipsis()
                            .child(status),
                    )
                })
                .when(xbox && category == "Controller", |view| {
                    view.child(console_shortcut(
                        fields.controller_mode_variant.as_deref() == Some("THREE_MODE"),
                    ))
                })
                .when(
                    ps && category == "Controller"
                        && fields.sub_category.as_deref() != Some("ARCADE_CONTROLLER"),
                    |view| {
                        view.child(
                            div()
                                .w(css(230.))
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .text_color(rgb(0xffffff))
                                .text_center()
                                .child(i18n::t("PS_MODE_DESC").to_uppercase()),
                        )
                    },
                ),
        );
        if device_state::preset_loading(device) {
            let observed = fields
                .device_state
                .as_ref()
                .and_then(|state| state.get("presetLoading"))
                .and_then(|state| state.get("value"));
            let label = observed
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_default();
            let target = observed
                .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
                .unwrap_or(0.) as f32;
            let progress = motion::transition(
                (SharedString::from(self.id.clone()), "preset-width"),
                target,
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseIn),
                window,
                cx,
            );
            content = content.child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .bg(rgba(0x00000080))
                    .pt(css(10.))
                    .px(css(20.))
                    .pb(css(9.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .justify_end()
                            .w(css(250.))
                            .h(css(140.))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .mb(css(5.))
                                    .text_size(css(12.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(rgb(0xffffff))
                                    .child(
                                        div()
                                            .flex()
                                            .child(i18n::t("PRESET_PROFILES_LOADING_TEXT"))
                                            .child(PresetDots),
                                    )
                                    .child(format!("{label}%")),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .bg(rgba(0x44d62c80))
                                    .rounded(css(10.))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(relative(progress.max(0.) / 100.))
                                            .h(css(5.))
                                            .rounded_r(css(10.))
                                            .bg(rgb(0x44d62c)),
                                    ),
                            ),
                    ),
            );
        }
        content = content.child(device_state::DashboardBattery::new(
            format!("{}-battery", self.id),
            device,
            observed_power,
            source_spinner,
            self.id.clone(),
            self.state.clone(),
        ));
        if firmware {
            content = content.child(Badge {
                card: self.id.clone(),
                state: self.state.clone(),
                kind: BadgeKind::Firmware(min_firmware),
                left: 10.,
                action: Some(self.actions.firmware),
            });
        }
        if fields.device_init_status_fail.as_deref() == Some("mixer_system_check_failed") {
            content = content.child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .bg(rgba(0x11111199))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .p(css(20.))
                    .text_center()
                    .child(
                        div()
                            .text_size(css(12.))
                            .child(i18n::t("MIXER_DEVICE_INIT_STATUS_FAILED_DES")),
                    ),
            );
        }
        if device.setup_status == SetupStatus::RestartRequired {
            content = content.child(Badge {
                card: self.id.clone(),
                state: self.state.clone(),
                kind: BadgeKind::Restart,
                left: 13.,
                action: None,
            });
        }
        content = content.children(retry_layer);
        if wdl {
            content = content.child(Badge {
                card: self.id.clone(),
                state: self.state.clone(),
                kind: BadgeKind::Wdl,
                left: if firmware_info { 39. } else { 10. },
                action: Some(self.actions.settings),
            });
        }
        content
    }
}

fn console_shortcut(three_mode: bool) -> impl IntoElement {
    div()
        .w(css(230.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(css(12.))
        .line_height(css(14.))
        .text_color(rgb(0xffffff))
        .child(i18n::t("PRESS"))
        .child(
            div()
                .flex()
                .items_center()
                .gap(css(5.))
                .line_height(css(21.))
                .ml(css(5.))
                .child(
                    img("synapse/dashboard-card/xbox-o-white.svg")
                        .h(css(18.))
                        .w(css(18.)),
                )
                .child(" +")
                .child(img("synapse/dashboard-card/xbox-menu.svg").size(css(21.)))
                .child(" +")
                .child(img("synapse/dashboard-card/xbox-a-white.svg").size(css(21.)))
                .when(three_mode, |view| {
                    view.child(" /")
                        .child(img("synapse/dashboard-card/xbox-y-white.svg").size(css(21.)))
                }),
        )
}

#[derive(Clone, Copy, PartialEq)]
enum BadgeKind {
    Restart,
    Wdl,
    Firmware(bool),
}
#[derive(IntoElement)]
struct Badge {
    card: String,
    state: Entity<DashboardState>,
    kind: BadgeKind,
    left: f32,
    action: Option<Action>,
}
#[derive(Default)]
struct BadgeHover {
    hovered: bool,
    right: Rc<Cell<Pixels>>,
}
impl RenderOnce for Badge {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (name, asset, tip) = match self.kind {
            BadgeKind::Restart => (
                "restart",
                "synapse/dashboard-card/warning.svg",
                "RESTART_REQUIRED",
            ),
            BadgeKind::Wdl => (
                "wdl",
                "synapse/dashboard-card/windows_dynamic_lighting_icon.svg",
                "DYNAMIC_LIGHTING_SYNAPSE_MSG",
            ),
            BadgeKind::Firmware(required) => (
                "firmware",
                "synapse/dashboard-card/icon_firmware_update-3.svg",
                if required {
                    "FIRMWARE_UPDATE_REQUIRED_TO_CONFIGURE_DESC"
                } else {
                    "FIRMWARE_UPDATE_NOTI"
                },
            ),
        };
        let id: ElementId = SharedString::from(format!("{}-{name}", self.card)).into();
        let hover = window.use_keyed_state((id.clone(), "badge-hover"), cx, |_, _| {
            BadgeHover::default()
        });
        let hovered = hover.read(cx).hovered;
        let anchor = hover.read(cx).right.clone();
        let flip = f32::from(window.viewport_size().width - anchor.get())
            < f32::from(window.rem_size()) / 16. * 300.;
        let firmware = matches!(self.kind, BadgeKind::Firmware(_));
        let opacity = motion::transition(
            (id.clone(), "badge-opacity"),
            if hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(if firmware { 0 } else { 300 })).easing(
                if self.kind == BadgeKind::Restart {
                    Easing::Ease
                } else {
                    Easing::Linear
                },
            ),
            window,
            cx,
        );
        let visible = if self.kind == BadgeKind::Restart {
            Presence::new((id.clone(), "visibility"), hovered)
                .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
                .sample(window, cx)
                .should_render()
        } else {
            hovered
        };
        let size = if self.kind == BadgeKind::Restart { 20. } else { 24. };
        let image = img(if self.kind == BadgeKind::Wdl && hovered {
            "synapse/dashboard-card/windows_dynamic_lighting_active_icon.svg"
        } else {
            asset
        })
        .size(css(size));
        let visual = if let Some(action) = self.action {
            override_button(
                razer_widgets::keyboard_navigation::Button::new("badge-action")
                    .accessibility_label(i18n::t(tip))
                    .size_full()
                    .relative()
                    .cursor_pointer()
                    .child(image)
                    .child(div().absolute().inset_0().border_1().border_color(
                        if firmware && hovered {
                            rgb(0x5d5d5d).into()
                        } else {
                            Hsla::transparent_black()
                        },
                    )),
                &self.card,
                &self.state,
                action,
            )
            .into_any_element()
        } else {
            image.into_any_element()
        };
        let card = self.card;
        let state = self.state;
        div()
            .id(id)
            .absolute()
            .top(css(10.))
            .left(css(self.left))
            .size(css(size))
            .on_prepaint(move |bounds, _, _| anchor.set(bounds.right()))
            .on_hover(window.listener_for(&hover, move |hover, value, _, cx| {
                hover.hovered = *value;
                state.update(cx, |state, cx| state.hover_card(&card, *value, cx));
                cx.notify();
            }))
            .child(visual)
            .child(
                div()
                    .absolute()
                    .top(css(if firmware {
                        30.
                    } else if self.kind == BadgeKind::Restart {
                        28.
                    } else {
                        24.
                    }))
                    .when(firmware && flip, |view| view.right(css(14.)))
                    .when(!firmware || !flip, |view| view.left(css(10.)))
                    .when(firmware, |view| view.w(css(300.)))
                    .when(self.kind == BadgeKind::Wdl, |view| view.w(css(280.)))
                    .when(self.kind == BadgeKind::Restart, |view| {
                        view.w_auto().whitespace_nowrap()
                    })
                    .bg(rgb(0x000000))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .px(css(10.))
                    .py(css(if firmware { 10. } else { 8. }))
                    .text_color(rgb(0xcccccc))
                    .font_family("Roboto")
                    .text_size(css(14.))
                    .line_height(css(16.))
                    .text_left()
                    .opacity(opacity)
                    .when(!visible, |view| view.invisible())
                    .child(i18n::t(tip)),
            )
    }
}

#[derive(IntoElement)]
struct PresetDots;
impl RenderOnce for PresetDots {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if cx.reduce_motion() {
            return div().child("...").into_any_element();
        }
        let start = *window
            .use_keyed_state("preset-dot-clock", cx, |_, _| std::time::Instant::now())
            .read(cx);
        div()
            .flex()
            .with_animation(
                "preset-dots",
                Animation::new(Duration::from_millis(600)).repeat(),
                move |view, _| {
                    view.children((0..3).map(|index| {
                        let elapsed = start.elapsed().as_secs_f32() - index as f32 * 0.15;
                        let phase = (elapsed / 0.6).rem_euclid(1.);
                        let opacity = if elapsed < 0. {
                            1.
                        } else if phase < 0.29 {
                            0.4
                        } else if phase < 0.30 {
                            0.4 + 0.6 * Easing::EaseIn.sample((phase - 0.29) / 0.01)
                        } else if phase < 0.98 {
                            1.
                        } else if phase < 0.99 {
                            1. - 0.6 * Easing::EaseIn.sample((phase - 0.98) / 0.01)
                        } else {
                            0.4
                        };
                        div().opacity(opacity).child(".")
                    }))
                },
            )
            .into_any_element()
    }
}

/// Sample the actual SMIL rotation and dash length; native SVG does not run SMIL.
#[derive(IntoElement)]
struct SourceSpinner;
fn spinner_frame(phase: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = f32::from(bounds.size.width) / 100.;
            let center = bounds.center();
            let rotation = if phase <= 0.5 {
                phase * 360.
            } else {
                180. + (phase - 0.5) * 1080.
            };
            let fraction = if phase <= 0.5 {
                0.1 + phase * 0.8
            } else {
                0.5 - (phase - 0.5) * 0.8
            };
            for (start, length, alpha) in [
                (0., std::f32::consts::TAU, 0.3),
                (rotation.to_radians(), fraction * std::f32::consts::TAU, 1.),
            ] {
                let radial = |angle: f32| {
                    point(px(angle.cos() * 30. * scale), px(angle.sin() * 30. * scale))
                };
                let mut path = PathBuilder::stroke(px(10. * scale));
                path.move_to(center + radial(start));
                // A complete circle needs two arcs instead of coincident endpoints.
                let segments = if length > std::f32::consts::PI { 2 } else { 1 };
                for segment in 1..=segments {
                    let end = start + length * segment as f32 / segments as f32;
                    path.arc_to(
                        point(px(30. * scale), px(30. * scale)),
                        px(0.),
                        false,
                        true,
                        center + radial(end),
                    );
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x44d62c).opacity(alpha));
                }
                if alpha == 1. {
                    // Original stroke-linecap="square": a half-stroke tangent
                    // extension at each endpoint of the default butt-capped arc.
                    for (angle, direction) in [(start, -1.), (start + length, 1.)] {
                        let endpoint = center + radial(angle);
                        let normal =
                            point(px(angle.cos() * 5. * scale), px(angle.sin() * 5. * scale));
                        let extension = point(
                            px(-angle.sin() * 5. * scale * direction),
                            px(angle.cos() * 5. * scale * direction),
                        );
                        let mut cap = PathBuilder::fill();
                        cap.add_polygon(
                            &[
                                endpoint + normal,
                                endpoint - normal,
                                endpoint - normal + extension,
                                endpoint + normal + extension,
                            ],
                            true,
                        );
                        if let Ok(cap) = cap.build() {
                            window.paint_path(cap, rgb(0x44d62c));
                        }
                    }
                }
            }
        },
    )
    .size(css(26.))
}
impl RenderOnce for SourceSpinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if cx.reduce_motion() {
            spinner_frame(0.25).into_any_element()
        } else {
            div()
                .size(css(26.))
                .with_animation(
                    "dashboard-spinner",
                    Animation::new(Duration::from_secs(2)).repeat(),
                    |view, phase| view.child(spinner_frame(phase)),
                )
                .into_any_element()
        }
    }
}
