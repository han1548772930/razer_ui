//! Native HID implementation of razer-device's host-neutral backend contract.
//! Razer commands, capability selection and receiver peers belong above this
//! module. See docs/re/cross-platform-hid-current.md for original-code receipts.
mod descriptor;
mod native;

pub use native::NativeBackend;
use std::cell::RefCell;

thread_local! {
    static BACKEND: RefCell<Option<NativeBackend>> = const { RefCell::new(None) };
}

/// One system HID manager per worker thread, shared by every request route.
pub fn with_backend<T>(
    action: impl FnOnce(&mut NativeBackend) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    BACKEND.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(NativeBackend::new()?);
        }
        action(slot.as_mut().expect("initialized HID backend"))
    })
}
