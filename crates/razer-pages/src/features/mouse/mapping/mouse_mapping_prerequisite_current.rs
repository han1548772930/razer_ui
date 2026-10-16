//! Mounted current RequireSynapse card and its outer MapFunction gate.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::{
    source_tooltip::SourceTooltip,
    theme::{MouseCustomizeColors, MouseMappingColors},
};

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    margin_bottom: f32,
    is_obm: bool,
    forced_keys: Vec<String>,
}

fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| crate::data::mouse_mapping_prerequisite_current_data!())
        .iter()
        .find(|policy| policy.product_id == product_id)
}

impl MouseProductWorkspace {
    pub(super) fn current_mouse_prerequisite(
        &self,
        button: &Value,
        reset: bool,
    ) -> Option<AnyElement> {
        let policy = policy(self.spec.product_id)?;
        // Child editors supply the current requirement observation. MapDefault's
        // AI subcomponent mounts its own notice separately.
        if reset
            || policy.is_obm
                && !self.current_mapping_requires_synapse()
                && !policy
                    .forced_keys
                    .iter()
                    .any(|key| button["buttonKey"] == *key)
        {
            return None;
        }
        self.current_mouse_synapse_notice()
    }

    /// MapDefault's AI description mounts this component without the outer
    /// MapFunction predicate; preserve that separate source mount.
    pub(super) fn current_mouse_synapse_notice(&self) -> Option<AnyElement> {
        let policy = policy(self.spec.product_id)?;
        let label = t("REQUIRES_RAZER_SYNAPSE");
        let label = if label.ends_with("Razer Synapse") {
            format!(
                "{}\nRazer Synapse",
                label.split("Razer Synapse").next().unwrap_or("")
            )
        } else {
            label
        };
        Some(
            h_flex()
                .w(surface::css(210.))
                .h(surface::css(44.))
                .mt(surface::css(10.))
                .mb(surface::css(policy.margin_bottom))
                .border_1()
                .border_color(MouseMappingColors::prerequisite())
                .rounded(surface::css(3.))
                .items_center()
                .font_family("Roboto")
                .child(
                    img("synapse/keyboard-mapping-notices/synapse.svg")
                        .size(surface::css(24.))
                        .ml(surface::css(10.))
                        .flex_shrink_0(),
                )
                .child(
                    div()
                        .w(surface::css(142.))
                        .mx(surface::css(5.))
                        .flex_shrink_0()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(14.))
                        .text_color(MouseMappingColors::prerequisite())
                        .child(label),
                )
                .child(
                    div().mr(surface::css(10.)).child(
                        SourceTooltip::new(
                            "mouse-current-map-prerequisite-tip",
                            t("REQUIRES_RAZER_SYNAPSE_TOOLTIP"),
                            250.,
                        )
                        .hovered_priority(9999)
                        .trigger(|hovered, window, cx| {
                            BaseButton::new("mouse-current-map-prerequisite-help")
                                .size(surface::css(14.))
                                .p_0()
                                .rounded_full()
                                .bg(surface::fade_color(
                                    "mouse-current-map-prerequisite-help-color",
                                    if hovered {
                                        MouseMappingColors::help_hover()
                                    } else {
                                        MouseMappingColors::prerequisite_help()
                                    },
                                    300,
                                    window,
                                    cx,
                                ))
                                .text_color(MouseCustomizeColors::text())
                                .child(
                                    img("synapse/automation-tooltip_questionmark.svg").size_full(),
                                )
                                .into_any_element()
                        }),
                    ),
                )
                .into_any_element(),
        )
    }
}
