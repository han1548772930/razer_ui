//! Function-key requests use the shared physical task/client cleanup owner.
use razer_device::keyboard_function_swap::{FunctionKeyPrimary, FunctionKeySwapWriteReport};
use razer_discovery::{direct::keyboard_metadata, discovery::ObservedDevice};
use std::sync::{Arc, atomic::AtomicBool};

pub type Completion = crate::mouse_worker::Completion<FunctionKeySwapWriteReport>;

/// Run on the retained background worker. `active` starts true and is cleared
/// to cancel requests; the shared owner still finishes shutdown and release.
pub fn write(
    route: ObservedDevice,
    primary: FunctionKeyPrimary,
    active: Arc<AtomicBool>,
) -> Completion {
    crate::mouse_worker::device(
        route,
        active,
        keyboard_metadata::function_swap_available,
        |client, route| {
            keyboard_metadata::resolve(client, route)?.write_function_primary(client, primary)
        },
    )
}
