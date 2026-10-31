//! OS-independent current DPI Matcher service coordinator. The backend must
//! provide real monitoring, source locks, DPI submission and durable storage.
//! No vendor DLL is loaded by this helper and unsupported input capture errors.
use anyhow::{Context as _, ensure};
use razer_device::{
    dpi_matcher::{Calibration, InputBatch, InputTimeout, Product, Session, State},
    dpi_matcher_storage::{self, Change, Mutation, ProfileName, ProfileUpdate, SourceData},
    mouse_current_dpi::SourceStages,
};
use serde_json::Value;

#[path = "dpi_matcher_worker.rs"]
pub mod worker;

#[cfg(test)]
#[path = "dpi_matcher_tests.rs"]
mod tests;

pub struct InputObservation {
    pub batch: InputBatch,
    pub initial_is_main: bool,
    pub received_ms: u64,
}

pub type InputObserver<'a> = dyn FnMut(InputObservation) -> anyhow::Result<()> + 'a;

pub trait Backend {
    fn read_source_data(&mut self) -> anyhow::Result<SourceData>;
    fn start_input_monitoring(&mut self) -> anyhow::Result<()>;
    fn stop_input_monitoring(
        &mut self,
        observe: Option<&mut InputObserver<'_>>,
    ) -> anyhow::Result<()>;
    /// Source uses shared/read and exclusive/write requestAllLocks for dpiMatcher.
    fn acquire_matcher_locks(&mut self) -> anyhow::Result<()>;
    fn release_matcher_locks(&mut self) -> anyhow::Result<()>;
    fn submit_dpi_stages(&mut self, stages: &SourceStages) -> anyhow::Result<()>;
    fn persist_source_data(&mut self, change: &Change) -> anyhow::Result<()>;
    fn report_failure(&mut self, error: String) -> anyhow::Result<()>;
    fn publish_stage_draft(&mut self, stages: &SourceStages) -> anyhow::Result<()>;
    fn publish_calibration(&mut self, calibration: Calibration) -> anyhow::Result<()>;
    /// Derive current source dual-link/source-device/computer locale name and
    /// allocate a real UUID, never a canned name or deterministic fake UUID.
    fn confirmation_identity(
        &mut self,
        source_container: &str,
        source_product: Option<u32>,
    ) -> anyhow::Result<(String, ProfileName)>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Calibration(Calibration),
    Profiles(ProfileUpdate),
}

fn merge_results(results: impl IntoIterator<Item = anyhow::Result<()>>) -> anyhow::Result<()> {
    let errors = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

#[derive(Default)]
pub struct Controller {
    session: Session,
    monitoring: bool,
    listeners: usize,
    locked: bool,
}
impl Controller {
    pub fn new(product: Product) -> Self {
        Self {
            session: Session::new(product),
            monitoring: false,
            listeners: 0,
            locked: false,
        }
    }

    pub fn response(&self) -> Calibration {
        self.session.response()
    }
    pub fn command(
        &mut self,
        command: &str,
        payload: Option<&Value>,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Vec<Response>> {
        match command {
            "GET_STATE" => Ok(vec![Response::Calibration(self.response())]),
            "START" => {
                let data = backend.read_source_data()?;
                let baseline = data.baseline()?;
                self.session.prepare_start(baseline);
                self.listeners = self
                    .listeners
                    .checked_add(1)
                    .context("Matcher listener count overflow")?;
                backend.start_input_monitoring()?;
                self.monitoring = true;
                if !matches!(self.state(), State::Ready | State::Error) {
                    return Ok(vec![]);
                }
                self.session.begin(baseline)?;
                if let Err(error) = backend.acquire_matcher_locks() {
                    self.session.lock_failed();
                    return Err(error);
                }
                self.locked = true;
                self.session.acquired_lock()?;
                Ok(vec![Response::Calibration(self.response())])
            }
            "CANCEL" => {
                // Source removes L before awaiting STOP. A failed STOP neither
                // sends CANCEL to the state machine nor runs its movement reset.
                self.detach_listener();
                if self.monitoring {
                    self.stop_monitoring(backend)?;
                }
                if self.session.state.accepts_cancel() && self.locked {
                    backend.release_matcher_locks()?;
                    self.locked = false;
                }
                Ok(self
                    .session
                    .cancel_after_stop()
                    .into_iter()
                    .map(Response::Calibration)
                    .collect())
            }
            "RESET" => {
                self.release_locks(backend)?;
                Ok(vec![Response::Calibration(self.session.reset())])
            }
            "CONFIRM" => {
                let measured = self
                    .session
                    .new_dpi()
                    .context("matcher has no measured result")?;
                let data = backend.read_source_data()?;
                let (guid, name) = backend.confirmation_identity(
                    self.session.source_container(),
                    self.session.source_product_id,
                )?;
                let mutation = dpi_matcher_storage::confirm(&data, measured, &guid, name)?;
                self.session.reset();
                backend.publish_calibration(self.response())?;
                self.persist(mutation, backend)
            }
            "DELETE_PROFILE" => {
                let guid = payload
                    .and_then(Value::as_str)
                    .context("DELETE_PROFILE requires GUID payload")?;
                let data = backend.read_source_data()?;
                let responses = self.persist(dpi_matcher_storage::delete(&data, guid)?, backend)?;
                self.session.reset_movement();
                Ok(responses)
            }
            "DELETE_ALL_PROFILES" => {
                let data = backend.read_source_data()?;
                let responses = self.persist(dpi_matcher_storage::delete_all(&data), backend)?;
                self.session.reset_movement();
                Ok(responses)
            }
            _ => anyhow::bail!("unsupported DPI_MATCHER_UI_COMMAND {command}"),
        }
    }
    /// The stage task maker saves and queues before the independent library task.
    pub fn select(
        &mut self,
        guid: &str,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Vec<Response>> {
        let data = backend.read_source_data()?;
        self.persist(dpi_matcher_storage::select(&data, guid)?, backend)
    }
    pub fn update_profiles(
        &mut self,
        payload: &ProfileUpdate,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Vec<Response>> {
        let data = backend.read_source_data()?;
        self.persist(dpi_matcher_storage::update(&data, payload), backend)
    }
    fn persist(
        &mut self,
        mutation: Mutation,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Vec<Response>> {
        if let Some(stages) = mutation.stages() {
            backend.publish_stage_draft(stages.stages())?;
            let stage_task = (|| {
                stages.validate()?;
                backend.persist_source_data(&Change::Stages(stages.clone()))?;
                backend.submit_dpi_stages(stages.stages())
            })();
            // The source task maker reports its failure and resolves. The next
            // independent Matcher-library task still runs without rollback.
            if let Err(error) = stage_task {
                backend.report_failure(format!("{error:#}"))?;
            }
        }
        backend.persist_source_data(&Change::Profiles(mutation.response().clone()))?;
        Ok(vec![Response::Profiles(mutation.response().clone())])
    }
    pub fn ingest(
        &mut self,
        batch: InputBatch,
        initial_is_main: bool,
        received_ms: u64,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Calibration> {
        ensure!(self.monitoring, "input monitor is unavailable");
        ensure!(self.is_listening(), "Matcher input listener is detached");
        self.record_input(InputObservation {
            batch,
            initial_is_main,
            received_ms,
        });
        match self.session.advance() {
            Ok(true) => {
                self.detach_listener();
                self.stop_monitoring(backend)?;
                let completion = self.session.complete_after_stop();
                let unlock = self.release_locks(backend);
                merge_results([completion, unlock])?;
            }
            Ok(false) => {}
            Err(error) => {
                self.session.fail();
                return Err(match self.cleanup(backend) {
                    Ok(()) => error,
                    Err(cleanup) => error.context(format!("Matcher cleanup failed: {cleanup:#}")),
                });
            }
        }
        Ok(self.response())
    }
    pub fn tick(
        &mut self,
        now_ms: u64,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Option<Calibration>> {
        let mut observation = None;
        while let Some(timeout) = self.session.check_timeout(now_ms) {
            let unlock = match timeout {
                InputTimeout::Failed(calibration) => {
                    observation = Some(calibration);
                    self.release_locks(backend)
                }
                InputTimeout::ResetMovement => Ok(()),
            };
            self.detach_listener();
            unlock?;
        }
        Ok(observation)
    }

    fn is_listening(&self) -> bool {
        self.listeners != 0
    }

    fn detach_listener(&mut self) {
        self.listeners = self.listeners.saturating_sub(1);
    }

    fn record_input(&mut self, input: InputObservation) {
        for _ in 0..self.listeners {
            self.session
                .record_input(&input.batch, input.initial_is_main, input.received_ms);
        }
    }

    fn stop_monitoring(&mut self, backend: &mut impl Backend) -> anyhow::Result<()> {
        if self.is_listening() {
            backend.stop_input_monitoring(Some(&mut |input| {
                self.record_input(input);
                Ok(())
            }))?;
        } else {
            backend.stop_input_monitoring(None)?;
        }
        self.monitoring = false;
        Ok(())
    }

    fn release_locks(&mut self, backend: &mut impl Backend) -> anyhow::Result<()> {
        if self.locked {
            backend.release_matcher_locks()?;
            self.locked = false;
        }
        Ok(())
    }
    /// Called on owner/device/workspace destruction. Keep flags on failure so a
    /// subsequent cleanup can retry; no successful cancellation is fabricated.
    pub fn cleanup(&mut self, backend: &mut impl Backend) -> anyhow::Result<()> {
        self.listeners = 0;
        let stop = if self.monitoring {
            backend.stop_input_monitoring(None)
        } else {
            Ok(())
        };
        if stop.is_ok() {
            self.monitoring = false;
        }
        let unlock = if self.locked {
            backend.release_matcher_locks()
        } else {
            Ok(())
        };
        if unlock.is_ok() {
            self.locked = false;
        }
        merge_results([stop, unlock])
    }
    pub fn state(&self) -> State {
        self.session.state
    }
}
