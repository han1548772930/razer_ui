//! Native product ownership. Source navigation identity and local profile data
//! stay independent from the original ten hand-written adapters.
use super::{Choice, WorkspaceEvent};
use crate::{
    model::{Device, Profile},
    product::{self, ProductPage, ProductPageId, ProductPageRole},
    ui::{scroll::SourceScrollable as _, surface},
};
use gpui_kit::component::{
    input::{InputEvent, InputState},
    list::ListState,
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde_json::Value;

mod accessory;
mod profile_actions;
mod profile_bar;
mod profile_linked_games;
mod profile_menu;
mod profile_transfer;
#[cfg(test)]
mod tests;
use accessory::AccessoryPage;

enum FamilyBody {
    Mouse(Entity<super::mouse_products::MouseProductWorkspace>),
    Keyboard(Entity<super::keyboard_products::KeyboardProductWorkspace>),
    Gamepad(Entity<super::gamepad_products::GamepadProductWorkspace>),
    Controls(Entity<super::source_controls::SourceControls>),
    Audio(Entity<super::audio_products::AudioProductWorkspace>),
    System(Entity<super::system_products::SystemProductWorkspace>),
    AccessorySystem(Entity<super::accessory_system_products::AccessorySystemProductWorkspace>),
    Hue(Entity<super::hue::HueWorkspace>),
    Pending,
}
pub(crate) struct SourceProductWorkspace {
    device: Device,
    saved: Device,
    page: Option<ProductPageId>,
    page_history: Vec<ProductPageId>,
    page_history_index: usize,
    body: FamilyBody,
    help: Entity<super::source_help::SourceHelp>,
    supplement: Option<Entity<super::source_controls::SourceControls>>,
    dock_pairing: Option<Entity<super::dock_pairing::DockPairing>>,
    accessory: Option<AccessoryPage>,
    profile: Entity<SelectState<Vec<Choice>>>,
    profile_name: Entity<InputState>,
    profile_rename: Option<String>,
    profile_menu: Entity<ListState<profile_menu::ProfileCommands>>,
    profile_confirmation: Option<profile_actions::ProfileConfirmation>,
    profile_confirm_focus: FocusHandle,
    profile_confirm_task: Option<Task<()>>,
    profile_transfer: Option<Entity<profile_transfer::SourceProfileTransfer>>,
    profile_transfer_subscription: Option<Subscription>,
    profile_linked_games: Option<Entity<profile_linked_games::ProfileLinkedGames>>,
    profile_linked_games_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkspaceEvent> for SourceProductWorkspace {}
impl EventEmitter<super::OledRuntimeRequested> for SourceProductWorkspace {}

impl SourceProductWorkspace {
    pub(crate) fn observe_dpi_editing_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_dpi_editing_enabled(enabled, window, cx)
            });
        }
    }
    pub(crate) fn observe_scroll_wheel(
        &mut self,
        observation: super::ScrollWheelObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_scroll_wheel(observation, window, cx)
            });
        }
    }
    pub(crate) fn captures_snap_keys(&self, cx: &App) -> bool {
        self.current_page()
            .is_some_and(|page| page.role() != ProductPageRole::Help)
            && matches!(&self.body, FamilyBody::Keyboard(body) if body.read(cx).captures_snap_keys())
    }
    pub(crate) fn capture_snap_key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if !self.captures_snap_keys(cx) {
            return;
        }
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.capture_snap_key_up(event, cx));
        }
    }
    pub(crate) fn observe_snap_tap(
        &mut self,
        observation: super::SnapTapObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_snap_tap(observation, window, cx)
            });
        }
    }
    pub(crate) fn observe_stream_mixer(
        &mut self,
        observation: super::StreamMixerObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_stream_mixer(observation, window, cx)
            });
        }
    }
    /// Reserved for the current host's observed validDevices stream, never preview data.
    #[expect(
        dead_code,
        reason = "Current host runtime publisher is not yet connected"
    )]
    pub(crate) fn observe_pod_runtime_devices(
        &mut self,
        devices: Vec<super::audio_products::RuntimeAudioDevice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_pod_runtime_devices(devices, window, cx)
            });
        }
    }
    pub(crate) fn observe_oled_runtime(
        &mut self,
        observation: super::OledRuntimeObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_oled_runtime(observation, window, cx)
            });
        }
    }
    /// Report pages backed by entities this workspace actually owns. A help
    /// renderer or a registered navigation with only Pending content is not a
    /// locally implemented product destination.
    pub(crate) fn has_local_page(&self) -> bool {
        let Some(navigation) = product::registered(self.device.product_id)
            .and_then(|product| product.primary_navigation())
        else {
            return false;
        };
        navigation.pages().iter().any(|page| {
            if page.role() == ProductPageRole::Help {
                return false;
            }
            let key = page.kind().key();
            if self.dock_pairing.is_some()
                && super::dock_pairing::supports_page(self.device.product_id, key)
            {
                return true;
            }
            if self
                .accessory
                .as_ref()
                .is_some_and(|view| view.supports_page(self.device.product_id, key))
                || (self.supplement.is_some() && self.use_supplement_for(key))
            {
                return true;
            }
            match &self.body {
                FamilyBody::Audio(_) => {
                    super::audio_products::supports_page(self.device.product_id, key)
                }
                FamilyBody::AccessorySystem(_) => {
                    super::accessory_system_products::supports_page(self.device.product_id, key)
                }
                FamilyBody::Controls(_) => {
                    super::source_controls::supports_page(self.device.product_id, key)
                }
                FamilyBody::Pending => false,
                FamilyBody::Mouse(_)
                | FamilyBody::Keyboard(_)
                | FamilyBody::Gamepad(_)
                | FamilyBody::System(_)
                | FamilyBody::Hue(_) => true,
            }
        })
    }

    pub(super) fn select_profile(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.device.profiles.iter().any(|profile| profile.id == id)
            || self.device.active_profile == id
        {
            return;
        }
        self.dismiss_profile_dialog(window, cx);
        self.device.active_profile = id.to_owned();
        let factory_default_profile = self
            .device
            .profiles
            .iter()
            .find(|profile| profile.id == self.device.active_profile)
            .is_some_and(|profile| {
                super::keyboard_products::is_factory_profile(self.device.product_id, &profile.guid)
            });
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |keyboard, cx| {
                keyboard.set_factory_default_profile(factory_default_profile, window, cx)
            });
        }
        let selected = self.device.active_profile.clone();
        self.profile.update(cx, |state, cx| {
            state.set_selected_value(&selected, window, cx)
        });
        self.restore_active(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }

    pub(super) fn change_profile_metadata(
        &mut self,
        id: &str,
        change: super::product_workspace::ProfileMetadata,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if super::product_workspace::edit_profile_metadata(&mut self.device, id, change) {
            self.profile.update(cx, |state, cx| {
                state.set_items(
                    self.device
                        .profiles
                        .iter()
                        .map(|p| Choice::new(&p.id, &p.name))
                        .collect(),
                    window,
                    cx,
                )
            });
            cx.emit(WorkspaceEvent::Changed);
            cx.notify();
        }
    }
    pub(crate) fn keyboard_preview_page(
        &self,
    ) -> Option<Entity<crate::features::keyboard_products::KeyboardProductWorkspace>> {
        match &self.body {
            FamilyBody::Keyboard(view) => Some(view.clone()),
            _ => None,
        }
    }
    pub(crate) fn aether_preview_page(
        &self,
    ) -> Option<Entity<crate::features::aether_strip::AetherStrip>> {
        match &self.accessory {
            Some(accessory::AccessoryPage::Aether(view)) => Some(view.clone()),
            _ => None,
        }
    }
    /// Whether this source product's family renderer can produce its lighting
    /// page. Audio products additionally need an audited root-to-page match:
    /// Hammerhead V3 Chroma calls that page `LIGHTING`, not `TAB_LIGHTING`.
    pub(crate) fn supports_lighting_page(&self) -> bool {
        if matches!(&self.body, FamilyBody::Audio(_)) {
            return super::audio_products::supports_chroma_lighting_page(self.device.product_id);
        }
        if !matches!(
            &self.body,
            FamilyBody::Mouse(_)
                | FamilyBody::Keyboard(_)
                | FamilyBody::Gamepad(_)
                | FamilyBody::System(_)
        ) {
            return false;
        }
        product::registered(self.device.product_id)
            .and_then(|product| product.primary_navigation())
            .is_some_and(|navigation| {
                navigation
                    .pages()
                    .iter()
                    .any(|page| page.kind().key() == "TAB_LIGHTING")
            })
    }
    /// The `displayMode=chromaApp` popup content for a source product: its
    /// lighting page built from the family renderer, without the product chrome.
    pub(crate) fn lighting_page_element(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.supports_lighting_page() {
            return None;
        }
        match &self.body {
            FamilyBody::Mouse(view) => {
                Some(view.update(cx, |view, cx| view.lighting_element(window, cx)))
            }
            FamilyBody::Keyboard(view) => {
                Some(view.update(cx, |view, cx| view.lighting_element(window, cx)))
            }
            FamilyBody::Gamepad(view) => {
                Some(view.update(cx, |view, cx| view.lighting_element(window, cx)))
            }
            FamilyBody::System(view) => Some(view.update(cx, |view, cx| view.lighting_element(cx))),
            FamilyBody::Audio(view) => {
                view.update(cx, |view, cx| view.lighting_element(window, cx))
            }
            _ => None,
        }
    }
    /// Whether the source product has an implemented Armory root. Some roots
    /// mount a product image or CoolerInfo instead of the Customize page.
    pub(crate) fn supports_mapping_page(&self) -> bool {
        if super::armory_product::supports(&self.device) {
            return true;
        }
        if !matches!(
            &self.body,
            FamilyBody::Mouse(_)
                | FamilyBody::Keyboard(_)
                | FamilyBody::Gamepad(_)
                | FamilyBody::System(_)
        ) {
            return false;
        }
        product::registered(self.device.product_id)
            .and_then(|product| product.primary_navigation())
            .is_some_and(|navigation| {
                navigation
                    .pages()
                    .iter()
                    .any(|page| page.kind().key() == "TAB_CUSTOMIZE")
            })
    }
    /// Select the audited independent Armory component before mapping roots.
    pub(crate) fn mapping_page_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if super::armory_product::supports(&self.device) {
            return Some(
                super::armory_product::ArmoryProduct::new(&self.device).into_any_element(),
            );
        }
        if !self.supports_mapping_page() {
            return None;
        }
        match &self.body {
            FamilyBody::Mouse(view) => Some(view.update(cx, |view, cx| view.customize_element(cx))),
            FamilyBody::Keyboard(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(cx)))
            }
            FamilyBody::Gamepad(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(cx)))
            }
            FamilyBody::System(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(cx)))
            }
            _ => None,
        }
    }
    pub(crate) fn new(mut device: Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if device.profiles.is_empty() {
            device.profiles.push(Profile {
                id: "local-default".into(),
                guid: "local-default".into(),
                name: "Default".into(),
                dpi_stages: None,
                settings: None,
                source_settings: None,
            });
        }
        if !device
            .profiles
            .iter()
            .any(|p| p.id == device.active_profile)
        {
            device.active_profile = device.profiles[0].id.clone();
        }
        let registered = product::registered(device.product_id);
        Self::migrate_device_settings(&mut device);
        let page = registered
            .and_then(|p| p.primary_navigation())
            .and_then(|n| n.pages().iter().find(|p| p.role() != ProductPageRole::Help))
            .map(|p| p.id());
        let mut subscriptions = vec![];
        let body = if device.product_id == 769 {
            let body = cx.new(|cx| super::hue::HueWorkspace::new(window, cx));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::hue::HueChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, _: &super::hue::HueChromaRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenChroma);
                },
            ));
            FamilyBody::Hue(body)
        } else if super::mouse_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::mouse_products::MouseProductWorkspace::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::mouse_products::MouseProductChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            FamilyBody::Mouse(body)
        } else if super::keyboard_products::source_product(device.product_id).is_some() {
            let factory_default_profile = device
                .profiles
                .iter()
                .find(|profile| profile.id == device.active_profile)
                .is_some_and(|profile| {
                    super::keyboard_products::is_factory_profile(device.product_id, &profile.guid)
                });
            let body = cx.new(|cx| {
                super::keyboard_products::KeyboardProductWorkspace::new(
                    device.product_id,
                    factory_default_profile,
                    window,
                    cx,
                )
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 _: &super::keyboard_products::KeyboardProductChanged,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            FamilyBody::Keyboard(body)
        } else if super::gamepad_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::gamepad_products::GamepadProductWorkspace::new(
                    device.product_id,
                    device.layout_id,
                    window,
                    cx,
                )
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::gamepad_products::GamepadProductChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 _: &super::gamepad_products::GamepadCalibrationRequested,
                 window,
                 cx| {
                    this.set_page_key("TAB_CALIBRATION", window, cx);
                },
            ));
            FamilyBody::Gamepad(body)
        } else if super::audio_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::audio_products::AudioProductWorkspace::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, event: &super::OledRuntimeRequested, cx| cx.emit(event.clone()),
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, _: &super::audio_products::AudioStudioRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenStudio)
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::audio_products::AudioProductChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 navigation: &super::audio_products::AudioNavigation,
                 window,
                 cx| {
                    match navigation {
                        super::audio_products::AudioNavigation::Page(page) => {
                            this.set_page(*page, window, cx)
                        }
                        super::audio_products::AudioNavigation::History(forward) => {
                            this.step_page_history(*forward, window, cx)
                        }
                    }
                },
            ));
            FamilyBody::Audio(body)
        } else if super::system_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::system_products::SystemProductWorkspace::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::system_products::SystemProductChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            FamilyBody::System(body)
        } else if super::accessory_system_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::accessory_system_products::AccessorySystemProductWorkspace::new(
                    device.product_id,
                    window,
                    cx,
                )
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 _: &super::accessory_system_products::AccessorySystemProductChanged,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            FamilyBody::AccessorySystem(body)
        } else if super::source_controls::supports(device.product_id) {
            let body = cx.new(|cx| {
                super::source_controls::SourceControls::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::source_controls::SourceControlsChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 _: Entity<super::source_controls::SourceControls>,
                 _: &super::source_controls::SourceControlsPairingRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::PairingRequested(this.device.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 _: &super::source_controls::SourceControlsHelpRequested,
                 window,
                 cx| {
                    this.set_page_key("HELP", window, cx);
                },
            ));
            FamilyBody::Controls(body)
        } else {
            FamilyBody::Pending
        };
        let supplement = if !matches!(body, FamilyBody::Controls(_))
            && super::source_controls::supports(device.product_id)
        {
            let controls = cx.new(|cx| {
                super::source_controls::SourceControls::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &controls,
                |this: &mut Self,
                 controls,
                 _: &super::source_controls::SourceControlsChanged,
                 cx| {
                    this.capture_supplement(controls.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &controls,
                |this: &mut Self,
                 _: Entity<super::source_controls::SourceControls>,
                 _: &super::source_controls::SourceControlsPairingRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::PairingRequested(this.device.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &controls,
                window,
                |this: &mut Self,
                 _,
                 _: &super::source_controls::SourceControlsHelpRequested,
                 window,
                 cx| {
                    this.set_page_key("HELP", window, cx);
                },
            ));
            Some(controls)
        } else {
            None
        };
        let profile = cx.new(|cx| {
            SelectState::new(
                device
                    .profiles
                    .iter()
                    .map(|p| Choice::new(&p.id, &p.name))
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            )
        });
        profile.update(cx, |state, cx| {
            state.set_selected_value(&device.active_profile, window, cx)
        });
        subscriptions.push(cx.subscribe_in(
            &profile,
            window,
            |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.select_profile(id, window, cx);
                }
            },
        ));
        let name_limit = profile_menu::spec(device.product_id).map_or(32, |spec| spec.name_limit());
        let profile_name = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(move |value, _| value.encode_utf16().count() <= name_limit)
        });
        subscriptions.push(cx.subscribe_in(
            &profile_name,
            window,
            |this: &mut Self, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.finish_profile_rename(window, cx);
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.profile.update(cx, |state, cx| state.focus(window, cx));
                    }
                }
            },
        ));
        let owner = cx.entity().downgrade();
        let profile_menu =
            cx.new(|cx| ListState::new(profile_menu::ProfileCommands::new(owner), window, cx));
        let help = cx.new(|cx| super::source_help::SourceHelp::new(device.clone(), cx));
        let dock_pairing = matches!(device.product_id, 164 | 241)
            .then(|| cx.new(|_| super::dock_pairing::DockPairing::new(&device)));
        if let Some(dock) = &dock_pairing {
            // 源配对文案里的设备名在命中应用设备列表时可点，点击切到该设备工作区。
            subscriptions.push(cx.subscribe(
                dock,
                |_, _, event: &super::dock_pairing::DeviceLinkRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenDevice {
                        product_id: event.product_id,
                        edition_id: event.edition_id,
                    });
                },
            ));
        }
        let accessory = AccessoryPage::new(&device, window, cx, &mut subscriptions);
        let mut this = Self {
            saved: device.clone(),
            device,
            page,
            page_history: page.into_iter().collect(),
            page_history_index: 0,
            body,
            help,
            supplement,
            dock_pairing,
            accessory,
            profile,
            profile_name,
            profile_rename: None,
            profile_menu,
            profile_confirmation: None,
            profile_confirm_focus: cx.focus_handle(),
            profile_confirm_task: None,
            profile_transfer: None,
            profile_transfer_subscription: None,
            profile_linked_games: None,
            profile_linked_games_subscription: None,
            _subscriptions: subscriptions,
        };
        this.restore_active(window, cx);
        this.select_body_page(window, cx);
        this.saved = this.device.clone();
        this
    }
    pub(crate) fn device(&self) -> &Device {
        &self.device
    }
    pub(crate) fn saved_snapshot(&self) -> Device {
        self.saved.clone()
    }
    pub(crate) fn dirty(&self) -> bool {
        self.device.source_device_settings != self.saved.source_device_settings
            || self.device.active_profile != self.saved.active_profile
            || self.device.profiles.len() != self.saved.profiles.len()
            || self
                .device
                .profiles
                .iter()
                .zip(&self.saved.profiles)
                .any(|(a, b)| {
                    a.id != b.id
                        || a.name != b.name
                        || a.source_settings != b.source_settings
                        || a.settings != b.settings
                })
    }
    pub(crate) fn mark_saved(&mut self, snapshot: Device, cx: &mut Context<Self>) {
        self.saved = snapshot;
        cx.notify();
    }
    pub(crate) fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_profile_dialog(window, cx);
        self.device = self.saved.clone();
        self.refresh_profile_choices(window, cx);
        self.restore_active(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn capture(&mut self, mut value: Value, cx: &mut Context<Self>) {
        self.capture_device_settings(&mut value);
        if let Some(p) = self
            .device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
        {
            for key in ["_supplement", "_accessory"] {
                if let Some(nested) = p.source_settings.as_ref().and_then(|s| s.get(key)).cloned() {
                    value[key] = nested;
                }
            }
            p.source_settings = Some(value);
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn capture_supplement(&mut self, mut value: Value, cx: &mut Context<Self>) {
        self.capture_device_settings(&mut value);
        if let Some(p) = self
            .device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
        {
            let settings = p
                .source_settings
                .get_or_insert_with(|| serde_json::json!({}));
            settings["_supplement"] = value;
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn capture_accessory(&mut self, value: Value, cx: &mut Context<Self>) {
        if accessory::device_layout(self.device.product_id) {
            self.device
                .source_device_settings
                .get_or_insert_with(|| serde_json::json!({}))["_accessory"] = value;
        } else if let Some(profile) = self
            .device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
        {
            let settings = profile
                .source_settings
                .get_or_insert_with(|| serde_json::json!({}));
            settings["_accessory"] = value;
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn restore_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = &self.dock_pairing {
            view.update(cx, |view, cx| view.dismiss(window, cx));
        }
        if let Some(view) = &self.accessory {
            view.dismiss(window, cx);
        }
        let mut value = self
            .device
            .profiles
            .iter()
            .find(|p| p.id == self.device.active_profile)
            .and_then(|p| p.source_settings.clone());
        // Views consume the source reducer shape; persistence splits device fields
        // back out. The selected profile can never override the device mirror.
        if let Some(fields) = self
            .device
            .source_device_settings
            .as_ref()
            .and_then(Value::as_object)
        {
            let target = value.get_or_insert_with(|| serde_json::json!({}));
            let target = if self.supplement.is_some() {
                target
                    .as_object_mut()
                    .expect("source profile object")
                    .entry("_supplement")
                    .or_insert_with(|| serde_json::json!({}))
            } else {
                target
            };
            if let Some(target) = target.as_object_mut() {
                target.extend(fields.clone());
            }
        }
        let accessory = self.accessory.as_ref().map(|view| {
            let settings = if accessory::device_layout(self.device.product_id) {
                self.device.source_device_settings.as_ref()
            } else {
                value.as_ref()
            };
            view.restore(settings.and_then(|v| v.get("_accessory")), window, cx)
        });
        let snapshot = match &self.body {
            FamilyBody::Mouse(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::Keyboard(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::Gamepad(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::Controls(e) => {
                e.update(cx, |v, cx| {
                    v.set_connection(self.device.use_ble, cx);
                    v.restore(value.as_ref(), window, cx);
                });
                Some(e.read(cx).snapshot())
            }
            FamilyBody::Audio(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::System(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::AccessorySystem(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::Hue(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
                Some(e.read(cx).snapshot())
            }
            FamilyBody::Pending => None,
        };
        let mut supplement = self.supplement.as_ref().map(|controls| {
            controls.update(cx, |v, cx| {
                v.set_connection(self.device.use_ble, cx);
                v.restore(
                    value
                        .as_ref()
                        .and_then(|v| v.get("_supplement").or(Some(v))),
                    window,
                    cx,
                )
            });
            controls.read(cx).snapshot()
        });
        if let Some(value) = &mut supplement {
            self.capture_device_settings(value);
        }
        if let Some(mut snapshot) = snapshot {
            self.capture_device_settings(&mut snapshot);
            if let Some(supplement) = supplement {
                snapshot["_supplement"] = supplement;
            }
            if let Some(accessory) = accessory {
                if accessory::device_layout(self.device.product_id) {
                    self.device
                        .source_device_settings
                        .get_or_insert_with(|| serde_json::json!({}))["_accessory"] = accessory;
                    if let Some(object) = snapshot.as_object_mut() {
                        object.remove("_accessory");
                    }
                } else {
                    snapshot["_accessory"] = accessory;
                }
            }
            if let Some(p) = self
                .device
                .profiles
                .iter_mut()
                .find(|p| p.id == self.device.active_profile)
            {
                p.source_settings = Some(snapshot);
            }
        }
    }
    fn capture_device_settings(&mut self, value: &mut Value) {
        if accessory::device_layout(self.device.product_id) {
            if let Some(object) = value.as_object_mut() {
                object.remove("_accessory");
            }
        }
        for field in super::source_controls::device_fields(self.device.product_id) {
            if let Some(value) = value.as_object_mut().and_then(|v| v.remove(field)) {
                let settings = self
                    .device
                    .source_device_settings
                    .get_or_insert_with(|| serde_json::json!({}));
                settings[field] = value;
            }
        }
    }
    fn migrate_device_settings(device: &mut Device) {
        let fields = super::source_controls::device_fields(device.product_id);
        // Prefer the last active profile for legacy conflicting values. Existing
        // device settings always win; inactive profiles only fill absent fields.
        let mut order: Vec<usize> = (0..device.profiles.len()).collect();
        order.sort_by_key(|&i| device.profiles[i].id != device.active_profile);
        for i in order {
            if let Some(profile) = device.profiles[i].source_settings.as_mut() {
                if accessory::device_layout(device.product_id) {
                    if let Some(layout) =
                        profile.as_object_mut().and_then(|p| p.remove("_accessory"))
                    {
                        let settings = device
                            .source_device_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        settings
                            .as_object_mut()
                            .expect("device settings object")
                            .entry("_accessory")
                            .or_insert(layout);
                    }
                }
                for field in fields {
                    let direct = profile.as_object_mut().and_then(|p| p.remove(field));
                    let nested = profile
                        .get_mut("_supplement")
                        .and_then(Value::as_object_mut)
                        .and_then(|p| p.remove(field));
                    if let Some(value) = direct.or(nested) {
                        let settings = device
                            .source_device_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        if let Some(settings) = settings.as_object_mut() {
                            settings.entry(field.clone()).or_insert(value);
                        }
                    }
                }
            }
        }
    }
    fn current_page(&self) -> Option<&'static ProductPage> {
        product::registered(self.device.product_id)?.page(self.page?)
    }

    /// 把应用设备列表转给 164/241 配对页（源 `Es` 的比对列表）。
    pub(crate) fn set_known_devices(&mut self, devices: Vec<(u32, u32)>, cx: &mut Context<Self>) {
        if let Some(dock) = &self.dock_pairing {
            dock.update(cx, |dock, cx| dock.set_known_devices(devices, cx));
        }
    }
    pub(crate) fn set_page_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(product) = product::registered(self.device.product_id) else {
            return;
        };
        let normalized = key.to_ascii_uppercase();
        let page = product
            .primary_navigation()
            .into_iter()
            .flat_map(|n| n.pages())
            .find(|p| {
                p.id().key() == key
                    || p.kind().key() == normalized
                    || p.kind().key().strip_prefix("TAB_") == Some(normalized.as_str())
            });
        if let Some(page) = page {
            self.set_page(page.id(), window, cx);
        }
    }
    fn set_page(&mut self, page: ProductPageId, window: &mut Window, cx: &mut Context<Self>) {
        if product::registered(self.device.product_id)
            .and_then(|p| p.page(page))
            .is_none()
        {
            return;
        }
        if self.page == Some(page) {
            return;
        }
        if let FamilyBody::Audio(body) = &self.body {
            if body.update(cx, |body, cx| {
                body.defer_pod_navigation(
                    super::audio_products::AudioNavigation::Page(page),
                    window,
                    cx,
                )
            }) {
                return;
            }
        }
        self.page_history.truncate(self.page_history_index + 1);
        self.page_history.push(page);
        self.page_history_index = self.page_history.len() - 1;
        self.page = Some(page);
        self.select_body_page(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    pub(crate) fn can_step_history(&self, forward: bool) -> bool {
        if forward {
            self.page_history_index + 1 < self.page_history.len()
        } else {
            self.page_history_index > 0
        }
    }
    pub(crate) fn step_page_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_step_history(forward) {
            return;
        }
        if let FamilyBody::Audio(body) = &self.body {
            if body.update(cx, |body, cx| {
                body.defer_pod_navigation(
                    super::audio_products::AudioNavigation::History(forward),
                    window,
                    cx,
                )
            }) {
                return;
            }
        }
        if forward {
            self.page_history_index += 1;
        } else {
            self.page_history_index -= 1;
        }
        self.page = Some(self.page_history[self.page_history_index]);
        self.select_body_page(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn select_body_page(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = &self.accessory {
            view.dismiss(window, cx);
        }
        if let Some(view) = &self.dock_pairing {
            view.update(cx, |view, cx| view.dismiss(window, cx));
        }
        if let FamilyBody::Hue(body) = &self.body {
            body.update(cx, |view, cx| view.dismiss_transient(cx));
        }
        let Some(page) = self.current_page() else {
            return;
        };
        if page.role() == ProductPageRole::Help {
            match &self.body {
                FamilyBody::Mouse(body) => {
                    body.update(cx, |body, cx| body.dismiss_editors(window, cx))
                }
                FamilyBody::Keyboard(body) => {
                    body.update(cx, |body, cx| body.leave_snap_tap(window, cx))
                }
                _ => {}
            }
            self.help.update(cx, |help, cx| {
                help.set_device(&self.device, cx);
                help.set_page(page.offset(), cx);
            });
            return;
        }
        if let Some(controls) = &self.supplement {
            controls.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx));
        }
        match &self.body {
            FamilyBody::Mouse(e) => e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Keyboard(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Gamepad(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Controls(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Audio(e) => e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::System(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::AccessorySystem(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Hue(_) | FamilyBody::Pending => {}
        }
    }
    fn use_supplement_for(&self, key: &str) -> bool {
        if !super::source_controls::supports_page(self.device.product_id, key) {
            return false;
        }
        match &self.body {
            FamilyBody::Keyboard(_) => key == "OLED",
            FamilyBody::Audio(_) => {
                !super::audio_products::supports_page(self.device.product_id, key)
            }
            FamilyBody::AccessorySystem(_) => {
                !super::accessory_system_products::supports_page(self.device.product_id, key)
            }
            FamilyBody::Pending => true,
            _ => false,
        }
    }
}
impl Render for SourceProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registered = product::registered(self.device.product_id);
        let navigation = registered.and_then(|p| p.primary_navigation());
        let pages: Vec<_> = navigation
            .into_iter()
            .flat_map(|n| n.pages())
            .filter(|page| page.role() != ProductPageRole::Help)
            .collect();
        let has_help = navigation
            .into_iter()
            .flat_map(|n| n.pages())
            .any(|page| page.role() == ProductPageRole::Help);
        let available = f32::from(window.viewport_size().width) * 16.
            / f32::from(window.rem_size())
            - self.profile_bar_width()
            - surface::device_right_width(&self.device, has_help, window)
            - surface::NAV_MORE_WIDTH
            - surface::NAV_MORE_MARGIN
            - 10.;
        let (visible, hidden) = surface::split_navs(&pages, |page| page.label(), available, window);
        let overflow = if hidden.is_empty() {
            None
        } else {
            let owner = cx.entity().downgrade();
            let items = hidden
                .iter()
                .map(|page| (page.label(), self.page == Some(page.id())))
                .collect();
            Some(surface::nav_overflow(
                "source-nav-overflow",
                items,
                move |index, window, cx| {
                    let _ =
                        owner.update(cx, |this, cx| this.set_page(hidden[index].id(), window, cx));
                },
                window,
                cx,
            ))
        };
        let is_help = self
            .current_page()
            .is_some_and(|page| page.role() == ProductPageRole::Help);
        let body = if is_help {
            self.help.clone().into_any_element()
        } else if let Some(view) = self.dock_pairing.as_ref().filter(|_| {
            self.current_page().is_some_and(|page| {
                super::dock_pairing::supports_page(self.device.product_id, page.kind().key())
            })
        }) {
            view.clone().into_any_element()
        } else if let Some(view) = self.accessory.as_ref().filter(|view| {
            self.current_page()
                .is_some_and(|page| view.supports_page(self.device.product_id, page.kind().key()))
        }) {
            view.element()
        } else if let Some(controls) = self.supplement.as_ref().filter(|_| {
            self.current_page()
                .is_some_and(|page| self.use_supplement_for(page.kind().key()))
        }) {
            controls.clone().into_any_element()
        } else {
            match &self.body {
                FamilyBody::Mouse(e) => e.clone().into_any_element(),
                FamilyBody::Keyboard(e) => e.clone().into_any_element(),
                FamilyBody::Gamepad(e) => e.clone().into_any_element(),
                FamilyBody::Controls(e) => e.clone().into_any_element(),
                FamilyBody::Audio(e) => e.clone().into_any_element(),
                FamilyBody::System(e) => e.clone().into_any_element(),
                FamilyBody::AccessorySystem(e) => e.clone().into_any_element(),
                FamilyBody::Hue(e) => e.clone().into_any_element(),
                FamilyBody::Pending => {
                    surface::note("此页面的原生控件仍在接入。", cx).into_any_element()
                }
            }
        };
        v_flex()
            .size_full()
            .min_h_0()
            .child(
                h_flex()
                    .h(surface::css(48.))
                    .when(self.profile_linked_games.is_some(), |bar| bar.opacity(0.5))
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(cx.theme().title_bar)
                    .child(surface::nav_left().child(self.profile_bar(window, cx)))
                    .child(
                        gpui_kit::base::Tabs::new("source-product-navigation")
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_grow(1.)
                            .flex_shrink_0()
                            .gap(surface::css(20.))
                            .children(visible.into_iter().map(|page| {
                                let id = page.id();
                                surface::navigation_button(
                                    SharedString::from(format!("product-page-{}", id.key())),
                                    page.label(),
                                    self.page == Some(id),
                                    cx,
                                )
                                .role(Role::Tab)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| this.set_page(id, window, cx),
                                ))
                            }))
                            .children(overflow),
                    )
                    .child(
                        surface::nav_right()
                            .id("source-navigation-right")
                            .min_w_0()
                            .children(crate::ui::battery::element(&self.device, cx))
                            .children(
                                navigation
                                    .into_iter()
                                    .flat_map(|n| n.pages())
                                    .filter(|p| p.role() == ProductPageRole::Help)
                                    .map(|page| {
                                        let id = page.id();
                                        div()
                                            .id(SharedString::from(format!(
                                                "source-help-{}",
                                                id.key()
                                            )))
                                            .child(
                                                surface::asset_button(
                                                    "source-product-help",
                                                    if self.page == Some(id) {
                                                        "synapse/help-active.svg"
                                                    } else {
                                                        "synapse/help-default.svg"
                                                    },
                                                    page.label(),
                                                    cx,
                                                )
                                                .size(surface::css(24.))
                                                .mr(surface::css(10.))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.set_page(id, window, cx)
                                                    },
                                                )),
                                            )
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("source-product-content")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(body),
            )
            .children(self.profile_transfer.clone())
            .children(self.profile_linked_games.clone())
    }
}
