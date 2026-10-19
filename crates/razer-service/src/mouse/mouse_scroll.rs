//! Scroll setters retain their route and typed partial observations until the
//! shared mouse worker completes service shutdown and process cleanup.
use razer_device::mouse_scroll::{Submission, SubmissionReport};
use razer_discovery::{direct, discovery::ObservedDevice};
use std::sync::{Arc, atomic::AtomicBool};

pub type Completion = crate::mouse_worker::Completion<SubmissionReport>;

pub fn device(
    route: ObservedDevice,
    submission: Submission,
    active: Arc<AtomicBool>,
) -> Completion {
    crate::mouse_worker::device(
        route,
        active,
        direct::mouse_scroll::available,
        |client, route| direct::mouse_scroll::request(client, route, submission),
    )
}
