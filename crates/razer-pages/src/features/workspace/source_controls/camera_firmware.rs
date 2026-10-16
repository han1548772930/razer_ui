//! Current 3595/3596 camera roots use module 1867's U5 comparison, with
//! minimumFWVersionSupportAF from module 5987. The complete current comparison
//! and semver dependency owners are retained by the camera-local-pages audit.
use regex::Regex;
use std::sync::OnceLock;

/// These product minima contain four components, so semver.valid(minimum)
/// fails and U5 always enters its coerce branch. coerce returns separate
/// SemVer objects: its `i === n` fourth-component comparison never succeeds.
/// Preserve that source behavior, including the early exact-string check and
/// the invalid/missing-version branch, instead of inventing a numeric gate.
pub(super) fn focus_mode_enabled(minimum: &str, current: Option<&str>) -> bool {
    let current = current.unwrap_or("");
    if minimum == current {
        return true;
    }
    let (Some(minimum), Some(current)) = (coerce(minimum), coerce(current)) else {
        return true;
    };
    current > minimum
}

fn coerce(version: &str) -> Option<[u64; 3]> {
    static COERCE: OnceLock<Regex> = OnceLock::new();
    // Current semver modules 1468/8816: ASCII digits, at most 16 per
    // component, and a non-digit boundary at either end of the match.
    let captures = COERCE
        .get_or_init(|| {
            Regex::new(
                r"(?:^|[^0-9])([0-9]{1,16})(?:\.([0-9]{1,16}))?(?:\.([0-9]{1,16}))?(?:$|[^0-9])",
            )
            .expect("source semver coerce pattern")
        })
        .captures(version)?;
    let mut values = [0; 3];
    for (index, target) in values.iter_mut().enumerate() {
        if let Some(value) = captures.get(index + 1) {
            let text = value.as_str();
            // coerce calls the strict SemVer constructor; a leading zero or
            // an unsafe integer rejects the whole match without trying again.
            if text.len() > 1 && text.starts_with('0') {
                return None;
            }
            *target = text.parse().ok()?;
            if *target > 9_007_199_254_740_991 {
                return None;
            }
        }
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn camera_firmware_branch_preserves_source_object_comparison() {
        for minimum in ["1.0.2.3", "1.0.2.0"] {
            assert!(focus_mode_enabled(minimum, Some(minimum)));
            assert!(focus_mode_enabled(minimum, Some("1.0.3.0")));
            assert!(focus_mode_enabled(minimum, Some("v1.1.0-beta")));
            assert!(!focus_mode_enabled(minimum, Some("1.0.1.9")));
            assert!(!focus_mode_enabled(minimum, Some("1.0.2")));
            assert!(!focus_mode_enabled(minimum, Some("1.0.2.9")));
            assert!(!focus_mode_enabled(minimum, Some("1.0.2.10")));
        }
    }

    #[test]
    fn camera_firmware_missing_or_rejected_coerce_retains_mounted_source_branch() {
        for current in [
            None,
            Some(""),
            Some("unknown"),
            Some("01.0.0"),
            Some("9007199254740992.0.0"),
        ] {
            assert!(focus_mode_enabled("1.0.2.3", current));
        }
        assert_eq!(coerce("firmware 1.0.3.7"), Some([1, 0, 3]));
        assert_eq!(coerce("7"), Some([7, 0, 0]));
    }
}
