//! Current host local fallback flags and the application's confirmed binding cache.
use super::*;
use serde_json::Value;

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub(super) fn multipairing_enabled(features: Option<&Value>) -> bool {
    features
        .and_then(|features| features.get("global.multipairing"))
        .and_then(|feature| feature.get("provisionFlags"))
        .is_some_and(truthy)
}

impl AppShell {
    pub(super) fn load_local_pairing_context(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let (features, records) = cx
                .background_spawn(async {
                    let features = razer_storage::app_features::installed_path()
                        .and_then(|path| razer_storage::app_features::read(&path));
                    let records = razer_storage::receiver_pairing::PairingCache::load(
                        &razer_storage::receiver_pairing::local_path(),
                    )
                    .map(|cache| Value::Array(cache.ordered_records()));
                    (features, records)
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                match features {
                    Ok(value) => this.local_app_features = value,
                    Err(error) => eprintln!("[local app features] {error:#}"),
                }
                match records {
                    Ok(value) => this.local_pairing_records = Some(value),
                    Err(error) => eprintln!("[local pairing cache] {error:#}"),
                }
                this.sync_known_devices(cx);
            });
        })
        .detach();
    }
}
