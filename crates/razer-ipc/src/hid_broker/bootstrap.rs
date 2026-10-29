use super::{BrokerClient, BrokerEndpoint};
use crate::worker_diagnostics::WorkerDiagnostics;
use anyhow::{Context as _, ensure};
use std::{
    path::Path,
    process::{Child, ChildStderr, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Child exit and stderr release are separate from device/session cleanup.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct BrokerExit {
    pub status: ExitStatus,
    pub stderr_joined: bool,
}

impl BrokerExit {
    pub fn new(status: ExitStatus, stderr_joined: bool) -> Self {
        Self {
            status,
            stderr_joined,
        }
    }
}

struct BootstrapProcess {
    child: Child,
    diagnostics: WorkerDiagnostics,
    stderr: Arc<Mutex<Option<ChildStderr>>>,
    diagnostic_worker: Option<JoinHandle<anyhow::Result<()>>>,
    diagnostic_failure: Option<String>,
    status: Option<ExitStatus>,
}

/// Process ownership belongs to application composition, independently of every
/// client session. This child is never assigned to a kill-on-close worker job.
pub struct BrokerBootstrap {
    process: Option<BootstrapProcess>,
    finished: Option<BrokerExit>,
}

impl BrokerBootstrap {
    /// Spawn the explicit headless entrypoint. The returned owner must remain at
    /// application root; dropping a ServiceClient cannot terminate this process.
    pub fn start(executable: &Path) -> anyhow::Result<Self> {
        let executable = executable
            .canonicalize()
            .context("broker executable could not be resolved")?;
        let mut command = Command::new(&executable);
        command
            .arg("--hid-broker")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .context("shared broker could not be started")?;
        let stderr = child.stderr.take();
        let mut owner = Self {
            process: Some(BootstrapProcess {
                child,
                diagnostics: WorkerDiagnostics::default(),
                stderr: Arc::new(Mutex::new(stderr)),
                diagnostic_worker: None,
                diagnostic_failure: None,
                status: None,
            }),
            finished: None,
        };
        owner.start_diagnostics()?;
        Ok(owner)
    }

    fn start_diagnostics(&mut self) -> anyhow::Result<()> {
        let process = self
            .process
            .as_mut()
            .context("broker bootstrap owner has no process")?;
        let source = process.stderr.clone();
        let diagnostics = process.diagnostics.clone();
        let worker = thread::Builder::new()
            .name("hid-broker-stderr".to_owned())
            .spawn(move || {
                let reader = source
                    .lock()
                    .map_err(|_| anyhow::anyhow!("broker stderr owner is poisoned"))?
                    .take()
                    .context("broker stderr handle was not captured")?;
                diagnostics.capture_stderr(reader)
            })
            .context("broker stderr reader could not be started")?;
        process.diagnostic_worker = Some(worker);
        Ok(())
    }

    pub fn process_id(&self) -> Option<u32> {
        self.process.as_ref().map(|process| process.child.id())
    }

    pub fn exit_confirmation(&self) -> Option<BrokerExit> {
        self.finished
    }

    /// This method never sends process termination. Timeout preserves the live
    /// Child and its stderr owner so another process's sessions remain usable.
    pub fn finish(&mut self, wait_timeout: Duration) -> anyhow::Result<BrokerExit> {
        if let Some(receipt) = self.finished {
            return Ok(receipt);
        }
        let deadline = Instant::now() + wait_timeout;
        let process = self
            .process
            .as_mut()
            .context("broker bootstrap owner has no process")?;
        loop {
            if let Some(error) = &process.diagnostic_failure {
                anyhow::bail!("broker stderr release failed: {error}");
            }
            if process.status.is_none() {
                process.status = process
                    .child
                    .try_wait()
                    .context("broker exit status could not be queried")?;
            }
            if let Some(status) = process.status {
                let stderr_finished = process
                    .diagnostic_worker
                    .as_ref()
                    .is_none_or(|worker| worker.is_finished());
                if stderr_finished {
                    if let Some(worker) = process.diagnostic_worker.take() {
                        let result = worker
                            .join()
                            .map_err(|_| anyhow::anyhow!("broker stderr reader panicked"))
                            .and_then(|result| result);
                        if let Err(error) = result {
                            process.diagnostic_failure = Some(format!("{error:#}"));
                            return Err(process.diagnostics.attach(error));
                        }
                    }
                    let receipt = BrokerExit {
                        status,
                        stderr_joined: true,
                    };
                    self.finished = Some(receipt);
                    self.process.take();
                    return Ok(receipt);
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(process.diagnostics.attach(anyhow::anyhow!(
                    "shared broker is still owned by active sessions or its stderr has not closed; process release is unconfirmed"
                )));
            }
            thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }
}

/// Connect first so established peers do not start a second process. Only a
/// genuinely absent local endpoint authorizes bootstrap; identity failures are
/// returned unchanged. Simultaneous starters compete at the server's exclusive
/// first listener, then all clients connect to the winning instance.
pub fn connect_or_start(
    executable: &Path,
    connect_timeout: Duration,
) -> anyhow::Result<(BrokerClient, Option<BrokerBootstrap>)> {
    ensure!(
        !connect_timeout.is_zero(),
        "broker bootstrap timeout must be positive"
    );
    let endpoint = BrokerEndpoint::for_installation(executable)?;
    match BrokerClient::connect(&endpoint, None, connect_timeout) {
        Ok(client) => return Ok((client, None)),
        Err(error) if endpoint_absent(&error) => {}
        Err(error) => return Err(error),
    }
    let owner = BrokerBootstrap::start(executable)?;
    let deadline = Instant::now() + connect_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let error = anyhow::anyhow!(
                "shared broker endpoint did not become available before the bootstrap deadline"
            );
            return Err(owner.attach(error));
        }
        match BrokerClient::connect(&endpoint, None, remaining) {
            Ok(client) => return Ok((client, Some(owner))),
            Err(error) if endpoint_absent(&error) => {
                thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(error) => {
                return Err(owner.attach(error));
            }
        }
    }
}

impl BrokerBootstrap {
    fn attach(&self, error: anyhow::Error) -> anyhow::Error {
        match &self.process {
            Some(process) => process.diagnostics.attach(error),
            None => error,
        }
    }
}

fn endpoint_absent(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<super::client::MissingEndpoint>()
        .is_some()
}

type RetainedProcess = Arc<Mutex<Option<BootstrapProcess>>>;
static FAILED_REAPERS: Mutex<Vec<RetainedProcess>> = Mutex::new(Vec::new());

impl Drop for BrokerBootstrap {
    fn drop(&mut self) {
        let Some(process) = self.process.take() else {
            return;
        };
        let retained = Arc::new(Mutex::new(Some(process)));
        let reaper_owner = retained.clone();
        let spawned = thread::Builder::new()
            .name("hid-broker-reaper".to_owned())
            .spawn(move || {
                let process = reaper_owner
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                let Some(mut process) = process else {
                    return;
                };
                if process.diagnostic_worker.is_none() {
                    let source = process
                        .stderr
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .take();
                    if let Some(source) = source {
                        if let Err(error) = process.diagnostics.capture_stderr(source) {
                            eprintln!("[shared broker reaper] {error:#}");
                        }
                    }
                }
                match process.child.wait() {
                    Ok(status) if !status.success() => {
                        eprintln!("[shared broker reaper] broker exited with {status}")
                    }
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!(
                            "[shared broker reaper] broker wait failed; ownership retained: {error}"
                        );
                        *reaper_owner
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(process);
                        FAILED_REAPERS
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .push(reaper_owner);
                        return;
                    }
                }
                if let Some(worker) = process.diagnostic_worker.take() {
                    match worker.join() {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => eprintln!("[shared broker reaper] {error:#}"),
                        Err(_) => eprintln!("[shared broker reaper] stderr reader panicked"),
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!(
                "[shared broker reaper] could not start reaper; process ownership retained: {error}"
            );
            FAILED_REAPERS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(retained);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_only_starts_for_a_missing_endpoint() {
        assert!(endpoint_absent(
            &anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound))
                .context(super::super::client::MissingEndpoint)
                .context("connect")
        ));
        assert!(!endpoint_absent(
            &anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound))
                .context("peer executable disappeared during identity verification")
        ));
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::TimedOut,
        ] {
            assert!(!endpoint_absent(&anyhow::Error::new(std::io::Error::from(
                kind
            ))));
        }
        assert!(!endpoint_absent(&anyhow::anyhow!(
            "broker endpoint identity mismatch"
        )));
    }
}
