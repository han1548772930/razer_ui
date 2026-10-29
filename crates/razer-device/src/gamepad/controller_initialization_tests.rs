use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Step {
    header: [u8; 3],
    request: Vec<u8>,
    response: Vec<u8>,
    status: u8,
}
fn step(header: [u8; 3], request: &[u8], response: &[u8]) -> Step {
    Step {
        header,
        request: request.to_vec(),
        response: response.to_vec(),
        status: 2,
    }
}
struct Transport {
    selector: u8,
    steps: RefCell<VecDeque<Step>>,
    requests: RefCell<Vec<Vec<u8>>>,
    pending: RefCell<Option<Vec<u8>>>,
}
impl Transport {
    fn new(selector: u8, steps: impl IntoIterator<Item = Step>) -> Self {
        let mut steps = steps.into_iter().collect::<VecDeque<_>>();
        if selector == 4 && !steps.is_empty() {
            // connectControllerRzDevice's current 2636 read precedes Rt.
            steps.push_front(step([4, 0, 147], &[], &[1, 0, 2, 0]));
        }
        Self {
            selector,
            steps: RefCell::new(steps),
            requests: RefCell::new(Vec::new()),
            pending: RefCell::new(None),
        }
    }
    fn finished(&self) {
        assert!(self.steps.borrow().is_empty());
        assert!(self.pending.borrow().is_none());
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Expected collection Feature transfer")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Expected collection Feature transfer")
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        let step = self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("Unexpected controller transfer");
        assert_eq!(report.len(), 91);
        assert_eq!(report[0], self.selector);
        assert_eq!(report[2] as usize, self.requests.borrow().len() % 31);
        assert_eq!(&report[6..9], &step.header);
        assert_eq!(&report[9..9 + step.request.len()], &step.request);
        assert!(
            report[9 + step.request.len()..89]
                .iter()
                .all(|byte| *byte == 0)
        );
        let mut reply = vec![0; 91];
        reply[0] = self.selector;
        reply[1] = step.status;
        reply[2] = report[2];
        reply[6] = step.response.len() as u8;
        reply[7..9].copy_from_slice(&step.header[1..]);
        reply[9..9 + step.response.len()].copy_from_slice(&step.response);
        self.pending.replace(Some(reply));
        self.requests.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .pending
            .borrow_mut()
            .take()
            .expect("Expected a request before its reply");
        report.copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn plan(roots: Vec<RootField>, fields: Vec<ProfileField>) -> InitializationPlan {
    InitializationPlan {
        roots,
        profiles: Vec::new(),
        new_profile_fields: fields,
        connection_features: None,
    }
}
fn inventory(ids: &[u8], active: u8) -> Vec<Step> {
    let mut list = vec![ids.len() as u8];
    list.extend_from_slice(ids);
    let mut result = vec![
        step([80, 2, 132], &[], &[2, 6, 5]),
        step([80, 5, 129], &[], &list),
        step([1, 5, 132], &[], &[active]),
        step([1, 5, 128], &[], &[4]),
    ];
    for id in ids {
        result.push(step(
            [80, 5, 136],
            &[*id, 0, 0, 0, 0],
            &[*id, 0, 0, 0, 4, b'A', 0, 0, 0],
        ));
    }
    result
}

fn retained_directory() -> ProfileDirectory {
    ProfileDirectory {
        button_ids: vec![16],
        profile_ids: vec![3, 1],
        active_profile: 1,
        number_of_profiles: 2,
        names: vec![
            ProfileName {
                slot: 3,
                name: Read::Observed {
                    value: "Real three".into(),
                },
            },
            ProfileName {
                slot: 1,
                name: Read::Observed {
                    value: "Real one".into(),
                },
            },
        ],
    }
}

#[test]
fn controller_initialization_socd_engine_zero_is_not_mistaken_for_wire_one_or_missing_defaults() {
    let mut directory = retained_directory();
    directory.profile_ids[0] = 0;
    directory.names[0].slot = 0;
    let io = Transport::new(10, [step([2, 12, 155], &[1], &[1, 2])]);
    let plan = InitializationPlan::connection_features(Some(directory.clone()), vec![1]);
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(&plan, || false)
        .unwrap();
    assert_eq!(
        result
            .connection_features
            .unwrap()
            .socd
            .iter()
            .map(|row| row.slot)
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(io.requests.borrow().len(), 1);
    io.finished();
    assert!(
        InitializationPlan::connection_features(Some(directory), vec![0])
            .validate(2676)
            .is_err()
    );
}

#[test]
fn controller_initialization_socd_connection_phase_uses_cached_engine_slots_without_new_import_reads()
 {
    let io = Transport::new(
        10,
        [
            step([2, 12, 155], &[3], &[3, 4]),
            step([2, 12, 155], &[1], &[1, 1]),
        ],
    );
    let plan = InitializationPlan::connection_features(Some(retained_directory()), vec![1, 3]);
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(&plan, || false)
        .unwrap();
    assert!(matches!(result.directory, Read::Unobserved));
    assert!(matches!(result.connection_probe, Read::Unobserved));
    assert!(matches!(result.power_saving, Read::Unobserved));
    assert!(result.profiles.is_empty());
    let features = result.connection_features.unwrap();
    assert!(matches!(features.refreshed_directory, Read::Unobserved));
    assert_eq!(
        features
            .socd
            .iter()
            .map(|read| read.slot)
            .collect::<Vec<_>>(),
        [3, 1]
    );
    assert_eq!(
        features.socd[0].result.value().unwrap().mode(),
        Some(SocdMode::AbsoluteUp)
    );
    assert_eq!(
        features.socd[1].result.value().unwrap().mode(),
        Some(SocdMode::Last)
    );
    assert_eq!(io.requests.borrow().len(), 2);
    io.finished();
}

#[test]
fn controller_initialization_socd_uninitialized_engine_reads_directory_then_only_matching_local_profiles()
 {
    let mut steps = inventory(&[3, 1], 1);
    steps.push(step([2, 12, 155], &[1], &[1, 2]));
    let io = Transport::new(10, steps);
    let plan = InitializationPlan::connection_features(None, vec![1, 9]);
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(&plan, || false)
        .unwrap();
    assert!(matches!(result.directory, Read::Unobserved));
    let features = result.connection_features.unwrap();
    assert_eq!(
        features.refreshed_directory.value().unwrap().profile_ids,
        [3, 1]
    );
    assert_eq!(features.socd.len(), 1);
    assert_eq!(features.socd[0].slot, 1);
    assert_eq!(
        features.socd[0].result.value().unwrap().mode(),
        Some(SocdMode::First)
    );
    io.finished();
}

#[test]
fn controller_initialization_socd_failed_reply_and_unknown_value_are_not_neutral_defaults() {
    let io = Transport::new(
        10,
        [
            step([2, 12, 155], &[3], &[1, 0]),
            step([2, 12, 155], &[1], &[1, 255]),
        ],
    );
    let plan = InitializationPlan::connection_features(Some(retained_directory()), vec![3, 1]);
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(&plan, || false)
        .unwrap();
    let features = result.connection_features.unwrap();
    assert!(
        matches!(&features.socd[0].result,Read::Failed{error} if error.contains("another real slot"))
    );
    assert_eq!(features.socd[1].result.value().unwrap().raw_option, 255);
    assert_eq!(features.socd[1].result.value().unwrap().mode(), None);
    io.finished();
    for raw in 0..=4 {
        let expected = [
            SocdMode::Neutral,
            SocdMode::Last,
            SocdMode::First,
            SocdMode::None,
            SocdMode::AbsoluteUp,
        ][raw as usize];
        assert_eq!(SocdObservation { raw_option: raw }.mode(), Some(expected));
    }
}

#[test]
fn controller_initialization_socd_cancellation_keeps_attempted_prefix_and_2636_mount_stays_disabled()
 {
    let io = Transport::new(10, [step([2, 12, 155], &[3], &[3, 0])]);
    let plan = InitializationPlan::connection_features(Some(retained_directory()), vec![3, 1]);
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(&plan, || !io.requests.borrow().is_empty())
        .unwrap();
    assert!(result.canceled);
    let features = result.connection_features.unwrap();
    assert!(features.canceled);
    assert_eq!(features.socd.len(), 1);
    io.finished();
    assert!(plan.validate(2636).is_err());
    let mut mixed = plan.clone();
    mixed.roots.push(RootField::PowerSaving);
    assert!(mixed.validate(2676).is_err());
}

#[test]
fn controller_initialization_reads_roots_then_real_directory_and_each_real_slot_in_order() {
    for (product_id, selector, seconds) in [(2636, 4, [0x2d, 0x01]), (2676, 10, [0x01, 0x2d])] {
        let mut steps = vec![step([2, 7, 131], &[0, 0], &seconds)];
        let mut roots = vec![RootField::PowerSaving];
        if product_id == 2676 {
            roots.push(RootField::ControllerMode);
            steps.push(step([1, 0, 211], &[], &[2]));
        }
        steps.extend(inventory(&[3, 1], 1));
        for slot in [3, 1] {
            steps.push(step([3, 12, 146], &[slot, 1], &[slot, 1, 0]));
        }
        let io = Transport::new(selector, steps);
        let result = InitializationProtocol::new(&io, product_id, |_| Ok(()))
            .unwrap()
            .execute(
                &plan(roots, vec![ProfileField::ControllerAnalogCircularity]),
                || false,
            )
            .unwrap();
        assert_eq!(result.power_saving.value().unwrap().seconds(), 301);
        if product_id == 2636 {
            assert!(result.connection_probe.value().is_some());
            assert_eq!(&io.requests.borrow()[0][6..9], &[4, 0, 147]);
        } else {
            assert!(matches!(result.connection_probe, Read::Unobserved));
        }
        assert_eq!(result.directory.value().unwrap().profile_ids, [3, 1]);
        assert_eq!(result.directory.value().unwrap().button_ids, [6, 5]);
        assert_eq!(result.directory.value().unwrap().active_profile, 1);
        assert_eq!(result.directory.value().unwrap().number_of_profiles, 4);
        assert_eq!(
            result.profiles.iter().map(|p| p.slot).collect::<Vec<_>>(),
            [3, 1]
        );
        assert!(
            io.requests.borrow().iter().all(|request| request[8] >= 128),
            "The importer must not send a default setter"
        );
        io.finished();
    }
}

#[test]
fn controller_initialization_zero_source_directory_and_names_stay_raw_while_every_profile_field_reads_wire_one()
 {
    for (product_id, selector) in [(2636, 4), (2676, 10)] {
        let mut fields = vec![];
        let mut steps = inventory(&[0], 0);
        if product_id == 2636 {
            fields.extend([
                ProfileField::Brightness,
                ProfileField::ControllerLightingEffects,
            ]);
            steps.extend([
                step([80, 15, 128], &[], &[0]),
                step([3, 15, 132], &[1, 0], &[1, 0, 128]),
                step([80, 15, 130], &[1, 0], &[1, 0, 4, 0, 2, 0]),
            ]);
        }
        fields.push(ProfileField::PollingRate);
        let command = if product_id == 2636 { 142 } else { 192 };
        let code = if product_id == 2636 { 1 } else { 8 };
        steps.push(step([2, 0, command], &[1], &[1, code]));
        if product_id == 2636 {
            fields.push(ProfileField::PollingRateWireless);
            steps.push(step([2, 0, 142], &[1], &[1, 1]));
        }
        fields.push(ProfileField::Mappings);
        for button in [6, 5] {
            steps.push(step(
                [80, 2, 140],
                &[1, button, 0],
                &[1, button, 0, 0, 0, 0, 0, 0, 0, 0],
            ));
        }
        fields.push(ProfileField::ControllerDigitalActuation);
        for side in [1, 2] {
            steps.push(step([4, 12, 150], &[1, side], &[1, side, 50, 1]));
            steps.push(step([3, 12, 149], &[1, side], &[1, side, 1]));
        }
        fields.push(ProfileField::ControllerAnalogActuation);
        for side in [1, 2] {
            steps.push(step([4, 12, 147], &[1, side], &[1, side, 0, 100]));
            steps.push(step([3, 12, 149], &[1, side], &[1, side, 1]));
        }
        fields.push(ProfileField::ControllerAnalogCircularity);
        steps.push(step([3, 12, 146], &[1, 1], &[1, 1, 0]));
        fields.push(ProfileField::ControllerDeadzone);
        for side in [1, 2] {
            steps.push(step(
                [9, 12, 133],
                &[1, side],
                &[1, side, 0, 20, 0, 0, 0, 0, 1],
            ));
        }
        fields.push(ProfileField::ControllerDPadIgnoreDiagonals);
        steps.push(step([2, 12, 151], &[1], &[1, 1]));
        fields.push(ProfileField::ControllerSensitivity);
        for side in [1, 2] {
            steps.push(step([3, 12, 130], &[1, side], &[1, side, 50]));
        }
        fields.push(ProfileField::HapticIntensity);
        steps.push(step([3, 12, 134], &[1, 5], &[1, 5, 128]));
        let request = plan(vec![], fields);
        let io = Transport::new(selector, steps);
        let mut result = InitializationProtocol::new(&io, product_id, |_| Ok(()))
            .unwrap()
            .execute(&request, || false)
            .unwrap();
        let directory = result.directory.value().unwrap();
        assert_eq!(directory.profile_ids, [0]);
        assert_eq!(directory.active_profile, 0);
        assert_eq!(directory.names[0].slot, 0);
        assert_eq!(result.profiles[0].slot, 0);
        assert!(
            result.profiles[0]
                .fields
                .iter()
                .all(|read| matches!(read.result, Read::Observed { .. }))
        );
        for read in &result.profiles[0].fields {
            match read.result.value().unwrap() {
                ProfileValue::Polling { observation, .. } => {
                    assert_eq!(observation.profile_slot, 1)
                }
                ProfileValue::Mappings { observation } => assert_eq!(observation.profile_slot, 1),
                ProfileValue::Lighting { effect } => assert_eq!(effect.profile_slot, 1),
                _ => {}
            }
        }
        let name = io
            .requests
            .borrow()
            .iter()
            .find(|bytes| bytes[7..9] == [5, 136])
            .unwrap()
            .clone();
        assert_eq!(name[9], 0, "Names retain the raw directory slot");
        result.validate_for(product_id, &request).unwrap();
        let polling = result.profiles[0]
            .fields
            .iter_mut()
            .find(|read| read.field == ProfileField::PollingRate)
            .unwrap();
        let Read::Observed {
            value: ProfileValue::Polling { observation, .. },
        } = &mut polling.result
        else {
            panic!("missing polling observation")
        };
        observation.profile_slot = 0;
        assert!(
            result.validate_for(product_id, &request).is_err(),
            "Reply wire identity must not be relabeled as the source slot"
        );
        io.finished();
    }
}

#[test]
fn controller_initialization_preserves_root_and_profile_field_order_instead_of_grouped_getters() {
    let mut steps = vec![
        step([1, 0, 211], &[], &[0]),
        step([2, 7, 131], &[0, 0], &[0x01, 0x2c]),
    ];
    steps.extend(inventory(&[7], 7));
    steps.extend([
        step([3, 12, 130], &[7, 1], &[7, 1, 41]),
        step([3, 12, 130], &[7, 2], &[7, 2, 52]),
        step([2, 12, 151], &[7], &[7, 1]),
        step([3, 12, 146], &[7, 1], &[7, 1, 1]),
    ]);
    let io = Transport::new(10, steps);
    let fields = vec![
        ProfileField::ControllerSensitivity,
        ProfileField::ControllerDPadIgnoreDiagonals,
        ProfileField::ControllerAnalogCircularity,
    ];
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(
            &plan(
                vec![RootField::ControllerMode, RootField::PowerSaving],
                fields.clone(),
            ),
            || false,
        )
        .unwrap();
    assert_eq!(result.controller_mode.value(), Some(&ControllerMode::Hid));
    assert_eq!(
        result.profiles[0]
            .fields
            .iter()
            .map(|read| read.field)
            .collect::<Vec<_>>(),
        fields
    );
    io.finished();
}

#[test]
fn controller_initialization_profile_names_use_two_chunks_and_restore_normal_delays() {
    let mut steps = inventory(&[9], 9);
    steps.pop();
    let mut first = vec![9, 0, 0, 0, 80];
    first.extend([0x2d, 0x4e].repeat(32));
    let mut second = vec![9, 0, 64, 0, 80];
    second.extend([0x87, 0x65].repeat(7));
    second.extend([0, 0]);
    steps.push(step([80, 5, 136], &[9, 0, 0, 0, 0], &first));
    steps.push(step([80, 5, 136], &[9, 0, 64, 0, 0], &second));
    steps.push(step([3, 12, 146], &[9, 1], &[9, 1, 0]));
    let io = Transport::new(4, steps);
    let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
    let waits = RefCell::new(Vec::new());
    let result = InitializationProtocol::new(&probe, 2636, |delay| {
        waits.borrow_mut().push(delay.as_millis());
        Ok(())
    })
    .unwrap()
    .execute(
        &plan(vec![], vec![ProfileField::ControllerAnalogCircularity]),
        || false,
    )
    .unwrap();
    assert_eq!(
        result.directory.value().unwrap().names[0]
            .name
            .value()
            .unwrap(),
        &format!("{}{}", "中".repeat(32), "文".repeat(7))
    );
    assert_eq!(
        *waits.borrow(),
        [0, 10, 0, 10, 0, 10, 0, 10, 0, 10, 0, 20, 0, 20, 0, 10]
    );
    assert_eq!(
        probe.timings(),
        vec![
            crate::backend::ExchangeTiming::MutexOut {
                interval: std::time::Duration::from_millis(10),
            };
            8
        ],
    );
    io.finished();
}

#[test]
fn controller_initialization_failed_root_and_unknown_mode_keep_failures_without_default_setters() {
    let mut power = step([2, 7, 131], &[0, 0], &[]);
    power.status = 5;
    let mut steps = vec![power, step([1, 0, 211], &[], &[3])];
    steps.extend(inventory(&[1], 1));
    let io = Transport::new(10, steps);
    let result = InitializationProtocol::new(&io, 2676, |_| Ok(()))
        .unwrap()
        .execute(
            &plan(
                vec![RootField::PowerSaving, RootField::ControllerMode],
                vec![],
            ),
            || false,
        )
        .unwrap();
    assert!(matches!(result.power_saving, Read::Failed { .. }));
    assert!(matches!(result.controller_mode, Read::Failed { .. }));
    assert!(result.directory.value().is_some());
    assert!(io.requests.borrow().iter().all(|request| request[8] >= 128));
    io.finished();
}

#[test]
fn controller_initialization_name_failures_are_not_successful_empty_names() {
    let mut steps = inventory(&[1], 1);
    steps.pop();
    for _ in 0..3 {
        let mut failed = step([80, 5, 136], &[1, 0, 0, 0, 0], &[]);
        failed.status = 5;
        steps.push(failed);
    }
    let io = Transport::new(4, steps);
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan(vec![], vec![]), || false)
        .unwrap();
    assert!(matches!(
        result.directory.value().unwrap().names[0].name,
        Read::Failed { .. }
    ));
    assert_eq!(io.requests.borrow().len(), 8);
    io.finished();
}

#[test]
fn controller_initialization_truncated_directory_stops_before_synthetic_slots() {
    let io = Transport::new(4, [step([80, 2, 132], &[], &[3, 6])]);
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan(vec![], vec![]), || false)
        .unwrap();
    assert!(matches!(result.directory, Read::Failed { .. }));
    assert!(result.profiles.is_empty());
    io.finished();
}

#[test]
fn controller_initialization_zero_profile_directory_does_not_create_a_default_slot() {
    let io = Transport::new(4, inventory(&[], 0));
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan(vec![], vec![]), || false)
        .unwrap();
    assert!(matches!(result.directory, Read::Failed { .. }));
    assert!(result.profiles.is_empty());
    assert_eq!(io.requests.borrow().len(), 5);
    io.finished();
}

#[test]
fn controller_initialization_failed_dock_probe_is_unknown_and_does_not_write_or_guess_wired() {
    let mut steps = vec![step([2, 7, 131], &[0, 0], &[0x2d, 0x01])];
    steps.extend(inventory(&[1], 1));
    let io = Transport::new(4, steps);
    io.steps.borrow_mut().front_mut().unwrap().status = 5;
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan(vec![RootField::PowerSaving], vec![]), || false)
        .unwrap();
    assert!(matches!(result.connection_probe, Read::Failed { .. }));
    assert_eq!(result.power_saving.value().unwrap().seconds(), 301);
    assert!(result.directory.value().is_some());
    assert!(io.requests.borrow().iter().all(|request| request[8] >= 128));
    io.finished();
}

#[test]
fn controller_initialization_partial_feature_failure_keeps_its_failure_and_reads_later_fields() {
    let mut steps = inventory(&[1], 1);
    let mut failed = step([4, 12, 150], &[1, 1], &[]);
    failed.status = 5;
    steps.push(failed);
    steps.push(step([3, 12, 146], &[1, 1], &[1, 1, 0]));
    let io = Transport::new(4, steps);
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(
            &plan(
                vec![],
                vec![
                    ProfileField::ControllerDigitalActuation,
                    ProfileField::ControllerAnalogCircularity,
                ],
            ),
            || false,
        )
        .unwrap();
    assert!(matches!(
        result.profiles[0].fields[0].result,
        Read::Failed { .. }
    ));
    assert!(matches!(
        result.profiles[0].fields[1].result,
        Read::Observed {
            value: ProfileValue::Circularity { option: 0 }
        }
    ));
    assert!(io.requests.borrow().iter().all(|request| request[8] >= 128));
    io.finished();
}

#[test]
fn controller_initialization_cancellation_keeps_observed_root_and_prevents_later_transfers() {
    let io = Transport::new(4, [step([2, 7, 131], &[0, 0], &[0x2d, 0x01])]);
    let checkpoints = Cell::new(0);
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan(vec![RootField::PowerSaving], vec![]), || {
            let n = checkpoints.get();
            checkpoints.set(n + 1);
            n > 1
        })
        .unwrap();
    assert!(result.canceled);
    assert_eq!(result.power_saving.value().unwrap().seconds(), 301);
    assert!(matches!(result.directory, Read::Unobserved));
    assert!(result.profiles.is_empty());
    io.finished();
    let io = Transport::new(4, []);
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan(vec![RootField::PowerSaving], vec![]), || true)
        .unwrap();
    assert!(result.canceled);
    assert!(matches!(result.power_saving, Read::Unobserved));
    io.finished();
}

#[test]
fn controller_initialization_rejects_duplicate_or_cross_product_plans_and_forged_field_results() {
    assert!(
        plan(vec![RootField::ControllerMode], vec![])
            .validate(2636)
            .is_err()
    );
    assert!(
        plan(vec![RootField::PowerSaving, RootField::PowerSaving], vec![])
            .validate(2676)
            .is_err()
    );
    assert!(
        plan(vec![], vec![ProfileField::Brightness])
            .validate(2676)
            .is_err()
    );
    let mut steps = inventory(&[1], 1);
    steps.push(step([3, 12, 146], &[1, 1], &[1, 1, 0]));
    let io = Transport::new(4, steps);
    let plan = plan(vec![], vec![ProfileField::ControllerAnalogCircularity]);
    let result = InitializationProtocol::new(&io, 2636, |_| Ok(()))
        .unwrap()
        .execute(&plan, || false)
        .unwrap();
    let mut forged = result.clone();
    forged.profiles[0].slot = 2;
    assert!(forged.validate_for(2636, &plan).is_err());
    let mut forged = result.clone();
    forged.profiles[0].fields.clear();
    assert!(forged.validate_for(2636, &plan).is_err());
    let mut forged = result;
    forged.profiles[0].fields[0].result = Read::Observed {
        value: ProfileValue::Dpad { option: 0 },
    };
    assert!(forged.validate_for(2636, &plan).is_err());
}
