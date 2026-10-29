//! One process owns collection handles, timing and connection epochs.
mod clock;
mod collection;
mod identity;
mod server;
mod topology;

use anyhow::{Context as _, ensure};
use razer_device::backend::HidBackend;
use std::{
    cell::RefCell,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

static POOL: OnceLock<collection::Pool> = OnceLock::new();
static TOPOLOGY_READY: AtomicBool = AtomicBool::new(false);
static BROKER_STOPPING: AtomicBool = AtomicBool::new(false);
static TOPOLOGY: Mutex<Option<topology::Topology>> = Mutex::new(None);
static TOPOLOGY_FAILURE: Mutex<Option<String>> = Mutex::new(None);
static SHUTDOWN: Mutex<ShutdownState> = Mutex::new(ShutdownState {
    failures: Vec::new(),
});

#[derive(Default)]
struct ShutdownState {
    failures: Vec<String>,
}

pub(super) enum ShutdownProgress {
    Pending { failures: Vec<String> },
    Released { failures: Vec<String> },
}

impl ShutdownState {
    fn record(&mut self, failure: impl Into<String>) {
        let failure = failure.into();
        if !self.failures.contains(&failure) {
            self.failures.push(failure);
        }
    }

    fn progress(&self, released: bool) -> ShutdownProgress {
        let failures = self.failures.clone();
        if released {
            ShutdownProgress::Released { failures }
        } else {
            ShutdownProgress::Pending { failures }
        }
    }
}

struct RequestContext {
    owner: Uuid,
    canceled: Arc<AtomicBool>,
}

thread_local! {
    static REQUEST_CONTEXT: RefCell<RequestContext> = RefCell::new(RequestContext {
        owner: Uuid::new_v4(), canceled: Arc::new(AtomicBool::new(false)),
    });
}

/// The same cancellation allocation belongs to a session throughout its life.
/// Only the session executor may reset it after the preceding request terminates.
pub(super) fn with_request_context<T>(
    owner: Uuid,
    canceled: Arc<AtomicBool>,
    action: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<RequestContext>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                REQUEST_CONTEXT.with(|slot| {
                    slot.replace(previous);
                });
            }
        }
    }
    let previous = REQUEST_CONTEXT.with(|slot| slot.replace(RequestContext { owner, canceled }));
    let _restore = Restore(Some(previous));
    action()
}

fn raw_backend() -> anyhow::Result<Box<dyn HidBackend>> {
    Ok(Box::new(razer_hid::NativeBackend::new()?))
}

fn backend() -> anyhow::Result<Box<dyn HidBackend>> {
    if !TOPOLOGY_READY.load(Ordering::Acquire) {
        let failure = TOPOLOGY_FAILURE
            .lock()
            .map_err(|_| anyhow::anyhow!("HID topology failure owner is poisoned"))?;
        anyhow::bail!(
            "HID topology is unavailable: {}",
            failure.as_deref().unwrap_or(
                "exact registration and identity scan have not established a connection epoch"
            )
        );
    }
    let pool = POOL.get().context("Shared HID owner was not installed")?;
    REQUEST_CONTEXT.with(|slot| {
        let context = slot.borrow();
        pool.backend_for(context.owner, context.canceled.clone())
    })
}

pub(super) fn run() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let endpoint = razer_ipc::hid_broker::BrokerEndpoint::for_installation(&executable)?;
    let pool = collection::Pool::new(raw_backend, identity::resolve);
    POOL.set(pool.clone())
        .map_err(|_| anyhow::anyhow!("Shared HID owner already exists"))?;
    razer_hid::install_backend_factory(backend)?;
    #[cfg(windows)]
    match topology::Topology::start(pool.clone()) {
        Ok(topology) => {
            *TOPOLOGY
                .lock()
                .map_err(|_| anyhow::anyhow!("HID topology owner is poisoned"))? = Some(topology)
        }
        Err(error) => {
            eprintln!("HID topology registration failed: {error:#}");
            *TOPOLOGY_FAILURE
                .lock()
                .map_err(|_| anyhow::anyhow!("HID topology failure owner is poisoned"))? = Some(
                format!("Windows notification registration failed: {error:#}"),
            );
        }
    }
    #[cfg(not(windows))]
    {
        *TOPOLOGY_FAILURE.lock().map_err(|_| anyhow::anyhow!("HID topology failure owner is poisoned"))? = Some("Exact HID connection epochs are unavailable on this platform; non-HID local services remain available".to_owned());
    }
    let result = server::run(endpoint, razer_ipc::hid_broker::BrokerInstanceId::new());
    // An early listener setup failure still owns the topology registration.
    let cleanup = loop {
        match prepare_shutdown() {
            ShutdownProgress::Pending { failures } => {
                eprintln!("[HID broker shutdown] {}", failures.join("; "));
                std::thread::park_timeout(std::time::Duration::from_millis(50));
            }
            ShutdownProgress::Released { failures } => break failures,
        }
    };
    let mut errors = Vec::new();
    if let Err(error) = result {
        errors.push(format!("{error:#}"));
    }
    for failure in cleanup {
        if !errors.contains(&failure) {
            errors.push(failure);
        }
    }
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

/// The listener retains its exclusive endpoint while this barrier drains native
/// actors, preventing a second process from replacing an unfinished owner.
pub(super) fn prepare_shutdown() -> ShutdownProgress {
    BROKER_STOPPING.store(true, Ordering::Release);
    TOPOLOGY_READY.store(false, Ordering::Release);
    let mut shutdown = SHUTDOWN.lock().unwrap_or_else(|poison| {
        let mut state = poison.into_inner();
        state.record("HID shutdown state was poisoned");
        state
    });
    let topology = match TOPOLOGY.lock() {
        Ok(mut topology) => topology.take(),
        Err(_) => {
            shutdown.record("HID topology owner is poisoned; release cannot be confirmed");
            return shutdown.progress(false);
        }
    };
    if let Some(topology) = topology {
        // shutdown joins the registration owner even when it reports an
        // earlier unregister failure after resources are actually released.
        if let Err(error) = topology.shutdown() {
            shutdown.record(format!("Topology cleanup: {error:#}"));
        }
    }
    let Some(pool) = POOL.get() else {
        shutdown.record("Shared HID owner is unavailable during shutdown");
        return shutdown.progress(false);
    };
    if let Err(error) = pool.begin_shutdown() {
        shutdown.record(format!("Collection shutdown admission: {error:#}"));
        return shutdown.progress(false);
    }
    if let Err(error) = pool.reconcile() {
        shutdown.record(format!("Collection cleanup: {error:#}"));
    }
    let released = match pool.is_drained() {
        Ok(drained) => drained,
        Err(error) => {
            shutdown.record(format!("Collection release observation: {error:#}"));
            false
        }
    };
    #[cfg(windows)]
    let released = {
        if let Err(error) = super::native::shutdown_ble_cleanup() {
            shutdown.record(format!("BLE cleanup: {error:#}"));
        }
        released && super::native::ble_cleanup_pending_count() == 0
    };
    shutdown.progress(released)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_release_and_historical_failure_are_independent() {
        let mut state = ShutdownState::default();
        state.record("unregister failed before retry");
        assert!(
            matches!(state.progress(false), ShutdownProgress::Pending { failures } if failures == ["unregister failed before retry"])
        );
        state.record("unregister failed before retry");
        assert!(
            matches!(state.progress(true), ShutdownProgress::Released { failures } if failures == ["unregister failed before retry"])
        );
        assert!(
            matches!(state.progress(true), ShutdownProgress::Released { failures } if !failures.is_empty())
        );
    }

    #[test]
    fn session_context_restores_the_previous_owner_and_cancel_allocation_after_unwind() {
        let previous = REQUEST_CONTEXT.with(|slot| {
            let context = slot.borrow();
            (context.owner, context.canceled.clone())
        });
        let owner = Uuid::new_v4();
        let canceled = Arc::new(AtomicBool::new(false));
        let result = std::panic::catch_unwind(|| {
            with_request_context(owner, canceled.clone(), || {
                REQUEST_CONTEXT.with(|slot| {
                    let context = slot.borrow();
                    assert_eq!(context.owner, owner);
                    assert!(Arc::ptr_eq(&context.canceled, &canceled));
                    context.canceled.store(true, Ordering::Release);
                });
                panic!("session request failed");
            });
        });
        assert!(result.is_err());
        assert!(canceled.load(Ordering::Acquire));
        REQUEST_CONTEXT.with(|slot| {
            let context = slot.borrow();
            assert_eq!(context.owner, previous.0);
            assert!(Arc::ptr_eq(&context.canceled, &previous.1));
        });
    }
}
