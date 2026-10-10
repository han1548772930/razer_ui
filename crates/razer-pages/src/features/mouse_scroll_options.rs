//! Product 167 OS / scrollOptionsReducer. Source defaults are not device reads.
use super::*;
use gpui_kit::base::Button as BaseButton;

const LOCAL: &str = "_scrollOptionsLocalV1";

impl MouseProductWorkspace {
    fn scroll_option(&self, key: &str) -> bool {
        self.draft
            .get(LOCAL)
            .and_then(|v| v.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    fn write_scroll_option(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.write(&format!("/{LOCAL}/{key}"), value, cx);
    }
    fn add_scroll_application(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scroll_application_picker || !self.scroll_option("browsingModeEnabled") {
            return;
        }
        self.scroll_application_picker = true;
        let generation = self.draft_generation;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t("ADD_AN_APPLICATION").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = picker.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.scroll_application_picker = false;
                if this.draft_generation != generation
                    || !this.active
                    || this.page != "TAB_SCROLLING"
                    || !this.scroll_option("browsingModeEnabled")
                {
                    return;
                }
                if let Ok(Ok(Some(paths))) = result {
                    let normalize = |s: &str| s.replace(' ', "").to_lowercase();
                    let mut apps = this
                        .draft
                        .get(LOCAL)
                        .and_then(|v| v.get("browsingModeApplicationList"))
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let previous = apps.len();
                    for path in paths {
                        // The original host dialog accepts .exe; GPUI's dialog has
                        // no extension filter, so enforce it before retaining data.
                        if !path
                            .extension()
                            .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
                        {
                            continue;
                        }
                        let path = path.to_string_lossy().into_owned();
                        let name = path
                            .rsplit(['\\', '/'])
                            .next()
                            .unwrap_or(&path)
                            .replacen(".exe", "", 1);
                        if apps.iter().any(|app| {
                            normalize(app["path"].as_str().unwrap_or("")) == normalize(&path)
                                || normalize(app["name"].as_str().unwrap_or("")) == normalize(&name)
                        }) {
                            continue;
                        }
                        // Source getApplicationIcons supplies logo bytes. A missing
                        // real icon query remains absent; never invent a host result.
                        apps.push(json!({"path":path,"name":name}));
                    }
                    if apps.len() != previous {
                        this.write_scroll_option("browsingModeApplicationList", json!(apps), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn scroll_options_panel(&self, stage: &str, cx: &Context<Self>) -> AnyElement {
        let browsing = self.scroll_option("browsingModeEnabled");
        let acceleration_locked = stage != "SW_SMOOTH_SCROLL";
        let mut panel = surface::panel_with_control(
            t("SCROLL_WHEEL_OPTION_HEADER"),
            surface::help_control(
                "mouse-scroll-options-help",
                t("SCROLL_WHEEL_OPTION_TOOLTIP"),
            ),
            cx,
        );
        for (field, key, locked) in [
            (
                "scrollAccelerationEnabled",
                "SCROLL_WHEEL_ACCELERATION",
                acceleration_locked,
            ),
            ("browsingModeEnabled", "BROWSER_DETECTION", false),
        ] {
            panel = panel.child(
                v_flex()
                    .mt(surface::css(if field == "scrollAccelerationEnabled" {
                        0.
                    } else {
                        10.
                    }))
                    .child(
                        h_flex()
                            .items_center()
                            .mb(surface::css(6.))
                            .child(
                                div()
                                    .mr(surface::css(10.))
                                    .opacity(if locked { 0.3 } else { 1. })
                                    .child(t(key)),
                            )
                            .child(
                                surface::SynapseSwitch::new(SharedString::from(format!(
                                    "mouse-scroll-option-{field}"
                                )))
                                .accessibility_label(t(key))
                                .checked(self.scroll_option(field))
                                .disabled(locked)
                                .on_change(cx.listener(
                                    move |this, value: &bool, _, cx| {
                                        if field == "scrollAccelerationEnabled"
                                            && this
                                                .draft
                                                .pointer("/scrollWheelStages/activeStage")
                                                .and_then(Value::as_str)
                                                != Some("SW_SMOOTH_SCROLL")
                                        {
                                            return;
                                        }
                                        this.write_scroll_option(field, json!(*value), cx);
                                    },
                                )),
                            )
                            .when(field == "scrollAccelerationEnabled", |v| {
                                v.child(div().ml(surface::css(10.)).child(surface::help_control(
                                    "mouse-scroll-acceleration-help",
                                    t("SCROLL_ACCELERATION_TOOLTIP"),
                                )))
                            }),
                    )
                    .child(
                        div()
                            .text_size(surface::css(13.))
                            .line_height(surface::css(15.))
                            .text_color(cx.theme().muted_foreground)
                            .opacity(if locked { 0.3 } else { 1. })
                            .child(t(&format!("{key}_DESC"))),
                    ),
            );
        }
        if browsing {
            let apps = self
                .draft
                .get(LOCAL)
                .and_then(|v| v.get("browsingModeApplicationList"))
                .and_then(Value::as_array);
            panel = panel
                .child(
                    h_flex()
                        .items_center()
                        .my(surface::css(20.))
                        .child(
                            Checkbox::new("mouse-browser-high-resolution")
                                .label(t("HIGH_RESOLUTION_SCROLLING"))
                                .checked(self.scroll_option("isHighResolutionScrolling"))
                                .on_click(cx.listener(|this, checked, _, cx| {
                                    if this.scroll_option("browsingModeEnabled") {
                                        this.write_scroll_option(
                                            "isHighResolutionScrolling",
                                            json!(*checked),
                                            cx,
                                        );
                                    }
                                })),
                        )
                        .child(div().ml(surface::css(10.)).child(surface::help_control(
                            "mouse-browser-high-resolution-help",
                            t("HIGH_RESOLUTION_SCROLLING_TOOLTIP"),
                        ))),
                )
                .child(
                    BaseButton::new("mouse-browser-add-application")
                        .disabled(self.scroll_application_picker)
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .size(surface::css(30.))
                                .mr(surface::css(10.))
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded(surface::css(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child("+"),
                        )
                        .child(t("ADD_AN_APPLICATION"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_scroll_application(window, cx)
                        })),
                )
                .children(apps.into_iter().flatten().enumerate().map(|(ix, app)| {
                    BaseButton::new(SharedString::from(format!(
                        "mouse-browser-application-{ix}"
                    )))
                    .flex()
                    .items_center()
                    .mt(surface::css(10.))
                    .child(div().size(surface::css(30.)).mr(surface::css(10.)))
                    .child(app["name"].as_str().unwrap_or("").to_owned())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.scroll_option("browsingModeEnabled") {
                            return;
                        }
                        let mut apps = this
                            .draft
                            .get(LOCAL)
                            .and_then(|v| v.get("browsingModeApplicationList"))
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        if ix < apps.len() {
                            apps.remove(ix);
                            this.write_scroll_option(
                                "browsingModeApplicationList",
                                json!(apps),
                                cx,
                            );
                        }
                    }))
                }));
        }
        panel.into_any_element()
    }
}
