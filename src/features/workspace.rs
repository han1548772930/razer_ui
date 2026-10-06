//! A device instance owns profile drafts, retained controls and mapping continuations.
//! Rendering never creates SliderState/SelectState entities or performs device I/O.
use super::{controls::*, settings::*};
use crate::{
    model::{Device, Profile},
    nav::Tab,
    ui::scroll::SourceScrollable as _,
    ui::source_alert::{AlertAction, AlertPlacement, SourceAlert},
    ui::surface,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    color_picker::{ColorPickerEvent, ColorPickerState},
    input::{InputEvent, InputState},
    select::{SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::collections::BTreeMap;

pub enum WorkspaceEvent {
    Changed,
    IntroDismissed,
    ShareProfile,
    /// Navigation request for the locally implemented Chroma app window.
    /// This does not assert external Chroma installation or service state.
    OpenChroma,
    PairingRequested(crate::model::Device),
    /// 164/241 配对页：源 `Es(peer, devices)` 命中应用设备列表时，配对文案里的设备名
    /// 渲染成 `.deviceNameLink` 可点链接，点击切到该设备的工作区（`z(e)`）。
    OpenDevice {
        product_id: u32,
        edition_id: u32,
    },
}
pub(super) struct MappingDraft {
    pub(super) input: String,
    pub(super) value: String,
    pub(super) original: String,
    pub(super) dial_mode: Option<String>,
}
#[derive(Clone)]
pub(super) enum Continue {
    NewProfile,
    DuplicateProfile,
    ImportProfile(Profile),
    ExportProfile,
    DeleteProfile(String),
    ResetProfile { id: String, bindings_only: bool },
    Page(Tab),
    HistoryPage { page: Tab, index: usize },
    Profile(String),
    Input(String),
    DrawerInput(String),
    DialInput { mode_uid: String, input: String },
    SnapCapture { pair: usize, key: usize },
    AddSnapPair,
    DeleteDial(String),
    ResetDial,
    Drawer(bool),
    Layer(bool),
    CloseMapping,
}

pub struct DeviceWorkspace {
    device: Device,
    saved: Device,
    pub(super) page: Tab,
    /// 原版设备页自己维护导航历史：`sessionStorage` 里的 `tabNavigations` 与
    /// `currentTabNavigation`（见 `addNavigationToStorage` / `navigateBack` /
    /// `navigateForward`），标签栏最左边就是 `.nav.back` / `.nav.forward`。
    page_history: Vec<Tab>,
    page_history_index: usize,
    /// 由历史前进/后退触发的页切换不再重复写入历史。
    pub(super) controls: Controls,
    pub(super) sensitivity_controls: super::sensitivity::SensitivityControls,
    pub(super) keyboard_controls: super::keyboard_controls::KeyboardControls,
    subscriptions: Vec<Subscription>,
    body_scroll: ScrollHandle,
    pub(super) customize_drawer: super::customize_drawer::CustomizeDrawer,
    pub(super) hypershift: bool,
    pub(super) mapping: Option<MappingDraft>,
    mapping_expanded: bool,
    mapping_key_group: String,
    mapping_recording: bool,
    mapping_modifiers_enabled: bool,
    mapping_optional_modifiers: Vec<String>,
    mapping_focus: FocusHandle,
    mapping_return_focus: Option<FocusHandle>,
    workspace_focus: FocusHandle,
    pub(super) intro_seen: bool,
    pub(super) dial_highlight: Option<String>,
    pub(super) hovered_input: Option<String>,
    /// 源 691 命令拨盘帮助：`<i className="help" onMouseEnter/onMouseLeave/>` 直接翻转
    /// `isMounted`，没有展示延迟，因此只需一个本地开关。
    pub(super) dial_help_hovered: bool,
    /// 源 691 拨盘的 `icon-add` 用 `tooltip={getTextItem(c.BYV)}`（`[tooltip]:before` 伪元素）。
    pub(super) dial_add_hovered: bool,
    profile_name: Entity<InputState>,
    profile_rename: Option<String>,
    profile_menu: Entity<gpui_kit::component::list::ListState<profile::ProfileCommands>>,
    profile_confirmation: Option<profile::ProfileConfirmation>,
    profile_confirm_focus: FocusHandle,
    source_alert: Option<Entity<SourceAlert>>,
    profile_dialog: Option<Entity<profile::ProfileDialog>>,
    /// 关联游戏弹层是否打开：源码在 `showLinkedGames` 时给整行加
    /// `div.nav-tabs.disabled{opacity:.5}`。
    linked_games_open: bool,
    pub(super) help: super::help_page::HelpState,
}
impl EventEmitter<WorkspaceEvent> for DeviceWorkspace {}
#[path = "mapping_editor.rs"]
mod mapping_editor;
pub(super) use mapping_editor::{MEDIA, WINDOWS, canonical_key, key_label, normalized_website};
#[cfg(test)]
#[path = "mapping_focus_tests.rs"]
mod mapping_focus_tests;
#[cfg(test)]
#[path = "mouse_mat_tests.rs"]
mod mouse_mat_tests;
#[path = "profile.rs"]
mod profile;
#[cfg(test)]
#[path = "sensitivity_tests.rs"]
mod sensitivity_tests;
#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
impl DeviceWorkspace {
    pub(super) fn change_profile_metadata(
        &mut self,
        id: &str,
        change: super::product_workspace::ProfileMetadata,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if super::product_workspace::edit_profile_metadata(&mut self.device, id, change) {
            self.refresh_profile_choices(window, cx);
            self.changed(cx);
            cx.notify();
        }
    }
    pub fn new(
        mut device: Device,
        intro_seen: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if device.profiles.is_empty() {
            device.profiles.push(Profile {
                source_settings: None,
                id: "local-default".into(),
                guid: "local-default".into(),
                name: "Default".into(),
                dpi_stages: None,
                settings: None,
            });
        }
        if !device
            .profiles
            .iter()
            .any(|p| p.id == device.active_profile)
        {
            device.active_profile = device.profiles[0].id.clone();
        }
        for index in 0..device.profiles.len() {
            let mut settings = device.profiles[index]
                .settings
                .clone()
                .unwrap_or_else(|| ProfileSettings::from_legacy(&device, &device.profiles[index]));
            settings.normalize(device.product_id);
            device.profiles[index].settings = Some(settings);
        }
        let choices = device
            .profiles
            .iter()
            .map(|p| Choice::new(&p.id, p.name.clone()))
            .collect::<Vec<_>>();
        let selected = device
            .profiles
            .iter()
            .position(|p| p.id == device.active_profile)
            .map(IndexPath::new);
        let profile = cx.new(|cx| SelectState::new(choices, selected, window, cx));
        let profile_name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("配置文件名称")
                .validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let profile_menu =
            profile::command_list(device.product_id, cx.entity().downgrade(), window, cx);
        let customize_drawer = super::customize_drawer::CustomizeDrawer::new(&device, window, cx);
        let effects = Effect::list(Self::product(&device), device.use_ble, false);
        let effect = cx.new(|cx| {
            SelectState::new(
                effects
                    .iter()
                    .map(|e| Choice::new(e.id(), e.label()))
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            )
        });
        let mapping = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let mapping_text =
            cx.new(|cx| InputState::new(window, cx).placeholder("程序路径或网站地址"));
        let mapping_paragraph =
            cx.new(|cx| gpui_kit::component::input::TextareaState::new(window, cx));
        let mapping_group =
            cx.new(|cx| SelectState::new(mapping_editor::key_groups(), None, window, cx));
        let mapping_x = cx.new(|cx| InputState::new(window, cx).placeholder("X DPI"));
        let mapping_y = cx.new(|cx| InputState::new(window, cx).placeholder("Y DPI"));
        let mapping_rate = cx.new(|cx| InputState::new(window, cx).placeholder("1–20"));
        let mapping_sliders = [
            cx.new(|_| {
                SliderState::new()
                    .min(100.)
                    .max(30000.)
                    .step(50.)
                    .default_value(800.)
            }),
            cx.new(|_| {
                SliderState::new()
                    .min(100.)
                    .max(30000.)
                    .step(50.)
                    .default_value(800.)
            }),
            cx.new(|_| {
                SliderState::new()
                    .min(1.)
                    .max(20.)
                    .step(1.)
                    .default_value(7.)
            }),
        ];
        let snap_left = cx.new(|cx| InputState::new(window, cx).placeholder("左键 ID"));
        let snap_right = cx.new(|cx| InputState::new(window, cx).placeholder("右键 ID"));
        let color_boost = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|text, _| super::lighting_input::valid_color_boost_draft(text))
        });
        let color = cx.new(|cx| ColorPickerState::new(window, cx));
        let color2 = cx.new(|cx| ColorPickerState::new(window, cx));
        let page = Tab::for_product(device.product_id)
            .first()
            .copied()
            .unwrap_or(Tab::Home);
        let sensitivity_controls = super::sensitivity::SensitivityControls::new(window, cx);
        let keyboard_controls = super::keyboard_controls::KeyboardControls::new(window, cx);
        let mut this = Self {
            saved: device.clone(),
            device,
            page,
            page_history: vec![page],
            page_history_index: 0,
            controls: Controls {
                sliders: BTreeMap::new(),
                profile,
                effect,
                mapping,
                mapping_text,
                mapping_paragraph,
                mapping_group,
                mapping_x,
                mapping_y,
                mapping_rate,
                mapping_sliders,
                snap_left,
                snap_right,
                color_boost,
                color,
                color2,
            },
            sensitivity_controls,
            keyboard_controls,
            subscriptions: vec![],
            body_scroll: ScrollHandle::default(),
            customize_drawer,
            hypershift: false,
            mapping: None,
            mapping_expanded: false,
            mapping_key_group: "record".into(),
            mapping_recording: false,
            mapping_modifiers_enabled: false,
            mapping_optional_modifiers: vec![],
            mapping_focus: cx.focus_handle().tab_stop(true),
            mapping_return_focus: None,
            workspace_focus: cx.focus_handle(),
            intro_seen,
            dial_highlight: None,
            hovered_input: None,
            dial_help_hovered: false,
            dial_add_hovered: false,
            profile_name,
            profile_rename: None,
            profile_menu,
            profile_confirmation: None,
            profile_confirm_focus: cx.focus_handle().tab_stop(true),
            source_alert: None,
            profile_dialog: None,
            linked_games_open: false,
            help: super::help_page::HelpState::default(),
        };
        this.install_profile_controls(window, cx);
        this.install_mapping_controls(window, cx);
        this.install_controls(window, cx);
        this.install_keyboard_controls(window, cx);
        this.sync_controls(window, cx);
        this
    }
    fn product(device: &Device) -> u32 {
        if device.product_id == crate::demo::DEMO_PRODUCT_ID {
            653
        } else {
            device.product_id
        }
    }
    pub(super) fn pid(&self) -> u32 {
        Self::product(&self.device)
    }
    pub fn device(&self) -> &Device {
        &self.device
    }
    /// 测试用快照/脏标记/保存前处理：只有 `#[path]` 引入的测试模块调用，
    /// 因此只在测试目标里编译，避免发布二进制里留下未使用的 API。
    #[cfg(test)]
    pub fn snapshot(&self) -> Device {
        self.device.clone()
    }
    pub fn saved_snapshot(&self) -> Device {
        self.saved.clone()
    }
    #[cfg(test)]
    pub fn dirty(&self) -> bool {
        self.committed_pending() || self.mapping_dirty()
    }
    #[cfg(test)]
    pub fn prepare_save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.finish_mapping(window, cx)
    }
    pub fn set_intro_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.intro_seen = seen;
        cx.notify();
    }
    pub fn identity(&self) -> String {
        format!(
            "{}:{}:{}",
            self.device.product_id, self.device.serial_number, self.device.device_container_id
        )
    }
    pub(crate) fn committed_pending(&self) -> bool {
        self.device.active_profile != self.saved.active_profile
            || self.device.profiles.len() != self.saved.profiles.len()
            || self
                .device
                .profiles
                .iter()
                .zip(&self.saved.profiles)
                .any(|(a, b)| a.id != b.id || a.name != b.name || a.settings != b.settings)
    }
    pub fn mark_saved(&mut self, snapshot: Device, cx: &mut Context<Self>) {
        self.saved = snapshot;
        cx.notify();
    }
    fn saved_mapping_value(&self) -> Option<String> {
        let draft = self.mapping.as_ref()?;
        if self.device.active_profile != self.saved.active_profile {
            return None;
        }
        let settings = self
            .saved
            .profiles
            .iter()
            .find(|profile| profile.id == self.saved.active_profile)?
            .settings
            .as_ref()?;
        if let Some(mode_uid) = &draft.dial_mode {
            let mode = settings
                .keyboard
                .dial_modes
                .iter()
                .find(|mode| mode.uid == *mode_uid && mode.is_custom)?;
            Some(
                mode.mappings
                    .get(&draft.input)
                    .cloned()
                    .unwrap_or_else(|| "disable".into()),
            )
        } else {
            let bindings = if self.hypershift {
                &settings.hypershift_bindings
            } else {
                &settings.bindings
            };
            Some(
                bindings
                    .get(&draft.input)
                    .cloned()
                    .unwrap_or_else(|| "default".into()),
            )
        }
    }
    pub(crate) fn discard_would_remove_mapping(&self) -> bool {
        self.mapping_dirty() && self.saved_mapping_value().is_none()
    }
    pub(crate) fn discard_committed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.discard_would_remove_mapping() {
            return false;
        }
        let Some(original) = self.saved_mapping_value() else {
            self.discard(window, cx);
            return true;
        };
        // The raw editor belongs to the same restored profile/input. Preserve
        // its text, recording and focus, updating only its comparison baseline.
        self.device = self.saved.clone();
        if let Some(draft) = &mut self.mapping {
            draft.original = original;
        }
        let items = self
            .device
            .profiles
            .iter()
            .map(|profile| Choice::new(&profile.id, profile.name.clone()))
            .collect();
        self.controls
            .profile
            .update(cx, |state, cx| state.set_items(items, window, cx));
        self.sync_controls(window, cx);
        self.changed(cx);
        true
    }
    pub fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_snap_capture(window, cx);
        let mapping_focused = self.mapping_focus.contains_focused(window, cx);
        self.device = self.saved.clone();
        self.mapping = None;
        self.mapping_recording = false;
        if mapping_focused {
            self.restore_mapping_focus(window, cx);
        } else {
            self.mapping_return_focus = None;
        }
        self.customize_drawer.source_input = None;
        self.profile_rename = None;
        let items = self
            .device
            .profiles
            .iter()
            .map(|p| Choice::new(&p.id, p.name.clone()))
            .collect();
        self.controls
            .profile
            .update(cx, |s, cx| s.set_items(items, window, cx));
        self.sync_controls(window, cx);
        self.changed(cx);
    }
    pub fn set_page(&mut self, page: Tab, window: &mut Window, cx: &mut Context<Self>) {
        if Tab::for_product(self.device.product_id).contains(&page)
            || (page == Tab::Help && !Tab::for_product(self.device.product_id).is_empty())
        {
            self.continue_with(Continue::Page(page), window, cx);
        }
    }
    /// 记录一次页切换：截断「前进」分支后追加，与原版
    /// `updateNavigationView` + `addNavigationToStorage` 的语义一致。
    fn record_page(&mut self, page: Tab) {
        if self.page_history.get(self.page_history_index) == Some(&page) {
            return;
        }
        self.page_history.truncate(self.page_history_index + 1);
        self.page_history.push(page);
        self.page_history_index = self.page_history.len() - 1;
    }
    pub(crate) fn can_step_history(&self, forward: bool) -> bool {
        if forward {
            self.page_history_index + 1 < self.page_history.len()
        } else {
            self.page_history_index > 0
        }
    }
    /// Source toolbar `.arrow.back/.forward`: replay this product's tab history.
    pub(crate) fn step_page_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_step_history(forward) {
            return;
        }
        let index = if forward {
            self.page_history_index + 1
        } else {
            self.page_history_index - 1
        };
        let target = self.page_history[index];
        self.continue_with(
            Continue::HistoryPage {
                page: target,
                index,
            },
            window,
            cx,
        );
    }

    pub(super) fn settings(&self) -> &ProfileSettings {
        self.device
            .profiles
            .iter()
            .find(|p| p.id == self.device.active_profile)
            .and_then(|p| p.settings.as_ref())
            .expect("normalized profile")
    }
    fn settings_mut(&mut self) -> &mut ProfileSettings {
        self.device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
            .and_then(|p| p.settings.as_mut())
            .expect("normalized profile")
    }
    pub(super) fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut ProfileSettings),
    ) {
        f(self.settings_mut());
        self.sync_controls(window, cx);
        self.changed(cx);
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    pub(super) fn slider(
        &self,
        key: Control,
        label: impl Into<SharedString>,
        disabled: bool,
        cx: &App,
    ) -> AnyElement {
        let state = &self.controls.sliders[&key];
        surface::note(label, cx)
            .id(SharedString::from(format!("control-{key:?}")))
            .child(
                v_flex()
                    .gap_2()
                    .mt_2()
                    .child(
                        div()
                            .text_color(cx.theme().foreground)
                            .child(format!("{}", state.read(cx).value().start())),
                    )
                    .child(Slider::new(state).disabled(disabled)),
            )
            .into_any_element()
    }
    pub(super) fn poll_rates(&self) -> &'static [u32] {
        // Higher mouse rates require a runtime dongle + firmware capability report.
        // No runtime transport exists yet; a PID 179 snapshot is not such a report.
        if self.pid() == 653 {
            &[125, 250, 500, 1000, 2000, 4000, 8000]
        } else {
            &[125, 500, 1000]
        }
    }
    fn install_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut sliders = vec![
            (Control::Volume, 0., 100., 1.),
            (Control::Tracking, 1., 3., 1.),
            (Control::Lift, 2., 26., 1.),
            (Control::Landing, 1., 25., 1.),
            (
                Control::Idle,
                if self.pid() == 777 { 5. } else { 1. },
                if self.pid() == 777 { 60. } else { 15. },
                1.,
            ),
            (Control::LowPower, 5., 100., 5.),
            (Control::Brightness, 0., 100., 1.),
            (Control::LightingIdle, 1., 15., 1.),
            (Control::EffectDuration, 1., 3., 1.),
        ];
        for b in &self.settings().audio.bands {
            sliders.push((Control::Audio(b.frequency), -5., 5., 1.));
        }
        for b in &self.settings().mic.bands {
            sliders.push((Control::Mic(b.frequency), -5., 5., 1.));
        }
        for (key, min, max, step) in sliders {
            self.install_slider(key, min, max, step, window, cx);
        }
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.profile,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.continue_with(Continue::Profile(id.clone()), window, cx);
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.effect,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if let Some(effect) = Effect::list(this.pid(), this.device.use_ble, false)
                        .iter()
                        .find(|e| e.id() == id)
                        .copied()
                    {
                        this.edit(window, cx, |s| s.lighting.effect = effect);
                    }
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.color_boost,
            window,
            |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.commit_color_boost(window, cx);
                }
            },
        ));
        for (input, index) in [
            (&self.controls.snap_left, 0usize),
            (&self.controls.snap_right, 1usize),
        ] {
            self.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        let value = input.read(cx).value().to_string();
                        this.edit(window, cx, |s| s.keyboard.set_snap_key(index, &value));
                        this.sync_controls(window, cx);
                    }
                },
            ));
        }
        for (index, picker) in [&self.controls.color, &self.controls.color2]
            .into_iter()
            .enumerate()
        {
            self.subscriptions.push(cx.subscribe_in(
                picker,
                window,
                move |this, _, event, window, cx| {
                    let ColorPickerEvent::Change(color) = event;
                    let color = color.map(|color| {
                        let rgb = gpui_kit::Rgba::from(color);
                        [
                            (rgb.r * 255.).round() as u8,
                            (rgb.g * 255.).round() as u8,
                            (rgb.b * 255.).round() as u8,
                        ]
                    });
                    this.edit(window, cx, |s| {
                        let params = s.lighting.params_mut();
                        if index == 0 {
                            params.color1 = color;
                        } else {
                            params.color2 = color;
                        }
                    });
                },
            ));
        }
    }
    fn install_slider(
        &mut self,
        key: Control,
        min: f32,
        max: f32,
        step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.controls.sliders.contains_key(&key) {
            return;
        }
        let state = cx.new(|_| {
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(min)
        });
        let target = key.clone();
        self.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |this, _, event, window, cx| {
                if let SliderEvent::Change(value) = event {
                    let value = value.start();
                    this.edit(window, cx, |s| match target {
                        Control::Volume => s.volume = value.round() as u8,
                        Control::Tracking => s.tracking.tracking = value.round() as u8,
                        Control::Lift => s.tracking.set_lift(value.round() as u8),
                        Control::Landing => s.tracking.set_landing(value.round() as u8),
                        Control::Idle => s.idle_minutes = value.round() as u8,
                        Control::LowPower => s.low_power = value.round() as u8,
                        Control::Brightness => {
                            s.lighting.brightness = value.round() as u8;
                            s.lighting.enabled = value > 0.;
                        }
                        Control::LightingIdle => s.lighting.idle_minutes = value.round() as u8,
                        Control::EffectDuration => {
                            s.lighting.params_mut().duration = value.round() as u8
                        }
                        Control::Audio(f) => s.audio.edit(f, value.round() as i8),
                        Control::Mic(f) => s.mic.edit(f, value.round() as i8),
                    });
                }
            },
        ));
        self.controls.sliders.insert(key, state);
    }
    pub(super) fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.settings().clone();
        self.sensitivity_controls.sync(&s.sensitivity, window, cx);
        let mut values = vec![
            (Control::Volume, s.volume as f32),
            (Control::Tracking, s.tracking.tracking as f32),
            (Control::Lift, s.tracking.lift as f32),
            (Control::Landing, s.tracking.landing as f32),
            (Control::Idle, s.idle_minutes as f32),
            (Control::LowPower, s.low_power as f32),
            (Control::Brightness, s.lighting.brightness as f32),
            (Control::LightingIdle, s.lighting.idle_minutes as f32),
        ];
        for b in &s.audio.bands {
            values.push((Control::Audio(b.frequency), b.decibel as f32));
        }
        for b in &s.mic.bands {
            values.push((Control::Mic(b.frequency), b.decibel as f32));
        }
        let params = s.lighting.params();
        values.push((Control::EffectDuration, params.duration as f32));
        for (key, value) in values {
            self.install_slider(key.clone(), -5., 5., 1., window, cx);
            let state = &self.controls.sliders[&key];
            if state.read(cx).value().start() != value {
                state.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        self.controls.profile.update(cx, |state, cx| {
            state.set_selected_value(&self.device.active_profile, window, cx)
        });
        self.controls.effect.update(cx, |state, cx| {
            state.set_selected_value(&s.lighting.effect.id().to_string(), window, cx)
        });
        for (input, value) in [
            (&self.controls.snap_left, s.keyboard.snap_keys[0].clone()),
            (&self.controls.snap_right, s.keyboard.snap_keys[1].clone()),
            (&self.controls.color_boost, params.color_boost.to_string()),
        ] {
            if input.read(cx).value().to_string() != value {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        for (picker, color) in [
            (&self.controls.color, params.color1),
            (&self.controls.color2, params.color2),
        ] {
            picker.update(cx, |state, cx| match color {
                Some(color) => state.set_value(
                    rgb((color[0] as u32) << 16 | (color[1] as u32) << 8 | color[2] as u32),
                    window,
                    cx,
                ),
                None => state.clear_value(window, cx),
            });
        }
        self.sync_keyboard_controls(window, cx);
    }
    pub fn add_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.continue_with(Continue::NewProfile, window, cx);
    }
    fn duplicate_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        profile::create_local_profile(&mut self.device, &self.saved, true);
        self.refresh_profile_choices(window, cx);
    }
    pub(super) fn mapping_dirty(&self) -> bool {
        self.mapping.as_ref().is_some_and(|m| m.value != m.original)
    }
    pub(super) fn commit_mapping(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.mapping_valid() {
            return false;
        }
        if let Some(draft) = self.mapping.take() {
            let layer = self.hypershift;
            let settings = self.settings_mut();
            let bindings = if let Some(uid) = &draft.dial_mode {
                &mut settings
                    .keyboard
                    .dial_modes
                    .iter_mut()
                    .find(|mode| &mode.uid == uid && mode.is_custom)
                    .expect("validated custom dial mapping")
                    .mappings
            } else if layer {
                &mut settings.hypershift_bindings
            } else {
                &mut settings.bindings
            };
            bindings.insert(draft.input, draft.value);
            self.changed(cx);
        }
        true
    }
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_controls(window, cx);
        cx.notify();
    }
    fn restore_mapping_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A Default assignment disappears immediately from the Customized
        // list. Check the committed rows, not only last frame's focus tree.
        let removed_drawer_input = self
            .customize_drawer
            .source_input
            .as_ref()
            .is_some_and(|id| !self.drawer_input_visible(id, cx));
        let previous = self.mapping_return_focus.take();
        let target = if removed_drawer_input {
            self.customize_drawer.filter_focus(cx)
        } else {
            previous
                .filter(|focus| {
                    self.workspace_focus.contains(focus, window)
                        && !self.mapping_focus.contains(focus, window)
                })
                .unwrap_or_else(|| self.workspace_focus.clone())
        };
        window.focus(&target, cx);
    }
    pub(super) fn finish_mapping(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // Saving all devices must not focus an editor in an inactive workspace.
        let restore_focus =
            self.mapping.is_some() && self.workspace_focus.contains_focused(window, cx);
        if !self.commit_mapping(cx) {
            return false;
        }
        self.mapping_recording = false;
        if restore_focus {
            self.restore_mapping_focus(window, cx);
        } else {
            self.mapping_return_focus = None;
        }
        self.customize_drawer.source_input = None;
        true
    }
    pub(super) fn continue_with(
        &mut self,
        next: Continue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Re-activating the current target must not discard a mapping draft
        // or ask for a decision when no navigation will take place.
        let unchanged = match &next {
            Continue::Page(page) | Continue::HistoryPage { page, .. } => self.page == *page,
            Continue::Profile(id) => self.device.active_profile == *id,
            Continue::Layer(layer) => self.hypershift == *layer,
            Continue::Drawer(open) => self.customize_drawer.open == *open,
            Continue::Input(input) | Continue::DrawerInput(input) => self
                .mapping
                .as_ref()
                .is_some_and(|mapping| mapping.input == *input && mapping.dial_mode.is_none()),
            Continue::DialInput { mode_uid, input } => {
                self.mapping.as_ref().is_some_and(|mapping| {
                    mapping.input == *input && mapping.dial_mode.as_ref() == Some(mode_uid)
                })
            }
            _ => false,
        };
        if unchanged {
            return;
        }
        self.finish_profile_rename(window, cx);
        if self.mapping_dirty() {
            let save = cx.entity().downgrade();
            let discard = save.clone();
            let save_next = next.clone();
            let discard_next = next.clone();
            self.source_alert = Some(SourceAlert::open(
                crate::i18n::t("SAVE_REMAPPED_BUTTON_HEADER"),
                format!(
                    "{}\n\n{}",
                    crate::i18n::t("SAVE_REMAPPED_BUTTON_MSG1"),
                    crate::i18n::t("SAVE_REMAPPED_BUTTON_MSG2")
                ),
                "mapping-keep-editing",
                vec![
                    AlertAction::new(
                        "mapping-discard",
                        crate::i18n::t("DONT_SAVE"),
                        move |window, cx| {
                            let _ = discard.update(cx, |this, cx| {
                                this.mapping = None;
                                this.apply_continue(discard_next.clone(), window, cx);
                            });
                        },
                    ),
                    AlertAction::new("mapping-save", crate::i18n::t("SAVE"), move |window, cx| {
                        let _ = save.update(cx, |this, cx| {
                            if this.commit_mapping(cx) {
                                this.apply_continue(save_next.clone(), window, cx);
                            }
                        });
                    })
                    .primary()
                    .disabled(!self.mapping_valid()),
                ],
                if self.pid() == 653 {
                    AlertPlacement::UpperCenter
                } else {
                    AlertPlacement::AboveCenter
                },
                window,
                cx,
            ));
            cx.notify();
            // Restore trigger while the continuation is awaiting a decision.
            self.controls.profile.update(cx, |s, cx| {
                s.set_selected_value(&self.device.active_profile, window, cx)
            });
        } else {
            self.apply_continue(next, window, cx);
        }
    }
    fn apply_continue(&mut self, next: Continue, window: &mut Window, cx: &mut Context<Self>) {
        let editor_focused = self.mapping_focus.contains_focused(window, cx);
        let closing_drawer_mapping =
            matches!(next, Continue::Drawer(false)) && self.customize_drawer.source_input.is_some();
        let closing_mapping = matches!(next, Continue::CloseMapping);
        let starting_snap = matches!(next, Continue::SnapCapture { .. } | Continue::AddSnapPair);
        // Capture only accepted navigation. A rejected dirty-draft switch must
        // retain the original return target, including through the dialog.
        if matches!(
            next,
            Continue::Input(_) | Continue::DrawerInput(_) | Continue::DialInput { .. }
        ) && !editor_focused
        {
            self.mapping_return_focus = window.focused(cx);
        }
        if !matches!(next, Continue::Drawer(_)) {
            self.cancel_snap_capture(window, cx);
            self.mapping = None;
            self.hovered_input = None;
            if !closing_mapping {
                self.customize_drawer.source_input = None;
            }
        }
        match next {
            Continue::NewProfile => {
                profile::create_local_profile(&mut self.device, &self.saved, false);
                self.refresh_profile_choices(window, cx);
            }
            Continue::DuplicateProfile => self.duplicate_profile(window, cx),
            Continue::ImportProfile(profile) => self.import_local_profile(profile, window, cx),
            Continue::ExportProfile => self.export_local_profile(window, cx),
            Continue::DeleteProfile(id) => {
                profile::delete_local_profile(&mut self.device, &id);
                self.refresh_profile_choices(window, cx);
            }
            Continue::ResetProfile { id, bindings_only } => {
                profile::reset_local_profile(&mut self.device, &id, bindings_only);
            }
            Continue::Page(page) => {
                self.page = page;
                self.record_page(page);
            }
            Continue::HistoryPage { page, index } => {
                self.page = page;
                // Apply only after the unsaved mapping decision is accepted.
                self.page_history_index = index;
            }
            Continue::Profile(id) => {
                if self.device.profiles.iter().any(|p| p.id == id) {
                    self.device.active_profile = id;
                }
            }
            Continue::Layer(layer) => {
                self.hypershift = layer;
                self.customize_drawer.reset_scroll();
                // KP closes the mouse drawer on layer change; keyboard km
                // deliberately keeps its input list available.
                if self.pid() == 182 {
                    self.apply_drawer_toggle(false);
                }
            }
            Continue::Drawer(open) => self.apply_drawer_toggle(open),
            Continue::DrawerInput(input) => {
                self.customize_drawer.source_input = Some(input.clone());
                self.open_mapping(input, window, cx);
            }
            Continue::CloseMapping => {
                self.mapping_recording = false;
                self.restore_mapping_focus(window, cx);
            }
            Continue::Input(input) => self.open_mapping(input, window, cx),
            Continue::DialInput { mode_uid, input } => {
                self.open_dial_mapping(mode_uid, input, window, cx)
            }
            Continue::SnapCapture { pair, key } => self.start_snap_capture(pair, key, window, cx),
            Continue::AddSnapPair => self.start_add_snap_pair(window, cx),
            Continue::DeleteDial(uid) => {
                self.settings_mut().keyboard.delete_dial(&uid);
                self.dial_highlight = None;
            }
            Continue::ResetDial => {
                self.settings_mut().keyboard.reset_dial();
                self.dial_highlight = None;
            }
        }
        if self.mapping.is_none() {
            self.mapping_recording = false;
            self.customize_drawer.source_input = None;
            if closing_drawer_mapping {
                self.customize_drawer.focus_toggle(window, cx);
            } else if editor_focused && !closing_mapping && !starting_snap {
                // Page, profile and layer changes may remove the old trigger.
                window.focus(&self.workspace_focus, cx);
            }
            self.mapping_return_focus = None;
        }
        self.sync_controls(window, cx);
        self.changed(cx);
    }
    /// `.navs-wrapper .dots3`：标签放不下时的溢出菜单。
    ///
    /// 触发按钮取 `.hover-border.dots3` 的方框（26×26、`border:1px solid #222`、
    /// `border-radius:13px`、`background-size:20px`、`margin-right:10px`；
    /// `:hover{border-color:#5d5d5d}`、`.active{border-color:#44d62c}`），图标默认
    /// `icon_more_default`，当前页被收进溢出时用 `.has-actived-option` 的
    /// `#44d62c` 底 + `icon_more_active`。菜单行取 `.act` 的
    /// `color:#ccc;font-size:14px`、`:hover{background-color:#1a1a1a}`、
    /// `.active{background-color:#000;color:#44d62c}`、
    /// `.active:hover{background-color:#1a1a1a;color:#44d62c}`。
    fn nav_overflow(
        &self,
        hidden: Vec<Tab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let items = hidden
            .iter()
            .map(|page| (page.label(), *page == self.page))
            .collect();
        surface::nav_overflow(
            "device-nav-overflow",
            items,
            move |index, window, cx| {
                let _ = owner.update(cx, |this, cx| this.set_page(hidden[index], window, cx));
            },
            window,
            cx,
        )
    }

    fn toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // 源码 `renderNavs()`：可见宽度 = 窗口宽 − profile 栏宽 − 右侧区宽 −
        // `.dots3` 宽 − 10，放不下的标签进 `.dots3` 下拉（`renderDropdownNavs`）。
        let show_profile_bar = !matches!(self.page, Tab::Power | Tab::Calibration | Tab::Help);
        let profile_bar_width = if !show_profile_bar {
            0.
        } else if self.pid() == 653 && !self.device.use_ble {
            surface::PROFILE_BAR_WIDTH_OBM
        } else {
            surface::PROFILE_BAR_WIDTH
        };
        let available = f32::from(window.viewport_size().width) * 16.
            / f32::from(window.rem_size())
            - profile_bar_width
            - surface::device_right_width(&self.device, true, window)
            - (surface::NAV_MORE_WIDTH + surface::NAV_MORE_MARGIN)
            - 10.;
        let navs = Tab::for_product(self.device.product_id);
        let (visible, hidden) = surface::split_navs(navs, |page| page.label(), available, window);
        h_flex()
            .id("device-navigation")
            .test_support()
            .h(surface::css(48.))
            .flex_shrink_0()
            // 源码 `div.nav-tabs.disabled{opacity:.5}`：关联游戏弹层打开时整行变淡。
            .when(self.linked_games_open, |row| row.opacity(0.5))
            .border_b_2()
            .border_color(cx.theme().title_bar)
            .child(
                surface::nav_left()
                    .children(show_profile_bar.then(|| self.profile_toolbar(window, cx))),
            )
            .child(
                gpui_kit::base::Tabs::new("device-tabs")
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_grow(1.)
                    .flex_shrink_0()
                    .gap(surface::css(20.))
                    .child(
                        // `.nav-tabs .navs-wrapper{display:flex;font-family:Roboto,
                        //  sans-serif;font-size:12px}`，可见标签放这里。
                        h_flex()
                            .id("device-navs")
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .gap(surface::css(20.))
                            .children(visible.iter().map(|page| {
                                let page = *page;
                                surface::navigation_button(
                                    SharedString::from(format!("device-tab-{}", page.id())),
                                    page.label(),
                                    page == self.page,
                                    cx,
                                )
                                .role(Role::Tab)
                                .on_click(
                                    cx.listener(move |this, _, w, cx| this.set_page(page, w, cx)),
                                )
                            }))
                            .children(
                                (!hidden.is_empty()).then(|| self.nav_overflow(hidden, window, cx)),
                            ),
                    ),
            )
            .child(
                // Mounted source `.right` contains battery and help together.
                surface::nav_right()
                    .id("device-navigation-right")
                    .min_w_0()
                    .children(crate::ui::battery::element(&self.device, cx))
                    .child(
                        surface::asset_button(
                            "device-help",
                            if self.page == Tab::Help {
                                "synapse/help-active.svg"
                            } else {
                                "synapse/help-default.svg"
                            },
                            "帮助",
                            cx,
                        )
                        .size(surface::css(24.))
                        .mr(surface::css(10.))
                        .selected(self.page == Tab::Help)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.set_page(Tab::Help, window, cx)),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// The product-side `displayMode=armory` root: the mapping surface the Armory
    /// application embeds for one device. The current bundles mount the mapping
    /// component with `showMouseUse:false`, so the mouse-use block is not part of
    /// this surface; the local mapping surface already has no such block, and the
    /// product navigation/profile chrome stays outside this call.
    pub(crate) fn armory_mapping_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.customize_surface(window, cx)
    }
}
impl Render for DeviceWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stacked = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        // Fixed source canvases and cards scroll horizontally instead of shrinking.
        // The 20px body padding is device CSS; frontend pages have another narrow rule.
        let content_min_width = match self.page {
            Tab::Customize if self.pid() == 653 => 830.8,
            Tab::Customize => surface::CONFIG_WRAPPER_MIN_WIDTH,
            Tab::Sound => 1024.,
            Tab::Mic => 940.,
            Tab::Lighting if crate::product::audited_mouse_mat(self.pid()).is_some() => 1024.,
            // Calibration has no .widget-col or its narrow-window side margins.
            Tab::Calibration => surface::WIDGET_WIDTH,
            _ if stacked => surface::WIDGET_WIDTH + surface::COMPACT_COLUMN_MARGIN * 2.,
            _ => surface::WIDGET_WIDTH,
        };
        let body = if self.page == Tab::Customize {
            self.customize_surface(window, cx)
        } else {
            let page = match self.page {
                Tab::Performance => self.performance_page(cx),
                Tab::Power => self.power_page(cx),
                Tab::Calibration => self.calibration_page(cx),
                Tab::Lighting => self.lighting_page(cx),
                Tab::Sound => self.sound_page(cx),
                Tab::Mic => self.eq_page(EqKind::Mic, cx),
                Tab::Help => self.help_page(cx),
                _ => surface::note("此产品的页面尚未接入。", cx).into_any_element(),
            };
            div()
                .id(SharedString::from(format!(
                    "device-body-{}",
                    self.identity()
                )))
                .test_support()
                .relative()
                .flex_1()
                .min_w(surface::css(surface::BODY_MIN_WIDTH))
                .min_h_0()
                .scrollable_both()
                .track_scroll(&self.body_scroll)
                .child(
                    v_flex()
                        .w_full()
                        .min_w(surface::css(content_min_width + 40.))
                        .max_w(surface::css(surface::BODY_MAX_WIDTH + 40.))
                        .mx_auto()
                        .pt(surface::css(10.))
                        .px(surface::css(20.))
                        .pb(surface::css(20.))
                        .gap_5()
                        .child(page),
                )
                .into_any_element()
        };
        v_flex()
            .id("device-workspace")
            .test_support()
            .track_focus(&self.workspace_focus)
            .size_full()
            .tab_group()
            .child(self.toolbar(window, cx))
            .child(body)
            .when_some(self.source_alert.clone(), |view, dialog| view.child(dialog))
            .when_some(self.profile_dialog.clone(), |view, dialog| {
                view.child(dialog)
            })
    }
}
