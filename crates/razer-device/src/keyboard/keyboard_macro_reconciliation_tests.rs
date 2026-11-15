use super::*;
use crate::{
    keyboard_control_test_transport::{Mock, reply, sequence},
    keyboard_macro_test_support::*,
};
use serde_json::json;

fn document(guid: &str, name: &str) -> LocalDocument {
    LocalDocument::parse(&json!({"guid":guid,"name":name,"macroEvents":[],
        "appEngine":{"mouseMoveType":"none","events":[
            {"type":"keyboard","scancode":30,"flag":0},
            {"type":"keyboard","scancode":30,"flag":1}]}}))
    .unwrap()
}

#[test]
fn calculated_replacement_uses_delete_storage_add_and_preserves_ack_distinction() {
    let mut reading = observation(550, true);
    reading.snapshot.macros.inventory.storage.available_memory = 2048;
    let cap = cap(550);
    let mut replies = vec![reply(&[]), storage(2048, 512), storage(2048, 512)];
    replies.extend((0..6).map(|_| reply(&[])));
    replies.push(storage(1536, 512));
    let transport = Mock::new(replies);
    let requirements = Requirements {
        referenced_guids: vec![GUID.into()],
        ..Default::default()
    };
    let mut runtime = device::DeviceRuntime::new(
        &transport,
        &cap,
        &reading,
        &requirements,
        BTreeSet::new(),
        sequence(),
        || Ok(()),
        || Ok(8),
        |_| Ok(()),
    )
    .unwrap();
    let report = reconcile(
        &reading,
        &requirements,
        &[document(GUID, "edited")],
        &mut runtime,
        &|| Ok(()),
    )
    .unwrap();
    assert!(report.phase_exhausted);
    assert!(report.caught_errors.is_empty());
    assert_eq!(runtime.receipt.projected_catalog[0].id, 8);
    assert_eq!(runtime.receipt.projected_catalog[0].name, "edited");
    assert!(
        runtime
            .receipt
            .commands
            .iter()
            .all(|entry| entry.report.acknowledged())
    );
    assert!(
        runtime
            .receipt
            .commands
            .iter()
            .all(|entry| !entry.report.verified())
    );
    let commands = transport
        .sent()
        .iter()
        .map(|frame| [frame[7], frame[8]])
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            [6, 3],
            [6, 142],
            [6, 142],
            [6, 8],
            [6, 12],
            [6, 12],
            [6, 12],
            [6, 12],
            [6, 9],
            [6, 142]
        ]
    );
}

#[derive(Default)]
struct Events {
    actions: Vec<String>,
}
impl Runtime for Events {
    fn delete_macro(&mut self, id: u16) -> Result<()> {
        self.actions.push(format!("delete:{id}"));
        Ok(())
    }
    fn add_macro(&mut self, compiled: &CompiledMacro, _: Addition) -> Result<()> {
        self.actions
            .push(format!("add:{}", compiled.request().name));
        Ok(())
    }
    fn enqueue_macro_import(
        &mut self,
        documents: Vec<onboard_macro::import::Document>,
    ) -> Result<()> {
        self.actions.push(format!("import:{}", documents[0].name()));
        Ok(())
    }
    fn enqueue_profile_import(&mut self) -> Result<()> {
        self.actions.push("profiles".into());
        Ok(())
    }
    fn record_failed_macro(&mut self, guid: &str, _: &str) {
        self.actions.push(format!("failed:{guid}"));
    }
}

#[test]
fn source_queue_order_and_last_eligible_duplicate_are_retained() {
    let reading = observation(550, true);
    let new_guid = "22113344-5566-7788-99aa-bbccddeeff12";
    let requirements = Requirements {
        new_device_guids: vec![new_guid.into()],
        hardware_reference_guids: BTreeSet::from([GUID.into()]),
        profile_import_pending: true,
        ..Default::default()
    };
    let unsupported = LocalDocument::parse(&json!({"guid":new_guid,"name":"unsupported",
        "appEngine":{"mouseMoveType":"absolute","events":[]}}))
    .unwrap();
    let mut events = Events::default();
    reconcile(
        &reading,
        &requirements,
        &[
            document(new_guid, "first"),
            document(new_guid, "last eligible"),
            unsupported,
        ],
        &mut events,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        events.actions,
        ["import:old", "profiles", "delete:7", "add:last eligible"]
    );
}

#[test]
fn cancelled_owner_stops_before_queued_local_imports_or_device_changes() {
    let reading = observation(550, true);
    let requirements = Requirements {
        hardware_reference_guids: BTreeSet::from([GUID.into()]),
        profile_import_pending: true,
        ..Default::default()
    };
    let mut events = Events::default();
    let failure = reconcile(&reading, &requirements, &[], &mut events, &|| {
        anyhow::bail!("canceled")
    })
    .unwrap_err();
    assert!(!failure.progress.phase_exhausted);
    assert!(events.actions.is_empty());
}

#[test]
fn standby_is_deleted_only_after_capacity_failure_and_then_upload_retries() {
    let mut reading = observation(550, true);
    reading.snapshot.macros.inventory.storage.available_memory = 1024;
    let cap = cap(550);
    let mut replies = vec![
        storage(1024, 0),
        reply(&[]),
        storage(1024, 512),
        storage(1024, 512),
    ];
    replies.extend((0..6).map(|_| reply(&[])));
    replies.push(storage(512, 0));
    let transport = Mock::new(replies);
    let requirements = Requirements::default();
    let mut runtime = device::DeviceRuntime::new(
        &transport,
        &cap,
        &reading,
        &requirements,
        BTreeSet::from([7]),
        sequence(),
        || Ok(()),
        || Ok(8),
        |_| Ok(()),
    )
    .unwrap();
    runtime
        .add_macro(
            &document(GUID, "new").compile(550).unwrap().unwrap(),
            Addition::NewWithStandbyCleanup,
        )
        .unwrap();
    assert_eq!(runtime.receipt.commands.len(), 3);
    assert!(runtime.receipt.commands[0].report.steps.is_empty());
    assert!(runtime.receipt.commands[0].error.is_some());
    assert!(runtime.receipt.commands[1].deleting);
    assert_eq!(runtime.receipt.projected_catalog.len(), 1);
    assert_eq!(runtime.receipt.projected_catalog[0].id, 8);
}

#[test]
fn source_name_error_still_writes_data_and_preserves_failed_ack_without_readback() {
    let mut reading = observation(550, false);
    reading.snapshot.macros.inventory.storage.available_memory = 2048;
    let cap = cap(550);
    let mut rejected = reply(&[]);
    rejected.status = 3;
    let mut replies = vec![storage(2048, 0), reply(&[])];
    replies.extend((0..40).map(|_| rejected.clone()));
    replies.extend((0..4).map(|_| reply(&[])));
    replies.push(storage(1536, 0));
    let transport = Mock::new(replies);
    let mut runtime = device::DeviceRuntime::new(
        &transport,
        &cap,
        &reading,
        &Requirements::default(),
        BTreeSet::new(),
        sequence(),
        || Ok(()),
        || Ok(8),
        |_| Ok(()),
    )
    .unwrap();
    runtime
        .add_macro(
            &document(GUID, "new").compile(550).unwrap().unwrap(),
            Addition::New,
        )
        .unwrap();
    let receipt = &runtime.receipt.commands[0];
    assert!(receipt.error.is_some());
    assert!(!receipt.report.acknowledged());
    assert!(!receipt.report.verified());
    assert!(receipt.report.local_write_returned);
    assert_eq!(receipt.report.steps.last().unwrap().command, [80, 6, 9]);
    assert_eq!(runtime.receipt.projected_catalog.len(), 1);
}

#[test]
fn zero_id_standby_macro_is_not_evicted() {
    let mut reading = observation(550, true);
    reading.snapshot.macros.inventory.storage.available_memory = 1024;
    reading.snapshot.macros.inventory.ids[0] = 0;
    reading.snapshot.macros.macros[0].metadata.macro_id = 0;
    reading.import_details.macros[0].macro_id = 0;
    let cap = cap(550);
    let transport = Mock::new([storage(1024, 0)]);
    let mut runtime = device::DeviceRuntime::new(
        &transport,
        &cap,
        &reading,
        &Requirements::default(),
        BTreeSet::from([0]),
        sequence(),
        || Ok(()),
        || Ok(8),
        |_| Ok(()),
    )
    .unwrap();
    assert!(
        runtime
            .add_macro(
                &document(GUID, "new").compile(550).unwrap().unwrap(),
                Addition::NewWithStandbyCleanup
            )
            .is_err()
    );
    assert_eq!(transport.sent().len(), 1);
    assert_eq!(runtime.receipt.projected_catalog[0].id, 0);
}

#[test]
fn firmware_runtime_uses_the_observed_reconstruction_chain() {
    for product in [579, 600, 602, 697] {
        let reading = observation(product, false);
        let cap = cap(product);
        let transport = Mock::new(reconstruction_replies(&reading, &[2]));
        let mut runtime = device::DeviceRuntime::new(
            &transport,
            &cap,
            &reading,
            &Requirements::default(),
            BTreeSet::new(),
            sequence(),
            || Ok(()),
            || Ok(8),
            |_| Ok(()),
        )
        .unwrap();
        runtime
            .add_macro(
                &document(GUID, "new").compile(product).unwrap().unwrap(),
                Addition::New,
            )
            .unwrap();
        assert!(runtime.receipt.firmware_writes[0].confirmed());
        assert!(runtime.receipt.firmware_writes[0].format_confirmed());
        assert_eq!(runtime.receipt.projected_catalog[0].id, 8);
    }
}

#[test]
fn native_interruption_survives_receipt_wrappers_and_stops_the_macro_phase() {
    for product in [550, 600] {
        let mut reading = observation(product, false);
        reading.snapshot.macros.inventory.storage.available_memory = 2048;
        let cap = cap(product);
        let transport = Mock::new([storage(2048, 0)]).interrupt_next_read();
        let requirements = Requirements {
            new_device_guids: vec![GUID.into(), "22113344-5566-7788-99aa-bbccddeeff12".into()],
            ..Default::default()
        };
        let documents = [
            document(GUID, "first"),
            document(&requirements.new_device_guids[1], "second"),
        ];
        let mut runtime = device::DeviceRuntime::new(
            &transport,
            &cap,
            &reading,
            &requirements,
            BTreeSet::new(),
            sequence(),
            || Ok(()),
            || Ok(8),
            |_| Ok(()),
        )
        .unwrap();
        let failure = reconcile(
            &reading,
            &requirements,
            &documents,
            &mut runtime,
            &|| Ok(()),
        )
        .unwrap_err();
        assert!(!failure.progress.phase_exhausted);
        assert!(failure.progress.caught_errors.is_empty());
        assert_eq!(transport.sent().len(), 1);
        assert!(runtime.receipt.projected_catalog.is_empty());
    }
}
