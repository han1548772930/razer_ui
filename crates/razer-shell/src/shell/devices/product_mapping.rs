//! Current MapInterDevice: focus the exact named UI, then SELECTED_TAB.
use super::*;
use razer_model::mapping::interdevice::{HeldInterdeviceTarget, TargetObservation};
use razer_pages::features::keyboard_products::KeyboardMappingInterDeviceConfigureRequested;
use razer_storage::chroma_profiles::{Change as ChromaChange, ProfileDirection};
use serde::Deserialize;
use serde_json::Value;

#[path = "keyboard_mapping_observation.rs"]
mod observation;
pub(super) use observation::ObservationState;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChromaAssignment {
    #[serde(default)]
    chroma_effect_assignment: Option<String>,
}

#[derive(Default)]
enum ChromaId {
    #[default]
    Missing,
    Present(Option<String>),
}
fn deserialize_chroma_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ChromaId, D::Error> {
    Option::<String>::deserialize(deserializer).map(ChromaId::Present)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NavigationOutput {
    #[serde(default, deserialize_with = "deserialize_chroma_id")]
    lightpac_id: ChromaId,
    #[serde(flatten)]
    chroma: ChromaAssignment,
}
#[derive(Deserialize)]
#[serde(tag = "type")]
enum CallbackOutput {
    #[serde(rename = "lightpac")]
    Lightpac {
        id: Option<String>,
        #[serde(flatten)]
        chroma: ChromaAssignment,
    },
    #[serde(rename = "switchProfile")]
    SwitchProfile(NavigationOutput),
    #[serde(rename = "navigateProfile")]
    NavigateProfile(NavigationOutput),
    #[serde(rename = "interdevice")]
    Interdevice { pid: u32, value: Value },
    #[serde(other)]
    Other,
}
impl ChromaAssignment {
    fn change(&self, id: Option<&str>, product_id: u32) -> ChromaChange {
        // The current keyboard task runners pass selectedChromaProfileGuid
        // alone to the storage consumer. Only the audited mouse consumers
        // receive and evaluate chromaEffectAssignment.
        let assignment = matches!(product_id, 70 | 158)
            .then_some(self.chroma_effect_assignment.as_deref())
            .flatten();
        match assignment {
            Some("CycleUpChromaEffect") => ChromaChange::CycleProfile(ProfileDirection::Previous),
            Some("CycleDownChromaEffect") => ChromaChange::CycleProfile(ProfileDirection::Next),
            _ => match id.filter(|id| !id.is_empty()) {
                Some(id) => ChromaChange::SelectProfile(id.to_owned()),
                None => ChromaChange::Toggle(true),
            },
        }
    }
}
impl CallbackOutput {
    fn chroma_changes(&self, interdevice: bool, product_id: u32) -> Vec<ChromaChange> {
        match self {
            Self::Lightpac { id, chroma } if !interdevice => {
                vec![chroma.change(id.as_deref(), product_id)]
            }
            Self::SwitchProfile(navigation) | Self::NavigateProfile(navigation) => {
                let ChromaId::Present(id) = &navigation.lightpac_id else {
                    return Vec::new();
                };
                // Current 6120/59490, 60168 and the audited keyboard callers:
                // B/W applies linked Chroma for switchProfile; M/N applies it
                // again for an interDevice navigation with a truthy lightpacId.
                let count = usize::from(matches!(self, Self::SwitchProfile(_)))
                    + usize::from(interdevice && id.as_ref().is_some_and(|id| !id.is_empty()));
                (0..count)
                    .map(|_| navigation.chroma.change(id.as_deref(), product_id))
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

pub(super) struct InterdeviceHold {
    origin: MappingTargetOwner,
    product_id: u32,
    raw: serde_json::Value,
    target: HeldInterdeviceTarget<MappingTargetOwner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::shell) struct MappingTargetOwner {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
}

impl MappingTargetOwner {
    pub(in crate::shell) fn capture(workspace: &Entity<ProductWorkspace>, cx: &App) -> Self {
        Self {
            workspace: workspace.downgrade(),
            identity: workspace.read(cx).identity(cx),
            connection_revision: workspace.read(cx).connection_revision(),
        }
    }

    pub(in crate::shell) fn matches(&self, workspace: &Entity<ProductWorkspace>, cx: &App) -> bool {
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
        if self.close_requested
            || !self.devices.contains(origin)
            || !origin.read(cx).accepts_mapping_callback(request, cx)
        {
            return;
        }
        let raw = request.raw();
        let Some(output_value) = raw["output"]
            .as_str()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        else {
            self.status = "映射动作未执行：设备回调不是有效 JSON".into();
            cx.notify();
            return;
        };
        let output: CallbackOutput = match serde_json::from_value(output_value.clone()) {
            Ok(output) => output,
            Err(error) => {
                self.status = format!("映射动作未执行：{error}");
                cx.notify();
                return;
            }
        };
        let CallbackOutput::Interdevice {
            pid: product_id,
            value,
        } = &output
        else {
            self.request_mapping_chroma(
                vec![MappingTargetOwner::capture(origin, cx)],
                output.chroma_changes(
                    raw["eventName"] == "interDevice",
                    origin.read(cx).device(cx).product_id,
                ),
                cx,
            );
            return;
        };
        let product_id = *product_id;
        let target = self.first_mapping_target(product_id, cx);
        let mut forwarded = raw.clone();
        forwarded["eventName"] = serde_json::json!("interDevice");
        forwarded["output"] = serde_json::json!(value.to_string());
        if value["type"] == "sensitivity"
            && value["sensitivityGroup"]["sensitivityAssignment"] == "DPI_OnTheFly"
        {
            match value["flag"].as_u64() {
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
                product_id
            );
        } else {
            match serde_json::from_value::<CallbackOutput>(value.clone()) {
                Ok(output) => self.request_mapping_chroma(
                    vec![
                        MappingTargetOwner::capture(origin, cx),
                        target.owner().clone(),
                    ],
                    output.chroma_changes(true, product_id),
                    cx,
                ),
                Err(error) => {
                    self.status = format!("关联 Chroma 动作未执行：{error}");
                    cx.notify();
                }
            }
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
        self.retain_mapping_chroma_connections(retained, cx);
        self.retain_mapping_observation_connections(retained, cx);
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

#[cfg(test)]
mod callback_tests {
    use super::{CallbackOutput, ChromaChange, ProfileDirection, Value};
    use serde_json::json;

    fn changes(value: Value, interdevice: bool) -> Vec<ChromaChange> {
        serde_json::from_value::<CallbackOutput>(value)
            .unwrap()
            .chroma_changes(interdevice, 158)
    }

    #[test]
    fn mapping_chroma_keeps_the_audited_direct_and_interdevice_enqueue_counts() {
        for (kind, direct, interdevice) in [("switchProfile", 1, 2), ("navigateProfile", 0, 1)] {
            let output = json!({"type":kind,"lightpacId":"profile-b","chromaEffectAssignment":"CycleDownChromaEffect"});
            assert_eq!(
                changes(output.clone(), false),
                vec![ChromaChange::CycleProfile(ProfileDirection::Next); direct]
            );
            assert_eq!(
                changes(output, true),
                vec![ChromaChange::CycleProfile(ProfileDirection::Next); interdevice]
            );
        }
    }

    #[test]
    fn mapping_chroma_distinguishes_missing_defined_empty_and_defined_null_ids() {
        assert!(changes(json!({"type":"switchProfile"}), false).is_empty());
        for id in [json!(""), Value::Null] {
            let output = json!({"type":"switchProfile","lightpacId":id});
            assert_eq!(changes(output.clone(), false), [ChromaChange::Toggle(true)]);
            assert_eq!(changes(output, true), [ChromaChange::Toggle(true)]);
        }
        assert_eq!(
            changes(json!({"type":"lightpac","id":"local-b"}), false),
            [ChromaChange::SelectProfile("local-b".into())]
        );
        assert_eq!(
            changes(
                json!({"type":"lightpac","id":"ignored","chromaEffectAssignment":"CycleUpChromaEffect"}),
                false
            ),
            [ChromaChange::CycleProfile(ProfileDirection::Previous)]
        );
    }

    #[test]
    fn mapping_chroma_rejects_wrong_boundary_types_and_does_not_dispatch_unmounted_actions() {
        for output in [
            json!({"type":"interdevice","pid":"158","value":{}}),
            json!({"type":"lightpac","id":3}),
            json!({"type":"switchProfile","lightpacId":false}),
            json!({"type":"lightpac","chromaEffectAssignment":[]}),
        ] {
            assert!(serde_json::from_value::<CallbackOutput>(output).is_err());
        }
        assert!(changes(json!({"type":"backlight"}), false).is_empty());
        assert!(changes(json!({"type":"lightpac","id":"local-b"}), true).is_empty());
    }

    #[test]
    fn mapping_chroma_selects_the_actual_target_storage_consumer_instead_of_copying_mouse_cycles() {
        let output: CallbackOutput = serde_json::from_value(json!({"type":"switchProfile","lightpacId":"local-b","chromaEffectAssignment":"CycleDownChromaEffect"})).unwrap();
        for product_id in [515, 679, 740, 746, 756] {
            assert_eq!(
                output.chroma_changes(false, product_id),
                [ChromaChange::SelectProfile("local-b".into())]
            );
            assert_eq!(
                output.chroma_changes(true, product_id),
                [
                    ChromaChange::SelectProfile("local-b".into()),
                    ChromaChange::SelectProfile("local-b".into())
                ]
            );
        }
        for product_id in [70, 158] {
            assert_eq!(
                output.chroma_changes(true, product_id),
                [
                    ChromaChange::CycleProfile(ProfileDirection::Next),
                    ChromaChange::CycleProfile(ProfileDirection::Next)
                ]
            );
        }
    }
}
