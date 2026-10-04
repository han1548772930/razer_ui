//! Shell-facing owner for the original adapters and source-specific products.
//! Selecting a registered product never creates another product's controls.
use super::{
    DeviceWorkspace, WorkspaceEvent, source_workspace::SourceProductWorkspace, workspace::Continue,
};
use crate::{model::Device, nav::Tab};
use gpui_kit::*;

enum Body {
    Existing(Entity<DeviceWorkspace>),
    Source(Entity<SourceProductWorkspace>),
}

/// Local metadata changes made by the Profiles application. They do not select
/// an active hardware profile or replace a product's source settings.
pub(super) enum ProfileMetadata {
    Rename(String),
    LinkGame {
        name: String,
        executable: String,
        linked: bool,
    },
}

pub(super) fn edit_profile_metadata(
    device: &mut Device,
    id: &str,
    change: ProfileMetadata,
) -> bool {
    let Some(index) = device.profiles.iter().position(|profile| profile.id == id) else {
        return false;
    };
    match change {
        ProfileMetadata::Rename(name) => {
            let name = name.trim();
            if name.is_empty()
                || name.encode_utf16().count() > 32
                || device.profiles.iter().any(|p| p.name == name && p.id != id)
                || device.profiles[index].name == name
            {
                return false;
            }
            device.profiles[index].name = name.into();
        }
        ProfileMetadata::LinkGame {
            name,
            executable,
            linked,
        } => {
            let key = |value: &str| value.replace('/', "\\").to_lowercase();
            let requested = key(&executable);
            // Source linkedDeviceToGame assigns at most one profile per device.
            for profile in &mut device.profiles {
                if let Some(settings) = &mut profile.settings {
                    settings
                        .linked_games
                        .retain(|game| key(&game.executable) != requested);
                }
            }
            if linked {
                device.profiles[index]
                    .settings
                    .get_or_insert_with(|| {
                        super::settings::ProfileSettings::for_product(device.product_id)
                    })
                    .linked_games
                    .push(super::settings::LinkedGame { name, executable });
            }
        }
    }
    true
}
pub(crate) struct ProductWorkspace {
    body: Body,
    _subscription: Subscription,
}
impl EventEmitter<WorkspaceEvent> for ProductWorkspace {}

impl ProductWorkspace {
    pub(crate) fn new(
        device: Device,
        intro: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if !Tab::for_product(device.product_id).is_empty() {
            let entity = cx.new(|cx| DeviceWorkspace::new(device, intro, window, cx));
            let subscription = cx.subscribe(&entity, |_, _, event, cx| {
                cx.emit(match event {
                    WorkspaceEvent::Changed => WorkspaceEvent::Changed,
                    WorkspaceEvent::IntroDismissed => WorkspaceEvent::IntroDismissed,
                    WorkspaceEvent::ShareProfile => WorkspaceEvent::ShareProfile,
                    WorkspaceEvent::PairingRequested(device) => {
                        WorkspaceEvent::PairingRequested(device)
                    }
                });
                cx.notify();
            });
            Self {
                body: Body::Existing(entity),
                _subscription: subscription,
            }
        } else {
            let entity = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
            let subscription = cx.subscribe(&entity, |_, _, event, cx| {
                cx.emit(match event {
                    WorkspaceEvent::Changed => WorkspaceEvent::Changed,
                    WorkspaceEvent::IntroDismissed => WorkspaceEvent::IntroDismissed,
                    WorkspaceEvent::ShareProfile => WorkspaceEvent::ShareProfile,
                    WorkspaceEvent::PairingRequested(device) => {
                        WorkspaceEvent::PairingRequested(device)
                    }
                });
                cx.notify();
            });
            Self {
                body: Body::Source(entity),
                _subscription: subscription,
            }
        }
    }
    pub(crate) fn device<'a>(&'a self, cx: &'a App) -> &'a Device {
        match &self.body {
            Body::Existing(e) => e.read(cx).device(),
            Body::Source(e) => e.read(cx).device(),
        }
    }
    pub(crate) fn identity(&self, cx: &App) -> String {
        let d = self.device(cx);
        format!(
            "{}:{}:{}",
            d.product_id, d.serial_number, d.device_container_id
        )
    }
    pub(crate) fn snapshot(&self, cx: &App) -> Device {
        self.device(cx).clone()
    }
    /// The current 653 chromaApp root mounts the same lighting content as its
    /// normal lighting tab, without the product navigation/profile chrome.
    pub(crate) fn chroma_lighting_workspace(&self, cx: &App) -> Option<Entity<DeviceWorkspace>> {
        match &self.body {
            Body::Existing(workspace) if workspace.read(cx).device().product_id == 653 => {
                Some(workspace.clone())
            }
            _ => None,
        }
    }
    /// Existing local profile associations; no executable is launched or scanned.
    pub(crate) fn profile_linked_games(&self, profile: &str, cx: &App) -> Vec<(String, String)> {
        self.device(cx)
            .profiles
            .iter()
            .find(|entry| entry.id == profile)
            .and_then(|entry| entry.settings.as_ref())
            .map(|settings| {
                settings
                    .linked_games
                    .iter()
                    .map(|game| (game.name.clone(), game.executable.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Select a profile from the Profiles module and reuse the product page's
    /// existing continuation path. Existing workspaces preserve their dirty
    /// mapping decision; source-backed products restore the selected profile
    /// snapshot locally without contacting hardware.
    pub(crate) fn select_profile(
        &mut self,
        profile: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(entity) => entity.update(cx, |workspace, cx| {
                workspace.continue_with(Continue::Profile(profile.to_owned()), window, cx)
            }),
            Body::Source(entity) => entity.update(cx, |workspace, cx| {
                workspace.select_profile(profile, window, cx)
            }),
        }
        cx.notify();
    }
    pub(crate) fn rename_profile(
        &mut self,
        profile: &str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_profile_metadata(profile, ProfileMetadata::Rename(name), window, cx);
    }
    pub(crate) fn link_profile_game(
        &mut self,
        profile: &str,
        name: String,
        executable: String,
        linked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_profile_metadata(
            profile,
            ProfileMetadata::LinkGame {
                name,
                executable,
                linked,
            },
            window,
            cx,
        );
    }
    fn change_profile_metadata(
        &mut self,
        profile: &str,
        change: ProfileMetadata,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(entity) => entity.update(cx, |value, cx| {
                value.change_profile_metadata(profile, change, window, cx)
            }),
            Body::Source(entity) => entity.update(cx, |value, cx| {
                value.change_profile_metadata(profile, change, window, cx)
            }),
        }
    }
    pub(crate) fn saved_snapshot(&self, cx: &App) -> Device {
        match &self.body {
            Body::Existing(e) => e.read(cx).saved_snapshot(),
            Body::Source(e) => e.read(cx).saved_snapshot(),
        }
    }
    pub(crate) fn committed_pending(&self, cx: &App) -> bool {
        match &self.body {
            Body::Existing(e) => e.read(cx).committed_pending(),
            Body::Source(e) => e.read(cx).dirty(),
        }
    }
    pub(crate) fn discard_would_remove_mapping(&self, cx: &App) -> bool {
        matches!(&self.body, Body::Existing(e) if e.read(cx).discard_would_remove_mapping())
    }
    pub(crate) fn mark_saved(&mut self, snapshot: Device, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.mark_saved(snapshot, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.mark_saved(snapshot, cx)),
        }
        cx.notify();
    }
    pub(crate) fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.discard(window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.discard(window, cx)),
        }
        cx.notify();
    }
    pub(crate) fn discard_committed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.discard_committed(window, cx)),
            Body::Source(e) => {
                e.update(cx, |v, cx| v.discard(window, cx));
                true
            }
        }
    }
    pub(crate) fn set_intro_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if let Body::Existing(e) = &self.body {
            e.update(cx, |v, cx| v.set_intro_seen(seen, cx));
        }
    }
    pub(crate) fn set_page(&mut self, page: Tab, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.set_page(page, window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.set_page_key(page.id(), window, cx)),
        }
        cx.notify();
    }
    pub(crate) fn can_step_history(&self, forward: bool, cx: &App) -> bool {
        match &self.body {
            Body::Existing(e) => e.read(cx).can_step_history(forward),
            Body::Source(e) => e.read(cx).can_step_history(forward),
        }
    }
    pub(crate) fn step_page_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.step_page_history(forward, window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.step_page_history(forward, window, cx)),
        }
        cx.notify();
    }
    pub(crate) fn set_source_page(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(e) = &self.body {
            e.update(cx, |v, cx| v.set_page_key(key, window, cx));
        }
    }
    pub(crate) fn dismiss_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.dismiss_profile_dialog(window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.dismiss_profile_dialog(window, cx)),
        }
    }
    pub(crate) fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.refresh_locale(window, cx)),
            Body::Source(e) => e.update(cx, |_, cx| cx.notify()),
        }
    }
}
impl Render for ProductWorkspace {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.body {
            Body::Existing(e) => e.clone().into_any_element(),
            Body::Source(e) => e.clone().into_any_element(),
        }
    }
}
