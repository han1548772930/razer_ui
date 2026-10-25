//! Controller observations from the current hardware HID endpoint stream.
//! The descriptor-free controller registration passes complete reports to the
//! product parser; interval batches follow 7f9fe/7fc20 before UI throttling.
use super::NativeRuntime;
use anyhow::{Context, Result, ensure};
use razer_device::{
    controller_input::{Event, Parser, Sample, Throttle},
    receiver_pairing::HardwareEvents,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Registration {
    owner: String,
    product: u32,
    info: Value,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
    events: mpsc::Receiver<Result<Update, String>>,
    overflow: Arc<AtomicBool>,
}
struct Update {
    samples: Vec<Sample>,
    notifications: Vec<Event>,
    last_tick: Option<u64>,
    unsupported: u64,
}
#[derive(Default)]
pub(super) struct State {
    registration: Option<Registration>,
}
impl State {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn owner(&self) -> Option<String> {
        self.registration.as_ref().map(|row| row.owner.clone())
    }
    fn registration(&self, owner: &str) -> Result<&Registration> {
        let row = self
            .registration
            .as_ref()
            .context("Controller input is not registered")?;
        ensure!(row.owner == owner, "Controller input owner changed");
        Ok(row)
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(mut row) = self.registration.take() {
            row.stop.store(true, Ordering::Release);
            if let Some(worker) = row.worker.take() {
                match worker.join() {
                    Ok(Ok(())) => (),
                    Ok(Err(error)) => eprintln!("Controller input cleanup: {error:#}"),
                    Err(_) => eprintln!("Controller input worker panicked"),
                }
            }
        }
    }
}

struct Batcher {
    interval: u64,
    last: u64,
    deadline: Option<u64>,
    frames: Vec<Vec<u8>>,
}
impl Batcher {
    fn new(milliseconds: u32) -> Self {
        Self {
            interval: u64::from(milliseconds) * 1000,
            last: 0,
            deadline: None,
            frames: Vec::new(),
        }
    }
    fn push(&mut self, frame: Vec<u8>, tick: u64) -> Option<Vec<Vec<u8>>> {
        self.frames.push(frame);
        if self.frames.len() >= 10001 {
            return self.flush();
        }
        if self.deadline.is_none() {
            let elapsed = tick.wrapping_sub(self.last);
            self.last = tick;
            if elapsed >= self.interval {
                return self.flush();
            }
            self.deadline = Some(tick.saturating_add(self.interval - elapsed));
        }
        None
    }
    fn poll(&mut self, tick: u64) -> Option<Vec<Vec<u8>>> {
        if self.deadline.is_some_and(|deadline| tick >= deadline) {
            self.flush()
        } else {
            None
        }
    }
    fn flush(&mut self) -> Option<Vec<Vec<u8>>> {
        self.deadline = None;
        (!self.frames.is_empty()).then(|| std::mem::take(&mut self.frames))
    }
}

fn physical_target(product: u32, physical_pid: u32, container: &str) -> anyhow::Result<Value> {
    ensure!(
        super::receiver::valid_container(container),
        "Controller input requires an observed Windows ContainerId"
    );
    Parser::new(product, physical_pid)?;
    let enumeration = super::usb::enumerate()?;
    ensure!(
        enumeration["complete"] == true
            && enumeration["failures"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "Controller USB identity enumeration is incomplete"
    );
    let candidates = enumeration["devices"]
        .as_array()
        .context("Controller USB identity list missing")?
        .iter()
        .filter(|device| {
            device["vendor_id"] == 0x1532
                && device["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect::<Vec<_>>();
    let [device] = candidates.as_slice() else {
        anyhow::bail!("Controller physical USB identity is absent or ambiguous");
    };
    ensure!(
        device["product_id"] == physical_pid,
        "Controller physical product changed"
    );
    for key in ["path", "device_instance_id"] {
        ensure!(
            device[key].as_str().is_some_and(|text| !text.is_empty()),
            "Controller USB identity has no {key}"
        );
    }
    Ok((*device).clone())
}
fn run(
    product: u32,
    physical_pid: u32,
    container: String,
    physical: Value,
    mut parser: Parser,
    stop: Arc<AtomicBool>,
    events: mpsc::SyncSender<Result<Update, String>>,
    overflow: Arc<AtomicBool>,
    ready: mpsc::SyncSender<Result<(), String>>,
) -> Result<()> {
    let mut source = super::receiver_events::Events::new(physical_pid.try_into()?, &container);
    let begin = (|| -> Result<()> {
        source.begin()?;
        ensure!(
            physical_target(product, physical_pid, &container)? == physical,
            "Controller USB identity changed during registration"
        );
        Ok(())
    })();
    if let Err(error) = begin {
        let cleanup = source.finish();
        let error = format!("Controller input registration: {error:#}; cleanup: {cleanup:?}");
        let _ = ready.send(Err(error.clone()));
        anyhow::bail!(error);
    }
    let _ = ready.send(Ok(()));
    let mut batch = parser.interval_ms().map(Batcher::new);
    let mut throttle = Throttle::default();
    let clock = Instant::now();
    let mut last_validation = Instant::now();
    let result = crate::mapping::guarded(|| -> Result<()> {
        while !stop.load(Ordering::Acquire) {
            if last_validation.elapsed() >= Duration::from_millis(250) {
                ensure!(
                    physical_target(product, physical_pid, &container)? == physical,
                    "Controller USB identity changed during observation"
                );
                last_validation = Instant::now();
            }
            let report = source.receive(Duration::from_millis(2))?;
            let tick = razer_platform::event_clock::tick()?;
            let mut inputs = Vec::new();
            if let Some(report) = report {
                if let Some(batch) = &mut batch {
                    if let Some(frames) = batch.push(report, tick) {
                        inputs.push(serde_json::to_value(frames)?);
                    }
                } else {
                    inputs.push(serde_json::to_value(report)?);
                }
            }
            if let Some(batch) = &mut batch {
                if let Some(frames) = batch.poll(tick) {
                    inputs.push(serde_json::to_value(frames)?);
                }
            }
            let mut samples = Vec::new();
            let mut notifications = Vec::new();
            let mut unsupported = 0;
            let observed = !inputs.is_empty();
            for input in inputs {
                for event in parser.parse(&input)? {
                    match event {
                        Event::Buttons { sample } => {
                            if let Some(sample) =
                                throttle.observe(sample, clock.elapsed().as_millis() as u64)
                            {
                                samples.push(sample);
                            }
                        }
                        event @ Event::Notification { .. } => notifications.push(event),
                        Event::Unsupported { .. } => unsupported += 1,
                    }
                }
            }
            if let Some(sample) = throttle.flush(clock.elapsed().as_millis() as u64) {
                samples.push(sample);
            }
            if observed || !samples.is_empty() {
                let value = Update {
                    samples,
                    notifications,
                    last_tick: observed.then_some(tick),
                    unsupported,
                };
                if events.try_send(Ok(value)).is_err() {
                    overflow.store(true, Ordering::Release);
                    anyhow::bail!("Controller input consumer fell behind");
                }
            }
        }
        Ok(())
    });
    let cleanup = source.finish();
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => {
            let _ = events.try_send(Err(format!("{error:#}")));
            Err(error)
        }
        (Err(error), Err(cleanup)) => {
            let error = anyhow::anyhow!("{error:#}; controller input cleanup: {cleanup:#}");
            let _ = events.try_send(Err(format!("{error:#}")));
            Err(error)
        }
    }
}

impl NativeRuntime {
    pub(super) fn start_controller_input(
        &mut self,
        product: u32,
        physical_pid: u32,
        container: &str,
        owner: &str,
    ) -> Result<Value> {
        ensure!(
            !owner.is_empty() && !owner.contains('\0'),
            "Invalid controller input owner"
        );
        ensure!(
            self.controller_input.registration.is_none(),
            "Controller input already has an owner"
        );
        let physical = physical_target(product, physical_pid, container)?;
        let info = json!({"vendorId":0x1532,"productId":physical_pid,"containerId":physical["device_container_id"],"usbInstanceId":physical["device_instance_id"],"guid":owner});
        let parser = Parser::new(product, physical_pid)?;
        let interval = parser.interval_ms();
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        let overflow = Arc::new(AtomicBool::new(false));
        let worker_overflow = overflow.clone();
        let (sender, events) = mpsc::sync_channel(512);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker_container = container.to_owned();
        let observed = physical.clone();
        let worker = thread::Builder::new()
            .name("razer-controller-input".into())
            .spawn(move || {
                run(
                    product,
                    physical_pid,
                    worker_container,
                    observed,
                    parser,
                    cancel,
                    sender,
                    worker_overflow,
                    ready,
                )
            })?;
        match initialized.recv() {
            Ok(Ok(())) => (),
            value => {
                let cleanup = worker.join();
                anyhow::bail!("Controller initialization: {value:?}; cleanup: {cleanup:?}");
            }
        }
        self.controller_input.registration = Some(Registration {
            owner: owner.into(),
            product,
            info: info.clone(),
            stop,
            worker: Some(worker),
            events,
            overflow,
        });
        Ok(
            json!({"started":true,"owner_guid":owner,"product_id":product,"device_info":info,"interval_ms":interval}),
        )
    }
    pub(super) fn controller_input_events(&mut self, owner: &str) -> Result<Value> {
        let row = self.controller_input.registration(owner)?;
        ensure!(
            !row.overflow.load(Ordering::Acquire),
            "Controller input consumer fell behind"
        );
        let mut samples = Vec::new();
        let mut notifications = Vec::new();
        let mut last_tick = None;
        let mut unsupported = 0u64;
        for event in row.events.try_iter() {
            let value = event.map_err(anyhow::Error::msg)?;
            samples.extend(value.samples);
            notifications.extend(value.notifications);
            if let Some(tick) = value.last_tick {
                last_tick = Some(tick);
            }
            unsupported += value.unsupported;
        }
        if row.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            anyhow::bail!("Controller input stream stopped before cleanup");
        }
        Ok(
            json!({"owner_guid":owner,"product_id":row.product,"device_info":row.info,"samples":samples,"notifications":notifications,"last_tick":last_tick,"unsupported_reports":unsupported}),
        )
    }
    pub(super) fn stop_controller_input(&mut self, owner: &str) -> Result<Value> {
        self.controller_input.registration(owner)?;
        let mut row = self.controller_input.registration.take().unwrap();
        row.stop.store(true, Ordering::Release);
        if let Some(worker) = row.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Controller input worker panicked"))??;
        }
        Ok(
            json!({"stopped":true,"owner_guid":owner,"product_id":row.product,"device_info":row.info}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_batch_deadline_uses_first_queued_report_and_cap_flushes() {
        let mut batch = Batcher::new(60);
        assert_eq!(batch.push(vec![1], 100_000), Some(vec![vec![1]]));
        assert_eq!(batch.push(vec![2], 110_000), None);
        assert_eq!(batch.push(vec![3], 120_000), None);
        assert_eq!(batch.poll(159_999), None);
        assert_eq!(batch.poll(160_000), Some(vec![vec![2], vec![3]]));
        assert_eq!(batch.push(vec![4], 170_000), Some(vec![vec![4]]));
        for _ in 0..10000 {
            assert_eq!(batch.push(vec![5], 171_000), None);
        }
        assert_eq!(batch.push(vec![6], 171_000).unwrap().len(), 10001);
        assert_eq!(batch.poll(999_999), None);
    }
}
