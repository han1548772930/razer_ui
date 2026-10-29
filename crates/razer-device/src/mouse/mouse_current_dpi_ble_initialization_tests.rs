//! Literal Protocol30 responses and It/Tt order from current226 factories
//! 85191/20063, plus 7303's loadProfilesFromDevice_DPI projection.
use super::super::{GET_MAX_PROFILES, GET_PROFILE_IDS, GET_PROFILE_NAME, SET_STAGES};
use super::*;
use crate::mouse_current_dpi::{Acknowledgement, SourceStage};
use std::{cell::Cell, collections::VecDeque, rc::Rc};

const GUID: &str = "12345678-9abc-def0-1234-56789abcdef0";
const GUID_BYTES: [u8; 16] = [
    0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0xf0, 0xde, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
];
const LEVEL: &[u8] = &[0x84, 3, 0x84, 3, 0, 0];
const STAGE_900: &[u8] = &[1, 1, 1, 0x84, 3, 0x84, 3, 0, 0];
const STAGE_1600: &[u8] = &[1, 1, 1, 0x40, 6, 0x40, 6, 0, 0];
const WRITE_1600: &[u8] = &[1, 1, 0, 0x40, 6, 0x40, 6, 0, 0, 0];

struct Step {
    command: [u8; 3],
    profile: u8,
    payload: Vec<u8>,
    reply: Result<Vec<u8>, &'static str>,
}

impl Step {
    fn read(command: [u8; 3], profile: u8, reply: &[u8]) -> Self {
        Self {
            command,
            profile,
            payload: vec![],
            reply: Ok(reply.to_vec()),
        }
    }
    fn write(profile: u8) -> Self {
        Self {
            command: SET_STAGES,
            profile,
            payload: WRITE_1600.to_vec(),
            reply: Ok(vec![0xA5]),
        }
    }
    fn fail(command: [u8; 3], profile: u8) -> Self {
        Self {
            command,
            profile,
            payload: vec![],
            reply: Err("actual scripted rejection"),
        }
    }
}

struct Script {
    steps: VecDeque<Step>,
    calls: Rc<Cell<usize>>,
}
impl Script {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            calls: Rc::new(Cell::new(0)),
        }
    }
    fn done(&self) {
        assert!(
            self.steps.is_empty(),
            "{} source transfers remain",
            self.steps.len()
        );
    }
}
impl Transport for Script {
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        payload: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement> {
        validate()?;
        let step = self.steps.pop_front().expect("unexpected source transfer");
        assert_eq!(
            (command, profile, payload),
            (step.command, step.profile, step.payload.as_slice())
        );
        self.calls.set(self.calls.get() + 1);
        let data = step.reply.map_err(anyhow::Error::msg)?;
        Ok(Acknowledgement {
            command,
            profile: Some(profile),
            transaction: 2,
            response_bytes: data.len(),
            data,
        })
    }
}

fn metadata_steps(profile: u8, name: &str, owner: &str) -> Vec<Step> {
    let mut data = [0; 250];
    data[..16].copy_from_slice(&GUID_BYTES);
    data[16..16 + name.len()].copy_from_slice(name.as_bytes());
    data[116..116 + owner.len()].copy_from_slice(owner.as_bytes());
    (0..250u16)
        .step_by(78)
        .map(|offset| {
            let count = (250 - offset).min(78);
            let payload = match offset {
                0 => vec![0, 0, 78, 0],
                78 => vec![78, 0, 78, 0],
                156 => vec![156, 0, 78, 0],
                234 => vec![234, 0, 16, 0],
                _ => unreachable!(),
            };
            let mut reply = vec![0, 0];
            reply.extend_from_slice(&data[usize::from(offset)..usize::from(offset + count)]);
            Step {
                command: GET_PROFILE_NAME,
                profile,
                payload,
                reply: Ok(reply),
            }
        })
        .collect()
}

fn local(guid: &str, dpi: u16) -> LocalProfile {
    LocalProfile {
        guid: guid.into(),
        name: format!("local {guid}"),
        obm_slot_ids: vec![],
        dpi_stages: Some(SourceStages::new(
            false,
            1,
            vec![SourceStage::new(
                Some(dpi.into()),
                Some(dpi.into()),
                Some(true),
                Some(false),
            )],
        )),
    }
}
fn request() -> Request {
    Request {
        profiles: vec![local(GUID, 1600), local("current-local", 900)],
        active_profile_guid: "current-local".into(),
        owner: " \u{feff}owner \u{feff}".into(),
    }
}
fn read_steps(ids: &[u8], names: &[(&str, &str)]) -> Vec<Step> {
    let mut steps = vec![
        Step::read(GET_MAX_PROFILES, 0, &[5]),
        Step::read(GET_PROFILE_IDS, 0, ids),
        Step::read(GET_PROFILE_IDS, 0, ids),
    ];
    for (&slot, &(name, owner)) in ids.iter().skip(1).zip(names) {
        steps.extend(metadata_steps(slot, name, owner));
    }
    for &slot in ids {
        steps.extend([
            Step::read(GET_LEVEL, slot, LEVEL),
            Step::read(GET_STAGES, slot, STAGE_900),
        ]);
    }
    steps
}
fn target() -> WindowsBleTarget {
    WindowsBleTarget {
        product_id: 226,
        physical_product_id: 228,
        device_container_id: "container".into(),
        hid_path: "hid".into(),
        device_instance_id: "device".into(),
        parent_instance_id: "parent".into(),
        mac_address: 1,
    }
}

#[test]
fn initialization_uses_all_raw_profiles_and_source_level_stage_order() {
    assert_eq!(
        super::super::frame(3, GET_LEVEL, 9, &[]).unwrap(),
        [3, 0, 0, 0, 11, 129, 9, 0]
    );
    let mut steps = read_steps(
        &[1, 9, 7],
        &[("device9", "ow\0ner"), ("device7", "foreign")],
    );
    steps.extend([Step::read(GET_STAGES, 9, STAGE_900), Step::write(9)]);
    let mut script = Script::new(steps);
    let report = execute(226, 228, &mut script, &request(), &|| Ok(())).unwrap();
    script.done();
    assert!(report.completed);
    assert_eq!(report.profiles[0].selected_slots, [9]);
    assert_eq!(report.profiles[0].differing_slots, [9]);
    assert_eq!(report.profiles[1].selected_slots, [1]);
    assert!(report.profiles[1].writes.is_empty());
    let raw = report.read.snapshot.as_ref().unwrap();
    assert_eq!(raw.inventory.profile_ids, [1, 9, 7]);
    assert!(matches!(raw.profiles[0].identity, Observation::Unobserved));
    assert_eq!(raw.profiles[1].stages.value().unwrap().records[0].x, 900);
    let write = &report.profiles[0].writes[0];
    assert_eq!(write.before.as_ref().unwrap().records[0].x, 900);
    assert_eq!(write.acknowledged.as_ref().unwrap().raw_data, [0xA5]);
    assert_eq!(
        report
            .projected_profiles
            .iter()
            .map(|profile| profile.slot_id)
            .collect::<Vec<_>>(),
        [9, 7]
    );
    assert!(
        report
            .projected_profiles
            .iter()
            .all(|profile| profile.origin == ProjectionOrigin::DeviceIdentity)
    );
    assert_eq!(report.projected_profiles[0].owner, "owner");
    // Neither a slot setter ACK nor the local request replaces this observation.
    assert_eq!(raw.profiles[1].stages.value().unwrap().records[0].x, 900);
    report.validate_reply(&request()).unwrap();
}

#[test]
fn current_profile_targets_numeric_slot_one_independent_of_inventory_ordinal() {
    let mut req = request();
    req.profiles = vec![local("current-local", 1600)];
    let mut steps = read_steps(&[9, 1], &[("device-slot1", "foreign")]);
    steps.extend([Step::read(GET_STAGES, 1, STAGE_900), Step::write(1)]);
    let mut script = Script::new(steps);
    let report = execute(226, 228, &mut script, &req, &|| Ok(())).unwrap();
    script.done();
    assert!(report.completed);
    assert_eq!(report.profiles[0].selected_slots, [1]);
    assert_eq!(report.projected_profiles.len(), 1);
    assert_eq!(report.projected_profiles[0].slot_id, 9);
    assert_eq!(
        report.projected_profiles[0].origin,
        ProjectionOrigin::LocalProfile
    );
}

#[test]
fn projected_fallback_guid_is_separate_from_raw_initialization_selection() {
    let mut req = request();
    req.profiles[0].obm_slot_ids = vec![7];
    req.profiles[0].dpi_stages = None;
    req.profiles[1].obm_slot_ids = vec![9];
    let mut script = Script::new(read_steps(&[1, 9, 7], &[("", "foreign"), ("", "foreign")]));
    let report = execute(226, 228, &mut script, &req, &|| Ok(())).unwrap();
    script.done();
    assert!(report.completed);
    assert!(report.profiles[0].selected_slots.is_empty());
    assert_eq!(report.projected_profiles[0].guid, "current-local");
    assert_eq!(report.projected_profiles[1].guid, GUID);
    assert_eq!(
        report.read.snapshot.as_ref().unwrap().profiles[1]
            .identity
            .value()
            .unwrap()
            .guid,
        GUID
    );
    let slots = report
        .observed_slots(target(), "workspace-owner".into(), 8)
        .unwrap();
    slots
        .validate_target(&target(), "workspace-owner", 8)
        .unwrap();
    assert!(
        slots
            .validate_target(&target(), "workspace-owner", 9)
            .is_err()
    );
    assert!(
        slots
            .validate_target(&target(), "replacement-owner", 8)
            .is_err()
    );
    let mut other = target();
    other.mac_address = 2;
    assert!(slots.validate_target(&other, "workspace-owner", 8).is_err());
}

#[test]
fn source_slot_task_getter_can_obviate_an_initial_snapshot_difference() {
    let mut steps = read_steps(&[1, 9], &[("device9", "owner")]);
    steps.push(Step::read(GET_STAGES, 9, STAGE_1600));
    let mut script = Script::new(steps);
    let report = execute(226, 228, &mut script, &request(), &|| Ok(())).unwrap();
    script.done();
    assert!(report.completed);
    assert!(report.profiles[0].writes[0].was_equal);
    assert!(report.profiles[0].writes[0].acknowledged.is_none());
}

#[test]
fn slot_errors_continue_remaining_slots_then_stop_the_next_local_profile() {
    let mut req = request();
    req.profiles[1] = local("current-local", 1600);
    let mut steps = read_steps(&[1, 9, 7], &[("device9", "owner"), ("device7", "owner")]);
    steps.extend([
        Step::fail(GET_STAGES, 9),
        Step::read(GET_STAGES, 7, STAGE_900),
        Step::write(7),
    ]);
    let mut script = Script::new(steps);
    let report = execute(226, 228, &mut script, &req, &|| Ok(())).unwrap();
    script.done();
    assert!(!report.completed && report.error.is_some());
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].writes.len(), 2);
    assert!(report.profiles[0].writes[0].error.is_some());
    assert!(report.profiles[0].writes[1].acknowledged.is_some());
    assert!(report.observed_slots(target(), "owner".into(), 0).is_err());
}

#[test]
fn failure_to_read_current_slot_never_writes_local_defaults() {
    let mut script = Script::new([
        Step::read(GET_MAX_PROFILES, 0, &[5]),
        Step::read(GET_PROFILE_IDS, 0, &[1]),
        Step::read(GET_PROFILE_IDS, 0, &[1]),
        Step::fail(GET_LEVEL, 1),
    ]);
    let report = execute(226, 228, &mut script, &request(), &|| Ok(())).unwrap();
    script.done();
    assert!(!report.completed && report.error.is_some());
    let profile = &report.read.snapshot.as_ref().unwrap().profiles[0];
    assert!(matches!(profile.active, Observation::Failed { .. }));
    assert!(matches!(profile.stages, Observation::Unobserved));
    assert!(
        report
            .profiles
            .iter()
            .all(|profile| profile.writes.is_empty())
    );
}

#[test]
fn cancellation_after_an_inventory_transfer_prevents_metadata_and_writes() {
    let mut script = Script::new([Step::read(GET_MAX_PROFILES, 0, &[5])]);
    let calls = Rc::clone(&script.calls);
    let report = execute(226, 228, &mut script, &request(), &|| {
        ensure!(calls.get() == 0, "connection canceled");
        Ok(())
    })
    .unwrap();
    script.done();
    assert!(!report.completed);
    assert!(report.read.snapshot.is_none());
    assert!(report.error.as_deref().unwrap().contains("canceled"));
    assert!(report.profiles.is_empty());
}

#[test]
fn malformed_requests_and_wrong_product_are_rejected_before_observation() {
    let mut script = Script::new([]);
    let mut req = request();
    req.profiles.clear();
    assert!(execute(226, 228, &mut script, &req, &|| Ok(())).is_err());
    let mut req = request();
    req.profiles[1].guid = GUID.into();
    assert!(req.validate().is_err());
    assert!(execute(226, 227, &mut script, &request(), &|| Ok(())).is_err());
    assert_eq!(script.calls.get(), 0);
}

#[test]
fn reply_validation_rejects_reassigned_raw_and_projected_identities_and_fake_success() {
    let mut req = request();
    req.profiles[0].dpi_stages = None;
    let mut script = Script::new(read_steps(&[1, 9], &[("device9", "owner")]));
    let valid = execute(226, 228, &mut script, &req, &|| Ok(())).unwrap();
    script.done();
    let mut report = valid.clone();
    report.profiles[0].selected_slots = vec![7];
    assert!(report.validate_reply(&req).is_err());
    let mut report = valid.clone();
    report.projected_profiles[0].guid = "wrong-guid".into();
    assert!(report.validate_reply(&req).is_err());
    let mut report = valid.clone();
    report.read.snapshot.as_mut().unwrap().profiles[1].identity =
        match report.read.snapshot.as_ref().unwrap().profiles[1]
            .identity
            .clone()
        {
            Observation::Observed { mut value } => {
                value.guid = "wrong-guid".into();
                Observation::Observed { value }
            }
            _ => unreachable!(),
        };
    assert!(report.validate_reply(&req).is_err());
    let mut report = valid.clone();
    report.completed = false;
    assert!(report.validate_reply(&req).is_err());
    let mut other = req.clone();
    other.active_profile_guid = GUID.into();
    assert!(valid.validate_reply(&other).is_err());
}

#[test]
fn enabled_comparison_uses_raw_array_order_active_and_independent_consumer() {
    let row = |dpi: u16, visible, independent| {
        SourceStage::new(
            Some(dpi.into()),
            Some(dpi.into()),
            Some(visible),
            Some(independent),
        )
    };
    let local = SourceStages::new(
        true,
        2,
        vec![row(900, false, false), row(1600, true, false)],
    );
    let actual = DpiStagesReading {
        profile_id: 9,
        active_stage: 2,
        number_of_active_stages: 2,
        records: vec![
            crate::mouse_dpi_stages::PackedDpiStage {
                index: 1,
                x: 900,
                y: 900,
                z: 0,
            },
            crate::mouse_dpi_stages::PackedDpiStage {
                index: 2,
                x: 1600,
                y: 1600,
                z: 0,
            },
        ],
        response_bytes: 16,
    };
    // It compares raw positions, before Le visibility packing, without turning
    // the observed count into a locally observed enable/independent flag.
    assert!(!differs(&local, &actual).unwrap());
    let mut changed = actual.clone();
    changed.active_stage = 1;
    assert!(differs(&local, &changed).unwrap());
    let mut changed = actual.clone();
    changed.number_of_active_stages = 1;
    assert!(differs(&local, &changed).unwrap());
    let independent =
        SourceStages::new(true, 2, vec![row(900, false, true), row(1600, true, false)]);
    assert!(differs(&independent, &actual).unwrap());
    let missing_local = SourceStages::new(true, 2, vec![row(900, true, false)]);
    assert!(differs(&missing_local, &actual).is_err());
    let mut truncated = actual;
    truncated.records.pop();
    assert!(differs(&local, &truncated).is_err());
}

#[test]
fn disabled_comparison_uses_first_returned_row_and_missing_axes_remain_unobserved() {
    let actual = DpiStagesReading {
        profile_id: 9,
        active_stage: 2,
        number_of_active_stages: 1,
        records: vec![
            crate::mouse_dpi_stages::PackedDpiStage {
                index: 1,
                x: 900,
                y: 900,
                z: 0,
            },
            crate::mouse_dpi_stages::PackedDpiStage {
                index: 2,
                x: 1600,
                y: 1600,
                z: 0,
            },
        ],
        response_bytes: 16,
    };
    let mut local = local(GUID, 1600).dpi_stages.unwrap();
    assert!(differs(&local, &actual).unwrap());
    local = local
        .with_replaced_stage(
            0,
            SourceStage::new(
                Some(900u16.into()),
                Some(900u16.into()),
                Some(true),
                Some(false),
            ),
        )
        .unwrap();
    assert!(!differs(&local, &actual).unwrap());
    let actual = DpiStagesReading {
        records: vec![],
        response_bytes: 2,
        ..actual
    };
    assert!(differs(&local, &actual).is_err());
}

#[test]
fn metadata_retry_is_bounded_and_preserves_the_actual_invalid_guid() {
    let mut steps = Vec::new();
    for _ in 0..10 {
        let mut attempt = metadata_steps(9, "device9", "owner");
        attempt[0].reply.as_mut().unwrap()[2..18].fill(0);
        steps.extend(attempt);
    }
    let mut script = Script::new(steps);
    let observed = read_profile_identity(&mut script, 9, &|| Ok(())).unwrap();
    script.done();
    assert_eq!(observed.guid, "00000000-0000-0000-0000-000000000000");
    assert!(!observed.source_guid_accepted);
    assert_eq!(observed.attempts, 10);
    assert_eq!(observed.raw_metadata.len(), 250);
    assert_eq!(script.calls.get(), 40);
}

#[test]
fn cancellation_during_metadata_stops_chunks_and_retry_rounds() {
    let first = metadata_steps(9, "device9", "owner").remove(0);
    let mut script = Script::new([first]);
    let calls = Rc::clone(&script.calls);
    assert!(
        read_profile_identity(&mut script, 9, &|| {
            ensure!(calls.get() == 0, "canceled");
            Ok(())
        })
        .is_err()
    );
    script.done();
    assert_eq!(script.calls.get(), 1);
}

#[test]
fn cancellation_after_first_slot_ack_retains_progress_and_prevents_next_slot() {
    let mut steps = read_steps(&[1, 9, 7], &[("device9", "owner"), ("device7", "owner")]);
    let read_count = steps.len();
    steps.extend([Step::read(GET_STAGES, 9, STAGE_900), Step::write(9)]);
    let mut script = Script::new(steps);
    let calls = Rc::clone(&script.calls);
    let checks = Cell::new(0);
    let report = execute(226, 228, &mut script, &request(), &|| {
        // The setter's own completion validation runs first. Stop at the
        // shared slot task's next boundary, after its ACK has been retained.
        if calls.get() == read_count + 2 {
            checks.set(checks.get() + 1);
            ensure!(checks.get() == 1, "canceled after actual ACK");
        }
        Ok(())
    })
    .unwrap();
    script.done();
    assert!(!report.completed);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].writes.len(), 1);
    assert!(report.profiles[0].writes[0].acknowledged.is_some());
    assert_eq!(report.profiles[0].differing_slots, [9, 7]);
    assert!(report.error.as_deref().unwrap().contains("canceled"));
}
