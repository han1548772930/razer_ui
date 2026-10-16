//! Current MapInterDevice: focus the exact named UI, then SELECTED_TAB.
use super::*;
use razer_pages::features::keyboard_products::KeyboardMappingInterDeviceConfigureRequested;

#[derive(Clone)]
pub(super) struct InterdeviceHold {
    product_id: u32,
    raw: serde_json::Value,
    target: Option<WeakEntity<ProductWorkspace>>,
}

fn connected(workspace: &Entity<ProductWorkspace>, cx: &App) -> bool {
    use razer_model::model::DeviceConnectionObservation;
    matches!(
        workspace
            .read(cx)
            .device(cx)
            .dashboard
            .connection_observation,
        Some(DeviceConnectionObservation::UsbPresent | DeviceConnectionObservation::HidPresent)
    )
}

fn release_hold(hold: &InterdeviceHold, cx: &mut App) {
    let Some(target) = hold.target.as_ref().and_then(WeakEntity::upgrade) else {
        return;
    };
    let mut raw = hold.raw.clone();
    if let Some(mut output) = raw["output"]
        .as_str()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
    {
        output["flag"] = serde_json::json!(1);
        raw["output"] = serde_json::json!(output.to_string());
        target.update(cx, |target, cx| {
            target.consume_interdevice_mapping(raw, cx);
        });
    }
}

impl AppShell {
    pub(super) fn route_mapping_callback(
        &mut self,
        origin: &Entity<ProductWorkspace>,
        request: &razer_pages::features::MappingCallbackRequested,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(origin) {
            return;
        }
        let raw = request.raw();
        let Some(output) = raw["output"]
            .as_str()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        else {
            return;
        };
        if output["type"] != "interdevice" {
            eprintln!("[mapping-callback] local Chroma dispatch remains unacknowledged: {output}");
            return;
        }
        // Current 6120/59490 R scans runtime storage in insertion order and
        // forwards to the first product-ID match; it does not broadcast.
        let target = self
            .devices
            .iter()
            .find(|workspace| {
                let device = workspace.read(cx).device(cx);
                output["pid"] == device.product_id && connected(workspace, cx)
            })
            .cloned();
        let mut forwarded = raw.clone();
        forwarded["eventName"] = serde_json::json!("interDevice");
        forwarded["output"] = serde_json::json!(output["value"].to_string());
        if output["value"]["type"] == "sensitivity"
            && output["value"]["sensitivityGroup"]["sensitivityAssignment"] == "DPI_OnTheFly"
        {
            if output["value"]["flag"] == 0 {
                if let Some(product_id) =
                    output["pid"].as_u64().and_then(|id| u32::try_from(id).ok())
                {
                    self.mapping_interdevice_holds.insert(
                        origin.entity_id(),
                        InterdeviceHold {
                            product_id,
                            raw: forwarded.clone(),
                            target: target.as_ref().map(Entity::downgrade),
                        },
                    );
                }
            } else {
                self.mapping_interdevice_holds.remove(&origin.entity_id());
            }
        }
        let Some(target) = target else {
            return;
        };
        if !target.update(cx, |target, cx| {
            target.consume_interdevice_mapping(forwarded, cx)
        }) {
            eprintln!(
                "[inter-device-mapping] target {} has no mounted native callback consumer",
                output["pid"]
            );
        }
    }
    pub(super) fn sync_local_mapping_targets(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snapshots: Vec<_> = self
            .devices
            .iter()
            .map(|device| device.read(cx).snapshot(cx))
            .collect();
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.set_local_mapping_targets(&snapshots, window, cx)
            });
        }
        // Current R retains a release callback and observes DEVICE_RUNTIME_DATA
        // ready transitions. Keep that lifecycle outside the target UI page.
        let holds: Vec<_> = self
            .mapping_interdevice_holds
            .iter()
            .map(|(id, hold)| (*id, hold.clone()))
            .collect();
        for (origin, mut hold) in holds {
            if !self
                .devices
                .iter()
                .any(|workspace| workspace.entity_id() == origin && connected(workspace, cx))
            {
                release_hold(&hold, cx);
                self.mapping_interdevice_holds.remove(&origin);
                continue;
            }
            if let Some(target) = self
                .devices
                .iter()
                .find(|workspace| {
                    workspace.read(cx).device(cx).product_id == hold.product_id
                        && connected(workspace, cx)
                })
                .cloned()
            {
                target.update(cx, |target, cx| {
                    target.consume_interdevice_mapping(hold.raw.clone(), cx);
                });
                hold.target = Some(target.downgrade());
                self.mapping_interdevice_holds.insert(origin, hold);
            }
        }
    }
    pub(super) fn configure_mouse_mapping_target(
        &mut self,
        origin: &Entity<ProductWorkspace>,
        request: &razer_pages::features::mouse_products::MouseMappingTargetRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(origin) {
            return;
        }
        let row = request.target();
        let Some(ui) = row["UIWindowName"].as_str().filter(|name| !name.is_empty()) else {
            return;
        };
        let Some(mw) = row["MWWindowName"].as_str().filter(|name| !name.is_empty()) else {
            return;
        };
        let targets = self
            .devices
            .iter()
            .filter(|workspace| {
                let device = workspace.read(cx).device(cx);
                row["productId"] == device.product_id
                    && device.ui_window_name == ui
                    && device.mw_window_name == mw
                    && row["deviceContainerId"] == device.device_container_id
            })
            .cloned()
            .collect::<Vec<_>>();
        let [target] = targets.as_slice() else {
            eprintln!("[mouse-inter-device] exact target workspace is unavailable");
            return;
        };
        let identity = target.read(cx).identity(cx);
        self.navigate(Location::Device(identity), window, cx);
        window.activate_window();
        target.update(cx, |target, cx| {
            target.set_page(Tab::Performance, window, cx)
        });
    }
    pub(super) fn read_keyboard_mapping_brightness(
        &mut self,
        origin: &Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardMappingBrightnessReadRequested,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(origin)
            || !origin
                .read(cx)
                .accepts_keyboard_mapping_brightness(request, cx)
        {
            return;
        }
        // The genuine host MemoryStorage producer for lighting-engine-chroma-devices
        // is unresolved. Keep original initial hasLed=false and the actual query
        // pending; source_local is not this process-local host store.
        eprintln!(
            "[keyboard-brightness] original lighting engine memory observation is not connected"
        );
    }
    pub(super) fn read_keyboard_mapping_dial(
        &mut self,
        origin: &Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardMappingDialReadRequested,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(origin)
            || !origin.read(cx).accepts_keyboard_mapping_dial(request, cx)
        {
            return;
        }
        // Current 659 requests SourceAudio.simpleEnumerateAudioDevices and
        // MemoryStorage[lighting-engine-chroma-devices]. Neither real producer
        // is connected. Retain the unobserved form rather than supplying an
        // empty successful enumeration or substituting local preference data.
        eprintln!(
            "[keyboard-dial] source audio and lighting memory observations are not connected"
        );
    }
    pub(super) fn configure_mapping_target(
        &mut self,
        origin: &Entity<ProductWorkspace>,
        request: &KeyboardMappingInterDeviceConfigureRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !origin
            .read(cx)
            .accepts_keyboard_mapping_inter_device_configure(request, cx)
        {
            return Err(
                "Mapping target request no longer belongs to the current source profile".into(),
            );
        }
        if request.ui_window_name().is_empty() || request.mw_window_name().is_empty() {
            return Ok(());
        }
        if request.source_message()["type"] != "SELECTED_TAB"
            || request.source_message()["payload"] != "TAB_PERFORMANCE"
        {
            return Err("Unresolved source mapping target navigation message".into());
        }
        let targets: Vec<_> = self
            .devices
            .iter()
            .filter(|workspace| {
                let device = workspace.read(cx).device(cx);
                device.product_id == request.target_product_id()
                    && device.ui_window_name == request.ui_window_name()
                    && device.mw_window_name == request.mw_window_name()
            })
            .cloned()
            .collect();
        let [target] = targets.as_slice() else {
            return Err(
                "The exact named mapping target UI and middleware owner are not uniquely available"
                    .into(),
            );
        };
        let key = target.read(cx).identity(cx);
        self.navigate(Location::Device(key), window, cx);
        window.activate_window();
        target.update(cx, |target, cx| {
            target.set_page(Tab::Performance, window, cx)
        });
        // This is native page navigation; no device read/write acknowledgement.
        // The original BroadcastChannel is posted and immediately closed.
        Ok(())
    }
}
