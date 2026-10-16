//! Each current MapDefault's imported localization and DEFAULTPROFILE policy.
//! A missing `dpiStages` property is a source mismatch, never filled from drafts.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::theme::{MouseCustomizeColors, MouseMappingColors};

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    labels: BTreeMap<String, String>,
    supports_boss_key: bool,
    stage_keys: Vec<String>,
    stage_values: Option<Vec<Value>>,
    configure_key: String,
    target: String,
    ai_labels: Vec<String>,
}

fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| {
            serde_json::from_str(include_str!("mouse_mapping_default_current_data.json"))
                .expect("audited current MapDefault policies")
        })
        .iter()
        .find(|policy| policy.product_id == product_id)
}

impl MouseProductWorkspace {
    pub(super) fn current_mouse_default_body(
        &self,
        button: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(policy) = policy(self.spec.product_id) else {
            return div().into_any_element();
        };
        // qt passes activeButton.defaultValue || "" to MapDefault.keyName.
        let name = button["defaultValue"].as_str().unwrap_or("");
        let normalized = name
            .replace([' ', '/', '\t', '\r', '\n'], "_")
            .to_uppercase();
        let label = if policy.supports_boss_key && normalized == "RAZER_HYPERSHIFT" {
            name.to_owned()
        } else if let Some(key) = policy.labels.get(&normalized) {
            t(key)
        } else {
            name.to_owned()
        };
        let mut body = v_flex().w(surface::css(210.)).font_family("Roboto").child(
            div()
                .w_full()
                .border_1()
                .border_color(MouseCustomizeColors::line())
                .p(surface::css(5.))
                .mb(surface::css(20.))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(MouseCustomizeColors::text())
                .text_center()
                .overflow_hidden()
                .child(label),
        );
        if button["buttonKey"] == "CycleUpSensitivityStages" {
            // The original uses the imported default profile, not the live
            // profile. Older CONFIGs omit the lowercase field it reads.
            if let Some(values) = &policy.stage_values {
                let target = policy.target.clone();
                body = body.child(
                    v_flex()
                        .h(surface::css(140.))
                        .mb(surface::css(20.))
                        .justify_between()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .children(policy.stage_keys.iter().zip(values).map(|(key, value)| {
                            div()
                                .text_color(MouseMappingColors::description())
                                .child(format!(
                                    "{} - {}",
                                    t(key),
                                    value.as_f64().map(|v| v.to_string()).unwrap_or_default()
                                ))
                        }))
                        .child(
                            BaseButton::new("mouse-current-default-configure-sensitivity")
                                .mt(surface::css(20.))
                                .mb(surface::css(10.))
                                .p_0()
                                .text_color(MouseCustomizeColors::text())
                                .cursor_pointer()
                                .child(div().underline().child(t(&policy.configure_key)))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_page(&target, window, cx);
                                })),
                        ),
                );
            }
        }
        if button["buttonKey"] == "RazerAI" {
            let mut description = v_flex()
                .text_color(MouseMappingColors::description())
                .text_size(surface::css(14.))
                .line_height(surface::css(17.));
            let has_description =
                button
                    .get("buttonDescription")
                    .is_some_and(|value| match value {
                        Value::Null => false,
                        Value::Bool(value) => *value,
                        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
                        Value::String(value) => !value.is_empty(),
                        _ => true,
                    });
            if has_description {
                description = description.child(
                    h_flex()
                        .mb(surface::css(15.))
                        .gap(surface::css(6.))
                        .items_start()
                        .child(
                            img("synapse/mouse-default-ai-long-press.svg")
                                .size(surface::css(18.))
                                .flex_shrink_0(),
                        )
                        .child(t(&policy.ai_labels[0])),
                );
            }
            description = description
                .child(div().mb(surface::css(15.)).child(t(&policy.ai_labels[1])))
                .child(div().child(t(&policy.ai_labels[2])));
            body = body
                .child(description)
                .child(
                    img("synapse/keyboard-mapping-ai/ai-launcher.5b68ac10.svg")
                        .w(surface::css(210.))
                        .h(surface::css(120.))
                        .my(surface::css(20.))
                        .object_fit(ObjectFit::Contain),
                )
                .children(self.current_mouse_synapse_notice());
        }
        body.into_any_element()
    }
}
