//! Direct HID Rotation requests own their service client on the I/O worker.
use razer_device::mouse_rotation::{Report, Setting};
use razer_discovery::{direct, discovery::ObservedDevice};
use std::sync::{Arc, atomic::AtomicBool};

pub type Completion = crate::mouse_worker::Completion<Report>;

/// Runs on the caller's retained background worker. The guard applies to
/// device requests; shutdown and process cleanup still run after cancellation.
pub fn device(
    route: ObservedDevice,
    change: Option<Setting>,
    active: Arc<AtomicBool>,
) -> Completion {
    crate::mouse_worker::device(
        route,
        active,
        direct::mouse_rotation::available,
        |client, route| direct::mouse_rotation::request(client, route, change),
    )
}
