//! Workspace composition and the single product follow-up of a local save.
use super::*;
pub(super) struct PreparedSave {
    pub(super) window: AnyWindowHandle,
    pub(super) file: store::WorkspaceFile,
    pub(super) product: Option<ProductSave>,
}
pub(super) enum ProductSave {
    Leviathan(leviathan::LeviathanSave),
    MousePolling(MousePollingSave),
    MouseDpiStages(MouseDpiStagesSave),
    MouseLegacyDpi(MouseLegacyDpiSave),
    MouseCurrentDpi(MouseCurrentDpiSave),
    MouseCurrentPower(MouseCurrentPowerSave),
    ControllerTrigger(ControllerTriggerSave),
    ControllerLighting(ControllerLightingSave),
    ControllerChroma(ControllerChromaSave),
    SeirenEq(seiren_eq::SeirenEqSave),
    MouseProfile(MouseLegacyProfileSave),
    KeyboardActuation(keyboard_actuation_write::KeyboardActuationSave),
}
impl AppShell {
    /// Shared workspace fields are composed once for every product and save path.
    /// The caller supplies device drafts or saved revisions according to its operation.
    pub(super) fn workspace_file(&self, devices: Vec<Device>, cx: &App) -> store::WorkspaceFile {
        store::WorkspaceFile::new(devices, self.tracking_intro_seen)
            .with_shortcuts(self.shortcuts.read(cx).saved_snapshot())
            .with_preferences(self.settings.read(cx).snapshot())
            .with_custom_colors(cx.global::<CustomColors>().colors())
            .with_host_tab_order(self.host_tabs.order())
            .with_dashboard(self.dashboard_state.read(cx).snapshot())
            .with_module_services(self.module_catalog.read(cx).service_snapshot())
            .with_macros(self.macro_library.read(cx).snapshot())
    }
}

impl AppShell {
    pub(super) fn capture_save_owners(
        &self,
        cx: &App,
    ) -> BTreeMap<String, WeakEntity<ProductWorkspace>> {
        self.devices
            .iter()
            .map(|owner| (owner.read(cx).identity(cx), owner.downgrade()))
            .collect()
    }
    pub(super) fn saved_revision_owner(
        &self,
        owners: &BTreeMap<String, WeakEntity<ProductWorkspace>>,
        snapshot: &Device,
        cx: &App,
    ) -> Option<Entity<ProductWorkspace>> {
        let identity = snapshot.workspace_identity();
        let owner = owners.get(&identity)?.upgrade()?;
        (self.devices.contains(&owner) && owner.read(cx).identity(cx) == identity).then_some(owner)
    }
}
