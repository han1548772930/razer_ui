//! Current product modules 21107/1107: tab-message focus helper, not OS activation.
//! Source hashes and exact spans: gamepad-calibration-focus-current-source.json.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostWindowStatus {
    pub is_visible: Option<bool>,
    pub is_minimized: Option<bool>,
}

/// Mounted popup's source ref: close only after a true helper observation.
/// Operation retries leave this state in place; popup mount creates a new one.
#[derive(Default)]
pub struct PopupTabFocus {
    initialized: bool,
    seen_true: bool,
}
impl PopupTabFocus {
    pub fn pending(&self) -> bool {
        !self.initialized
    }
    pub fn observe(&mut self, active: bool) -> bool {
        self.initialized = true;
        if active {
            self.seen_true = true;
            false
        } else {
            self.seen_true
        }
    }
}

/// Initial query and focus(false) use visibility and minimization. Missing JS
/// fields differ from explicit false/true; an absent status object is false.
pub fn query_focus(
    window_name: &str,
    frame_name: Option<&str>,
    status: Option<HostWindowStatus>,
    selected_frame: Option<&str>,
) -> bool {
    let Some(frame_name) = frame_name.filter(|name| !name.is_empty()) else {
        return true;
    };
    let Some(status) = status else { return false };
    status.is_visible != Some(false)
        && status.is_minimized != Some(true)
        && selected_focus(window_name, frame_name, selected_frame)
}

/// focus(true) queries only the selected frame. Empty selection means the host
/// window itself. It deliberately does not consult visibility or OS focus.
pub fn selected_focus(window_name: &str, frame_name: &str, selected_frame: Option<&str>) -> bool {
    selected_frame == Some(frame_name) || (selected_frame == Some("") && window_name == frame_name)
}

/// changeActiveTab(data.name) / activeTabChanged(data.tabName) compare names.
pub fn active_tab_changed(frame_name: &str, selected_frame: &str) -> bool {
    frame_name.is_empty() || selected_frame == frame_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_popup_requires_true_then_false_and_remount_resets() {
        let mut popup = PopupTabFocus::default();
        assert!(popup.pending());
        assert!(!popup.observe(false));
        assert!(!popup.pending());
        assert!(!popup.observe(false));
        assert!(!popup.observe(true));
        assert!(!popup.observe(true));
        assert!(popup.observe(false));
        assert!(!PopupTabFocus::default().observe(false));
    }

    #[test]
    fn missing_fields_preserve_javascript_strict_comparisons() {
        let absent_fields = HostWindowStatus {
            is_visible: None,
            is_minimized: None,
        };
        assert!(query_focus(
            "synapse",
            Some("device"),
            Some(absent_fields),
            Some("device")
        ));
        assert!(!query_focus(
            "synapse",
            Some("device"),
            None,
            Some("device")
        ));
        assert!(query_focus("synapse", None, None, None));
        assert!(query_focus("synapse", Some(""), None, None));
    }

    #[test]
    fn lost_os_focus_is_not_a_tab_change() {
        let status = HostWindowStatus {
            is_visible: Some(true),
            is_minimized: Some(false),
        };
        // focus(false), with no tab/window visibility change, remains true.
        assert!(query_focus(
            "synapse",
            Some("device"),
            Some(status),
            Some("device")
        ));
        assert!(!query_focus(
            "synapse",
            Some("device"),
            Some(status),
            Some("other")
        ));
        assert!(!active_tab_changed("device", "other"));
        assert!(active_tab_changed("device", "device"));
    }

    #[test]
    fn hidden_or_minimized_applies_only_to_status_query() {
        for status in [
            HostWindowStatus {
                is_visible: Some(false),
                is_minimized: Some(false),
            },
            HostWindowStatus {
                is_visible: Some(true),
                is_minimized: Some(true),
            },
        ] {
            assert!(!query_focus(
                "synapse",
                Some("device"),
                Some(status),
                Some("device")
            ));
            assert!(selected_focus("synapse", "device", Some("device")));
        }
        assert!(selected_focus("synapse", "synapse", Some("")));
        assert!(!selected_focus("synapse", "device", Some("")));
        assert!(!selected_focus("synapse", "synapse", None));
    }
}
