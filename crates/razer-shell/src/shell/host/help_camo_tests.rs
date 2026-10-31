use super::*;
use razer_platform::camera_apps::{CamoDriver, CamoStudio};
use std::{cell::Cell, rc::Rc, thread};

#[derive(Debug, PartialEq, Eq)]
enum SessionEvent {
    Opened(thread::ThreadId),
    Read(u8, thread::ThreadId),
    Released(thread::ThreadId),
}

struct SerialSession {
    events: mpsc::Sender<SessionEvent>,
    reads: u8,
    panic_on_release: bool,
    _thread_affinity: Rc<()>,
}
impl SerialSession {
    fn new(events: mpsc::Sender<SessionEvent>) -> Self {
        events
            .send(SessionEvent::Opened(thread::current().id()))
            .unwrap();
        Self {
            events,
            reads: 0,
            panic_on_release: false,
            _thread_affinity: Rc::new(()),
        }
    }
}
impl SerialReader for SerialSession {
    fn read(
        &mut self,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<CameraSerialProgress> {
        validate()?;
        self.reads += 1;
        self.events
            .send(SessionEvent::Read(self.reads, thread::current().id()))
            .unwrap();
        anyhow::bail!("scripted wire failure {}", self.reads)
    }
}
impl Drop for SerialSession {
    fn drop(&mut self) {
        let _ = self
            .events
            .send(SessionEvent::Released(thread::current().id()));
        assert!(!self.panic_on_release, "scripted serial release failure");
    }
}

fn signal() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
fn scope() -> CameraScope {
    CameraScope {
        product_id: 3594,
        container_id: "{e528a5e9-5fb3-4e75-a64f-6bab5ad57124}".into(),
    }
}
fn failure(reply: Reply, connection_id: u64, job_id: Option<u64>) -> String {
    match reply {
        Reply::Serial {
            connection,
            id,
            result,
        } => {
            assert_eq!(connection, connection_id);
            assert_eq!(id, job_id);
            result.unwrap_err()
        }
        _ => panic!("expected serial reply"),
    }
}

#[::core::prelude::v1::test]
fn help_retry_and_initial_identity_share_one_thread_owned_session() {
    let owner = Owner::new();
    let cancelled = signal();
    let worker_cancelled = cancelled.clone();
    let (commands, incoming) = mpsc::channel();
    let (finished, replies) = mpsc::channel();
    let (events, observed) = mpsc::channel();
    let completion = owner
        .spawn("test-camera-identity", cancelled, move || {
            run_serial_worker(31, scope(), worker_cancelled, incoming, finished, |_, _| {
                Ok(SerialSession::new(events))
            });
        })
        .unwrap();
    assert_eq!(
        failure(replies.recv().unwrap(), 31, None),
        "scripted wire failure 1"
    );
    let canceled_help = signal();
    canceled_help.store(true, Ordering::Release);
    commands
        .send(SerialWork {
            id: 1,
            cancelled: canceled_help,
        })
        .unwrap();
    assert!(failure(replies.recv().unwrap(), 31, Some(1)).contains("Help serial request canceled"));
    commands
        .send(SerialWork {
            id: 2,
            cancelled: signal(),
        })
        .unwrap();
    assert_eq!(
        failure(replies.recv().unwrap(), 31, Some(2)),
        "scripted wire failure 2"
    );
    assert!(!owner.is_stopped());
    drop(commands);
    completion.recv().unwrap();
    owner.shutdown().unwrap();
    assert_eq!(owner.active_workers(), 0);
    let events: Vec<_> = observed.try_iter().collect();
    let SessionEvent::Opened(worker) = events[0] else {
        panic!("missing session open")
    };
    assert_ne!(worker, thread::current().id());
    assert_eq!(
        events,
        [
            SessionEvent::Opened(worker),
            SessionEvent::Read(1, worker),
            SessionEvent::Read(2, worker),
            SessionEvent::Released(worker),
        ]
    );
}

#[::core::prelude::v1::test]
fn canceled_camera_connection_does_not_open_or_read_a_session() {
    let cancelled = signal();
    cancelled.store(true, Ordering::Release);
    let (commands, incoming) = mpsc::channel();
    let (finished, replies) = mpsc::channel();
    drop(commands);
    run_serial_worker::<SerialSession>(7, scope(), cancelled, incoming, finished, |_, _| {
        panic!("canceled connection must not initialize a native filter")
    });
    assert!(failure(replies.recv().unwrap(), 7, None).contains("Camera connection canceled"));
}

#[::core::prelude::v1::test]
fn failed_connection_initialization_is_retained_for_help_retry_without_new_native_owners() {
    let (commands, incoming) = mpsc::channel();
    commands
        .send(SerialWork {
            id: 21,
            cancelled: signal(),
        })
        .unwrap();
    drop(commands);
    let (finished, replies) = mpsc::channel();
    let opens = Cell::new(0);
    run_serial_worker::<SerialSession>(7, scope(), signal(), incoming, finished, |_, _| {
        opens.set(opens.get() + 1);
        anyhow::bail!("source initialization exhausted its bind attempts")
    });
    assert_eq!(opens.get(), 1);
    assert_eq!(
        failure(replies.recv().unwrap(), 7, None),
        "source initialization exhausted its bind attempts"
    );
    assert_eq!(
        failure(replies.recv().unwrap(), 7, Some(21)),
        "source initialization exhausted its bind attempts"
    );
}

#[::core::prelude::v1::test]
fn cleanup_reports_release_panic_even_after_the_camera_read_reply_was_delivered() {
    let owner = Arc::new(Owner::new());
    let (commands, incoming) = mpsc::channel();
    let (finished, replies) = mpsc::channel();
    let (events, observed) = mpsc::channel();
    let canceled = signal();
    let worker_canceled = canceled.clone();
    owner
        .spawn("test-camera-release-panic", canceled, move || {
            run_serial_worker(3, scope(), worker_canceled, incoming, finished, |_, _| {
                let mut session = SerialSession::new(events);
                session.panic_on_release = true;
                Ok(session)
            });
        })
        .unwrap();
    assert_eq!(
        failure(replies.recv().unwrap(), 3, None),
        "scripted wire failure 1"
    );
    drop(commands);
    let next = Arc::new(Owner::new());
    let released = Arc::new(AtomicBool::new(false));
    let completed = released.clone();
    next.cleanup("test-camera-next-release", move || {
        completed.store(true, Ordering::Release);
        Ok(())
    })
    .unwrap();
    let error = Cleanup {
        owners: vec![owner.clone(), next.clone()],
        failures: vec!["previously reaped failure".into()],
    }
    .drain()
    .unwrap_err()
    .to_string();
    assert!(error.contains("scripted serial release failure"));
    assert!(error.contains("previously reaped failure"));
    assert!(released.load(Ordering::Acquire));
    assert_eq!(owner.active_workers(), 0);
    assert_eq!(next.active_workers(), 0);
    assert!(
        observed
            .try_iter()
            .any(|event| matches!(event, SessionEvent::Released(_)))
    );
}

fn installed() -> CamoInstallation {
    CamoInstallation::from_observations(
        Ok(CamoStudio::Installed {
            version: Ok(Some("2.3.4".into())),
        }),
        Ok(CamoDriver::Installed {
            enabled: Err("driver status query failed".into()),
        }),
    )
}

#[::core::prelude::v1::test]
fn help_local_app_work_preserves_query_errors_and_rejects_canceled_results() {
    let canceled = AtomicBool::new(false);
    let stop = AtomicBool::new(false);
    let observed = installed();
    let result = run_app_work(
        AppCommand::Observe,
        &canceled,
        &stop,
        || observed.clone(),
        |_| panic!("observe must not launch"),
    );
    let AppResult::Installation(Ok(result)) = result else {
        panic!("lost independent installation observations")
    };
    assert_eq!(result, observed);
    assert_eq!(
        result.camera_available(),
        Err("driver status query failed".into())
    );
    assert_eq!(result.help_launch_available(), Ok(true));
    let result = run_app_work(
        AppCommand::Observe,
        &canceled,
        &stop,
        || {
            canceled.store(true, Ordering::Release);
            installed()
        },
        |_| panic!("observe must not launch"),
    );
    assert!(matches!(result, AppResult::Installation(Err(error)) if error.contains("canceled")));
    let result = run_app_work(
        AppCommand::Observe,
        &canceled,
        &stop,
        || panic!("canceled query"),
        |_| panic!("canceled launch"),
    );
    assert!(matches!(result, AppResult::Installation(Err(error)) if error.contains("canceled")));
}

#[::core::prelude::v1::test]
fn help_local_launch_uses_the_platform_result_and_independent_stop_guard() {
    let canceled = AtomicBool::new(false);
    let stop = AtomicBool::new(false);
    let result = run_app_work(
        AppCommand::LaunchInstalled,
        &canceled,
        &stop,
        || panic!("launch owns its observation"),
        |validate| {
            validate()?;
            anyhow::bail!("package and shortcut failed")
        },
    );
    assert!(
        matches!(result, AppResult::Launch(Err(error)) if error == "package and shortcut failed")
    );
    let result = run_app_work(
        AppCommand::LaunchInstalled,
        &canceled,
        &stop,
        || panic!("launch owns its observation"),
        |validate| {
            validate()?;
            stop.store(true, Ordering::Release);
            Ok(())
        },
    );
    assert!(matches!(result, AppResult::Launch(Err(error)) if error.contains("canceled")));
    assert!(!canceled.load(Ordering::Acquire));
}

#[gpui_kit::test]
fn camera_delivery_retains_a_worker_joined_after_the_current_reply_drain(
    cx: &mut gpui_kit::TestAppContext,
) {
    use gpui_kit::{AppContext, component::Root, px, size};
    use razer_model::model::DeviceConnectionObservation;
    use razer_pages::nav::Tab;
    cx.update(gpui_kit::init);
    let mut device = razer_model::product_preview::registered_preview(3594).unwrap();
    device.serial_number = "LOCAL-SAVED-SERIAL".into();
    device.device_container_id = scope().container_id;
    device.observe_connection(None);
    let mut workspace = None;
    let window = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| ProductWorkspace::new(device, false, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let workspace = workspace.unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.set_page(Tab::Help, window, cx);
            workspace.observe_connection(Some(DeviceConnectionObservation::UsbPresent), cx);
        });
    })
    .unwrap();
    let request = cx.update(|cx| workspace.read(cx).camera_identity_request(cx).unwrap());
    let key = request.owner().as_u64();
    let owner = Arc::new(Owner::new());
    let mut runtime = Runtime::default();
    let pending = Pending {
        request,
        mount: Mount {
            workspace: workspace.downgrade(),
            connection_revision: cx.update(|cx| workspace.read(cx).connection_revision()),
        },
        help: BTreeMap::new(),
    };
    let (release, wait) = mpsc::channel();
    let sender = runtime.finished.clone();
    let result = owner
        .spawn("test-camera-late-final-reply", signal(), move || {
            wait.recv().unwrap();
            sender
                .send(Reply::Serial {
                    connection: 17,
                    id: None,
                    result: Err("last real reply".into()),
                })
                .unwrap();
        })
        .unwrap();
    runtime.connections.insert(
        key,
        Connection {
            id: 17,
            pending,
            owner: owner.clone(),
            commands: None,
            initial_delivered: false,
            joined: false,
            join_error: None,
        },
    );
    assert!(runtime.poll().is_empty());
    release.send(()).unwrap();
    result.recv().unwrap();
    owner.shutdown().unwrap();
    runtime.retire_finished();
    assert!(runtime.connections.contains_key(&key));
    let mut replies = runtime.poll();
    assert_eq!(replies.len(), 1);
    assert_eq!(failure(replies.pop().unwrap(), 17, None), "last real reply");
    runtime.retire_finished();
    assert!(runtime.idle());
    assert!(runtime.take_cleanup().drain().is_ok());
}
