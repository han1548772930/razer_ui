//! Current MapInterDevice: focus the exact named UI, then SELECTED_TAB.
use super::*;
use razer_model::mapping::interdevice::{HeldInterdeviceTarget, TargetObservation};
use razer_pages::features::keyboard_products::KeyboardMappingInterDeviceConfigureRequested;

pub(super) struct InterdeviceHold {
    origin: MappingTargetOwner,
    product_id: u32,
    raw: serde_json::Value,
    target: HeldInterdeviceTarget<MappingTargetOwner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MappingTargetOwner {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
}

impl MappingTargetOwner {
    fn capture(workspace: &Entity<ProductWorkspace>, cx: &App) -> Self {
        Self {
            workspace: workspace.downgrade(),
            identity: workspace.read(cx).identity(cx),
            connection_revision: workspace.read(cx).connection_revision(),
        }
    }

    fn matches(&self, workspace: &Entity<ProductWorkspace>, cx: &App) -> bool {
        self.workspace == *workspace
            && workspace.read(cx).identity(cx) == self.identity
            && workspace.read(cx).connection_revision() == self.connection_revision
    }
}

fn dispatch_to_target(owner: &MappingTargetOwner, raw: serde_json::Value, cx: &mut App) -> bool {
    let Some(target) = owner.workspace.upgrade() else {
        return false;
    };
    if !owner.matches(&target, cx) {
        return false;
    }
    target.update(cx, |target, cx| target.consume_interdevice_mapping(raw, cx))
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

fn release_target(owner: &MappingTargetOwner, pressed: &serde_json::Value, cx: &mut App) {
    let mut raw = pressed.clone();
    if let Some(mut output) = raw["output"]
        .as_str()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
    {
        output["flag"] = serde_json::json!(1);
        raw["output"] = serde_json::json!(output.to_string());
        if !dispatch_to_target(owner, raw, cx) {
            eprintln!("[inter-device-mapping] held target is unavailable during release");
        }
    }
}

fn release_hold(hold: InterdeviceHold, cx: &mut App) {
    if let Some(target) = hold.target.release() {
        release_target(&target, &hold.raw, cx);
    }
}

impl AppShell {
    fn first_mapping_target(
        &self,
        product_id: u32,
        cx: &App,
    ) -> Option<TargetObservation<MappingTargetOwner>> {
        // Current 6120/59490 R scans runtime storage in insertion order and
        // forwards to the first product-ID match; it does not broadcast.
        let workspace = self.devices.iter().find(|workspace| {
            workspace.read(cx).device(cx).product_id == product_id && connected(workspace, cx)
        })?;
        Some(TargetObservation::new(
            MappingTargetOwner::capture(workspace, cx),
            workspace.read(cx).device(cx).setup_status,
        ))
    }

    pub(super) fn route_mapping_callback(
        &mut self,
        origin: &Entity<ProductWorkspace>,
        request: &razer_pages::features::MappingCallbackRequested,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || !self.devices.contains(origin) {
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
        let Some(product_id) = output["pid"]
            .as_u64()
            .and_then(|product_id| u32::try_from(product_id).ok())
        else {
            eprintln!("[inter-device-mapping] callback has no valid target product ID");
            return;
        };
        let target = self.first_mapping_target(product_id, cx);
        let mut forwarded = raw.clone();
        forwarded["eventName"] = serde_json::json!("interDevice");
        forwarded["output"] = serde_json::json!(output["value"].to_string());
        if output["value"]["type"] == "sensitivity"
            && output["value"]["sensitivityGroup"]["sensitivityAssignment"] == "DPI_OnTheFly"
        {
            match output["value"]["flag"].as_u64() {
                Some(0) => {
                    let origin_owner = MappingTargetOwner::capture(origin, cx);
                    if let Some(mut previous) =
                        self.mapping_interdevice_holds.remove(&origin.entity_id())
                    {
                        if previous.origin != origin_owner {
                            release_hold(previous, cx);
                        } else if let Some(released) =
                            previous.target.observe(target.clone()).released()
                        {
                            release_target(released, &previous.raw, cx);
                        }
                    }
                    self.mapping_interdevice_holds.insert(
                        origin.entity_id(),
                        InterdeviceHold {
                            origin: origin_owner,
                            product_id,
                            raw: forwarded.clone(),
                            target: HeldInterdeviceTarget::new(target.clone()),
                        },
                    );
                }
                Some(1) => {
                    if let Some(hold) = self.mapping_interdevice_holds.remove(&origin.entity_id()) {
                        if let Some(target) = hold.target.release()
                            && !dispatch_to_target(&target, forwarded, cx)
                        {
                            eprintln!(
                                "[inter-device-mapping] held target is unavailable during release"
                            );
                        }
                        return;
                    }
                }
                _ => {
                    eprintln!(
                        "[inter-device-mapping] held callback has no valid press/release flag"
                    );
                    return;
                }
            }
        }
        let Some(target) = target else {
            return;
        };
        if !dispatch_to_target(target.owner(), forwarded, cx) {
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
        self.sync_interdevice_holds(cx);
    }

    fn sync_interdevice_holds(&mut self, cx: &mut Context<Self>) {
        let holds = std::mem::take(&mut self.mapping_interdevice_holds);
        for (origin, mut hold) in holds {
            if !self
                .devices
                .iter()
                .any(|workspace| hold.origin.matches(workspace, cx) && connected(workspace, cx))
            {
                release_hold(hold, cx);
                continue;
            }
            let transition = hold
                .target
                .observe(self.first_mapping_target(hold.product_id, cx));
            if let Some(released) = transition.released() {
                release_target(released, &hold.raw, cx);
            }
            if let Some(replayed) = transition.replayed()
                && !dispatch_to_target(replayed, hold.raw.clone(), cx)
            {
                eprintln!("[inter-device-mapping] ready target has no mounted callback consumer");
            }
            self.mapping_interdevice_holds.insert(origin, hold);
        }
    }

    pub(super) fn release_interdevice_holds(&mut self, cx: &mut Context<Self>) {
        for hold in std::mem::take(&mut self.mapping_interdevice_holds).into_values() {
            release_hold(hold, cx);
        }
    }

    /// Release callbacks must reach the outgoing owner before its mapping
    /// worker is stopped. Holds from retained origins survive target removal.
    pub(super) fn retain_interdevice_connections(
        &mut self,
        retained: &[Entity<ProductWorkspace>],
        cx: &mut Context<Self>,
    ) {
        for (origin, mut hold) in std::mem::take(&mut self.mapping_interdevice_holds) {
            if !retained
                .iter()
                .any(|workspace| hold.origin.matches(workspace, cx))
            {
                release_hold(hold, cx);
                continue;
            }
            if hold.target.owner().is_some_and(|owner| {
                !retained
                    .iter()
                    .any(|workspace| owner.matches(workspace, cx))
            }) {
                let transition = hold.target.observe(None);
                if let Some(target) = transition.released() {
                    release_target(target, &hold.raw, cx);
                }
            }
            self.mapping_interdevice_holds.insert(origin, hold);
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
