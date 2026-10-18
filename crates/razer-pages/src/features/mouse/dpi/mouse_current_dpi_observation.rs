use razer_device::mouse_current_dpi::{DpiPair, DpiReport, pack_for};
use razer_device::mouse_dpi_stages::{DpiStagesDraft, DpiStagesReading};

/// Local stage identities come from the selected source profile. A getter's
/// compressed ordinals do not supply hidden, independent or enabled flags.
#[derive(Clone)]
pub(super) struct Selection {
    pub(super) selected: usize,
    wire_rows: Vec<usize>,
}

impl Selection {
    pub(super) fn capture(product_id: u32, draft: &DpiStagesDraft) -> Option<Self> {
        if draft.stages.len() != 5 {
            return None;
        }
        let selected = draft.active_stage.checked_sub(1)?;
        draft.stages.get(selected)?;
        let packed = pack_for(product_id, draft).ok();
        let visible: Vec<_> = draft
            .stages
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.visible.then_some(index))
            .collect();
        let count = packed.as_ref().map(|packed| packed.records.len());
        let wire_rows = if count == Some(visible.len()) {
            visible
        } else if count == Some(1) && draft.stages[selected].visible {
            vec![selected]
        } else {
            Vec::new()
        };
        Some(Self {
            selected,
            wire_rows,
        })
    }

    fn table_values(&self, table: &DpiStagesReading) -> Option<[Option<DpiPair>; 5]> {
        if usize::from(table.number_of_active_stages) != self.wire_rows.len()
            || self.wire_rows.is_empty()
            || table
                .records
                .iter()
                .skip(self.wire_rows.len())
                .any(|record| record.index != 0)
        {
            return None;
        }
        let mut values = [None; 5];
        for (ordinal, row) in self.wire_rows.iter().copied().enumerate() {
            let record = table.records.get(ordinal)?;
            if record.index as usize != ordinal + 1 || row >= values.len() {
                return None;
            }
            values[row] = Some(DpiPair {
                x: record.x,
                y: record.y,
            });
        }
        Some(values)
    }
}

#[derive(Default)]
pub(super) struct Observation {
    pub(super) current: Option<DpiPair>,
    pub(super) stage_level: Option<DpiPair>,
    pub(super) table: Option<DpiStagesReading>,
    pub(super) active_error: Option<String>,
    pub(super) stages_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowUpdate {
    Unchanged,
    Unobserved,
    Observed(DpiPair),
}

impl Observation {
    /// Independent source tasks may succeed separately. In particular, the
    /// profile-0 pair must survive a failed profile-1 table getter.
    pub(super) fn accept(
        &mut self,
        selection: Option<&Selection>,
        report: &DpiReport,
    ) -> [RowUpdate; 5] {
        let mut values = [RowUpdate::Unchanged; 5];
        if report
            .requested
            .as_ref()
            .is_none_or(|change| change.stages.is_some())
        {
            self.table = None;
            self.stage_level = None;
            values.fill(RowUpdate::Unobserved);
        }
        if report
            .requested
            .as_ref()
            .is_none_or(|change| change.active.is_some())
        {
            self.current = None;
            if let Some(selection) = selection {
                values[selection.selected] = RowUpdate::Unobserved;
            }
        }
        if let Some(table) = &report.stages {
            if let Some(mapped) = selection.and_then(|selection| selection.table_values(table)) {
                values =
                    mapped.map(|value| value.map_or(RowUpdate::Unobserved, RowUpdate::Observed));
            }
            self.table = Some(table.clone());
            self.stages_error = None;
        }
        if let Some(level) = report.stage_level {
            self.stage_level = Some(level);
            self.stages_error = None;
            if let Some(selection) = selection {
                values[selection.selected] = RowUpdate::Observed(level);
            }
        }
        if let Some(pair) = report.active {
            self.current = Some(pair);
            self.active_error = None;
            // The current control belongs to the source-selected local row.
            // Preserve the table separately: transient DPI can differ from it.
            if let Some(selection) = selection {
                values[selection.selected] = RowUpdate::Observed(pair);
            }
        }
        if let Some(error) = report.read_errors.active() {
            self.active_error = Some(error.into());
            // A profile-1 table does not observe profile-0 current DPI.
            if let Some(selection) = selection {
                values[selection.selected] = RowUpdate::Unobserved;
            }
        }
        if let Some(error) = report.read_errors.stages() {
            self.stages_error = Some(error.into());
        }
        values
    }

    pub(super) fn errors(&self) -> impl Iterator<Item = &str> {
        [self.active_error.as_deref(), self.stages_error.as_deref()]
            .into_iter()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::mouse_dpi_stages::{DpiStage, PackedDpiStage};
    use serde_json::json;

    fn values(updates: [RowUpdate; 5]) -> [Option<DpiPair>; 5] {
        updates.map(|update| match update {
            RowUpdate::Observed(value) => Some(value),
            RowUpdate::Unchanged | RowUpdate::Unobserved => None,
        })
    }

    fn profile() -> DpiStagesDraft {
        DpiStagesDraft {
            enabled: true,
            active_stage: 3,
            stages: [400, 800, 1600, 3200, 6400]
                .map(|value| DpiStage {
                    x: value,
                    y: value,
                    visible: true,
                    independent: false,
                })
                .into(),
        }
    }

    fn report(active: Option<DpiPair>, table: Option<DpiStagesReading>) -> DpiReport {
        let table_error = table.is_none().then_some("table getter failed");
        serde_json::from_value(json!({
            "requested": null, "stages": table, "active": active,
            "acknowledgements": [], "verified": false,
            "read_errors": {"stages": table_error, "active": null}
        }))
        .unwrap()
    }

    fn table(values: &[u16]) -> DpiStagesReading {
        DpiStagesReading {
            profile_id: 1,
            active_stage: 1,
            number_of_active_stages: values.len() as u8,
            records: values
                .iter()
                .enumerate()
                .map(|(index, value)| PackedDpiStage {
                    index: index as u8 + 1,
                    x: *value,
                    y: *value,
                    z: 0,
                })
                .collect(),
            response_bytes: 3 + values.len() * 7,
        }
    }

    #[test]
    fn current_dpi_observation_partial_current_2000_reaches_source_selected_control() {
        let profile = profile();
        let selection = Selection::capture(190, &profile).unwrap();
        let mut observation = Observation::default();
        let actual = DpiPair { x: 2000, y: 2100 };
        let values = values(observation.accept(Some(&selection), &report(Some(actual), None)));
        assert_eq!(values[2], Some(actual));
        assert_eq!(observation.current, Some(actual));
        assert!(observation.table.is_none());
        assert!(observation.stages_error.is_some());
        assert_eq!(values.iter().flatten().count(), 1);
        assert_eq!(profile.stages[2].x, 1600);
    }

    #[test]
    fn current_dpi_observation_count_one_does_not_invent_flags_or_stage_identity() {
        let profile = profile();
        let selection = Selection::capture(190, &profile).unwrap();
        let mut observation = Observation::default();
        let values =
            values(observation.accept(Some(&selection), &report(None, Some(table(&[2000])))));
        assert_eq!(values, [None; 5]);
        assert_eq!(
            observation.table.as_ref().unwrap().number_of_active_stages,
            1
        );
        assert!(profile.enabled);
        assert!(
            profile
                .stages
                .iter()
                .all(|row| row.visible && !row.independent)
        );
    }

    #[test]
    fn current_dpi_observation_known_hidden_mapping_preserves_local_row_identity() {
        let mut profile = profile();
        profile.stages[0].visible = false;
        profile.stages[3].visible = false;
        let selection = Selection::capture(190, &profile).unwrap();
        let mut observation = Observation::default();
        let values = values(observation.accept(
            Some(&selection),
            &report(None, Some(table(&[500, 2000, 7000]))),
        ));
        assert_eq!(
            values.map(|value| value.map(|value| value.x)),
            [None, Some(500), Some(2000), None, Some(7000)]
        );
        assert!(!profile.stages[0].visible);
        assert!(!profile.stages[3].visible);
    }

    #[test]
    fn current_dpi_observation_disabled_profile_maps_single_record_to_selected_row() {
        let mut profile = profile();
        profile.enabled = false;
        let selection = Selection::capture(190, &profile).unwrap();
        let mut observation = Observation::default();
        let values =
            values(observation.accept(Some(&selection), &report(None, Some(table(&[2300])))));
        assert_eq!(values[2], Some(DpiPair { x: 2300, y: 2300 }));
        assert_eq!(values.iter().flatten().count(), 1);
        assert!(!profile.enabled);
    }

    #[test]
    fn current_dpi_observation_transient_pair_remains_distinct_from_stage_table() {
        let selection = Selection::capture(190, &profile()).unwrap();
        let mut observation = Observation::default();
        let values = values(observation.accept(
            Some(&selection),
            &report(
                Some(DpiPair { x: 2000, y: 2000 }),
                Some(table(&[400, 800, 1600, 3200, 6400])),
            ),
        ));
        assert_eq!(values[2].unwrap().x, 2000);
        assert_eq!(observation.table.as_ref().unwrap().records[2].x, 1600);
        assert_eq!(observation.current.unwrap().x, 2000);
    }

    #[test]
    fn current_dpi_observation_active_failure_does_not_substitute_table_for_current() {
        let selection = Selection::capture(190, &profile()).unwrap();
        let mut observation = Observation::default();
        let mut actual = report(None, Some(table(&[400, 800, 1600, 3200, 6400])));
        actual.read_errors =
            serde_json::from_value(json!({"active":"getter failed","stages":null})).unwrap();
        let values = values(observation.accept(Some(&selection), &actual));
        assert_eq!(values[2], None);
        assert_eq!(values[1].unwrap().x, 800);
        assert!(observation.current.is_none());
        assert!(observation.active_error.is_some());
    }

    #[test]
    fn current_dpi_observation_failed_refresh_invalidates_previous_values() {
        let selection = Selection::capture(190, &profile()).unwrap();
        let mut observation = Observation::default();
        observation.accept(
            Some(&selection),
            &report(
                Some(DpiPair { x: 2000, y: 2100 }),
                Some(table(&[500, 1000, 2000, 4000, 8000])),
            ),
        );
        let mut failed = report(None, None);
        failed.read_errors = serde_json::from_value(json!({
            "stages":"stage getter failed", "active":"active getter failed"
        }))
        .unwrap();
        assert_eq!(
            observation.accept(Some(&selection), &failed),
            [RowUpdate::Unobserved; 5]
        );
        assert!(observation.current.is_none());
        assert!(observation.table.is_none());
        assert!(observation.stage_level.is_none());
    }

    #[test]
    fn current_dpi_observation_failed_write_preserves_actual_mismatching_readback() {
        let profile = profile();
        let selection = Selection::capture(190, &profile).unwrap();
        let mut observation = Observation::default();
        let actual = DpiPair { x: 2300, y: 2400 };
        let report: DpiReport = serde_json::from_value(json!({
            "requested": {"stages": null, "active":{"x":2200,"y":2200}},
            "stages":null, "active":actual, "acknowledgements":[], "verified":false,
            "write_errors":{"active":"readback differs","stages":null}
        }))
        .unwrap();
        let updates = observation.accept(Some(&selection), &report);
        assert_eq!(updates[2], RowUpdate::Observed(actual));
        assert_eq!(updates[0], RowUpdate::Unchanged);
        assert_eq!(observation.current, Some(actual));
        assert_eq!(profile.stages[2].x, 1600);
    }
}
