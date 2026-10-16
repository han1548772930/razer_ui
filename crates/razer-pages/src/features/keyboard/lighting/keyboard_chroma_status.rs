//! Current Effects footer for the application controlling Chroma lighting.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::{source_tooltip::SourceTooltip, theme::KeyboardRgbEffectColors as Colors};

fn source_app_name(name: &str) -> String {
    name.to_lowercase()
        .split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .replace(".exe", "")
}

impl KeyboardProductWorkspace {
    pub(super) fn chroma_status(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let observation = self.lighting_observation.as_ref()?;
        if !observation.is_chroma_visualizer_enabled || self.rgb_bluetooth_keyboard() {
            return None;
        }
        let app = observation.chroma_connect_app.as_ref();
        let name = app
            .and_then(|app| app["appName"].as_str())
            .unwrap_or_default();
        let icon = if name == "Chroma Visualizer App" {
            Some("synapse/keyboard-logo-visualizer.svg")
        } else {
            app.and_then(|app| app["appIcon"].as_str())
        };
        let manage = BaseButton::new("keyboard-manage-chroma-apps")
            .disabled(
                self.lighting_controls_disabled() || observation.nano_leaf_enabled == Some(false),
            )
            .size(surface::css(14.))
            .p_0()
            .cursor_pointer()
            .accessibility_label(t("MANAGE_CHROMA_APPS"))
            .child(img("synapse/keyboard-chroma-settings.svg").size(surface::css(14.)))
            .on_click(cx.listener(|this, _, _, cx| {
                if this.lighting_controls_disabled() {
                    return;
                }
                let Some(observation) = &this.lighting_observation else {
                    return;
                };
                if observation.nano_leaf_enabled == Some(false) {
                    return;
                }
                this.request_lighting_command(
                    KeyboardLightingCommand::ManageChromaApps {
                        install_required: observation.check_is_fully_installed != Some(true)
                            || !observation.chroma_installed_module,
                    },
                    cx,
                );
            }));
        Some(
            div()
                .relative()
                .mt(surface::css(20.))
                .w_full()
                .h(surface::css(45.))
                .rounded(surface::css(5.))
                .bg(Colors::application_background())
                .text_size(surface::css(14.))
                .text_center()
                .children(icon.map(|icon| {
                    img(SharedString::from(icon.to_owned()))
                        .absolute()
                        .left(surface::css(10.))
                        .top(surface::css(10.))
                        .size(surface::css(25.))
                }))
                .child(
                    v_flex()
                        .w(surface::css(500.))
                        .h_full()
                        .justify_center()
                        .child(
                            div()
                                .text_color(Colors::application_label())
                                .child(format!("{}:", t("APP_IN_USE"))),
                        )
                        .child(div().text_color(Colors::text()).child(format!(
                            "{} ({})",
                            source_app_name(name),
                            t("CHROMA_APPS")
                        ))),
                )
                .child(
                    div()
                        .absolute()
                        .right(surface::css(30.))
                        .top(surface::css(15.))
                        .child(
                            SourceTooltip::new(
                                "keyboard-chroma-apps-manage-tip",
                                t("MANAGE_CHROMA_APPS"),
                                165.,
                            )
                            .trigger(move |_, _, _| manage.into_any_element()),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .right(surface::css(10.))
                        .top(surface::css(15.))
                        .child(
                            SourceTooltip::new(
                                "keyboard-chroma-apps-tip",
                                format!(
                                    "{}. \n{}.",
                                    t("CHROMA_INDICATOR_TIP_1"),
                                    t("CHROMA_INDICATOR_TIP_2")
                                ),
                                410.,
                            )
                            .trigger(|_, _, _| {
                                BaseButton::new("keyboard-chroma-apps-help")
                                    .size(surface::css(14.))
                                    .p_0()
                                    .rounded_full()
                                    .bg(Colors::help_background())
                                    .hover(|button| {
                                        button.cursor_pointer().bg(Colors::help_hover())
                                    })
                                    .child(
                                        img("synapse/automation-tooltip_questionmark.svg")
                                            .size_full(),
                                    )
                                    .into_any_element()
                            }),
                        ),
                )
                .into_any_element(),
        )
    }
}
