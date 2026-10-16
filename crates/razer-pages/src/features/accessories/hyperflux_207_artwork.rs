//! PID 207 main.ca032f7b's aM and Km PluginImages producers. Their 3x
//! requests are local resources; neither producer has an edition/image retry.
use super::*;

fn image_id(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

pub(super) fn paired_mouse_image(device: &Value) -> Option<&'static str> {
    let pid = image_id(&device["productId"])?;
    let edition = image_id(&device["editionId"])?;
    // aM uses layoutId || 0 and the same dashboard3x asset for both srcSet
    // density labels. Display scaling never changes this request to 1x.
    let layout = match device.get("layoutId").filter(|value| truthy(value)) {
        Some(value) => image_id(value)?,
        None => 0,
    };
    razer_assets::dashboard_plugin_image(pid, edition, edition, layout, false, 3)
}

pub(super) fn pairing_device_image(device: &Value) -> Option<&'static str> {
    let pid = image_id(&device["productId"])?;
    let edition = image_id(&device["editionId"])?;
    // Km defaults only undefined layouts. An explicit zero or null retains
    // its own failed image request, including for keyboards.
    let layout = match device.get("layoutId") {
        Some(value) => image_id(value)?,
        None if device["category"] == "KEYBOARD" => 1,
        None => 0,
    };
    razer_assets::dashboard_plugin_image(pid, edition, edition, layout, false, 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn pairing_keyboard_defaults_only_an_absent_layout() {
        let mut keyboard = serde_json::json!({
            "productId": 756, "editionId": 0, "category": "KEYBOARD"
        });
        assert_eq!(
            pairing_device_image(&keyboard),
            Some("synapse/current-dashboard-756-0-1-product-3x.png")
        );
        keyboard["layoutId"] = serde_json::json!(0);
        assert!(pairing_device_image(&keyboard).is_none());
        keyboard["layoutId"] = Value::Null;
        assert!(pairing_device_image(&keyboard).is_none());
        keyboard["layoutId"] = serde_json::json!(1);
        keyboard["editionId"] = serde_json::json!(999);
        assert!(pairing_device_image(&keyboard).is_none());
    }

    #[core::prelude::v1::test]
    fn paired_mouse_keeps_the_source_edition_and_three_x_request() {
        let mouse = serde_json::json!({ "productId": "190", "editionId": "128" });
        assert_eq!(
            paired_mouse_image(&mouse),
            Some("synapse/current-dashboard-190-128-0-product-3x.png")
        );
        assert_eq!(
            paired_mouse_image(&serde_json::json!({
                "productId": 190, "editionId": 128, "layoutId": null
            })),
            paired_mouse_image(&mouse)
        );
        assert!(paired_mouse_image(&serde_json::json!({ "productId": 190 })).is_none());
    }
}
