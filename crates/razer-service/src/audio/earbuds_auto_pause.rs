//! Worker entry point; the service client and physical task lease outlive I/O.
use razer_device::earbuds_auto_pause::{Operation, Report};
use razer_discovery::{direct, discovery::ObservedDevice};
use std::sync::{Arc, atomic::AtomicBool};

pub enum MonitorUpdate {
    Ready,
    Notification(razer_device::earbuds_events::Notification),
}

pub fn monitor(
    route: ObservedDevice,
    active: Arc<AtomicBool>,
    mut publish: impl FnMut(MonitorUpdate) -> anyhow::Result<()>,
) -> crate::mouse_worker::Completion<()> {
    use razer_device::earbuds_events::{PollReply, StartReply};
    use razer_ipc::ServiceRequest;
    use std::{sync::atomic::Ordering, time::Duration};
    let keep = active.clone();
    crate::mouse_worker::device(
        route,
        active,
        |route| razer_device::earbuds_protocol40::supports_auto_pause(route.product_id()),
        move |client, route| {
            let session_id = uuid::Uuid::new_v4().to_string();
            let target = direct::earbuds_auto_pause::event_target(client, route)?;
            let started: StartReply =
                serde_json::from_value(client.request(ServiceRequest::EarbudsEventsStart {
                    session_id: session_id.clone(),
                    target,
                })?)?;
            anyhow::ensure!(
                started.session_id == session_id && started.started,
                "Earbud event source did not start"
            );
            publish(MonitorUpdate::Ready)?;
            while keep.load(Ordering::Acquire) {
                let batch: PollReply =
                    serde_json::from_value(client.request(ServiceRequest::EarbudsEventsPoll {
                        session_id: session_id.clone(),
                    })?)?;
                anyhow::ensure!(
                    batch.session_id == session_id,
                    "Earbud event response belongs to another owner"
                );
                for event in batch.events {
                    publish(MonitorUpdate::Notification(event))?;
                }
                if let Some(error) = batch.error {
                    anyhow::bail!("Earbud event source failed: {error}");
                }
                if batch.terminal {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            // The common worker's unconditional Shutdown invokes the event owner's
            // cancel/join/finish chain even after the request guard is canceled.
            Ok(())
        },
    )
}

pub fn device(
    route: ObservedDevice,
    operation: Operation,
    active: Arc<AtomicBool>,
) -> crate::mouse_worker::Completion<Report> {
    crate::mouse_worker::device(
        route,
        active,
        |route| razer_device::earbuds_protocol40::supports_auto_pause(route.product_id()),
        |client, route| direct::earbuds_auto_pause::request(client, route, operation),
    )
}
