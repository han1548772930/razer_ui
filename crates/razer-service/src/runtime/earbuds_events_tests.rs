use super::*;
use std::sync::{Mutex, mpsc::Sender};

struct Events {
    input: Receiver<Vec<u8>>,
    receiving: Sender<()>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    begin_gate: Option<Receiver<()>>,
    entered: Option<Sender<()>>,
    fail_begin: bool,
    fail_finish: bool,
    first_receive: bool,
    received_packet: bool,
}
impl HardwareEvents for Events {
    fn begin(&mut self) -> Result<()> {
        self.trace.lock().unwrap().push("begin");
        if let Some(entered) = &self.entered {
            entered.send(()).unwrap();
        }
        if let Some(gate) = &self.begin_gate {
            gate.recv().unwrap();
        }
        ensure!(!self.fail_begin, "native begin failed");
        Ok(())
    }
    fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        // Match the actual broker: cancellation is checked both before native
        // dispatch and after its terminal return, not just by the outer loop.
        super::super::hid_broker::check_current_request()?;
        if self.first_receive || self.received_packet {
            let _ = self.receiving.send(());
            self.first_receive = false;
            self.received_packet = false;
        }
        let result = match self.input.recv_timeout(timeout) {
            Ok(bytes) => {
                self.received_packet = true;
                Ok(Some(bytes))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(error) => Err(error.into()),
        };
        super::super::hid_broker::check_current_request()?;
        result
    }
    fn finish(&mut self) -> Result<()> {
        self.trace.lock().unwrap().push("finish");
        ensure!(!self.fail_finish, "native finish failed");
        Ok(())
    }
}

struct Fixture {
    input: Sender<Vec<u8>>,
    receiving: Receiver<()>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    source: Events,
}
fn fixture() -> Fixture {
    let (send, input) = mpsc::channel();
    let (receiving, called) = mpsc::channel();
    let trace = Arc::new(Mutex::new(vec![]));
    Fixture {
        input: send,
        receiving: called,
        trace: trace.clone(),
        source: Events {
            input,
            receiving,
            trace,
            begin_gate: None,
            entered: None,
            fail_begin: false,
            fail_finish: false,
            first_receive: true,
            received_packet: false,
        },
    }
}

#[test]
fn start_waits_for_real_begin_and_stop_waits_for_finish() {
    let mut f = fixture();
    let (entered, began) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (replied, response) = mpsc::channel();
    f.source.entered = Some(entered);
    f.source.begin_gate = Some(gate);
    let join = thread::spawn(move || {
        let mut controller = Controller::default();
        let reply = controller
            .start_with("one".into(), move || Ok(Box::new(f.source)))
            .unwrap();
        replied.send(reply).unwrap();
        controller
    });
    began.recv().unwrap();
    assert!(response.try_recv().is_err());
    release.send(()).unwrap();
    assert!(response.recv().unwrap().started);
    let mut controller = join.join().unwrap();
    assert!(controller.stop_reply("one").unwrap().stopped);
    assert_eq!(*f.trace.lock().unwrap(), vec!["begin", "finish"]);
    assert!(controller.stop_reply("one").unwrap().stopped);
    drop(f.input);
}

#[test]
fn begin_failure_is_not_reported_as_started_and_cleanup_error_is_retained() {
    let mut f = fixture();
    f.source.fail_begin = true;
    f.source.fail_finish = true;
    let mut controller = Controller::default();
    let error = controller
        .start_with("failed".into(), move || Ok(Box::new(f.source)))
        .unwrap_err()
        .to_string();
    assert!(error.contains("native begin failed"));
    assert!(error.contains("native finish failed"));
    assert_eq!(*f.trace.lock().unwrap(), vec!["begin", "finish"]);
    assert!(controller.active.is_none());
    assert!(controller.stop_reply("failed").is_err());
    assert!(
        controller
            .poll_reply("failed")
            .unwrap()
            .error
            .unwrap()
            .contains("native finish failed")
    );
}

#[test]
fn poll_consumes_real_notifications_and_stale_stop_cannot_cancel_successor() {
    let f = fixture();
    let mut controller = Controller::default();
    controller
        .start_with("first".into(), move || Ok(Box::new(f.source)))
        .unwrap();
    f.receiving.recv().unwrap();
    f.input.send(vec![5, 49, 27, 255]).unwrap();
    f.receiving.recv().unwrap();
    let reply = controller.poll_reply("first").unwrap();
    assert_eq!(
        reply.events,
        vec![Notification::Battery {
            left: 27,
            right: 255
        }]
    );
    assert!(!reply.terminal);
    assert!(controller.stop_reply("different").is_err());
    assert!(controller.stop_reply("first").unwrap().stopped);
    let next = fixture();
    controller
        .start_with("second".into(), move || Ok(Box::new(next.source)))
        .unwrap();
    assert!(controller.stop_reply("first").is_err());
    assert_eq!(controller.active.as_ref().unwrap().id, "second");
    assert!(controller.stop_reply("second").unwrap().stopped);
    assert_eq!(*next.trace.lock().unwrap(), vec!["begin", "finish"]);
}

#[test]
fn malformed_notification_finishes_the_owner_and_remains_failed_on_shutdown() {
    let f = fixture();
    let mut controller = Controller::default();
    controller
        .start_with("malformed".into(), move || Ok(Box::new(f.source)))
        .unwrap();
    f.receiving.recv().unwrap();
    f.input.send(vec![5, 56, 1]).unwrap();
    // The sender closes only after parsing failed and finish returned.
    assert!(f.receiving.recv().is_err());
    assert!(controller.stop_reply("malformed").is_err());
    let report = controller.poll_reply("malformed").unwrap();
    assert!(report.terminal);
    assert!(report.error.unwrap().contains("truncated"));
    let mut cleanup = super::super::shutdown::Cleanup::default();
    controller.shutdown(&mut cleanup);
    assert!(!cleanup.succeeded());
    assert_eq!(*f.trace.lock().unwrap(), vec!["begin", "finish"]);
}

#[test]
fn drop_cancels_and_joins_the_owned_subscription() {
    let f = fixture();
    let mut controller = Controller::default();
    controller
        .start_with("drop".into(), move || Ok(Box::new(f.source)))
        .unwrap();
    drop(controller);
    assert_eq!(*f.trace.lock().unwrap(), vec!["begin", "finish"]);
}

#[test]
fn canceled_start_keeps_request_identity_and_finishes_before_returning_failure() {
    let mut f = fixture();
    let owner = uuid::Uuid::new_v4();
    let canceled = Arc::new(AtomicBool::new(false));
    let parent = canceled.clone();
    let (entered, began) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    f.source.entered = Some(entered);
    f.source.begin_gate = Some(gate);
    let join = thread::spawn(move || {
        super::super::hid_broker::with_request_context(owner, parent, || {
            let mut controller = Controller::default();
            controller
                .start_with("canceled".into(), move || {
                    assert_eq!(super::super::hid_broker::current_request_context().0, owner);
                    Ok(Box::new(f.source))
                })
                .unwrap_err()
                .to_string()
        })
    });
    began.recv().unwrap();
    canceled.store(true, Ordering::Release);
    release.send(()).unwrap();
    assert!(join.join().unwrap().contains("canceled"));
    assert_eq!(*f.trace.lock().unwrap(), vec!["begin", "finish"]);
}
