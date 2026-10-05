//! Host TabUI consumes the page's first favicon, independent of module logos.
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) fn device_favicon(pid: u32) -> Option<&'static str> {
    static ICONS: OnceLock<BTreeMap<String, Option<String>>> = OnceLock::new();
    ICONS
        .get_or_init(|| {
            serde_json::from_str(include_str!("../host_device_favicons.json"))
                .expect("current product HTML favicon receipts")
        })
        .get(&pid.to_string())
        .and_then(|icon| icon.as_deref())
}
