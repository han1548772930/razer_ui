use super::*;

#[test]
fn diagnostic_shutdown_drains_accepted_records_and_closes_retained_clients() {
    let written = Arc::new(Mutex::new(Vec::new()));
    let output = written.clone();
    let runtime = Runtime::start(8, move |receiver, _| {
        for line in receiver {
            output.lock().unwrap().push(line);
        }
    })
    .unwrap();
    let client = runtime.queue.clone();
    client.record("HID read failed: timeout\n".into());
    client.record("device disconnected\n".into());
    drop(runtime);
    client.record("late record\n".into());
    assert_eq!(
        *written.lock().unwrap(),
        ["HID read failed: timeout\n", "device disconnected\n"]
    );
    assert!(client.sender.lock().unwrap().is_none());
}

#[test]
fn diagnostic_slow_writer_does_not_block_admission_and_overflow_is_counted() {
    let (release, wait) = mpsc::channel();
    let runtime = Runtime::start(1, move |receiver, _| {
        wait.recv().unwrap();
        assert_eq!(receiver.into_iter().collect::<Vec<_>>(), ["first"]);
    })
    .unwrap();
    runtime.queue.record("first".into());
    runtime.queue.record("second".into());
    assert_eq!(runtime.queue.overflow.load(Ordering::Relaxed), 1);
    release.send(()).unwrap();
    drop(runtime);
}

#[test]
fn diagnostic_files_append_and_rotate_with_the_previous_log_preserved() {
    let root = std::env::temp_dir().join(format!(
        "razer-diagnostic-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = root.join("runtime.log");
    {
        let mut writer = FileOutput::open(path.clone()).unwrap();
        writer.append(b"first session\n").unwrap();
    }
    let mut writer = FileOutput::open(path.clone()).unwrap();
    writer.append(b"HID error\n").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"first session\nHID error\n");
    writer
        .append(&vec![b'.'; (MAX_FILE_BYTES - writer.bytes) as usize])
        .unwrap();
    writer.append(b"next session\n").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"next session\n");
    let previous = path.with_extension("previous.log");
    assert_eq!(std::fs::metadata(&previous).unwrap().len(), MAX_FILE_BYTES);
    writer
        .append(&vec![b'.'; (MAX_FILE_BYTES - writer.bytes) as usize])
        .unwrap();
    writer.append(b"third session\n").unwrap();
    drop(writer);
    assert_eq!(std::fs::read(&path).unwrap(), b"third session\n");
    assert!(
        std::fs::read(&previous)
            .unwrap()
            .starts_with(b"next session\n")
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(previous).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn diagnostic_filters_keep_application_status_and_framework_errors() {
    use log::Log;
    let (sender, _) = mpsc::sync_channel(1);
    let logger = Logger(Arc::new(Queue {
        sender: Mutex::new(Some(sender)),
        overflow: AtomicUsize::new(0),
    }));
    let enabled = |level, target| {
        logger.enabled(&log::Metadata::builder().level(level).target(target).build())
    };
    assert!(enabled(log::Level::Info, "razer_ui::status"));
    assert!(enabled(log::Level::Error, "gpui"));
    assert!(!enabled(log::Level::Info, "http_client"));
    assert!(!enabled(log::Level::Debug, "razer_ui::status"));
}
