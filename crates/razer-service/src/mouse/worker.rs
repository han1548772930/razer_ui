//! Mouse requests retain their service process until pipes and workers exit.
use anyhow::ensure;
use razer_discovery::discovery::ObservedDevice;
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub struct Completion<T> {
    pub result: Result<T, String>,
    pub cleanup_failure: Option<String>,
}

pub(crate) fn check(active: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        active.load(Ordering::Acquire),
        "Mouse request owner was canceled"
    );
    Ok(())
}

pub(crate) fn guarded<T>(run: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .map_err(|_| "Mouse transport owner panicked".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

pub(crate) trait Cleanup {
    fn stop(&mut self) -> anyhow::Result<()>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn terminate(&mut self) -> anyhow::Result<()>;
}

pub(crate) fn release(backend: &mut impl Cleanup, mut wait: impl FnMut()) -> Option<String> {
    let mut errors = Vec::new();
    for result in [guarded(|| backend.stop()), guarded(|| backend.shutdown())] {
        if let Err(error) = result {
            errors.push(error);
        }
    }
    loop {
        match guarded(|| backend.terminate()) {
            Ok(()) => break,
            Err(error) => {
                if !errors.contains(&error) {
                    errors.push(error);
                }
                // Process, pipe workers and any input lease remain owned until
                // actual termination succeeds; retry never erases a failure.
                wait();
            }
        }
    }
    (!errors.is_empty()).then(|| errors.join("; "))
}

pub(crate) fn shutdown(client: &mut ServiceClient) -> anyhow::Result<()> {
    ensure!(
        !client.is_stopped(),
        "Mouse service terminated without a shutdown acknowledgement"
    );
    #[derive(serde::Deserialize)]
    struct Receipt {
        stopped: bool,
    }
    let receipt: Receipt = serde_json::from_value(client.request(ServiceRequest::Shutdown)?)?;
    ensure!(
        receipt.stopped,
        "Mouse service did not acknowledge shutdown"
    );
    Ok(())
}

struct DeviceBackend {
    client: ServiceClient,
}
impl Cleanup for DeviceBackend {
    fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        shutdown(&mut self.client)
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

pub(crate) fn device<T>(
    route: ObservedDevice,
    active: Arc<AtomicBool>,
    available: impl FnOnce(&ObservedDevice) -> bool,
    request: impl FnOnce(&mut ServiceClient, &ObservedDevice) -> anyhow::Result<T>,
) -> Completion<T> {
    device_with(
        &active,
        || {
            ensure!(available(&route), "Mouse physical route is unavailable");
            let mut client = ServiceClient::spawn()?;
            client.set_request_guard(active.clone());
            Ok(DeviceBackend { client })
        },
        |backend| request(&mut backend.client, &route),
        || std::thread::sleep(Duration::from_millis(100)),
    )
}

fn device_with<T, B: Cleanup>(
    active: &AtomicBool,
    create: impl FnOnce() -> anyhow::Result<B>,
    request: impl FnOnce(&mut B) -> anyhow::Result<T>,
    wait: impl FnMut(),
) -> Completion<T> {
    let backend = guarded(|| {
        check(active)?;
        create()
    });
    let mut backend = match backend {
        Ok(backend) => backend,
        Err(error) => {
            return Completion {
                result: Err(error),
                cleanup_failure: None,
            };
        }
    };
    let result = guarded(|| {
        check(active)?;
        let report = request(&mut backend)?;
        check(active)?;
        Ok(report)
    });
    let cleanup_failure = release(&mut backend, wait);
    drop(backend);
    Completion {
        result,
        cleanup_failure,
    }
}

#[cfg(test)]
#[path = "worker/tests.rs"]
mod tests;
