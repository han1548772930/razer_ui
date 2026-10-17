//! Hanbo 3893 command owner. The page owns only the optimistic draft; this
//! module owns discovery selection, cancellation and the service receipt.
use super::*;
use razer_discovery::discovery::ObservedTransport;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::HanboCoolerRequested;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

impl AppShell {
    pub(super) fn request_hanbo_cooler(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &HanboCoolerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = window;
        let product_id = workspace.read(cx).device(cx).product_id;
        if product_id != razer_device::hanbo_cooler::PRODUCT_ID || self.close_requested {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_hanbo_command(
                    request.generation,
                    Err("Hanbo command canceled before submission".into()),
                    cx,
                )
            });
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && route.physical_product_id() == razer_device::hanbo_cooler::PRODUCT_ID
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(ObservedTransport::Wired))
                    && route.hid_node().is_some()
            })
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_hanbo_command(
                    request.generation,
                    Err("Hanbo cooler has no unique wired HID collection".into()),
                    cx,
                )
            });
            return;
        };
        let Some(node) = route.hid_node().cloned() else {
            return;
        };
        if let Some(previous) = self.hanbo_cooler_guards.insert(
            identity.clone(),
            Arc::new(AtomicBool::new(true)),
        ) {
            previous.store(false, Ordering::Release);
        }
        self.spawn_hanbo_cooler(workspace, request, identity, route.clone(), node, cx);
    }

    pub(super) fn cancel_hanbo_cooler(&mut self) {
        for guard in self.hanbo_cooler_guards.values() {
            guard.store(false, Ordering::Release);
        }
        self.hanbo_cooler_guards.clear();
    }

    fn spawn_hanbo_cooler(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &HanboCoolerRequested,
        identity: String,
        route: razer_discovery::discovery::ObservedDevice,
        node: razer_device::backend::HidNode,
        cx: &mut Context<Self>,
    ) {
        let guard = self
            .hanbo_cooler_guards
            .get(&identity)
            .cloned()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(true)));
        let command = request.command.clone();
        let generation = request.generation;
        let revision = self.discovery_revision;
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                anyhow::ensure!(guard.load(Ordering::Acquire), "Hanbo command canceled before service creation");
                let mut client = ServiceClient::spawn()?;
                client.set_request_guard(guard.clone());
                let result = client.request(ServiceRequest::HidNodeHanboCooler {
                    node,
                    product_id: razer_device::hanbo_cooler::PRODUCT_ID,
                    command,
                });
                let shutdown = if client.is_stopped() {
                    None
                } else {
                    client.request(ServiceRequest::Shutdown).err().map(|error| format!("{error:#}"))
                };
                match (result, shutdown) {
                    (Ok(reply), None) => {
                        anyhow::ensure!(reply["acknowledged"].as_bool() == Some(true), "Hanbo service did not acknowledge the command");
                        Ok::<_, anyhow::Error>(())
                    }
                    (Ok(_), Some(error)) => Err(anyhow::anyhow!("Hanbo command acknowledged; service shutdown failed: {error}")),
                    (Err(error), _) => Err(error),
                }
            }).await;
            let _ = this.update(cx, |this, cx| {
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).hanbo_command_matches(generation, cx)
                    && this.discovery_revision == revision
                    && this.device_observations.iter().any(|current| {
                        current.matches(workspace.read(cx).device(cx))
                            && current.physical_product_id() == route.physical_product_id()
                            && current.hid_node() == route.hid_node()
                    });
                if current {
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_hanbo_command(generation, result.map_err(|error| format!("{error:#}")), cx)
                    });
                }
                cx.notify();
            });
        }).detach();
    }
}
