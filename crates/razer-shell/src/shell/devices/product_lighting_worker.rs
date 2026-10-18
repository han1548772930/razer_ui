//! Blocking device work owns its service process through shutdown and reaping.
use anyhow::ensure;
use razer_device::mouse_brightness::Submission;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) enum Work {
    Observe,
    Brightness {
        route: ObservedDevice,
        submission: Submission,
    },
}
pub(super) enum Reply {
    Observation(super::product_chroma::Readback),
    Submitted,
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| anyhow::anyhow!("Product lighting transport panicked"))?
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<()>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    submission: Submission,
}
impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<()> {
        let report = direct::mouse_brightness::request(
            &mut self.client,
            &self.route,
            self.submission.clone(),
        )?;
        ensure!(
            report.completed(),
            "Mouse brightness task was not confirmed: {}; progress: {:?}",
            report
                .failure()
                .unwrap_or("source task or readback incomplete"),
            report.writes()
        );
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "Lighting service stopped before its shutdown receipt"
        );
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn check(active: &AtomicBool, canceled: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        active.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
        "Product lighting request owner or physical connection was replaced"
    );
    Ok(())
}

fn run_with<T: Transport>(
    active: &AtomicBool,
    canceled: &AtomicBool,
    create: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<Reply> {
    check(active, canceled)?;
    let mut transport = guarded(create)?;
    let result = guarded(|| {
        check(active, canceled)?;
        transport.execute()
    });
    let shutdown = guarded(|| transport.shutdown());
    let termination = guarded(|| transport.finish_termination());
    let destruction = guarded(|| {
        drop(transport);
        Ok(())
    });
    let cleanup = [
        ("shutdown", shutdown),
        ("process reaping", termination),
        ("owner release", destruction),
    ]
    .into_iter()
    .filter_map(|(stage, result)| result.err().map(|error| format!("{stage}: {error:#}")))
    .collect::<Vec<_>>();
    let result = result.and_then(|()| {
        check(active, canceled)?;
        Ok(Reply::Submitted)
    });
    if cleanup.is_empty() {
        result
    } else {
        let message = format!(
            "Product lighting transport cleanup failed: {}",
            cleanup.join("; ")
        );
        match result {
            Ok(_) => Err(anyhow::anyhow!(message)),
            Err(error) => Err(error.context(message)),
        }
    }
}

pub(super) fn run(
    work: Work,
    active: Arc<AtomicBool>,
    canceled: Arc<AtomicBool>,
) -> anyhow::Result<Reply> {
    check(&active, &canceled)?;
    if matches!(work, Work::Observe) {
        let observation = guarded(|| Ok(super::product_chroma::readback()))?;
        check(&active, &canceled)?;
        return Ok(Reply::Observation(observation));
    }
    let Work::Brightness { route, submission } = work else {
        unreachable!("Observation does not open a device service");
    };
    run_with(&active, &canceled, || {
        let mut client = ServiceClient::spawn()?;
        client.set_scoped_request_guard(active.clone(), canceled.clone());
        Ok(NativeTransport {
            client,
            route,
            submission,
        })
    })
}

#[cfg(test)]
#[path = "product_lighting_worker_tests.rs"]
mod tests;
