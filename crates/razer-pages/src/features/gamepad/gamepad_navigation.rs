//! 2647 current mounted `op.componentDidMount/componentDidUpdate` remove only
//! TRIGGERS after `31867.U5("1.4.0.0", currentFWVersion)` becomes true. Source
//! does not add it back in that component lifetime or navigate to another page.
use std::sync::OnceLock;

#[derive(Default)]
pub struct TriggerNavigation {
    hidden: bool,
}
impl TriggerNavigation {
    pub fn observe_firmware(&mut self, product: u32, version: &str) {
        if product == 2647 && !version.is_empty() && below_trigger_firmware(version) {
            self.hidden = true;
        }
    }
    pub fn available(&self) -> bool {
        !self.hidden
    }
}

fn below_trigger_firmware(version: &str) -> bool {
    // The source minimum has four components, so semver.valid(minimum) is
    // false: U5 always follows its semver.coerce branch. Preserve its fourth
    // component's string comparison, including an absent fourth component.
    static COERCE: OnceLock<regex::Regex> = OnceLock::new();
    let expression = COERCE.get_or_init(|| {
        regex::Regex::new(
            r"(?:^|[^0-9])([0-9]{1,16})(?:\.([0-9]{1,16}))?(?:\.([0-9]{1,16}))?(?:$|[^0-9])",
        )
        .expect("current semver COERCE expression")
    });
    let Some(parts) = expression.captures(version) else {
        return false;
    };
    let mut coerced = [0u64; 3];
    for (index, result) in coerced.iter_mut().enumerate() {
        let part = parts.get(index + 1).map_or("0", |part| part.as_str());
        if part.len() > 1 && part.starts_with('0') {
            return false;
        }
        let Ok(number) = part.parse::<u64>() else {
            return false;
        };
        if number > 9_007_199_254_740_991 {
            return false;
        }
        *result = number;
    }
    match coerced.cmp(&[1, 4, 0]) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => version.split('.').nth(3).is_none_or(|fourth| "0" > fourth),
    }
}

#[cfg(test)]
mod tests {
    use super::{TriggerNavigation, below_trigger_firmware};
    #[test]
    fn current_minimum_uses_source_four_component_and_invalid_version_semantics() {
        for version in ["1.3.9.9", "1.4.0", "1.4.0.-1", "1.2"] {
            assert!(below_trigger_firmware(version), "{version}");
        }
        for version in [
            "1.4.0.0", "1.4.0.1", "1.4.1.0", "2.0.0.0", "unknown", "", "01.3.0.0",
        ] {
            assert!(!below_trigger_firmware(version), "{version}");
        }
    }
    #[test]
    fn only_current_2647_navigation_is_removed_and_source_never_reinserts_it() {
        let mut navigation = TriggerNavigation::default();
        navigation.observe_firmware(2636, "1.0.0.0");
        assert!(navigation.available());
        navigation.observe_firmware(2647, "");
        assert!(navigation.available());
        navigation.observe_firmware(2647, "1.3.0.0");
        assert!(!navigation.available());
        navigation.observe_firmware(2647, "1.4.0.1");
        assert!(!navigation.available());
    }
}
