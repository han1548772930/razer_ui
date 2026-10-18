//! Pure queue, lifecycle and error tests; no DLL, kernel driver or device opens.
use super::*;
fn sample(temperature: Option<f64>) -> SystemSample { SystemSample { cpu_temperature: temperature, cpu_clock_mhz: Some(4321.25), gpu_temperature: Some(64.5), gpu_clock_mhz: None } }
#[test]
fn system_monitor_preserves_actual_samples_and_poll_boundaries() {
    let mut queue = Queue::default();
    for value in [Some(35.25), None, Some(42.75)] { queue.push(sample(value), Vec::new()); }
    let first = queue.drain("owner", 2); first.validate_for("owner").unwrap();
    assert_eq!(first.samples.iter().map(|sample| sample.cpu_temperature).collect::<Vec<_>>(), [Some(35.25), None]);
    assert_eq!(queue.drain("owner", 2).samples[0].cpu_temperature, Some(42.75));
}
#[test]
fn system_monitor_queue_is_bounded_and_reports_discarded_samples() {
    let mut queue = Queue::default();
    for index in 0..MAX_PENDING_SAMPLES + 3 { queue.push(sample(Some(index as f64)), Vec::new()); }
    let result = queue.drain("owner", usize::MAX);
    assert_eq!(result.samples.len(), MAX_PENDING_SAMPLES); assert_eq!(result.samples[0].cpu_temperature, Some(3.0)); assert!(result.diagnostics[0].starts_with("3 old"));
}
#[test]
fn system_monitor_read_failure_never_uses_an_old_or_default_value() {
    let mut diagnostics = Vec::new();
    assert_eq!(observed(Ok(0.0), "temperature", &mut diagnostics), Some(0.0));
    assert_eq!(observed(Err(anyhow::anyhow!("sensor disconnected")), "temperature", &mut diagnostics), None);
    assert_eq!(observed(Ok(f64::NAN), "clock", &mut diagnostics), None);
    assert_eq!(diagnostics.len(), 2);
}
struct MockSource { stop: mpsc::Sender<()>, closed: Arc<Mutex<bool>>, fail_close: bool }
impl Source for MockSource {
    fn sample(&mut self) -> (SystemSample, Vec<String>) { self.stop.send(()).unwrap(); (sample(Some(51.75)), vec!["actual simulated response".into()]) }
    fn close(&mut self) -> anyhow::Result<()> { *self.closed.lock().unwrap() = true; ensure!(!self.fail_close, "simulated close failed"); Ok(()) }
}
#[test]
fn system_monitor_cancel_wakes_wait_and_joins_cleanup() {
    let (stop, receiver) = mpsc::channel(); let queue = Arc::new(Mutex::new(Queue::default())); let closed = Arc::new(Mutex::new(false));
    run(MockSource { stop, closed: closed.clone(), fail_close: false }, queue.clone(), receiver, Duration::from_secs(3600)).unwrap();
    assert!(*closed.lock().unwrap()); assert_eq!(queue.lock().unwrap().samples[0].cpu_temperature, Some(51.75));
}
#[test]
fn system_monitor_cleanup_failure_is_not_a_success() {
    let (stop, receiver) = mpsc::channel(); let closed = Arc::new(Mutex::new(false));
    assert!(run(MockSource { stop, closed: closed.clone(), fail_close: true }, Arc::new(Mutex::new(Queue::default())), receiver, Duration::from_secs(3600)).is_err()); assert!(*closed.lock().unwrap());
}
#[test]
fn system_monitor_old_owner_cannot_poll_or_stop_a_successor() {
    let mut state = State { active: Some(Session { owner_guid: "new-owner".into(), queue: Arc::new(Mutex::new(Queue::default())), stop: None, thread: None }) };
    assert!(state.poll("old-owner").is_err()); assert!(state.stop("old-owner").is_err()); assert!(state.poll("new-owner").is_ok());
}
