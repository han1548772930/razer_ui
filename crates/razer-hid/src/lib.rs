//! Native HID implementation of razer-device's host-neutral backend contract.
//! Razer commands, capability selection and receiver peers belong above this
//! module. See docs/re/cross-platform-hid-current.md for original-code receipts.
mod descriptor;
#[path = "transport/hidapi.rs"]
mod native;

use anyhow::{Context as _, ensure};
pub use native::NativeBackend;
use razer_device::backend::HidBackend;
use std::cell::RefCell;
use std::sync::Mutex;

/// A factory creates its backend on the calling thread. Native handles never
/// move through this registration boundary.
pub type BackendFactory = fn() -> anyhow::Result<Box<dyn HidBackend>>;

#[derive(Default)]
struct Registration {
    factory: Option<BackendFactory>,
    used: bool,
}

impl Registration {
    fn install(&mut self, factory: BackendFactory) -> anyhow::Result<()> {
        ensure!(
            !self.used,
            "HID backend factory must be installed before first use"
        );
        ensure!(
            self.factory.is_none(),
            "HID backend factory is already installed"
        );
        self.factory = Some(factory);
        Ok(())
    }

    fn select(&mut self) -> Option<BackendFactory> {
        self.used = true;
        self.factory
    }
}

static REGISTRATION: Mutex<Registration> = Mutex::new(Registration {
    factory: None,
    used: false,
});

/// Install the process's backend owner before any thread initializes its TLS
/// backend. The registration and first-use check share the same lock.
pub fn install_backend_factory(factory: BackendFactory) -> anyhow::Result<()> {
    REGISTRATION
        .lock()
        .map_err(|_| anyhow::anyhow!("HID backend registration is poisoned"))?
        .install(factory)
}

thread_local! {
    static BACKEND: RefCell<Option<Box<dyn HidBackend>>> = const { RefCell::new(None) };
}

/// One backend per worker thread. A broker installs its proxy factory; explicit
/// independent paths retain the real native default and NativeBackend::new.
pub fn with_backend<T>(
    action: impl FnOnce(&mut dyn HidBackend) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    BACKEND.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let factory = REGISTRATION
                .lock()
                .map_err(|_| anyhow::anyhow!("HID backend registration is poisoned"))?
                .select();
            *slot = Some(match factory {
                Some(factory) => factory().context("Create registered HID backend")?,
                None => Box::new(NativeBackend::new()?),
            });
        }
        action(slot.as_mut().expect("initialized HID backend").as_mut())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unused_factory() -> anyhow::Result<Box<dyn HidBackend>> {
        anyhow::bail!("registration tests never create a backend")
    }

    #[test]
    fn registration_cannot_replace_an_existing_owner() {
        let mut registration = Registration::default();
        registration.install(unused_factory).unwrap();
        assert!(registration.install(unused_factory).is_err());
        assert!(registration.select().is_some());
    }

    #[test]
    fn first_use_prevents_a_late_factory_even_before_native_creation() {
        let mut registration = Registration::default();
        assert!(registration.select().is_none());
        assert!(registration.install(unused_factory).is_err());
    }
}
