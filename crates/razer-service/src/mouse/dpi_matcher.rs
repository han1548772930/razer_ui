//! OS-independent current DPI Matcher service coordinator. The backend must
//! provide real monitoring, source locks, DPI submission and durable storage.
//! No vendor DLL is loaded by this helper and unsupported input capture errors.
use anyhow::{Context as _, ensure};
use razer_device::{
    dpi_matcher::{Calibration, InputBatch, Product, Session, State},
    dpi_matcher_storage::{self, Change, Mutation, ProfileName, ProfileUpdate, SourceData},
    mouse_current_dpi::SourceStages,
};
use serde_json::Value;

#[path = "dpi_matcher_worker.rs"]
pub mod worker;

#[cfg(test)]
#[path = "dpi_matcher_tests.rs"]
mod tests;

pub trait Backend {
    fn read_source_data(&mut self) -> anyhow::Result<SourceData>;
    fn start_input_monitoring(&mut self) -> anyhow::Result<()>;
    fn stop_input_monitoring(&mut self) -> anyhow::Result<()>;
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

#[derive(Default)]
pub struct Controller {
    session: Session,
    monitoring: bool,
    listening: bool,
    locked: bool,
}
impl Controller {
    pub fn new(product: Product) -> Self {
        Self {
            session: Session::new(product),
            monitoring: false,
            listening: false,
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
                self.session.begin(data.baseline()?)?;
                if let Err(error) = backend.start_input_monitoring() {
                    self.session.reset();
                    return Err(error);
                }
                self.monitoring = true;
                self.listening = true;
                if let Err(error) = backend.acquire_matcher_locks() {
                    self.session.reset();
                    self.cleanup(backend)?;
                    return Err(error);
                }
                self.locked = true;
                self.session.acquired_lock()?;
                Ok(vec![Response::Calibration(self.response())])
            }
            "CANCEL" => {
                // Source removes L before awaiting STOP. A failed STOP neither
                // sends CANCEL to the state machine nor runs its movement reset.
                self.listening = false;
                if self.monitoring {
                    backend.stop_input_monitoring()?;
                    self.monitoring = false;
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
                self.cleanup(backend)?;
                self.session.reset();
                Ok(vec![Response::Calibration(self.response())])
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
                self.session.reset();
                Ok(responses)
            }
            "DELETE_ALL_PROFILES" => {
                let data = backend.read_source_data()?;
                let responses = self.persist(dpi_matcher_storage::delete_all(&data), backend)?;
                self.session.reset();
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
        ensure!(self.listening, "Matcher input listener is detached");
        match self.session.ingest(batch, initial_is_main, received_ms) {
            Ok(true) => {
                self.listening = false;
                backend.stop_input_monitoring()?;
                self.monitoring = false;
                let completion = self.session.complete_after_stop();
                let cleanup = self.cleanup(backend);
                completion?;
                cleanup?;
            }
            Ok(false) => {}
            Err(error) => {
                self.session.fail();
                self.cleanup(backend)?;
                return Err(error);
            }
        }
        Ok(self.response())
    }
    pub fn tick(
        &mut self,
        now_ms: u64,
        backend: &mut impl Backend,
    ) -> anyhow::Result<Option<Calibration>> {
        if self.session.check_timeout(now_ms) {
            self.cleanup(backend)?;
            return Ok(Some(self.response()));
        }
        Ok(None)
    }
    /// Called on owner/device/workspace destruction. Keep flags on failure so a
    /// subsequent cleanup can retry; no successful cancellation is fabricated.
    pub fn cleanup(&mut self, backend: &mut impl Backend) -> anyhow::Result<()> {
        self.listening = false;
        let stop = if self.monitoring {
            backend.stop_input_monitoring()
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
        stop?;
        unlock?;
        Ok(())
    }
    pub fn state(&self) -> State {
        self.session.state
    }
}
