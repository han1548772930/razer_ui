use super::*;
use razer_pages::features::HelpRuntimeVersionStatus;
use serde_json::json;

static WORKER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn make_work() -> Work {
    Work {
        id: 1,
        product_id: 3594,
        container_id: "mounted".into(),
        middleware: Ok(Some(
            r#"[{"windowName":"https://host.example/products/3594/mw/","value":"actual-mw.9"}]"#
                .into(),
        )),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
fn retained_worker(handle: JoinHandle<()>) -> Worker {
    Worker {
        handle,
        activity: WorkerActivity::new(),
    }
}
fn publisher(storage: &mut HostStorage, id: u64, value: &str) {
    storage.register_view(HostStorageView {
        id,
        url: format!("https://host.example/products/3594/mw/?containerId={id}"),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: true,
    });
    storage
        .call(HostStorageCall {
            store: HostStoreKind::Window,
            sender_id: id,
            action: "setWindowStorageItem".into(),
            payload: json!({"key":"mwVersion", "value":value}),
            target_url_array: Vec::new(),
        })
        .unwrap();
}

#[::core::prelude::v1::test]
fn help_reads_the_existing_host_store_and_closes_only_its_query_view() {
    let mut storage = HostStorage::default();
    publisher(&mut storage, 101, "first-runtime.1");
    publisher(&mut storage, 102, "second-runtime.2");
    let raw = read_window_versions(&mut storage, EntityId::from(200))
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap(),
        json!([
            {"windowName":"https://host.example/products/3594/mw/?containerId=101", "value":"first-runtime.1"},
            {"windowName":"https://host.example/products/3594/mw/?containerId=102", "value":"second-runtime.2"}
        ])
    );
    assert!(storage.drain_events(200).is_err());
    assert!(storage.drain_events(101).is_ok());
    assert!(storage.drain_events(102).is_ok());
    assert_eq!(
        read_window_versions(&mut storage, EntityId::from(201))
            .unwrap()
            .as_deref(),
        Some(raw.as_str())
    );
    assert!(storage.drain_events(201).is_err());
}

#[::core::prelude::v1::test]
fn help_query_view_guard_releases_the_view_when_storage_rejects_the_call() {
    let mut storage = HostStorage::default();
    publisher(&mut storage, 101, "keep-runtime");
    let result = (|| -> anyhow::Result<()> {
        let view = ReadView {
            storage: &mut storage,
            id: 101,
        };
        view.storage.call(HostStorageCall {
            store: HostStoreKind::Window,
            sender_id: 101,
            action: "getWindowStorageItem".into(),
            payload: json!({"key":[]}),
            target_url_array: Vec::new(),
        })?;
        Ok(())
    })();
    assert!(result.is_err());
    assert!(storage.drain_events(101).is_err());
}

#[::core::prelude::v1::test]
fn help_worker_cancellation_never_publishes_or_starts_an_unneeded_file_read() {
    let work = make_work();
    work.cancelled.store(true, Ordering::Release);
    assert!(
        run_work_with(work, || panic!(
            "Cancelled Help must not read application storage"
        ))
        .is_none()
    );
    let work = make_work();
    let cancelled = work.cancelled.clone();
    assert!(
        run_work_with(work, || {
            cancelled.store(true, Ordering::Release);
            Ok(Some(r#"[{"name":"synapse","version":"5.6.7"}]"#.into()))
        })
        .is_none()
    );
}

#[::core::prelude::v1::test]
fn help_worker_preserves_independent_storage_results_without_version_fallbacks() {
    let status = run_work_with(make_work(), || Err("actual file read denied".into())).unwrap();
    assert_eq!(
        status.middleware,
        HelpRuntimeVersionStatus::Observed("actual-mw.9".into())
    );
    assert_eq!(
        status.application,
        HelpRuntimeVersionStatus::Failed("actual file read denied".into())
    );
    let mut work = make_work();
    work.middleware = Err("actual window read failed".into());
    let status = run_work_with(work, || {
        Ok(Some(r#"[{"name":"synapse","version":"6.7.8"}]"#.into()))
    })
    .unwrap();
    assert_eq!(
        status.middleware,
        HelpRuntimeVersionStatus::Failed("actual window read failed".into())
    );
    assert_eq!(
        status.application,
        HelpRuntimeVersionStatus::Observed("4.7.8".into())
    );
    let mut storage = HostStorage::default();
    let mut work = make_work();
    work.middleware = read_window_versions(&mut storage, EntityId::from(44));
    let status = run_work_with(work, || Ok(None)).unwrap();
    assert_eq!(status.middleware, HelpRuntimeVersionStatus::Missing);
    assert_eq!(status.application, HelpRuntimeVersionStatus::Missing);
}

struct LocalStore(std::path::PathBuf);
impl LocalStore {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("help-versions-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("source-local-storage.json")
    }
}
impl Drop for LocalStore {
    fn drop(&mut self) {
        for path in [self.path(), self.path().with_extension("json.lock")] {
            if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}

#[::core::prelude::v1::test]
fn help_background_read_consumes_actual_local_app_records_without_mutating_them() {
    let store = LocalStore::new();
    let path = store.path();
    let raw = r#"[{"name":"razer_ui","version":"9.0.0"},{"name":"synapse","version":"6.23.45.7","extension":"keep"}]"#;
    razer_storage::source_local::set_raw(&path, "apps", raw).unwrap();
    let initial_bytes = std::fs::read(&path).unwrap();
    let main_thread = thread::current().id();
    let status = thread::spawn(move || {
        run_work_with(make_work(), || {
            assert_ne!(thread::current().id(), main_thread);
            razer_storage::source_local::get_raw(&path, "apps").map_err(|error| error.to_string())
        })
        .unwrap()
    })
    .join()
    .unwrap();
    assert_eq!(
        status.application,
        HelpRuntimeVersionStatus::Observed("4.23.45.7".into())
    );
    assert_eq!(std::fs::read(store.path()).unwrap(), initial_bytes);
}

#[::core::prelude::v1::test]
fn help_cancelled_exit_waits_for_the_previous_worker_before_resuming_reads() {
    let _guard = WORKER_TEST_LOCK.lock().unwrap();
    let (release, waiting) = mpsc::channel();
    let (commands, _incoming) = mpsc::channel();
    let previous = thread::spawn(move || waiting.recv().unwrap());
    let previous_id = previous.thread().id();
    let mut runtime = Runtime::default();
    runtime.worker = Some(retained_worker(previous));
    runtime.commands = Some(commands);
    runtime.cancel_all();
    assert!(runtime.stopping);
    assert!(runtime.commands.is_none());
    assert!(runtime.admit(make_work()).is_err());
    runtime.resume_after_cancelled_exit();
    let work = make_work();
    work.cancelled.store(true, Ordering::Release);
    runtime.admit(work).unwrap();
    assert_eq!(runtime.pending.len(), 1);
    assert_eq!(
        runtime.worker.as_ref().unwrap().handle.thread().id(),
        previous_id
    );
    release.send(()).unwrap();
    while !runtime.worker.as_ref().unwrap().is_finished() {
        thread::yield_now();
    }
    assert!(runtime.poll().is_empty());
    assert!(runtime.pending.is_empty());
    assert!(runtime.commands.is_some());
    let reply = runtime
        .replies
        .as_ref()
        .unwrap()
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert_eq!(reply.id, 1);
    assert!(reply.result.unwrap().is_none());
    runtime.cancel_all();
    runtime.worker.take().unwrap().join().unwrap();
}

#[::core::prelude::v1::test]
fn help_worker_final_reply_survives_the_thread_exit_boundary() {
    let _guard = WORKER_TEST_LOCK.lock().unwrap();
    let (finished, replies) = mpsc::channel();
    let worker = thread::spawn(move || {
        finished
            .send(Reply {
                id: 71,
                result: Ok(None),
            })
            .unwrap();
    });
    while !worker.is_finished() {
        thread::yield_now();
    }
    let mut runtime = Runtime::default();
    runtime.worker = Some(retained_worker(worker));
    runtime.replies = Some(replies);
    let replies = runtime.poll();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].id, 71);
    assert!(runtime.worker.is_none());
    assert!(runtime.replies.is_none());
}

#[gpui_kit::test]
fn help_worker_panic_is_a_failed_observation_for_the_current_owner(
    cx: &mut gpui_kit::TestAppContext,
) {
    let _guard = WORKER_TEST_LOCK.lock().unwrap();
    use gpui_kit::component::Root;
    use razer_pages::nav::Tab;
    cx.update(gpui_kit::init);
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| {
            ProductWorkspace::new(
                razer_model::product_preview::registered_preview(2684).unwrap(),
                false,
                window,
                cx,
            )
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let workspace = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        workspace.update(cx, |view, cx| {
            view.set_active(true, window, cx);
            view.set_page(Tab::Help, window, cx);
        });
    })
    .unwrap();
    let mut runtime = Runtime::default();
    cx.update(|cx| {
        runtime.sessions.insert(
            19,
            Session {
                request: workspace
                    .read(cx)
                    .help_runtime_versions_request(cx)
                    .unwrap(),
                workspace: workspace.downgrade(),
                device: workspace.read(cx).device(cx).clone(),
                connection_revision: workspace.read(cx).connection_revision(),
            },
        );
    });
    runtime.worker = Some(retained_worker(thread::spawn(|| {
        panic!("simulated storage worker failure")
    })));
    while !runtime.worker.as_ref().unwrap().is_finished() {
        thread::yield_now();
    }
    let reply = runtime.poll().pop().unwrap();
    assert_eq!(reply.id, 19);
    let error = reply.result.unwrap_err();
    assert!(error.contains("panicked"));
    let session = runtime.sessions.remove(&19).unwrap();
    cx.update(|cx| {
        assert!(
            workspace.update(cx, |view, cx| view.observe_help_runtime_versions(
                &session.device,
                HelpRuntimeVersionsObservation::failed(session.request.clone(), error.clone()),
                cx
            ))
        );
        let status = workspace.read(cx).help_runtime_versions_status(cx).unwrap();
        assert_eq!(
            status.middleware,
            HelpRuntimeVersionStatus::Failed(error.clone())
        );
        assert_eq!(status.application, HelpRuntimeVersionStatus::Failed(error));
    });
}

#[::core::prelude::v1::test]
fn help_runtime_drop_retains_the_running_handle_until_actual_join() {
    let _guard = WORKER_TEST_LOCK.lock().unwrap();
    let initial = active_workers();
    let (release, waiting) = mpsc::channel();
    let mut runtime = Runtime::default();
    runtime.worker = Some(retained_worker(thread::spawn(move || {
        waiting.recv().unwrap()
    })));
    assert_eq!(active_workers(), initial + 1);
    drop(runtime);
    assert_eq!(active_workers(), initial + 1);
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while active_workers() != initial && std::time::Instant::now() < deadline {
        thread::park_timeout(Duration::from_millis(1));
    }
    assert_eq!(active_workers(), initial);
    // Only reap completed registry helpers; other camera owners retain their
    // admission and deferred native cleanup.
    razer_service::camera::lifecycle::active_workers();
}
