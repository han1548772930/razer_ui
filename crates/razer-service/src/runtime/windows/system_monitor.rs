//! Owned Rust sampling worker. GPU APIs and the sensor driver remain on its thread.
use super::NativeRuntime;
use anyhow::{Context as _, ensure};
use razer_device::system_monitor::{self, MonitorObservation, SystemSample};
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MAX_POLL_SAMPLES: usize = 256;
const MAX_PENDING_SAMPLES: usize = 1024;
#[derive(Default)]
struct Queue {
    samples: VecDeque<SystemSample>,
    diagnostics: VecDeque<String>,
    discarded: usize,
}
impl Queue {
    fn push(&mut self, sample: SystemSample, diagnostics: Vec<String>) {
        if self.samples.len() == MAX_PENDING_SAMPLES {
            self.samples.pop_front();
            self.discarded += 1;
        }
        self.samples.push_back(sample);
        for diagnostic in diagnostics {
            if self.diagnostics.len() == MAX_PENDING_SAMPLES {
                self.diagnostics.pop_front();
            }
            self.diagnostics.push_back(diagnostic);
        }
    }
    fn drain(&mut self, owner_guid: &str, limit: usize) -> MonitorObservation {
        let count = self.samples.len().min(limit);
        let diagnostics = self.diagnostics.len().min(limit);
        let mut result = MonitorObservation {
            owner_guid: owner_guid.into(),
            samples: self.samples.drain(..count).collect(),
            diagnostics: self.diagnostics.drain(..diagnostics).collect(),
        };
        if self.discarded != 0 {
            result.diagnostics.push(format!(
                "{} old hardware samples exceeded the bounded queue",
                std::mem::take(&mut self.discarded)
            ));
        }
        result
    }
}
trait Source {
    fn sample(&mut self) -> (SystemSample, Vec<String>);
    fn close(&mut self) -> anyhow::Result<()>;
}
#[cfg(target_arch = "x86_64")]
impl Source for razer_platform::hardware_monitor::Monitor {
    fn sample(&mut self) -> (SystemSample, Vec<String>) {
        let cpu = self.cpu();
        let mut graphics = self.graphics();
        let gpu = razer_platform::hardware_monitor::selected_graphics(&graphics)
            .map(|index| graphics.swap_remove(index).readings);
        let mut diagnostics = self.take_diagnostics();
        let cpu_temperature =
            observed(cpu.temperature_celsius, "CPU temperature", &mut diagnostics);
        let cpu_clock_mhz = observed(cpu.clock_mhz, "CPU core clock", &mut diagnostics);
        let (gpu_temperature, gpu_clock_mhz) = match gpu {
            Some(gpu) => (
                observed(gpu.temperature_celsius, "GPU temperature", &mut diagnostics),
                observed(gpu.clock_mhz, "GPU graphics clock", &mut diagnostics),
            ),
            None => {
                diagnostics.push("No graphics adapter was observed".into());
                (None, None)
            }
        };
        (
            SystemSample {
                cpu_temperature,
                cpu_clock_mhz,
                gpu_temperature,
                gpu_clock_mhz,
            },
            diagnostics,
        )
    }
    fn close(&mut self) -> anyhow::Result<()> {
        razer_platform::hardware_monitor::Monitor::close(self)
    }
}
fn observed(value: anyhow::Result<f64>, label: &str, diagnostics: &mut Vec<String>) -> Option<f64> {
    match value {
        Ok(value) if value.is_finite() && (value * 100.0).is_finite() => {
            Some((value * 100.0).round() / 100.0)
        }
        Ok(_) => {
            diagnostics.push(format!("{label}: non-finite observation"));
            None
        }
        Err(error) => {
            diagnostics.push(format!("{label}: {error:#}"));
            None
        }
    }
}
/// SystemCommon's two SetTimer registrations retain their phase and coalesce
/// late ticks. Missed ticks do not create additional reads or callbacks.
struct Timer {
    interval: Duration,
    next: Instant,
}
impl Timer {
    fn new(started_at: Instant, interval: Duration) -> Self {
        assert!(!interval.is_zero());
        Self {
            interval,
            next: started_at + interval,
        }
    }
    fn tick(&mut self, now: Instant) -> bool {
        if now < self.next {
            return false;
        }
        let remainder = now.duration_since(self.next).as_nanos() % self.interval.as_nanos();
        let remainder = Duration::new(
            (remainder / 1_000_000_000) as u64,
            (remainder % 1_000_000_000) as u32,
        );
        self.next = now + (self.interval - remainder);
        true
    }
}
struct Publication {
    timer: Timer,
    last: Instant,
}
impl Publication {
    fn new(registered_at: Instant, interval: Duration) -> Self {
        Self {
            timer: Timer::new(registered_at, interval),
            last: registered_at,
        }
    }
    fn tick(&mut self, now: Instant, frame: Option<&SystemSample>) -> Option<SystemSample> {
        // SystemCommon 0x67850 requires both the registration interval and an
        // observed hardware frame; registration alone never supplies a value.
        if !self.timer.tick(now) || now.duration_since(self.last) < self.timer.interval {
            return None;
        }
        let frame = frame?.clone();
        self.last = now;
        Some(frame)
    }
}
#[derive(Default)]
struct Stream {
    stopping: bool,
    source_finished: bool,
    latest: Option<SystemSample>,
    diagnostics: VecDeque<String>,
}
#[derive(Default)]
struct Control {
    stream: Mutex<Stream>,
    wake: Condvar,
}
impl Control {
    fn stop(&self) {
        self.stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopping = true;
        self.wake.notify_all();
    }
    fn source_finished(&self) {
        self.stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .source_finished = true;
        self.wake.notify_all();
    }
    fn wait_until(&self, deadline: Instant) -> anyhow::Result<bool> {
        let mut stream = self
            .stream
            .lock()
            .map_err(|_| anyhow::anyhow!("Hardware monitor control is poisoned"))?;
        loop {
            if stream.stopping || stream.source_finished {
                return Ok(false);
            }
            let wait = deadline.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                return Ok(true);
            }
            stream = self
                .wake
                .wait_timeout(stream, wait)
                .map_err(|_| anyhow::anyhow!("Hardware monitor control is poisoned"))?
                .0;
        }
    }
    fn observe(&self, sample: SystemSample, diagnostics: Vec<String>) -> anyhow::Result<bool> {
        let mut stream = self
            .stream
            .lock()
            .map_err(|_| anyhow::anyhow!("Hardware monitor control is poisoned"))?;
        if stream.stopping {
            return Ok(false);
        }
        sample.validate()?;
        stream.latest = Some(sample);
        for diagnostic in diagnostics {
            if stream.diagnostics.len() == MAX_PENDING_SAMPLES {
                stream.diagnostics.pop_front();
            }
            stream.diagnostics.push_back(diagnostic);
        }
        Ok(true)
    }
    fn publish(
        &self,
        publication: &mut Publication,
        now: Instant,
        queue: &Mutex<Queue>,
    ) -> anyhow::Result<bool> {
        let mut stream = self
            .stream
            .lock()
            .map_err(|_| anyhow::anyhow!("Hardware monitor control is poisoned"))?;
        if stream.stopping || stream.source_finished {
            return Ok(false);
        }
        let Some(sample) = publication.tick(now, stream.latest.as_ref()) else {
            return Ok(false);
        };
        // Admission and queue publication share the cancellation lock. A read
        // finishing after unregister cannot become another callback.
        queue
            .lock()
            .map_err(|_| anyhow::anyhow!("Hardware monitor queue is poisoned"))?
            .push(sample, stream.diagnostics.drain(..).collect());
        Ok(true)
    }
}
struct SourceCompletion(Arc<Control>);
impl Drop for SourceCompletion {
    fn drop(&mut self) {
        self.0.source_finished();
    }
}
fn sample_source<S: Source>(
    factory: impl FnOnce() -> S,
    control: &Control,
    mut timer: Timer,
) -> anyhow::Result<()> {
    if !control.wait_until(timer.next)? {
        return Ok(());
    }
    timer.tick(Instant::now());
    // RzDLLService 0x2fb50 initializes the sensor source while handling the
    // first Timer2 request, before invoking that request's GetAllHardwareInfo.
    let mut source = factory();
    let sampling = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            if !control.wait_until(Instant::now())? {
                break;
            }
            let (sample, diagnostics) = source.sample();
            if !control.observe(sample, diagnostics)? {
                break;
            }
            // Common 0x6c440 suppresses pending-request ticks until response or
            // timeout. Completed reads resume on the existing timer phase.
            timer.tick(Instant::now());
            if !control.wait_until(timer.next)? {
                break;
            }
            timer.tick(Instant::now());
        }
        Ok(())
    }))
    .map_err(|_| anyhow::anyhow!("Hardware sensor sampling panicked"))
    .and_then(|result| result);
    let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.close()))
        .map_err(|_| anyhow::anyhow!("Hardware sensor cleanup panicked"))
        .and_then(|result| result);
    match (sampling, cleanup) {
        (result, Ok(())) => result,
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Hardware monitor cleanup also failed: {cleanup:#}")))
        }
    }
}
fn run<S: Source + 'static>(
    factory: impl FnOnce() -> S + Send + 'static,
    queue: Arc<Mutex<Queue>>,
    control: Arc<Control>,
    sampling: Timer,
    mut publication: Publication,
) -> anyhow::Result<()> {
    let worker_control = control.clone();
    let worker = std::thread::Builder::new()
        .name("hardware-sensors".into())
        .spawn(move || {
            let _completion = SourceCompletion(worker_control.clone());
            sample_source(factory, &worker_control, sampling)
        })?;
    let publishing: anyhow::Result<()> = (|| {
        while control.wait_until(publication.timer.next)? {
            control.publish(&mut publication, Instant::now(), &queue)?;
        }
        Ok(())
    })();
    control.stop();
    let sampling = worker
        .join()
        .map_err(|_| anyhow::anyhow!("Hardware sensor worker panicked"))
        .and_then(|result| result);
    match (publishing, sampling) {
        (result, Ok(())) | (Ok(()), result) => result,
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Hardware sensor cleanup also failed: {cleanup:#}")))
        }
    }
}
struct Session {
    owner_guid: String,
    queue: Arc<Mutex<Queue>>,
    control: Arc<Control>,
    thread: Option<JoinHandle<anyhow::Result<()>>>,
    terminal_error: Option<String>,
}
impl Session {
    fn observation(&self, limit: usize) -> anyhow::Result<MonitorObservation> {
        Ok(self
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("Hardware monitor queue is poisoned"))?
            .drain(&self.owner_guid, limit))
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.control.stop();
        if let Some(thread) = self.thread.take() {
            let result = thread
                .join()
                .map_err(|_| anyhow::anyhow!("Hardware monitoring worker panicked"))
                .and_then(|result| result);
            if let Err(error) = result {
                self.terminal_error = Some(format!("{error:#}"));
            }
        }
        ensure!(
            self.terminal_error.is_none(),
            "{}",
            self.terminal_error.as_deref().unwrap_or("")
        );
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Hardware monitoring cleanup failed: {error:#}");
        }
    }
}
#[derive(Default)]
pub(super) struct State {
    active: Option<Session>,
}
impl State {
    pub(super) fn cleanup_pending(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|session| session.thread.is_some())
    }

    fn start(
        &mut self,
        owner_guid: String,
        container_id: String,
        product_id: u32,
    ) -> anyhow::Result<MonitorObservation> {
        system_monitor::validate_identity(&owner_guid)?;
        system_monitor::validate_identity(&container_id)?;
        ensure!(
            product_id == system_monitor::HANBO_PRODUCT_ID,
            "System monitor product is not verified"
        );
        ensure!(self.active.is_none(), "System monitor already has an owner");
        #[cfg(not(target_arch = "x86_64"))]
        anyhow::bail!("Hardware sensor driver has no implementation for this architecture");
        #[cfg(target_arch = "x86_64")]
        {
            let queue = Arc::new(Mutex::new(Queue::default()));
            let control = Arc::new(Control::default());
            let interval = Duration::from_millis(system_monitor::CALLBACK_INTERVAL_MS as u64);
            // Common 0x6cb70 establishes Timer2 during client initialization;
            // 0x6d360 separately records the subscription's callback interval.
            let sampling = Timer::new(Instant::now(), interval);
            let publication = Publication::new(Instant::now(), interval);
            let worker_queue = queue.clone();
            let worker_control = control.clone();
            let thread = std::thread::Builder::new()
                .name("hardware-monitor".into())
                .spawn(move || {
                    run(
                        razer_platform::hardware_monitor::Monitor::new,
                        worker_queue,
                        worker_control,
                        sampling,
                        publication,
                    )
                })?;
            self.active = Some(Session {
                owner_guid: owner_guid.clone(),
                queue,
                control,
                thread: Some(thread),
                terminal_error: None,
            });
            self.poll(&owner_guid)
        }
    }
    fn owned(&self, owner_guid: &str) -> anyhow::Result<&Session> {
        system_monitor::validate_identity(owner_guid)?;
        let session = self
            .active
            .as_ref()
            .context("System monitor has not started")?;
        ensure!(
            session.owner_guid == owner_guid,
            "System monitor owner changed"
        );
        Ok(session)
    }
    pub(super) fn poll(&self, owner_guid: &str) -> anyhow::Result<MonitorObservation> {
        self.owned(owner_guid)?.observation(MAX_POLL_SAMPLES)
    }
    pub(super) fn stop(&mut self, owner_guid: &str) -> anyhow::Result<MonitorObservation> {
        self.owned(owner_guid)?;
        let session = self
            .active
            .as_mut()
            .context("System monitor has no owner")?;
        session.shutdown()?;
        let observation = session.observation(usize::MAX)?;
        self.active.take();
        Ok(observation)
    }
    pub(super) fn shutdown(&mut self) -> anyhow::Result<Option<MonitorObservation>> {
        let Some(owner) = self
            .active
            .as_ref()
            .map(|session| session.owner_guid.clone())
        else {
            return Ok(None);
        };
        self.stop(&owner).map(Some)
    }
}
impl NativeRuntime {
    pub(super) fn start_system_monitor(
        &mut self,
        owner_guid: String,
        container_id: String,
        product_id: u32,
    ) -> anyhow::Result<serde_json::Value> {
        super::native_target(&container_id, Some(product_id))?;
        Ok(serde_json::to_value(self.system_monitor.start(
            owner_guid,
            container_id,
            product_id,
        )?)?)
    }
}
#[cfg(test)]
#[path = "system_monitor_tests.rs"]
mod tests;
