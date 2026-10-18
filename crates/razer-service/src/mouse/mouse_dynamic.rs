//! Dynamic setters and input-monitor ownership stay on a background worker.
//! Source setter acknowledgement and independent GET diagnostics are distinct.
use crate::mouse_worker::{Cleanup, check, guarded, release, shutdown};
use razer_device::mouse_dynamic::{Report, Setting, input::Speed};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{
    ServiceClient,
    mouse_input_monitoring::{InputEvent, Session},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub enum Outcome {
    Device(Report),
    MonitoringStopped,
}
pub type Completion = crate::mouse_worker::Completion<Outcome>;
pub enum Update {
    MonitoringStarted,
    Speed(f64),
}

pub fn device(
    route: ObservedDevice,
    change: Option<Setting>,
    active: Arc<AtomicBool>,
) -> Completion {
    crate::mouse_worker::device(
        route,
        active,
        direct::mouse_dynamic::available,
        |client, route| direct::mouse_dynamic::request(client, route, change).map(Outcome::Device),
    )
}

trait Monitor: Cleanup {
    fn start(&mut self) -> anyhow::Result<()>;
    fn events(&mut self) -> anyhow::Result<Vec<InputEvent>>;
}
struct NativeMonitor {
    client: ServiceClient,
    session: Session,
}
impl Cleanup for NativeMonitor {
    fn stop(&mut self) -> anyhow::Result<()> {
        self.session.stop(|request| self.client.request(request))
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        shutdown(&mut self.client)
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}
impl Monitor for NativeMonitor {
    fn start(&mut self) -> anyhow::Result<()> {
        self.session.start(|request| self.client.request(request))
    }
    fn events(&mut self) -> anyhow::Result<Vec<InputEvent>> {
        self.session.poll(|request| self.client.request(request))
    }
}

fn admits(route: &ObservedDevice, event: &InputEvent) -> bool {
    event.vendor_id() == 5426
        && route.container().eq_ignore_ascii_case(event.container_id())
        && (event.product_id() == route.product_id()
            || event.product_id() == route.physical_product_id()
            || route.peer_product_id() == Some(event.product_id())
            || matches!(razer_device::device_identity::lookup(event.product_id()),
                razer_device::device_identity::IdentityLookup::Unique(identity) if identity.product_id==route.product_id()))
}

fn monitor_with(
    backend: &mut impl Monitor,
    active: &AtomicBool,
    admit: impl Fn(&InputEvent) -> bool,
    mut publish: impl FnMut(Update),
    mut now: impl FnMut() -> Duration,
    mut wait: impl FnMut(),
) -> Completion {
    let result = guarded(|| {
        check(active)?;
        backend.start()?;
        publish(Update::MonitoringStarted);
        let mut speed = Speed::default();
        let mut pending = false;
        let mut last_publish = None;
        while active.load(Ordering::Acquire) {
            for event in backend.events()? {
                if admit(&event) {
                    speed.observe(event.json_event())?;
                    pending = true;
                }
            }
            let current = now();
            if pending
                && last_publish.is_none_or(|last| {
                    current.saturating_sub(last) >= Duration::from_secs_f64(1. / 60.)
                })
            {
                publish(Update::Speed(speed.publish()));
                last_publish = Some(current);
                pending = false;
            }
            wait();
        }
        Ok(Outcome::MonitoringStopped)
    });
    let cleanup_failure = release(backend, &mut wait);
    Completion {
        result,
        cleanup_failure,
    }
}

pub fn monitor(
    route: ObservedDevice,
    active: Arc<AtomicBool>,
    publish: impl FnMut(Update),
) -> Completion {
    // No canceled request guard is installed on this client: the same retained
    // owner must still send STOP after its page guard becomes false.
    let lease = guarded(|| {
        check(&active)?;
        razer_ipc::KEY_CAPTURE_LIFECYCLE
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Input capture lifecycle is occupied"))
    });
    let _lease = match lease {
        Ok(lease) => lease,
        Err(error) => {
            return Completion {
                result: Err(error),
                cleanup_failure: None,
            };
        }
    };
    let backend = guarded(|| {
        check(&active)?;
        Ok(NativeMonitor {
            client: ServiceClient::spawn()?,
            session: Session::new(uuid::Uuid::new_v4().to_string())?,
        })
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
    let started = Instant::now();
    let completion = monitor_with(
        &mut backend,
        &active,
        |event| admits(&route, event),
        publish,
        || started.elapsed(),
        || std::thread::sleep(Duration::from_millis(8)),
    );
    drop(backend);
    completion
}

#[cfg(test)]
#[path = "mouse_dynamic_tests.rs"]
mod tests;
