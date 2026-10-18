//! Owned Rust sampling worker. GPU APIs and the sensor driver remain on its thread.
use super::NativeRuntime;
use anyhow::{Context as _, ensure};
use razer_device::system_monitor::{self, MonitorObservation, SystemSample};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
    time::Duration,
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
fn run(
    mut source: impl Source,
    queue: Arc<Mutex<Queue>>,
    stop: mpsc::Receiver<()>,
    interval: Duration,
) -> anyhow::Result<()> {
    let sampling = (|| {
        loop {
            match stop.try_recv() {
                Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }
            let (sample, diagnostics) = source.sample();
            sample.validate()?;
            queue
                .lock()
                .map_err(|_| anyhow::anyhow!("Hardware monitor queue is poisoned"))?
                .push(sample, diagnostics);
            match stop.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        Ok(())
    })();
    match (sampling, source.close()) {
        (result, Ok(())) => result,
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Hardware monitor cleanup also failed: {cleanup:#}")))
        }
    }
}
struct Session {
    owner_guid: String,
    queue: Arc<Mutex<Queue>>,
    stop: Option<mpsc::Sender<()>>,
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
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
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
            let (stop, receiver) = mpsc::channel();
            let worker_queue = queue.clone();
            let thread = std::thread::Builder::new()
                .name("hardware-monitor".into())
                .spawn(move || {
                    run(
                        razer_platform::hardware_monitor::Monitor::new(),
                        worker_queue,
                        receiver,
                        Duration::from_millis(system_monitor::CALLBACK_INTERVAL_MS as u64),
                    )
                })?;
            self.active = Some(Session {
                owner_guid: owner_guid.clone(),
                queue,
                stop: Some(stop),
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
