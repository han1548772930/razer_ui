//! Current Alexa `Ya` media input semantics. This is not a HID/audio-DLL list.
//! Native capture enumeration still needs an adapter for browser media identity.
use serde::{Deserialize, Serialize};

/// A media enumeration record, with the same fields as MediaDeviceInfo.toJSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInputDevice {
    device_id: String,
    group_id: String,
    kind: String,
    label: String,
}

impl MediaInputDevice {
    pub fn new(
        device_id: impl Into<String>,
        group_id: impl Into<String>,
        kind: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            group_id: group_id.into(),
            kind: kind.into(),
            label: label.into(),
        }
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Default)]
pub(super) struct MediaInputState {
    devices: Vec<MediaInputDevice>,
    selected: Option<MediaInputDevice>,
    observed: bool,
}

impl MediaInputState {
    pub(super) fn devices(&self) -> &[MediaInputDevice] {
        &self.devices
    }

    pub(super) fn selected(&self) -> Option<&MediaInputDevice> {
        self.selected.as_ref()
    }

    pub(super) fn restore(&mut self, selected: Option<MediaInputDevice>) -> bool {
        self.selected = selected;
        self.observed && self.clear_missing_selection()
    }

    fn clear_missing_selection(&mut self) -> bool {
        if let Some(selected) = &self.selected
            && !self
                .devices
                .iter()
                .any(|device| device.device_id == selected.device_id)
        {
            self.selected = None;
            return true;
        }
        false
    }

    /// Ya emits the filtered list before clearing a disappeared selection.
    /// Preserve order, duplicates, and the stored selected record if its ID survives.
    pub(super) fn observe(&mut self, records: Vec<MediaInputDevice>) -> bool {
        self.observed = true;
        self.devices = records
            .into_iter()
            .filter(|device| {
                device.kind == "audioinput"
                    && device.device_id != "default"
                    && device.device_id != "communications"
                    && !device.label.is_empty()
            })
            .collect();
        self.clear_missing_selection()
    }

    /// Default maps to JS null. A choice changes the source record, not hardware.
    pub(super) fn select(&mut self, id: &str) -> bool {
        let next = if id == "default" {
            None
        } else if let Some(device) = self.devices.iter().find(|device| device.device_id == id) {
            Some(device.clone())
        } else {
            return false;
        };
        if next == self.selected {
            return false;
        }
        self.selected = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, kind: &str, label: &str) -> MediaInputDevice {
        MediaInputDevice::new(id, "group", kind, label)
    }

    #[test]
    fn source_filters_aliases_and_empty_labels_without_sorting_or_deduplication() {
        let mut state = MediaInputState::default();
        state.observe(vec![
            record("default", "audioinput", "Default microphone"),
            record("communications", "audioinput", "Communication microphone"),
            record("speaker", "audiooutput", "Speaker"),
            record("hidden", "audioinput", ""),
            record("z", "audioinput", "Microphone Z"),
            record("a", "audioinput", "Microphone A"),
            record("z", "audioinput", "Microphone Z"),
        ]);
        assert_eq!(
            state
                .devices()
                .iter()
                .map(|d| d.device_id())
                .collect::<Vec<_>>(),
            vec!["z", "a", "z"]
        );
        assert!(state.selected().is_none());
    }

    #[test]
    fn removal_clears_selection_but_matching_id_preserves_original_selected_object() {
        let mut state = MediaInputState::default();
        state.observe(vec![record("mic", "audioinput", "Original")]);
        assert!(state.select("mic"));
        assert!(!state.select("mic"));
        assert!(!state.observe(vec![record("mic", "audioinput", "Renamed")]));
        assert_eq!(state.selected().unwrap().label(), "Original");
        assert!(state.observe(vec![]));
        assert!(state.selected().is_none());
        assert!(!state.select("missing"));
    }

    #[test]
    fn default_selects_null_and_does_not_become_an_observed_endpoint() {
        let mut state = MediaInputState::default();
        state.observe(vec![record("mic", "audioinput", "Microphone")]);
        state.select("mic");
        assert!(state.select("default"));
        assert!(state.selected().is_none());
        assert_eq!(state.devices().len(), 1);
        assert!(!state.select("default"));
    }

    #[test]
    fn storage_record_is_not_an_enumeration_and_late_loading_reconciles_observed_ids() {
        let mut state = MediaInputState::default();
        assert!(!state.restore(Some(record("mic", "audioinput", "Stored"))));
        assert!(state.devices().is_empty());
        assert_eq!(state.selected().unwrap().label(), "Stored");
        state.observe(vec![record("mic", "audioinput", "Current")]);
        assert_eq!(state.selected().unwrap().label(), "Stored");
        let mut late = MediaInputState::default();
        late.observe(vec![]);
        assert!(late.restore(Some(record("missing", "audioinput", "Stored"))));
        assert!(late.selected().is_none());
    }
}
