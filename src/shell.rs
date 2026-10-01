//! App navigation and persistence. Device feature state lives in DeviceWorkspace.
use crate::{
    features::{DeviceWorkspace, WorkspaceEvent},
    model::Device,
    nav::Tab,
    store,
    ui::surface,
};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    scroll::ScrollableElement as _,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

mod main_pages;
mod pairing_page;
mod runtime_page;
mod service_pages;
mod settings_page;

#[derive(Clone, PartialEq)]
enum Location {
    Main(Tab),
    Device(String),
    Pairing,
}
pub struct AppShell {
    devices: Vec<Entity<DeviceWorkspace>>,
    location: Location,
    history: Vec<Location>,
    history_index: usize,
    tracking_intro_seen: bool,
    saved_intro_seen: bool,
    subscriptions: Vec<Subscription>,
    save_task: Option<Task<()>>,
    storage_error: Option<String>,
    status: String,
    dashboard_collapsed: bool,
    shortcuts: Entity<crate::features::shortcuts::Shortcuts>,
    settings: Entity<settings_page::SettingsPage>,
    gamer_room: Entity<service_pages::GamerRoomPage>,
    module_catalog: Entity<service_pages::ModuleCatalog>,
    pairing: Entity<pairing_page::PairingPage>,
}
impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (devices, intro, shortcuts, preferences, error) =
            match store::read_workspace(&store::store_path()) {
                Ok(Some(file)) => (
                    file.devices,
                    file.tracking_intro_seen,
                    file.shortcuts,
                    file.preferences,
                    None,
                ),
                Ok(None) => (
                    crate::model::measured_devices(),
                    false,
                    vec![],
                    Default::default(),
                    None,
                ),
                Err(error) => (
                    crate::model::measured_devices(),
                    false,
                    vec![],
                    Default::default(),
                    Some(error.to_string()),
                ),
            };
        let shortcuts =
            cx.new(|cx| crate::features::shortcuts::Shortcuts::new(shortcuts, window, cx));
        let runtime = cx.new(|_| runtime_page::RuntimePanel::new());
        let gamer_room_seen = preferences.gamer_room_tutorial_seen;
        let settings =
            cx.new(|cx| settings_page::SettingsPage::new(preferences, runtime, window, cx));
        let mut this = Self {
            devices: vec![],
            location: Location::Main(Tab::Home),
            history: vec![],
            history_index: 0,
            tracking_intro_seen: intro,
            saved_intro_seen: intro,
            subscriptions: vec![],
            save_task: None,
            storage_error: error,
            status: "本地配置预览 · 尚未写入硬件".into(),
            dashboard_collapsed: false,
            shortcuts,
            settings,
            gamer_room: cx.new(|_| service_pages::GamerRoomPage::new()),
            module_catalog: cx.new(|_| service_pages::ModuleCatalog::new()),
            pairing: cx.new(|cx| pairing_page::PairingPage::new(window, cx)),
        };
        this.gamer_room
            .update(cx, |page, cx| page.set_tutorial_seen(gamer_room_seen, cx));
        this.subscriptions.push(cx.subscribe_in(
            &this.settings,
            window,
            |this, _, event, window, cx| match event {
                settings_page::SettingsEvent::Changed => {
                    let seen = this.settings.read(cx).snapshot().gamer_room_tutorial_seen;
                    this.gamer_room
                        .update(cx, |page, cx| page.set_tutorial_seen(seen, cx));
                    cx.notify();
                }
                settings_page::SettingsEvent::Language => {
                    for device in &this.devices {
                        device.update(cx, |device, cx| device.refresh_locale(window, cx));
                    }
                    cx.refresh_windows();
                }
                settings_page::SettingsEvent::Save => this.save(false, window, cx),
                settings_page::SettingsEvent::ResetTutorials => {
                    this.tracking_intro_seen = false;
                    for device in &this.devices {
                        device.update(cx, |device, cx| device.set_intro_seen(false, cx));
                    }
                    this.gamer_room
                        .update(cx, |page, cx| page.reset_tutorial(cx));
                    this.settings
                        .update(cx, |settings, cx| settings.tutorial_seen(false, cx));
                    this.save_intro(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.gamer_room,
            |this, _, _: &service_pages::GamerRoomEvent, cx| {
                this.settings
                    .update(cx, |settings, cx| settings.tutorial_seen(true, cx));
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.pairing,
            window,
            |this, _, _: &pairing_page::PairingPageEvent, window, cx| {
                this.navigate(Location::Main(Tab::Home), window, cx);
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.shortcuts,
            |_, _, _: &crate::features::shortcuts::ShortcutsChanged, cx| cx.notify(),
        ));
        for device in devices {
            this.add_device(device, window, cx);
        }
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--demo-keyboard") {
            this.add_device(crate::demo::demo_keyboard(), window, cx);
        }
        if let Some(pid) = args
            .iter()
            .position(|a| a == "--preview-product")
            .and_then(|p| args.get(p + 1))
            .and_then(|v| v.parse::<u32>().ok())
        {
            if [653, 777].contains(&pid) {
                this.add_preview(pid, window, cx);
            }
        }
        if let Some(tab) = args
            .iter()
            .position(|a| a == "--tab")
            .and_then(|p| args.get(p + 1))
            .and_then(|v| Tab::from_arg(v))
        {
            if tab.is_main() {
                this.location = Location::Main(tab);
            } else if tab == Tab::Pairing {
                this.location = Location::Pairing;
            } else if let Some(device) = this
                .devices
                .iter()
                .find(|d| {
                    let pages = Tab::for_product(d.read(cx).device().product_id);
                    pages.contains(&tab) || (tab == Tab::Help && !pages.is_empty())
                })
                .cloned()
            {
                let key = device.read(cx).identity();
                device.update(cx, |d, cx| d.set_page(tab, window, cx));
                this.location = Location::Device(key);
            } else {
                this.status = "所选页面不适用于当前设备。".into();
            }
        }
        this.history = vec![this.location.clone()];
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let Some(entity) = weak.upgrade() else {
                return true;
            };
            if !entity.read(cx).dirty(cx) {
                return true;
            }
            entity.update(cx, |this, cx| this.confirm_close(window, cx));
            false
        });
        this
    }
    fn add_device(&mut self, mut device: Device, window: &mut Window, cx: &mut Context<Self>) {
        // Migrate only the explicit previews created before layout identity was stored.
        if device.layout_id == 0
            && (device.serial_number == "PREVIEW-653"
                || device.serial_number == "DEMO-KEYBOARD-0001")
        {
            device.layout_id = 1;
        }
        let entity =
            cx.new(|cx| DeviceWorkspace::new(device, self.tracking_intro_seen, window, cx));
        self.subscriptions.push(
            cx.subscribe_in(&entity, window, |this, _, event, window, cx| match event {
                WorkspaceEvent::Changed => cx.notify(),
                WorkspaceEvent::SaveRequested => this.save(false, window, cx),
                WorkspaceEvent::IntroDismissed => {
                    this.tracking_intro_seen = true;
                    for device in &this.devices {
                        device.update(cx, |d, cx| d.set_intro_seen(true, cx));
                    }
                    this.save_intro(cx);
                }
            }),
        );
        self.devices.push(entity);
    }
    fn dirty(&self, cx: &App) -> bool {
        self.tracking_intro_seen != self.saved_intro_seen
            || self.settings.read(cx).dirty()
            || self.shortcuts.read(cx).dirty()
            || self.devices.iter().any(|d| d.read(cx).dirty())
    }
    fn navigate(&mut self, next: Location, window: &mut Window, cx: &mut Context<Self>) {
        self.request_navigation(next, None, window, cx);
    }
    fn request_navigation(
        &mut self,
        next: Location,
        history_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if next == self.location {
            return;
        }
        if self.location == Location::Main(Tab::Shortcuts) && self.shortcuts.read(cx).draft_dirty()
        {
            let entity = cx.entity();
            let invalid = !self.shortcuts.read(cx).valid();
            window.open_dialog(cx, move |dialog, _, _| {
                let save = entity.clone();
                let discard = entity.clone();
                let save_next = next.clone();
                let discard_next = next.clone();
                dialog
                    .title("保存全局快捷键更改？")
                    .child("离开页面前，保存快捷键或丢弃本次编辑。")
                    .footer(
                        h_flex()
                            .gap_3()
                            .justify_end()
                            .child(
                                Button::new("shortcuts-nav-keep")
                                    .label("继续编辑")
                                    .on_click(|_, w, cx| w.close_dialog(cx)),
                            )
                            .child(
                                Button::new("shortcuts-nav-discard")
                                    .label("丢弃并继续")
                                    .on_click(move |_, w, cx| {
                                        w.close_dialog(cx);
                                        discard.update(cx, |s, cx| {
                                            s.shortcuts
                                                .update(cx, |s, cx| s.dismiss_for_navigation(cx));
                                            s.navigate_now(discard_next.clone(), history_index, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("shortcuts-nav-save")
                                    .label("保存并继续")
                                    .primary()
                                    .disabled(invalid)
                                    .on_click(move |_, w, cx| {
                                        w.close_dialog(cx);
                                        save.update(cx, |s, cx| {
                                            if s.shortcuts.update(cx, |s, cx| s.prepare_save(w, cx))
                                            {
                                                s.navigate_now(
                                                    save_next.clone(),
                                                    history_index,
                                                    cx,
                                                );
                                            }
                                        });
                                    }),
                            ),
                    )
            });
        } else {
            if self.location == Location::Main(Tab::Shortcuts) {
                self.shortcuts
                    .update(cx, |s, cx| s.dismiss_for_navigation(cx));
            }
            self.navigate_now(next, history_index, cx);
        }
    }
    fn navigate_now(
        &mut self,
        next: Location,
        history_index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if next != self.location {
            if self.location == Location::Pairing {
                self.pairing.update(cx, |page, cx| page.deactivate(cx));
            }
            if let Some(index) = history_index {
                self.history_index = index;
            } else {
                self.history.truncate(self.history_index + 1);
                self.history.push(next.clone());
                self.history_index = self.history.len() - 1;
            }
            self.location = next;
            cx.notify();
        }
    }
    fn move_history(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.history_index as isize + delta;
        if target >= 0 && target < self.history.len() as isize {
            self.request_navigation(
                self.history[target as usize].clone(),
                Some(target as usize),
                window,
                cx,
            );
        }
    }
    fn save(&mut self, close_after: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_task.is_some() {
            return;
        }
        if let Some(error) = &self.storage_error {
            self.status = format!("未保存：原配置文件无法读取（{error}）。请先修复配置文件。");
            cx.notify();
            return;
        }
        for device in &self.devices {
            if !device.update(cx, |d, cx| d.prepare_save(window, cx)) {
                self.status = "请先补全按键映射，或关闭编辑并丢弃映射草稿。".into();
                cx.notify();
                return;
            }
        }
        if !self
            .shortcuts
            .update(cx, |s, cx| s.prepare_save(window, cx))
        {
            self.status = "请先补全全局快捷键，或丢弃快捷键草稿。".into();
            cx.notify();
            return;
        }
        let devices: Vec<Device> = self.devices.iter().map(|d| d.read(cx).snapshot()).collect();
        let shortcuts = self.shortcuts.read(cx).snapshot();
        let preferences = self.settings.read(cx).snapshot();
        let intro = self.tracking_intro_seen;
        let file = store::WorkspaceFile::new(devices.clone(), intro)
            .with_shortcuts(shortcuts.clone())
            .with_preferences(preferences.clone());
        let path = store::store_path();
        self.status = "正在保存到本机…".into();
        cx.notify();
        self.save_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { store::write_workspace(&path, &file) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.save_task = None;
                match result {
                    Ok(()) => {
                        // Commit exactly the captured revision. New edits during I/O stay dirty.
                        for (entity, snapshot) in this.devices.iter().zip(devices) {
                            entity.update(cx, |d, cx| d.mark_saved(snapshot, cx));
                        }
                        this.saved_intro_seen = intro;
                        this.settings
                            .update(cx, |settings, cx| settings.mark_saved(preferences, cx));
                        this.shortcuts
                            .update(cx, |s, cx| s.mark_saved(shortcuts, cx));
                        this.status = "已保存到本机 · 尚未发送到设备".into();
                        if this.settings.read(cx).snapshot().notifications && !close_after {
                            window.push_notification("配置已保存到本机。", cx);
                        }
                        if close_after && !this.dirty(cx) {
                            cx.quit();
                        } else if this.tracking_intro_seen != this.saved_intro_seen {
                            this.save_intro(cx);
                        }
                    }
                    Err(error) => this.status = format!("保存失败：{error}"),
                }
                cx.notify();
            });
        }));
    }
    fn save_intro(&mut self, cx: &mut Context<Self>) {
        // Persist the preference with saved device revisions; never commit unrelated drafts.
        if self.save_task.is_some() || self.storage_error.is_some() {
            cx.notify();
            return;
        }
        let intro = self.tracking_intro_seen;
        let file = store::WorkspaceFile::new(
            self.devices
                .iter()
                .map(|d| d.read(cx).saved_snapshot())
                .collect(),
            intro,
        )
        .with_shortcuts(self.shortcuts.read(cx).saved_snapshot())
        .with_preferences(self.settings.read(cx).saved_snapshot());
        let path = store::store_path();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { store::write_workspace(&path, &file) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.save_task = None;
                match result {
                    Ok(()) => this.saved_intro_seen = intro,
                    Err(error) => this.status = format!("介绍偏好保存失败：{error}"),
                }
                cx.notify();
            });
        }));
    }
    fn confirm_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            dialog
                .title("保存本地更改？")
                .child("尚有未保存的配置。关闭前可以保存，或放弃本次更改。")
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("close-cancel")
                                .label("取消")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("close-discard")
                                .label("不保存并关闭")
                                .on_click(|_, _, cx| cx.quit()),
                        )
                        .child(
                            Button::new("close-save")
                                .label("保存并关闭")
                                .primary()
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    entity.update(cx, |this, cx| this.save(true, w, cx));
                                }),
                        ),
                )
        });
    }
    fn add_preview(&mut self, pid: u32, window: &mut Window, cx: &mut Context<Self>) {
        let serial = format!("PREVIEW-{pid}");
        if !self
            .devices
            .iter()
            .any(|d| d.read(cx).device().serial_number == serial)
        {
            let mut device = crate::demo::demo_keyboard();
            device.product_id = pid;
            device.real_product_id = pid;
            device.serial_number = serial;
            device.device_container_id = format!("preview-{pid}");
            if pid == 777 {
                device.category = crate::model::DeviceCategory::Headset;
                device.has_battery = true;
                device.features =
                    crate::domain::DeviceFeatures::for_category(device.category, true, true);
            }
            let name = if pid == 653 {
                "Razer BlackWidow V4 Pro · 预览"
            } else {
                "Razer Kraken BT · 预览"
            };
            for value in device.name.values.values_mut() {
                *value = name.into();
            }
            device.product_name = device.name.clone();
            self.add_device(device, window, cx);
        }
        let key = self
            .devices
            .iter()
            .find(|d| d.read(cx).device().serial_number == format!("PREVIEW-{pid}"))
            .unwrap()
            .read(cx)
            .identity();
        self.navigate(Location::Device(key), window, cx);
    }
    fn title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .id("host-titlebar")
            .h(rems(2.625))
            .flex_shrink_0()
            .bg(cx.theme().title_bar)
            .child(
                Button::new("host-main")
                    .accessibility_label("雷云")
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(if matches!(self.location, Location::Main(_)) {
                                cx.theme().background
                            } else {
                                cx.theme().transparent
                            })
                            .hover(cx.theme().secondary_hover)
                            .active(cx.theme().background),
                    )
                    .min_w(surface::css(90.))
                    .px(surface::css(20.))
                    .child(
                        h_flex()
                            .gap(surface::css(8.))
                            .child(img("synapse/synapse.svg").size(surface::css(20.)))
                            .child("雷云"),
                    )
                    .h_full()
                    .selected(matches!(self.location, Location::Main(_)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(Location::Main(Tab::Home), window, cx)
                    })),
            )
            .children(
                self.devices
                    .iter()
                    .filter(|d| !Tab::for_product(d.read(cx).device().product_id).is_empty())
                    .map(|device| {
                        let state = device.read(cx);
                        let key = state.identity();
                        Button::new(SharedString::from(format!("host-{key}")))
                            .label(state.device().display_name())
                            .text_size(surface::css(12.))
                            .min_w(surface::css(90.))
                            .max_w(surface::css(240.))
                            .mt(surface::css(7.))
                            .h(surface::css(35.))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(if self.location == Location::Device(key.clone()) {
                                        cx.theme().background
                                    } else {
                                        cx.theme().transparent
                                    })
                                    .hover(cx.theme().secondary_hover)
                                    .active(cx.theme().background),
                            )
                            .border_t_1()
                            .border_color(if self.location == Location::Device(key.clone()) {
                                cx.theme().primary
                            } else {
                                cx.theme().transparent
                            })
                            .selected(self.location == Location::Device(key.clone()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.navigate(Location::Device(key.clone()), window, cx)
                            }))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                Button::new("window-minimize")
                    .icon(gpui_kit::assets::IconName::Minus)
                    .ghost()
                    .w(surface::css(48.))
                    .h_full()
                    .rounded(px(0.))
                    .accessibility_label("最小化")
                    .on_click(|_, w, _| w.minimize_window()),
            )
            .child(
                Button::new("window-maximize")
                    .icon(gpui_kit::assets::IconName::Square)
                    .ghost()
                    .w(surface::css(48.))
                    .h_full()
                    .rounded(px(0.))
                    .accessibility_label("最大化或还原")
                    .on_click(|_, w, _| w.zoom_window()),
            )
            .child(
                Button::new("window-close")
                    .icon(gpui_kit::assets::IconName::X)
                    .ghost()
                    .w(surface::css(48.))
                    .h_full()
                    .rounded(px(0.))
                    .accessibility_label("关闭窗口")
                    .on_click(cx.listener(|this, _, w, cx| {
                        if this.dirty(cx) {
                            this.confirm_close(w, cx);
                        } else {
                            cx.quit();
                        }
                    })),
            )
            .into_any_element()
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = match &self.location {
            Location::Device(key) => self
                .devices
                .iter()
                .find(|d| d.read(cx).identity() == *key)
                .map(|d| d.read(cx).device().display_name())
                .unwrap_or_default(),
            Location::Main(Tab::Setting) => "设置".into(),
            Location::Main(_) => "RAZER SYNAPSE".into(),
            Location::Pairing => "多设备配对".into(),
        };
        h_flex()
            .id("app-toolbar")
            .h(rems(2.375))
            .flex_shrink_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        surface::asset_button(
                            "history-back",
                            "synapse/history-back.svg",
                            "后退",
                            cx,
                        )
                        .w(surface::css(40.))
                        .h_full()
                        .disabled(self.history_index == 0)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.move_history(-1, window, cx)),
                        ),
                    )
                    .child(
                        surface::asset_button(
                            "history-forward",
                            "synapse/history-forward.svg",
                            "前进",
                            cx,
                        )
                        .w(surface::css(40.))
                        .h_full()
                        .disabled(self.history_index + 1 >= self.history.len())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.move_history(1, window, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .id("toolbar-title")
                    .flex_grow(3.)
                    .flex_basis(surface::css(340.))
                    .min_w_0()
                    .text_size(surface::css(14.))
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .text_ellipsis()
                    .child(title),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .justify_end()
                    .child(
                        surface::asset_button("app-settings", "synapse/settings.svg", "设置", cx)
                            .w(surface::css(46.))
                            .h_full()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate(Location::Main(Tab::Setting), window, cx)
                            })),
                    )
                    .child(
                        Button::new("save-all")
                            .label("保存")
                            .small()
                            .disabled(
                                !self.dirty(cx)
                                    || self.save_task.is_some()
                                    || self.storage_error.is_some(),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save(false, window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn main_page(&self, page: Tab, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let layout = main_pages::MainLayout::new(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
            if page == Tab::Home {
                self.devices.len()
            } else {
                0
            },
        );
        let body = match page {
            Tab::Home => self.dashboard(&layout, cx),
            Tab::Modules => self.modules_page(cx),
            Tab::GamerRoom => self.gamer_room_page(cx),
            Tab::Shortcuts => self.shortcuts_page(cx),
            Tab::Setting => v_flex()
                .gap_4()
                .child(self.settings.clone())
                .child(
                    surface::panel("本地工作区", cx)
                        .child(surface::note(
                            format!("本地配置位置：{}", store::store_path().display()),
                            cx,
                        ))
                        .child(self.preview_controls(cx))
                        .child(
                            Button::new("open-pairing-page")
                                .label("多设备配对")
                                .outline()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate(Location::Pairing, window, cx)
                                })),
                        ),
                )
                .when_some(self.storage_error.clone(), |this, error| {
                    this.child(
                        div()
                            .text_color(cx.theme().danger)
                            .child(format!("配置读取失败：{error}")),
                    )
                })
                .into_any_element(),
            _ => div().into_any_element(),
        };
        v_flex()
            .size_full()
            .when(page != Tab::Setting, |this| {
                this.child(
                    gpui_kit::base::Tabs::new("main-navigation")
                        .h(surface::css(48.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .tab_group()
                        .gap(surface::css(20.))
                        .border_b_2()
                        .border_color(cx.theme().title_bar)
                        .children(Tab::MAIN.map(|tab| {
                            surface::navigation_button(
                                SharedString::from(format!("main-tab-{}", tab.id())),
                                tab.label(),
                                tab == page,
                                cx,
                            )
                            .role(Role::Tab)
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.navigate(Location::Main(tab), window, cx)
                                },
                            ))
                        })),
                )
            })
            .child(
                div()
                    .id("main-page-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(surface::css(layout.body_max_width))
                            .mx_auto()
                            .pt(surface::css(10.))
                            .px(surface::css(layout.gutter))
                            .pb(surface::css(20.))
                            .gap(surface::css(20.))
                            .child(body),
                    ),
            )
            .into_any_element()
    }
}
impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.location {
            Location::Device(key) => self
                .devices
                .iter()
                .find(|d| d.read(cx).identity() == *key)
                .map(|d| d.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Main(page) => self.main_page(*page, window, cx),
            Location::Pairing => self.pairing.clone().into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(self.toolbar(cx))
            .child(div().flex_1().min_h_0().child(content))
            .child(
                div()
                    .h(surface::css(24.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status.clone()),
            )
    }
}
