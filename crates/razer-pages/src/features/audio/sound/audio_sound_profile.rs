//! Active local profile -> current Sound spatial reducer payload.
//! The original producers copy the complete stored value; no native observation
//! or acknowledgement supplies this UI state. See the independent receipts in
//! `docs/re/evidence/thx-profile-spatial-source-current.json`.
use serde_json::Value;

fn stored_spatial(saved: Option<&Value>) -> Option<&Value> {
    let saved = saved?;
    // Workspace snapshots use {profile:{...}}. Original local source profiles
    // use the profile object directly. Do not fall through a present wrapper
    // to a conflicting top-level field.
    saved.get("profile").unwrap_or(saved).get("thxSpatialAudio")
}

pub(super) fn restore_profile_spatial(draft: &mut Value, saved: Option<&Value>) {
    let Some(stored) = stored_spatial(saved) else {
        // Original undefined guard emits no replacement. A cold owner keeps
        // its initial Sound payload; an existing reducer keeps its current one.
        return;
    };
    draft["sound"]["thxSpatialAudio"] = stored.clone();
    // Generic schema merging would reintroduce default `mode` after the
    // original updateStorage replaced this field with {isEnabled}.
    draft["profile"]["thxSpatialAudio"] = stored.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn initial() -> Value {
        json!({"isEnabled":true,"mode":"auto"})
    }

    fn retained(enabled: bool) -> Value {
        json!({
            "profile":{"thxSpatialAudio":{"isEnabled":enabled,"mode":"auto"}},
            "sound":{
                "thxSpatialAudio":{"isEnabled":enabled,"mode":"auto"},
                "cloudEQ":{"selectedPresetIndex":3},
                "gameChatBalance":{"balanceMode":10}
            },
            "device":{"volume":{"isEnabled":true,"value":75}}
        })
    }

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn stored_active_profile_replaces_previous_profile_spatial_state() {
        let mut draft = retained(true);
        let saved = json!({
            "profile":{"thxSpatialAudio":{"isEnabled":false}},
            "sound":{"thxSpatialAudio":{"isEnabled":true,"mode":"stale"}}
        });
        restore_profile_spatial(&mut draft, Some(&saved));
        assert_eq!(
            draft["sound"]["thxSpatialAudio"],
            json!({"isEnabled":false})
        );
        assert_eq!(
            draft["profile"]["thxSpatialAudio"],
            json!({"isEnabled":false})
        );
        assert_eq!(draft["device"]["volume"]["value"], json!(75));
        assert_eq!(draft["sound"]["cloudEQ"]["selectedPresetIndex"], json!(3));
        assert_eq!(draft["sound"]["gameChatBalance"]["balanceMode"], json!(10));
    }

    #[test]
    fn complete_stored_payload_preserves_mode_and_unknown_fields() {
        let mut draft = retained(false);
        let payload = json!({
            "isEnabled":true,"mode":"stored-mode",
            "extra":{"channel":4,"values":[1,2,3]},"sourceFlag":null
        });
        let saved = json!({"profile":{"thxSpatialAudio":payload}});
        restore_profile_spatial(&mut draft, Some(&saved));
        assert_eq!(draft["sound"]["thxSpatialAudio"], payload);
        assert_eq!(draft["profile"]["thxSpatialAudio"], payload);
    }

    #[test]
    fn original_direct_profile_shape_has_the_same_restore() {
        let mut draft = retained(true);
        let saved = json!({"guid":"stored-profile","thxSpatialAudio":{"isEnabled":false}});
        restore_profile_spatial(&mut draft, Some(&saved));
        assert_eq!(
            draft["sound"]["thxSpatialAudio"],
            json!({"isEnabled":false})
        );
    }

    #[test]
    fn wrapper_wins_over_conflicting_top_level_and_saved_sound_values() {
        let mut draft = retained(true);
        let saved = json!({
            "profile":{"thxSpatialAudio":{"isEnabled":false}},
            "thxSpatialAudio":{"isEnabled":true,"mode":"top-level"},
            "sound":{"thxSpatialAudio":{"isEnabled":true,"mode":"stale"}}
        });
        restore_profile_spatial(&mut draft, Some(&saved));
        assert_eq!(
            draft["sound"]["thxSpatialAudio"],
            json!({"isEnabled":false})
        );
    }

    #[test]
    fn absent_stored_field_keeps_cold_initial_or_existing_mounted_sound() {
        for saved in [
            None,
            Some(json!({"profile":{}})),
            Some(json!({"guid":"empty"})),
        ] {
            let mut cold = json!({"sound":{"thxSpatialAudio":initial()},"profile":{}});
            let cold_before = cold.clone();
            restore_profile_spatial(&mut cold, saved.as_ref());
            assert_eq!(cold, cold_before);
            assert!(cold["profile"].get("thxSpatialAudio").is_none());
            let mut mounted = retained(false);
            let mounted_before = mounted.clone();
            restore_profile_spatial(&mut mounted, saved.as_ref());
            assert_eq!(mounted, mounted_before);
        }
    }

    #[test]
    fn present_wrapper_missing_field_does_not_consume_top_level_collision() {
        let mut draft = retained(false);
        let saved = json!({"profile":{},"thxSpatialAudio":{"isEnabled":false}});
        let before = draft.clone();
        restore_profile_spatial(&mut draft, Some(&saved));
        assert_eq!(draft, before);
    }

    #[test]
    fn stored_source_shape_is_copied_without_repairing_missing_is_enabled() {
        let mut draft = retained(true);
        let payload = json!({"thxEnabled":false,"mode":"source-field-shape"});
        let saved = json!({"profile":{"thxSpatialAudio":payload}});
        restore_profile_spatial(&mut draft, Some(&saved));
        assert_eq!(draft["sound"]["thxSpatialAudio"], payload);
        assert!(draft["sound"]["thxSpatialAudio"].get("isEnabled").is_none());
    }
}
