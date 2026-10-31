//! Mounted preview source and Camo branches from the three current camera roots.
//! Device enumeration and media playback remain at the transport boundary.
use super::*;
use razer_widgets::theme::CameraProductColors as Colors;

/// Actual media enumeration/preview facts supplied by a local media adapter.
/// This is deliberately separate from the persisted device profile and USB
/// property getter observations. The local media consumer publishes these
/// facts only after actual OS enumeration, frame delivery or capture failure.
#[derive(Clone, Debug, Default)]
pub struct CameraPreviewObservation {
    pub sources: Vec<CameraPreviewSource>,
    pub selected_device_id: Option<String>,
    pub preview_enabled: Option<bool>,
    pub video_loading: bool,
    pub used_by_other_app: bool,
    pub windows_properties_open: bool,
    pub third_party_selected: bool,
    pub selected_app_name: String,
    pub resolutions: Vec<Value>,
    pub higher_gen_cable_required: bool,
}

#[derive(Clone, Debug)]
pub struct CameraPreviewSource {
    pub device_id: String,
    pub name: String,
}

/// Parameters of current local actions. Emitting a request is not media
/// activation, enumeration success, a launched process, or a device ACK.
#[derive(Clone, Debug)]
pub enum CameraLocalActionRequested {
    SetPreviewEnabled(bool),
    RefreshSources,
    SelectSource { device_id: String, name: String },
    OpenWindowsSettings,
    LaunchInstalledApp(String),
}

#[derive(Deserialize)]
pub(super) struct CameraPreviewSpec {
    pub(super) source_title: String,
    pub(super) preview_off: String,
    pub(super) enable_preview: String,
}

fn settings_blocked(product_id: u32, observation: &CameraPreviewObservation) -> bool {
    let enabled = observation.preview_enabled == Some(true);
    enabled && observation.video_loading
        || product_id == 3596
            && (!enabled || observation.used_by_other_app || observation.windows_properties_open)
}

pub(super) struct CameraPreviewState {
    // The current reducer initializes this to null, not to true.
    pub(super) enabled: Option<bool>,
    source: Entity<SelectState<Vec<Choice>>>,
    pub(super) observation: CameraPreviewObservation,
    pub(super) media: super::camera_media::State,
}

impl CameraPreviewState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SourceControls>) -> Self {
        Self {
            enabled: None,
            // `cameraDeviceList: []`. Never synthesize a connected camera.
            source: cx.new(|cx| SelectState::new(Vec::new(), None, window, cx)),
            observation: CameraPreviewObservation::default(),
            media: super::camera_media::State::default(),
        }
    }
}

impl SourceControls {
    pub(super) fn init_camera_preview_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &self.camera_preview else {
            return;
        };
        self.subscriptions.push(
            cx.subscribe_in(&state.source, window, |this, _, event, _, cx| {
                if this.syncing {
                    return;
                }
                let SelectEvent::Confirm(Some(device_id)) = event else {
                    return;
                };
                let Some(state) = &this.camera_preview else {
                    return;
                };
                if state.observation.selected_device_id.as_deref() == Some(device_id.as_str()) {
                    return;
                }
                let Some(source) = state
                    .observation
                    .sources
                    .iter()
                    .find(|source| source.device_id == *device_id)
                else {
                    return;
                };
                cx.emit(CameraLocalActionRequested::SelectSource {
                    device_id: source.device_id.clone(),
                    name: source.name.clone(),
                });
            }),
        );
    }

    pub fn observe_camera_preview(
        &mut self,
        observation: CameraPreviewObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        self.syncing = true;
        let items = observation
            .sources
            .iter()
            .map(|source| Choice::new(source.device_id.clone(), source.name.clone()))
            .collect();
        state.source.update(cx, |select, cx| {
            select.set_items(items, window, cx);
            select.set_selected_value(
                &observation.selected_device_id.clone().unwrap_or_default(),
                window,
                cx,
            );
        });
        state.enabled = observation.preview_enabled;
        state.observation = observation;
        self.syncing = false;
        if super::camera_media::supported(self.spec.product_id) {
            if let Some(control) = self.control(&format!("{}:resolution", self.spec.product_id)) {
                let choices = self.camera_resolution_choices(control);
                if let Some(select) = self.selects.get(&control.key) {
                    select.update(cx, |select, cx| select.set_items(choices, window, cx));
                }
            }
        }
        self.sync(window, cx);
        cx.notify();
    }

    pub(super) fn camera_third_party_selected(&self) -> bool {
        super::camera_media::supported(self.spec.product_id)
            && self
                .camera_preview
                .as_ref()
                .is_some_and(|state| state.observation.third_party_selected)
    }

    pub(super) fn camera_higher_gen_cable_required(&self) -> bool {
        self.camera_preview
            .as_ref()
            .is_some_and(|state| state.observation.higher_gen_cable_required)
    }

    pub(super) fn render_camera_third_party_notice(
        &self,
        description: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app = self
            .camera_preview
            .as_ref()
            .map(|state| state.observation.selected_app_name.as_str())
            .unwrap_or_default();
        let (description_key, link_key, executable) = if app.contains("Camo") {
            (
                "CONFIGURE_IN_CAMO_STUDIO",
                "CAMO_STUDIO_LAUNCH",
                "Camo Studio",
            )
        } else if app.contains("NVIDIA Broadcast") {
            (
                "CONFIGURE_IN_NVIDIA_BROADCAST",
                "NVIDIA_BROADCAST_LAUNCH",
                "NVIDIA Broadcast",
            )
        } else if app.contains("VCam Camera") {
            (
                "CONFIGURE_IN_VCAM_CAMERA",
                "VCAM_CAMERA_LAUNCH",
                "XSplit VCam",
            )
        } else {
            return div().into_any_element();
        };
        v_flex()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .when(description, |notice| {
                notice
                    .mb(surface::css(15.))
                    .child(razer_i18n::t(description_key))
            })
            .child(
                razer_widgets::keyboard_navigation::Button::new("camera-launch-selected-app")
                    .accessibility_label(razer_i18n::t(link_key))
                    .p_0()
                    .self_start()
                    .cursor_pointer()
                    .hover(|button| button.text_color(Colors::focus()))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(
                        h_flex()
                            .items_center()
                            .child(div().underline().child(razer_i18n::t(link_key)))
                            .child(
                                img("synapse/camera-preview-external.svg").size(surface::css(24.)),
                            ),
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(CameraLocalActionRequested::LaunchInstalledApp(
                            executable.into(),
                        ))
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_camera_windows_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .items_center()
            .child(
                img("synapse/camera-windows-settings.svg")
                    .size(surface::css(20.))
                    .mr(surface::css(10.)),
            )
            .child(
                razer_widgets::keyboard_navigation::Button::new("camera-windows-settings")
                    .disabled(self.camera_settings_blocked())
                    .p_0()
                    .text_size(surface::css(14.))
                    .underline()
                    .cursor_pointer()
                    .accessibility_label(razer_i18n::t("WINDOWS_UVC_SETTINGS"))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(razer_i18n::t("WINDOWS_UVC_SETTINGS"))
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(CameraLocalActionRequested::OpenWindowsSettings)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn camera_control_blocked(&self, control: &ControlSpec) -> bool {
        self.camera_path_blocked(&control.path)
    }

    pub(super) fn camera_settings_blocked(&self) -> bool {
        self.camera_preview
            .as_ref()
            .is_some_and(|state| settings_blocked(self.spec.product_id, &state.observation))
    }

    pub(super) fn camera_path_blocked(&self, path: &str) -> bool {
        if !super::camera_media::supported(self.spec.product_id) {
            return false;
        }
        let Some(state) = &self.camera_preview else {
            return false;
        };
        // Current 3594/3595 export DisplayPreviewWhenDisabled=true; 3596
        // exports false and also gates off, capture failure and UVC settings.
        settings_blocked(self.spec.product_id, &state.observation)
            || state.observation.third_party_selected
                && (path.starts_with("@image/")
                    || matches!(
                        path,
                        "/imageMode" | "/camera/antiFlicker" | "/camera/mirrorVideo"
                    )
                    || path.starts_with("@view/")
                    || path.starts_with("/camera/viewPresets/"))
    }

    pub(super) fn camera_group_blocked(&self, group: &super::camera_sections::CameraGroup) -> bool {
        group
            .controls
            .iter()
            .filter_map(|key| self.control(key))
            .any(|control| self.camera_control_blocked(control))
    }

    pub(super) fn camera_resolution_choices(&self, control: &ControlSpec) -> Vec<Choice> {
        let Some(state) = &self.camera_preview else {
            return Vec::new();
        };
        control
            .options
            .iter()
            .enumerate()
            .filter(|(_, option)| state.observation.resolutions.contains(&option.value))
            .map(|(index, option)| Choice::new(index.to_string(), option.label.clone()))
            .collect()
    }
    fn preview_spec(&self) -> Option<&'static CameraPreviewSpec> {
        self.spec
            .pages
            .iter()
            .find_map(|page| page.camera_preview.as_ref())
    }

    pub(super) fn set_camera_preview_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.camera_source.cancel_footer_preview();
        if enabled {
            self.request_camera_cache(true, std::time::Duration::ZERO);
        }
        self.commit_camera_preview_enabled(enabled, cx);
    }
    pub(super) fn commit_camera_preview_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.camera_preview {
            state.enabled = Some(enabled);
            cx.emit(CameraLocalActionRequested::SetPreviewEnabled(enabled));
            cx.notify();
        }
    }

    pub(super) fn render_camera_preview_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self
            .camera_preview
            .as_ref()
            .is_some_and(|state| state.enabled == Some(true));
        h_flex()
            .ml_auto()
            .items_center()
            .child(
                razer_widgets::keyboard_navigation::Button::new("camera-preview-refresh")
                    .accessibility_label(razer_i18n::t("REFRESH"))
                    .p_0()
                    .size(surface::css(20.))
                    .min_w(surface::css(20.))
                    .mr(surface::css(10.))
                    .cursor_pointer()
                    .hover(|button| button.opacity(0.7))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(
                        razer_assets::resource_svg()
                            .flex_shrink_0()
                            .path("synapse/camera-section-reset.svg")
                            .size(surface::css(20.))
                            .text_color(Colors::text())
                            .with_transformation(Transformation::scale(size(-1., 1.))),
                    )
                    .on_click(cx.listener(|_, _, _, cx| {
                        // Source requests enumeration/reconnect. No source state changes the
                        // refresh icon, and no transport result is manufactured here.
                        cx.emit(SourceControlsPreviewRefreshRequested);
                        cx.emit(CameraLocalActionRequested::RefreshSources);
                    })),
            )
            .child(
                razer_widgets::keyboard_navigation::Button::new("camera-preview-toggle")
                    .accessibility_label(
                        self.preview_spec()
                            .map_or(String::new(), |spec| razer_i18n::t(&spec.source_title)),
                    )
                    .p_0()
                    .size(surface::css(24.))
                    .min_w(surface::css(24.))
                    .cursor_pointer()
                    .hover(|button| button.opacity(0.7))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(
                        img(if enabled {
                            "synapse/camera-preview-open.svg"
                        } else {
                            "synapse/camera-preview-close.svg"
                        })
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_camera_preview_enabled(!enabled, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_camera_source_select(&self) -> AnyElement {
        let Some(state) = &self.camera_preview else {
            return div().into_any_element();
        };
        surface::select(&state.source)
            .id("camera-preview-source")
            .items(
                state
                    .observation
                    .sources
                    .iter()
                    .map(|source| Choice::new(source.device_id.clone(), source.name.clone()))
                    .collect(),
            )
            .placeholder("")
            .accessibility_label(
                self.preview_spec()
                    .map_or(String::new(), |spec| razer_i18n::t(&spec.source_title)),
            )
            .w_full()
            .into_any_element()
    }

    pub(super) fn render_camera_preview_off(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(spec) = self.preview_spec() else {
            return div().into_any_element();
        };
        let enabled = self
            .camera_preview
            .as_ref()
            .is_some_and(|state| state.enabled == Some(true));
        let loading = self
            .camera_preview
            .as_ref()
            .is_some_and(|state| state.observation.video_loading);
        let used = self
            .camera_preview
            .as_ref()
            .is_some_and(|state| state.observation.used_by_other_app);
        if enabled && loading {
            return div()
                .absolute()
                .inset_0()
                .bg(super::camera_theme::preview_background())
                .flex()
                .items_center()
                .justify_center()
                .child(img("synapse/camera-preview-loading.svg").size(surface::css(26.)))
                .into_any_element();
        }
        if enabled && !used {
            return div().into_any_element();
        }
        let label = razer_i18n::t(if enabled && used {
            "RECONNECT"
        } else {
            &spec.enable_preview
        })
        .to_lowercase();
        // Source `.camera-reconnect` uses text-transform:capitalize after lowercasing.
        let label = label
            .split(' ')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or(String::new(), |first| {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                })
            })
            .collect::<Vec<_>>()
            .join(" ");
        v_flex()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .text_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(
                img("synapse/camera-preview-unable.svg")
                    .size(surface::css(18.))
                    .mb(surface::css(6.)),
            )
            .child(razer_i18n::t(if enabled && used {
                "CAMERA_DISABLED_DESCRIPTION"
            } else {
                &spec.preview_off
            }))
            .child(
                razer_widgets::keyboard_navigation::Button::new("camera-preview-enable")
                    .p_0()
                    .mt(surface::css(16.))
                    .cursor_pointer()
                    .underline()
                    .accessibility_label(label.clone())
                    .hover(|button| button.text_color(Colors::focus()))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if enabled && used {
                            cx.emit(CameraLocalActionRequested::RefreshSources);
                        } else {
                            this.set_camera_preview_enabled(true, cx);
                        }
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{CameraPreviewObservation, settings_blocked};

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn kiyo_v2_x_preserves_current_preview_and_property_window_disabled_states() {
        let mut observation = CameraPreviewObservation::default();
        for product_id in [3594, 3595] {
            assert!(!settings_blocked(product_id, &observation));
        }
        assert!(settings_blocked(3596, &observation));
        observation.preview_enabled = Some(true);
        assert!(!settings_blocked(3596, &observation));
        observation.video_loading = true;
        for product_id in [3594, 3595, 3596] {
            assert!(settings_blocked(product_id, &observation));
        }
        observation.video_loading = false;
        for field in ["used", "properties", "off"] {
            observation.used_by_other_app = field == "used";
            observation.windows_properties_open = field == "properties";
            observation.preview_enabled = Some(field != "off");
            assert!(settings_blocked(3596, &observation));
            for product_id in [3594, 3595] {
                assert!(!settings_blocked(product_id, &observation));
            }
        }
    }
}
