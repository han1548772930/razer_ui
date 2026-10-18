use super::*;

fn make_work(query: Query) -> (Work, Transfer) {
    let transfer = Transfer::new();
    let work = Work {
        id: 1,
        product_id: 737,
        query,
        file_name: "Measured system - 123.synapse4".into(),
        window: 12,
        cancelled: Arc::new(AtomicBool::new(false)),
        operation: transfer.accept().unwrap(),
    };
    (work, transfer)
}
fn response(query: Query) -> Response {
    Response { product_id: 737, data: Data::decode(query, match query {
        Query::Display => br#"{"DeviceName":"Observed host","CPU":"Observed CPU","Display":null}"#,
        Query::Full => br#"{"Display":{"DeviceName":"Observed host","Display":null},"Firmware":{"BiosFW":"1.2"},"Driver":{"Native driver":"9.1"}}"#,
    }).unwrap() }
}

#[::core::prelude::v1::test]
fn help_system_info_display_uses_native_response_without_opening_a_save_dialog() {
    let (work, _) = make_work(Query::Display);
    let outcome = run_work_with(
        &work,
        &mut None,
        |product, query, _| {
            assert_eq!(product, 737);
            assert_eq!(query, Query::Display);
            Ok(response(query))
        },
        |_, _, _| panic!("Display observation must not open a save dialog"),
    )
    .unwrap();
    let Outcome::Display(details) = outcome else {
        panic!()
    };
    assert_eq!(details.cpu.as_deref(), Some("Observed CPU"));
    assert_eq!(details.device_name.as_deref(), Some("Observed host"));
}

#[::core::prelude::v1::test]
fn help_system_info_full_export_matches_save_dialog_contract_and_replaces_atomically() {
    let directory =
        std::env::temp_dir().join(format!("razer-help-report-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("system.synapse4");
    std::fs::write(&path, b"old valid report").unwrap();
    let (work, _) = make_work(Query::Full);
    let mut last_directory = None;
    let outcome = run_work_with(
        &work,
        &mut last_directory,
        |_, query, _| Ok(response(query)),
        |window, options, _| {
            assert_eq!(window, 12);
            assert_eq!(options.title, "Save as");
            assert_eq!(options.file_name, "Measured system - 123.synapse4");
            assert_eq!(options.button_label, "Save file");
            assert_eq!(options.filter_name, "Synapse 4");
            assert_eq!(options.extension, "synapse4");
            assert!(options.directory.is_none());
            Ok(Some(path.clone()))
        },
    )
    .unwrap();
    assert!(matches!(outcome, Outcome::Saved(ref saved) if saved == &path));
    let expected = br#"{"Display":{"DeviceName":"Observed host","Display":null},"Firmware":{"BiosFW":"1.2"},"Driver":{"Native driver":"9.1"}}"#;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap(),
        serde_json::from_slice::<serde_json::Value>(expected).unwrap()
    );
    assert_eq!(last_directory.as_ref(), Some(&directory));
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[::core::prelude::v1::test]
fn help_system_info_cancellation_prevents_dialog_and_file_publication() {
    let (work, _) = make_work(Query::Full);
    let cancelled = work.cancelled.clone();
    let outcome = run_work_with(
        &work,
        &mut None,
        |_, query, _| {
            cancelled.store(true, Ordering::Release);
            Ok(response(query))
        },
        |_, _, _| panic!("Unmounted Help must not open a save dialog"),
    )
    .unwrap();
    assert!(matches!(outcome, Outcome::Cancelled));

    let (work, _) = make_work(Query::Full);
    let mut directory = None;
    assert!(matches!(
        run_work_with(
            &work,
            &mut directory,
            |_, query, _| Ok(response(query)),
            |_, _, _| Ok(None)
        )
        .unwrap(),
        Outcome::Cancelled
    ));
    assert!(directory.is_none());

    let (work, transfer) = make_work(Query::Full);
    transfer.cancel();
    assert!(
        run_work_with(
            &work,
            &mut None,
            |_, _, _| panic!("Cancelled file transfer cannot start native I/O"),
            |_, _, _| panic!()
        )
        .is_err()
    );
}

#[::core::prelude::v1::test]
fn help_system_info_wrong_reply_and_query_failure_never_open_the_dialog() {
    let (work, _) = make_work(Query::Full);
    for wrong_identity in [true, false] {
        let result = run_work_with(
            &work,
            &mut None,
            |_, _, _| {
                if wrong_identity {
                    Ok(response(Query::Display))
                } else {
                    anyhow::bail!("Local native report failed")
                }
            },
            |_, _, _| panic!("Invalid report must not reach the save dialog"),
        );
        assert!(result.is_err());
    }
}

#[::core::prelude::v1::test]
fn help_system_info_cancelled_exit_queues_new_work_until_the_old_worker_is_joined() {
    let (release, waiting) = mpsc::channel();
    let (commands, _incoming) = mpsc::channel();
    let expected_directory = PathBuf::from("retained-save-directory");
    let returned_directory = expected_directory.clone();
    let previous = thread::spawn(move || {
        waiting.recv().unwrap();
        Some(returned_directory)
    });
    let previous_id = previous.thread().id();
    let mut runtime = Runtime::default();
    runtime.worker = Some(previous);
    runtime.commands = Some(commands);
    runtime.cancel_all();
    assert!(runtime.stopping);
    assert!(runtime.commands.is_none());
    runtime.resume_after_cancelled_exit();
    assert!(!runtime.stopping);
    let (work, _) = make_work(Query::Display);
    work.cancelled.store(true, Ordering::Release);
    runtime.admit(work).unwrap();
    assert_eq!(runtime.pending.len(), 1);
    assert_eq!(runtime.worker.as_ref().unwrap().thread().id(), previous_id);
    release.send(()).unwrap();
    while !runtime.worker.as_ref().unwrap().is_finished() {
        thread::yield_now();
    }
    runtime.reap();
    assert!(runtime.pending.is_empty());
    assert!(runtime.commands.is_some());
    let reply = runtime
        .replies
        .as_ref()
        .unwrap()
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(matches!(reply.result.unwrap(), Outcome::Cancelled));
    runtime.cancel_all();
    assert_eq!(
        runtime.worker.take().unwrap().join().unwrap(),
        Some(expected_directory)
    );
}
