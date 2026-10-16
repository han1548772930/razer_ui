//! Current 3594 GU -> LU/DU/NU and 3595/3596 Gh/$U footer/ph/jU reset.
//! MW flash save; only Pro uses module 88116 firmware restore/sync.
use super::*;
use razer_widgets::theme::CameraProductColors as Colors;
use std::time::Duration;

#[derive(Default)]
pub(super) struct State {
    pub(super) changed: bool,
    pub(super) reset: Option<Task<()>>,
}

#[cfg(test)]
mod tests {
    use super::{data, family_reset_steps};
    use razer_device::{
        camera_processing_family::CameraFamilyProcessingRequest as P,
        camera_submission::{CameraCommand, compile_camera_edit},
    };
    use serde_json::Value;

    fn defaults(pid: u32) -> Value {
        super::super::specs()
            .iter()
            .find(|p| p.product_id == pid)
            .unwrap()
            .profile
            .clone()
    }

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn family_footer_reset_replays_ordered_snapshots_and_keeps_other_image_presets() {
        for pid in [3595, 3596] {
            let defaults = defaults(pid);
            let mut profile = defaults.clone();
            profile["camera"]["autoFocus"] = serde_json::json!({"isEnabled":false,"manualFocus":99,"mode":2,"tracking":8,"lightingType":9});
            profile["camera"]["autoExposure"]["isEnabled"] = false.into();
            profile["camera"]["autoExposure"]["exposureValue"] = (-7).into();
            profile["image"][4]["dataSet"]["brightness"] = 37.into();
            profile["imageMode"] = 4.into();
            profile["camera"]["viewPresets"]["viewMode"] = 3.into();
            profile["camera"]["viewPresets"]["data"][2]["zoom"] = 1.3.into();
            let custom = profile["image"][4].clone();
            let steps = family_reset_steps(&mut profile, &defaults);
            assert_eq!(
                &steps[..8].iter().map(|(key, _)| *key).collect::<Vec<_>>(),
                &[
                    "zoom",
                    "pan-tilt",
                    "autofocus",
                    "focus-mode",
                    "focus-tracking",
                    "focus-lighting",
                    "focus",
                    "image-preset"
                ]
            );
            assert_eq!(steps[2].1["camera"]["autoFocus"]["manualFocus"], 99);
            assert_eq!(steps[2].1["camera"]["autoFocus"]["mode"], 2);
            assert_eq!(
                steps[3].1["camera"]["autoFocus"]["mode"],
                defaults["camera"]["autoFocus"]["mode"]
            );
            assert_eq!(steps[3].1["camera"]["autoFocus"]["tracking"], 8);
            assert_eq!(steps[4].1["camera"]["autoFocus"]["lightingType"], 9);
            assert_eq!(steps[6].1["camera"]["autoFocus"]["manualFocus"], 100);
            let exposure = steps
                .iter()
                .position(|(key, _)| *key == "auto-exposure")
                .unwrap();
            assert_eq!(
                steps[exposure - 1].1["camera"]["autoExposure"]["isEnabled"],
                false
            );
            assert_eq!(
                steps[exposure].1["camera"]["autoExposure"],
                defaults["camera"]["autoExposure"]
            );
            assert_eq!(profile["image"][4], custom);
            assert_eq!(profile["imageMode"], defaults["imageMode"]);
            assert_eq!(
                profile["camera"]["viewPresets"],
                defaults["camera"]["viewPresets"]
            );
            assert_eq!(profile["camera"]["cameraMode"], "auto");
            for (action, snapshot) in &steps {
                if *action != "resolution" {
                    let plan = compile_camera_edit(pid, action, snapshot)
                        .unwrap_or_else(|e| panic!("{pid} {action}: {e:#}"));
                    assert!(
                        !plan
                            .commands
                            .iter()
                            .any(|c| matches!(c, CameraCommand::Extension(_)))
                    );
                }
            }
        }
    }

    #[test]
    fn family_footer_reset_respects_missing_model_defaults_and_exposure_equality() {
        let defaults = defaults(3595);
        let mut profile = defaults.clone();
        profile["camera"]["ldc"] = true.into();
        let steps = family_reset_steps(&mut profile, &defaults);
        assert!(!steps.iter().any(|(key, _)| *key == "auto-exposure"));
        assert!(profile["camera"].get("ldc").is_none());
        let snapshot = &steps.iter().find(|(key, _)| *key == "ldc").unwrap().1;
        assert_eq!(
            compile_camera_edit(3595, "ldc", snapshot).unwrap().commands,
            [CameraCommand::FamilyProcessing(P::SetLdc {
                enabled: false,
                mode: 0
            })]
        );
        let defaults = self::defaults(3596);
        let mut profile = defaults.clone();
        profile["camera"]["autoFraming"] =
            serde_json::json!({"isEnabled":false,"view":"Wide","trackingSpeed":"Quick"});
        let old = profile["camera"]["autoFraming"].clone();
        let steps = family_reset_steps(&mut profile, &defaults);
        assert!(!steps.iter().any(|(key, _)| *key == "auto-framing"));
        assert_eq!(profile["camera"]["autoFraming"], old);
        assert_eq!(steps.last().unwrap().0, "mirror");
    }

    #[test]
    fn family_footer_lookup_keeps_source_product_tables_and_hidden_x_iso_wording() {
        let lookup = |pid, ldc, zoom| {
            data::fov(pid, ldc)
                .iter()
                .find(|(k, _)| *k == zoom)
                .unwrap()
                .1
        };
        assert_eq!(lookup(3595, false, 100), "94");
        assert_eq!(lookup(3595, false, 280), "43.3");
        assert_eq!(lookup(3595, false, 400), "31.3");
        assert_eq!(lookup(3596, false, 101), "80.35");
        assert_eq!(lookup(3596, false, 140), "62.9");
        assert_eq!(lookup(3595, true, 100), "72.5");
        assert_eq!(lookup(3596, true, 100), "72.5");
        assert_eq!(data::iso(Some(&4.into())), "800");
        assert_eq!(data::shutter(Some(&16666.into())), "1/60");
        assert_eq!(data::shutter(Some(&(-2).into())), "");
    }
}
const FOV: &[(i64, &str)] = include!("camera_fov.rs");
#[path = "camera_footer_data.rs"]
mod data;

/// Current ph/jU dispatch snapshots. The reset sends individual actions while
/// preserving old companions until their later action; it does not replace the
/// whole profile or call firmware RestoreDefaults. Only the selected default
/// image preset is reset, leaving the other local image presets intact.
fn family_reset_steps(profile: &mut Value, defaults: &Value) -> Vec<(&'static str, Value)> {
    let mut actions = Vec::new();
    profile["camera"]["viewPresets"] = defaults["camera"]["viewPresets"].clone();
    actions.push(("zoom", profile.clone()));
    actions.push(("pan-tilt", profile.clone()));
    for (field, key) in [
        ("isEnabled", "autofocus"),
        ("mode", "focus-mode"),
        ("tracking", "focus-tracking"),
        ("lightingType", "focus-lighting"),
        ("manualFocus", "focus"),
    ] {
        profile["camera"]["autoFocus"][field] = defaults["camera"]["autoFocus"][field].clone();
        profile["camera"]["cameraMode"] = if profile["camera"]["autoFocus"]["isEnabled"] == true {
            "auto".into()
        } else {
            profile["camera"]["cameraValue"] =
                profile["camera"]["autoFocus"]["manualFocus"].clone();
            "manual".into()
        };
        actions.push((key, profile.clone()));
    }
    let mode = defaults["imageMode"].as_u64().unwrap_or(0) as usize;
    if let Some(default) = defaults["image"].as_array().and_then(|p| p.get(mode)) {
        if let Some(index) = profile["image"]
            .as_array()
            .and_then(|p| p.iter().position(|p| p["name"] == default["name"]))
        {
            profile["image"][index]["dataSet"] = default["dataSet"].clone();
            profile["image"][index]["dataSet"]["name"] = default["name"].clone();
            profile["imageMode"] = index.into();
            actions.push(("image-preset", profile.clone()));
        }
    }
    // X has no default autoFraming payload. The generic local-storage handler
    // only replaces truthy payloads; its undefined protocol argument throws
    // before IO. Do not invent a framing write or erase its persisted object.
    if let Some(framing) = defaults["camera"].get("autoFraming") {
        profile["camera"]["autoFraming"] = framing.clone();
        actions.push(("auto-framing", profile.clone()));
    }
    // YEo's equality check suppresses only this full-object action.
    if profile["camera"]["autoExposure"] != defaults["camera"]["autoExposure"] {
        profile["camera"]["autoExposure"] = defaults["camera"]["autoExposure"].clone();
        actions.push(("auto-exposure", profile.clone()));
    }
    for (field, key) in [
        ("resolution", "resolution"),
        ("mjpegQuality", "mjpeg-quality"),
        ("hdr", "hdr"),
        ("ldc", "ldc"),
        ("lowLightCompensation", "lowLightCompensation"),
        ("dynamicNoiseReduction", "state3D"),
        ("antiFlicker", "anti-flicker"),
        ("watermark", "watermark"),
        ("mirrorVideo", "mirror"),
    ] {
        if let Some(value) = defaults["camera"].get(field) {
            profile["camera"][field] = value.clone();
        } else if let Some(camera) = profile["camera"].as_object_mut() {
            camera.remove(field);
        }
        actions.push((key, profile.clone()));
    }
    actions
}

impl SourceControls {
    fn footer_disabled(&self) -> bool {
        self.camera_settings_blocked()
    }

    fn reset_camera(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.footer_disabled() {
            return;
        }
        let scope = self.camera_runtime.queue.scope().cloned();
        // NU's source debounce owns the pending reset. Replacing the task or
        // unmounting/profile replacement cancels it before any device request.
        self.camera_footer.reset = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                if this.camera_runtime.queue.scope() != scope.as_ref() {
                    return;
                }
                // All reset handlers replace view presets first. Pro then
                // requests firmware defaults; V2/X dispatch ordered actions.
                let changed = this.draft["camera"]["viewPresets"]
                    != this.spec.profile["camera"]["viewPresets"];
                this.draft["camera"]["viewPresets"] =
                    this.spec.profile["camera"]["viewPresets"].clone();
                this.camera_footer.changed |= changed;
                this.listening = None;
                if this.spec.product_id == 3594 {
                    this.request_camera_edit("3594:restore-camera", window, cx);
                } else {
                    let previous_resolution = this.draft["camera"]["resolution"].clone();
                    this.camera_source.auto_iso = None;
                    for (action, profile) in family_reset_steps(&mut this.draft, &this.spec.profile)
                    {
                        if action != "resolution" {
                            this.request_camera_profile_edit(
                                &format!("{}:{action}", this.spec.product_id),
                                &profile,
                                window,
                                cx,
                            );
                        }
                    }
                    if previous_resolution != this.draft["camera"]["resolution"] {
                        this.schedule_camera_media_restart(cx);
                    }
                }
                this.request_camera_cache(false, Duration::from_millis(500));
                this.sync(window, cx);
                cx.emit(SourceControlsChanged);
                cx.notify();
            });
        }));
    }

    pub(super) fn camera_restore_completed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Module 88116 m mutates/persists these fields and broadcasts source
        // state only after RestoreDefaults returned true. Never infer them
        // from a diagnostic getter or from the optimistic view-preset reset.
        let resolution_changed = self.draft.pointer("/camera/resolution")
            != self.spec.profile.pointer("/camera/resolution");
        self.camera_source.auto_iso = None;
        for key in ["camera", "image", "imageMode"] {
            if let Some(value) = self.spec.profile.get(key) {
                self.draft[key] = value.clone();
            }
        }
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
        if resolution_changed {
            self.schedule_camera_media_restart(cx);
        }
    }

    fn footer_option(&self, key: &str) -> String {
        self.control(key)
            .and_then(|control| {
                control
                    .options
                    .iter()
                    .find(|o| self.value(control) == Some(&o.value))
            })
            .map(|option| option.label.clone())
            .unwrap_or_default()
    }

    pub(super) fn render_camera_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.footer_disabled();
        let preview = self
            .camera_preview
            .as_ref()
            .is_some_and(|s| s.enabled == Some(true));
        let zoom = self.axis_value("@view/zoom");
        let zoom_key = (f64::from(zoom) * 100.).round() as i64;
        let ldc = self.draft["camera"]["ldc"] == true;
        let fov_table = match self.spec.product_id {
            3595 | 3596 => data::fov(self.spec.product_id, ldc),
            _ => FOV,
        };
        let fov = fov_table
            .iter()
            .find(|(key, _)| *key == zoom_key)
            .map(|(_, v)| *v)
            .unwrap_or("");
        let hdr = self
            .draft
            .pointer("/camera/hdr/isEnabled")
            .and_then(Value::as_bool)
            == Some(true);
        let framing = self
            .draft
            .pointer("/camera/autoFraming/isEnabled")
            .and_then(Value::as_bool)
            == Some(true);
        let mut resolution = self.footer_option(&format!("{}:resolution", self.spec.product_id));
        if (hdr || framing)
            && self
                .draft
                .pointer("/camera/resolution/fps")
                .and_then(Value::as_u64)
                .is_some_and(|fps| fps > 30)
        {
            if let Some((name, _)) = resolution.split_once(' ') {
                resolution = format!("{name} 30FPS");
            }
        }
        let dnr = ["state2D", "state3D"]
            .into_iter()
            .zip(["2D", "3D"])
            .filter(|(key, _)| {
                self.draft["camera"]["dynamicNoiseReduction"][key].as_bool() == Some(true)
            })
            .map(|(_, label)| label)
            .collect::<Vec<_>>()
            .join("&");
        let mut labels = vec![resolution, format!("{zoom}x")];
        // V2 supplies disabledFOV=autoFraming.isEnabled; X supplies no gate.
        if self.spec.product_id == 3596 || !framing {
            labels.push(format!("FOV {fov}°"));
        }
        labels.extend([
            format!(
                "ISO {}",
                if self.spec.product_id == 3594 {
                    self.footer_option("3594:iso")
                } else {
                    data::iso(
                        self.camera_source
                            .auto_iso
                            .as_ref()
                            .or_else(|| self.draft.pointer("/camera/autoExposure/isoSensitivity")),
                    )
                    .into()
                }
            ),
            format!(
                "SS {}s",
                if self.spec.product_id == 3594 {
                    self.footer_option("3594:shutter")
                } else {
                    data::shutter(self.draft.pointer("/camera/autoExposure/shutterSpeed")).into()
                }
            ),
            format!("DNR {dnr}"),
        ]);
        h_flex()
            .w_full()
            .h(surface::css(45.))
            .flex_shrink_0()
            .bg(super::camera_theme::preview_background())
            .font_family("Roboto")
            .text_color(Colors::text())
            .child(
                h_flex()
                    .w(surface::css(400.))
                    .h_full()
                    .flex_shrink_0()
                    .items_center()
                    .justify_end()
                    .when(disabled, |view| view.opacity(0.3))
                    .child(
                        gpui_kit::base::Button::new("camera-reset-all")
                            .accessibility_label(razer_i18n::t("RESET_ALL_SETTING"))
                            .p_0()
                            .size(surface::css(16.))
                            .mr(surface::css(10.))
                            .disabled(disabled)
                            .cursor_pointer()
                            .hover(|button| button.opacity(0.7))
                            .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                            .child(img("synapse/camera-section-reset.svg").size_full())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.reset_camera(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .mr(surface::css(10.))
                            .child(super::camera_sections::help(
                                "camera-save-help".into(),
                                razer_i18n::t("SAVE_SETTINGS_TOOLTIP_CONTENT"),
                                false,
                            )),
                    )
                    .child(
                        gpui_kit::base::Button::new("camera-save-flash")
                            .accessibility_label(razer_i18n::t("SAVE"))
                            .disabled(disabled || !self.camera_footer.changed)
                            .h(surface::css(27.))
                            .min_w(surface::css(90.))
                            .px(surface::css(16.))
                            .pt(surface::css(7.))
                            .pb(surface::css(6.))
                            .mr(surface::css(12.))
                            .rounded(surface::css(3.))
                            .bg(Colors::focus())
                            .text_color(Colors::background())
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .when(!self.camera_footer.changed, |button| button.opacity(0.3))
                            .cursor_pointer()
                            .hover(|button| button.bg(super::camera_theme::save_hover()))
                            .focus_visible(|button| button.border_1().border_color(Colors::text()))
                            .child(razer_i18n::t("SAVE").to_uppercase())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.camera_footer.changed = false;
                                this.pause_camera_media_for_save(cx);
                                this.request_camera_edit(
                                    &format!("{}:save-camera", this.spec.product_id),
                                    window,
                                    cx,
                                );
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .justify_between()
                    .px(surface::css(10.))
                    .child(
                        h_flex()
                            .items_center()
                            .children(labels.into_iter().map(|label| {
                                div()
                                    .m(surface::css(10.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(16.))
                                    .child(label.to_uppercase())
                            })),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .child(
                                div()
                                    .m(surface::css(10.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(16.))
                                    .child(razer_i18n::t("PREVIEW").to_uppercase()),
                            )
                            .child(
                                surface::SynapseSwitch::new("camera-footer-preview")
                                    .checked(preview)
                                    .accessibility_label(razer_i18n::t("PREVIEW"))
                                    .on_change(cx.listener(|this, enabled, _, cx| {
                                        if *enabled {
                                            this.camera_source.request_footer_preview();
                                            this.request_camera_cache(true, Duration::ZERO);
                                        } else {
                                            this.set_camera_preview_enabled(false, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
