//! Current camera wrapper mount, title controls and conditional help. The
//! generated groups hold each source row's children and effective mount props.
use super::*;
use gpui_kit::base::{
    Collapsible,
    motion::{self, Easing, Transition},
};
use razer_widgets::source_tooltip::SourceTooltip;
use std::time::Duration;

#[derive(Deserialize)]
pub(super) struct CameraGroup {
    key: String,
    title: String,
    controls: Vec<String>,
    collapsible: bool,
    #[serde(default)]
    presentation: Option<String>,
    #[serde(default)]
    divider: Option<String>,
    #[serde(default)]
    header_switch: Option<String>,
    #[serde(default)]
    header_stepper: Option<String>,
    #[serde(default)]
    header_checkbox: Option<String>,
    #[serde(default)]
    header_reset: Option<String>,
    #[serde(default)]
    tooltip: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    warning_any: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    description_when_enabled: Option<String>,
    #[serde(default)]
    description_min_height: Option<u64>,
}

fn help(id: SharedString, text: String, warning: bool) -> AnyElement {
    let animation_id = id.clone();
    // Current Pa / module 1050 portals `.drop-tips`, measuring `.tip` at
    // 210px (280px in mode-name-s2) and copying its client dimensions.
    SourceTooltip::new(id, text.clone(), if warning { 280. } else { 210. })
        // `.drop-tips.on{z-index:9999}` in all four current camera bundles.
        .hovered_priority(9999)
        .trigger(move |hovered, window, cx| {
            let size = if warning { 20. } else { 14. };
            let background = motion::transition(
                (
                    ElementId::from(animation_id.clone()),
                    "camera-help-background",
                ),
                if hovered {
                    super::camera_theme::help_hover()
                } else {
                    super::camera_theme::help()
                },
                Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            );
            gpui_kit::base::Button::new((ElementId::from(animation_id.clone()), "icon"))
                .accessibility_label(text.clone())
                .w(surface::css(size))
                .h(surface::css(size))
                .p_0()
                .cursor_pointer()
                .when(!warning, |button| {
                    button.rounded(surface::css(7.)).bg(background)
                })
                .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                .child(
                    img(if warning {
                        "synapse/camera-section-warning.svg"
                    } else {
                        "synapse/camera-section-help.svg"
                    })
                    .size_full(),
                )
                .into_any_element()
        })
        .into_any_element()
}

impl SourceControls {
    pub(super) fn render_camera_group(
        &self,
        group: &CameraGroup,
        separator: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use razer_widgets::theme::CameraProductColors as Colors;
        let open = !self.collapsed_camera_groups.contains(&group.key);
        let collapse_key = group.key.clone();
        let title = razer_i18n::t(&group.title).to_uppercase();
        let header_label = div()
            .flex()
            .items_center()
            .when(group.collapsible, |view| {
                view.child(
                    Icon::default()
                        .path("synapse/camera-section-down.svg")
                        .size(surface::css(10.))
                        .mr(surface::css(10.))
                        .text_color(Colors::text())
                        .transform(Transformation::rotate(radians(if open {
                            0.
                        } else {
                            -std::f32::consts::FRAC_PI_2
                        }))),
                )
            })
            .child(title.clone());
        let mut header = h_flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(if group.collapsible {
                gpui_kit::base::Button::new(SharedString::from(format!("{}:toggle", group.key)))
                    .accessibility_label(title)
                    .p_0()
                    .cursor_pointer()
                    .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                    .child(header_label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.collapsed_camera_groups.remove(&collapse_key) {
                            this.collapsed_camera_groups.insert(collapse_key.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                header_label.into_any_element()
            });
        if let Some(control) = group
            .header_switch
            .as_deref()
            .and_then(|key| self.control(key))
        {
            let key = control.key.clone();
            header = header.child(
                div().ml(surface::css(10.)).child(
                    surface::SynapseSwitch::new(SharedString::from(format!("{key}:header")))
                        .accessibility_label(razer_i18n::t(&control.label))
                        .checked(
                            self.value(control)
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                        .disabled(self.disabled(control))
                        .on_change(cx.listener(move |this, next, window, cx| {
                            this.edit(&key, Value::Bool(*next), window, cx)
                        })),
                ),
            );
        }
        if let Some(control) = group
            .header_reset
            .as_deref()
            .and_then(|key| self.control(key))
        {
            let key = control.key.clone();
            header = header.child(
                gpui_kit::base::Button::new(SharedString::from(format!("{key}:header")))
                    .accessibility_label(razer_i18n::t(&control.label))
                    .ml(surface::css(10.))
                    .w(surface::css(20.))
                    .h(surface::css(20.))
                    .p_0()
                    .disabled(self.disabled(control))
                    .cursor_pointer()
                    .hover(|button| button.opacity(0.7))
                    .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                    .child(img("synapse/camera-section-reset.svg").size_full())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(&key, Value::Null, window, cx)
                    })),
            );
        }
        if let Some(text) = &group.tooltip {
            header = header.child(
                div()
                    .ml(surface::css(10.))
                    .when(
                        group.presentation.as_deref() == Some("preview-source"),
                        |view| view.mr(surface::css(25.)),
                    )
                    .child(help(
                        SharedString::from(format!("{}:help", group.key)),
                        razer_i18n::t(text),
                        false,
                    )),
            );
        }
        if group.presentation.as_deref() == Some("preview-source") {
            header = header.child(self.render_camera_preview_actions(cx));
        }
        if group
            .warning_any
            .iter()
            .any(|path| self.draft.pointer(path).and_then(Value::as_bool) == Some(true))
        {
            if let Some(text) = &group.warning {
                header = header.child(div().ml_auto().child(help(
                    SharedString::from(format!("{}:warning", group.key)),
                    razer_i18n::t(text),
                    true,
                )));
            }
        }
        if let Some(stepper) = group
            .header_stepper
            .as_deref()
            .and_then(|key| self.steppers.get(key))
        {
            header = header.child(div().ml(surface::css(10.)).child(stepper.clone()));
        }
        if let Some(control) = group
            .header_checkbox
            .as_deref()
            .and_then(|key| self.control(key))
        {
            header = header.child(
                div()
                    .ml(surface::css(10.))
                    .mt(surface::css(3.))
                    .child(self.render_control(control, window, cx)),
            );
        }
        let mut content = v_flex().gap(surface::css(10.));
        let mut has_content = false;
        if group.presentation.as_deref() == Some("preview-source") {
            content = content.child(self.render_camera_source_select());
            has_content = true;
        } else if group.presentation.as_deref() == Some("camera-promo") {
            content = content.child(self.render_camera_promo(cx));
            has_content = true;
        }
        for key in &group.controls {
            if [
                group.header_switch.as_ref(),
                group.header_checkbox.as_ref(),
                group.header_reset.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|header| header == key)
            {
                continue;
            }
            let Some(control) = self.control(key) else {
                continue;
            };
            if control.visible_when.as_ref().is_some_and(|condition| {
                self.draft.pointer(&self.resolve_path(&condition.path)) != Some(&condition.value)
            }) {
                continue;
            }
            content = content.child(self.render_control_with_label(
                control,
                control.label != group.title,
                window,
                cx,
            ));
            has_content = true;
        }
        let description_visible = group
            .description_when_enabled
            .as_ref()
            .is_none_or(|path| self.draft.pointer(path).and_then(Value::as_bool) == Some(true))
            && group.description_min_height.is_none_or(|minimum| {
                self.draft
                    .pointer("/camera/resolution/height")
                    .and_then(Value::as_u64)
                    .is_some_and(|height| height >= minimum)
            });
        if description_visible {
            if let Some(description) = &group.description {
                content = content.child(
                    div()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .child(razer_i18n::t(description)),
                );
                has_content = true;
            }
        }
        let wrapper = Collapsible::new()
            .open(open)
            .child(header)
            .when(has_content, |wrapper| {
                wrapper.content(content.mt(surface::css(10.)))
            });
        v_flex()
            .when(
                separator && group.divider.as_deref() != Some("none"),
                |view| {
                    view.child(
                        div()
                            .w_full()
                            .my(surface::css(20.))
                            .border_1()
                            .border_color(if group.divider.as_deref() == Some("transparent") {
                                cx.theme().transparent
                            } else {
                                Colors::divider()
                            })
                            .rounded(surface::css(2.)),
                    )
                },
            )
            .child(wrapper)
            .into_any_element()
    }
}
