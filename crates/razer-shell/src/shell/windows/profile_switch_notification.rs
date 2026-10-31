//! Current StreamMixer -> WindowStorage -> resident notification-manager window.
//! Source: PID1422 44857.w/P; notification-manager 5568, 2864 and Root CSS.
use super::AppShell;
use razer_widgets::keyboard_navigation::Button;
use gpui_kit::*;
use razer_storage::{
    host::{HostStorage, HostStorageCall, HostStorageView, HostStoreKind},
    profile_notification::{Consumer, Notification, Producer, STORAGE_KEY},
};
use razer_widgets::{surface, theme::ProfileNotificationColors as Colors};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub(in crate::shell) const NOTIFICATION_VIEW: u64 = 0xffff_0102;

pub(in crate::shell) struct State {
    consumer: Consumer,
    producers: BTreeMap<u64, Producer>,
    window: Option<(AnyWindowHandle, Entity<ProfileWindow>, Subscription)>,
    pump: Option<Task<()>>,
    display_enabled: Option<bool>,
    display_preference: Option<Task<()>>,
    display_revision: u64,
    display_snapshot: Vec<Notification>,
    local_write: Option<Task<()>>,
    local_snapshot: Option<Value>,
    origin: Instant,
    initialized: bool,
    suspended: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            consumer: Consumer::default(),
            producers: BTreeMap::new(),
            window: None,
            pump: None,
            display_enabled: None,
            display_preference: None,
            display_revision: 0,
            display_snapshot: vec![],
            local_write: None,
            local_snapshot: None,
            origin: Instant::now(),
            initialized: false,
            suspended: false,
        }
    }
}
impl State {
    fn now(&self) -> u64 {
        self.origin.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn next_delay(&self) -> Option<Duration> {
        self.producers
            .values()
            .filter_map(Producer::next_deadline)
            .chain(self.consumer.next_deadline())
            .min()
            .map(|deadline| Duration::from_millis(deadline.saturating_sub(self.now())))
    }
}

fn call(sender_id: u64, action: &str, payload: Value) -> HostStorageCall {
    HostStorageCall {
        store: HostStoreKind::Window,
        sender_id,
        action: action.into(),
        payload,
        target_url_array: vec![],
    }
}

fn register_notification_host(storage: &mut HostStorage) -> anyhow::Result<()> {
    storage.register_view(HostStorageView {
        id: NOTIFICATION_VIEW,
        url: "razer-local:notification-manager-main".into(),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: true,
    });
    storage.call(call(
        NOTIFICATION_VIEW,
        "registerWindowStorageEvent",
        json!({}),
    ))?;
    Ok(())
}

impl AppShell {
    pub(in crate::shell) fn install_profile_switch_notification_cleanup(
        &mut self,
        _cx: &mut Context<Self>,
    ) {
        self.register_exit_resume(|this, cx| {
            if !this.profile_notifications.suspended {
                return;
            }
            this.profile_notifications.suspended = false;
            if this.profile_notifications.window.is_some()
                && let Err(error) = this.start_profile_switch_notifications(cx)
            {
                this.status = format!("Profile notification: {error:#}");
                cx.notify();
            }
        });
        self.register_exit_cleanup(|this, _| {
            this.profile_notifications.suspended = true;
            this.profile_notifications.initialized = false;
            this.profile_notifications.display_revision =
                this.profile_notifications.display_revision.wrapping_add(1);
            this.profile_notifications.display_snapshot.clear();
            this.profile_notifications.display_enabled = None;
            let _ = this.host_storage.call(call(
                NOTIFICATION_VIEW,
                "unRegisterWindowStorageEvent",
                json!({}),
            ));
            this.host_storage.close_view(NOTIFICATION_VIEW);
            this.profile_notifications.pump = None;
            this.profile_notifications.producers.clear();
            let preference = this.profile_notifications.display_preference.take();
            let local_write = this.profile_notifications.local_write.take();
            async move {
                if let Some(task) = preference {
                    task.await;
                }
                if let Some(task) = local_write {
                    task.await;
                }
            }
        });
    }

    pub(in crate::shell) fn initialize_profile_switch_notifications(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.start_profile_switch_notifications(cx)
    }

    fn start_profile_switch_notifications(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.profile_notifications.suspended,
            "Profile notification owner is stopping"
        );
        if self.profile_notifications.initialized {
            return Ok(());
        }
        register_notification_host(&mut self.host_storage)?;
        if self.profile_notifications.window.is_none() {
            self.create_profile_notification_window(cx)?;
        }
        let aggregate = self
            .host_storage
            .call(call(
                NOTIFICATION_VIEW,
                "getWindowStorageItem",
                json!({"key":STORAGE_KEY}),
            ))?
            .value;
        self.profile_notifications.initialized = true;
        let now = self.profile_notifications.now();
        self.profile_notifications.consumer.advance(now);
        let restored = self
            .profile_notifications
            .consumer
            .restore(&aggregate, now, || {
                is_fullscreen().map_err(|error| format!("{error:#}"))
            });
        self.start_profile_notification_pump(cx);
        self.refresh_profile_notification_window(cx)?;
        restored.map_err(anyhow::Error::msg)
    }

    fn create_profile_notification_window(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(380.), px(250.)),
            ))),
            titlebar: None,
            kind: WindowKind::PopUp,
            is_resizable: false,
            is_minimizable: false,
            is_movable: false,
            focus: false,
            show: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };
        let (handle, view) =
            gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| ProfileWindow::new()))?;
        let subscription = cx.subscribe(&view, move |this, _, event, cx| {
            if this.profile_notifications.suspended {
                return;
            }
            let result = match event {
                ProfileWindowEvent::Dismissed(notification) => {
                    this.profile_notifications.consumer.close(notification);
                    this.refresh_profile_notification_window(cx)
                }
                ProfileWindowEvent::Measured => {
                    this.request_profile_notification_visibility(cx);
                    this.persist_profile_notification_measurements(cx);
                    this.refresh_profile_notification_window(cx)
                }
            };
            if let Err(error) = result {
                this.status = format!("Profile notification: {error:#}");
                cx.notify();
            }
        });
        self.profile_notifications.window = Some((handle, view, subscription));
        Ok(())
    }

    pub(in crate::shell) fn publish_stream_mixer_profile_notification(
        &mut self,
        sender_id: u64,
        container_id: String,
        product_name: String,
        preset_name: String,
        timer_tick: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.initialize_profile_switch_notifications(window, cx)?;
        // w obtains this source window's screen.availLeft. The current
        // consumer stores it but positions from record.screen instead.
        let available_left = window
            .display(cx)
            .map(|display| f32::from(display.bounds().origin.x) as f64)
            .unwrap_or(0.);
        let notification = Notification::stream_mixer(
            container_id,
            product_name,
            preset_name,
            timer_tick,
            available_left,
        );
        let now = self.profile_notifications.now();
        let encoded = self
            .profile_notifications
            .producers
            .entry(sender_id)
            .or_default()
            .publish(notification, now);
        self.host_storage.call(call(
            sender_id,
            "setWindowStorageItem",
            json!({"key":STORAGE_KEY,"value":encoded}),
        ))?;
        self.drain_profile_switch_notifications(window, cx)
    }

    pub(in crate::shell) fn cancel_profile_notification_sender(&mut self, sender_id: u64) {
        // Closing the source destroys its JS timers. Current Tab cleanup does
        // not establish a broadcast or delete its actual URL's stored record.
        self.profile_notifications.producers.remove(&sender_id);
    }

    pub(in crate::shell) fn drain_profile_switch_notifications(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.initialize_profile_switch_notifications(window, cx)?;
        self.consume_profile_notification_events(cx)
    }

    fn consume_profile_notification_events(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let events = self.host_storage.drain_events(NOTIFICATION_VIEW)?;
        let mut failure = None;
        for event in events.into_iter().filter(|event| {
            event.channel == "windowStorageEvent" && event.payload["key"] == STORAGE_KEY
        }) {
            let aggregate = self
                .host_storage
                .call(call(
                    NOTIFICATION_VIEW,
                    "getWindowStorageItem",
                    json!({"key":STORAGE_KEY}),
                ))?
                .value;
            let now = self.profile_notifications.now();
            if let Err(error) =
                self.profile_notifications
                    .consumer
                    .consume(&event.payload, &aggregate, now, || {
                        is_fullscreen().map_err(|error| format!("{error:#}"))
                    })
            {
                failure = Some(error);
            }
        }
        self.refresh_profile_notification_window(cx)?;
        self.start_profile_notification_pump(cx);
        failure.map_or(Ok(()), |error| Err(anyhow::Error::msg(error)))
    }

    fn start_profile_notification_pump(&mut self, cx: &mut Context<Self>) {
        if self.profile_notifications.suspended
            || !self.profile_notifications.initialized
            || self.profile_notifications.pump.is_some()
            || self.profile_notifications.next_delay().is_none()
        {
            return;
        }
        // This task belongs to notification-manager, independently of each
        // audio sender's host drain task. Dropping an audio owner cannot strand H.
        self.profile_notifications.pump = Some(cx.spawn(async move |this, cx| {
            loop {
                let delay = this
                    .update(cx, |this, _| this.profile_notifications.next_delay())
                    .ok()
                    .flatten();
                let Some(delay) = delay else {
                    let _ = this.update(cx, |this, _| this.profile_notifications.pump = None);
                    break;
                };
                cx.background_executor().timer(delay).await;
                let current = this.update(cx, |this, cx| {
                    let now = this.profile_notifications.now();
                    let writes = this
                        .profile_notifications
                        .producers
                        .iter_mut()
                        .flat_map(|(sender, producer)| {
                            producer
                                .advance(now)
                                .into_iter()
                                .map(move |value| (*sender, value))
                        })
                        .collect::<Vec<_>>();
                    for (sender, value) in writes {
                        if let Err(error) = this.host_storage.call(call(
                            sender,
                            "setWindowStorageItem",
                            json!({"key":STORAGE_KEY,"value":value}),
                        )) {
                            this.profile_notifications.producers.remove(&sender);
                            this.status = format!("Profile notification: {error:#}");
                            cx.notify();
                        }
                    }
                    // Browser callbacks execute independently; advancing H
                    // before consuming queued storage clears preserves its own
                    // close and topmost side effects at the deadline.
                    this.profile_notifications.consumer.advance(now);
                    if let Err(error) = this.consume_profile_notification_events(cx) {
                        this.status = format!("Profile notification: {error:#}");
                        cx.notify();
                    }
                });
                if current.is_err() {
                    break;
                }
            }
        }));
    }

    fn refresh_profile_notification_window(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if self.profile_notifications.suspended {
            return Ok(());
        }
        let current = self.profile_notifications.consumer.notifications();
        if self.profile_notifications.display_snapshot != current {
            self.profile_notifications.display_snapshot = current.to_vec();
            self.request_profile_notification_visibility(cx);
        }
        let Some((handle, view, _)) = self.profile_notifications.window.as_ref() else {
            return Ok(());
        };
        let handle = *handle;
        let view = view.clone();
        let notifications = self.profile_notifications.consumer.notifications().to_vec();
        let topmost = self.profile_notifications.consumer.always_on_top();
        let enabled = self.profile_notifications.display_enabled.unwrap_or(false);
        handle.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.replace(notifications, window, cx));
            let view = view.read(cx);
            let dimensions = view.window_dimensions(window);
            let point = view
                .notifications
                .first()
                .map(Notification::screen_point)
                .unwrap_or((0., 0.));
            position_and_show(
                window,
                dimensions,
                point,
                !view.notifications.is_empty() && enabled,
                topmost,
            )
        })??;
        Ok(())
    }

    fn request_profile_notification_visibility(&mut self, cx: &mut Context<Self>) {
        if self.profile_notifications.suspended
            || self
                .profile_notifications
                .consumer
                .notifications()
                .is_empty()
        {
            return;
        }
        self.profile_notifications.display_revision =
            self.profile_notifications.display_revision.wrapping_add(1);
        let revision = self.profile_notifications.display_revision;
        let previous = self.profile_notifications.display_preference.take();
        self.profile_notifications.display_preference = Some(cx.spawn(async move |this, cx| {
            if let Some(task) = previous {
                task.await;
            }
            let result = cx
                .background_spawn(
                    async move { razer_storage::notification_preferences::visibility() },
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.profile_notifications.display_revision != revision {
                    return;
                }
                this.profile_notifications.display_preference = None;
                match result {
                    Ok(Some(enabled)) => this.profile_notifications.display_enabled = Some(enabled),
                    Ok(None) => {}
                    Err(error) => {
                        this.status = format!("Profile notification local storage: {error:#}");
                        cx.notify();
                    }
                }
                if let Err(error) = this.refresh_profile_notification_window(cx) {
                    this.status = format!("Profile notification: {error:#}");
                    cx.notify();
                }
            });
        }));
    }

    fn persist_profile_notification_measurements(&mut self, cx: &mut Context<Self>) {
        if self.profile_notifications.suspended {
            return;
        }
        let Some((_, view, _)) = self.profile_notifications.window.as_ref() else {
            return;
        };
        let view = view.read(cx);
        let scale = view.measured_scale;
        let snapshot = Value::Array(
            view.notifications
                .iter()
                .filter_map(|item| {
                    let (width, height) = view.measured.get(&item.key())?;
                    let mut value = item.value().clone();
                    value["width"] = json!(width + 26. * scale);
                    value["height"] = json!(height + 9. * scale);
                    Some(value)
                })
                .collect(),
        );
        if self.profile_notifications.local_snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        self.profile_notifications.local_snapshot = Some(snapshot.clone());
        let previous = self.profile_notifications.local_write.take();
        self.profile_notifications.local_write = Some(cx.spawn(async move |_, cx| {
            if let Some(task) = previous {
                task.await;
            }
            let result = cx
                .background_spawn(async move {
                    // 2864.d/h assign r=f(r), whose implicit return is
                    // undefined. 8531.set then passes JSON.stringify(undefined)
                    // to localStorage, which coerces it to this exact raw string.
                    razer_storage::source_local::set_raw(
                        &razer_storage::source_local::path()?,
                        "notifications",
                        "undefined",
                    )
                })
                .await;
            if let Err(error) = result {
                eprintln!("[profile notification measured storage] {error:#}");
            }
        }));
    }
}

enum ProfileWindowEvent {
    Dismissed(Notification),
    Measured,
}
struct Motion {
    entered: Option<Instant>,
    closing: Option<Instant>,
    _task: Option<Task<()>>,
}
struct ProfileWindow {
    notifications: Vec<Notification>,
    motion: BTreeMap<String, Motion>,
    measured: BTreeMap<String, (f32, f32)>,
    measured_scale: f32,
}
impl EventEmitter<ProfileWindowEvent> for ProfileWindow {}
impl ProfileWindow {
    fn new() -> Self {
        Self {
            notifications: vec![],
            motion: BTreeMap::new(),
            measured: BTreeMap::new(),
            measured_scale: 1.,
        }
    }
    fn replace(
        &mut self,
        notifications: Vec<Notification>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.notifications == notifications {
            return;
        }
        self.motion
            .retain(|key, _| notifications.iter().any(|item| item.key() == *key));
        self.measured
            .retain(|key, _| notifications.iter().any(|item| item.key() == *key));
        for item in &notifications {
            let key = item.key();
            if self.motion.contains_key(&key) {
                continue;
            }
            let enter_key = key.clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(motion) = this.motion.get_mut(&enter_key) {
                        motion.entered = Some(Instant::now());
                        cx.notify();
                    }
                });
            });
            self.motion.insert(
                key,
                Motion {
                    entered: None,
                    closing: None,
                    _task: Some(task),
                },
            );
        }
        self.notifications = notifications;
        cx.notify();
    }
    fn dismiss(&mut self, item: Notification, window: &mut Window, cx: &mut Context<Self>) {
        let Some(motion) = self.motion.get_mut(&item.key()) else {
            return;
        };
        if motion.closing.is_some() {
            return;
        }
        motion.closing = Some(Instant::now());
        motion._task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let _ = this.update(cx, |_, cx| cx.emit(ProfileWindowEvent::Dismissed(item)));
        }));
        cx.notify();
    }
    fn window_dimensions(&self, window: &Window) -> Size<Pixels> {
        let mut width = 0f32;
        let mut height = 0f32;
        let scale = f32::from(window.rem_size()) / 16.;
        for item in &self.notifications {
            if let Some((w, h)) = self.measured.get(&item.key()) {
                width = width.max(*w + 6. * scale);
                height += h + 9. * scale;
            }
        }
        size(
            px((width + 20. * scale).max(380. * scale)),
            px(height.max(250. * scale)),
        )
    }
}

fn ease_in_out(progress: f32) -> f32 {
    // CSS ease-in-out = cubic-bezier(.42,0,.58,1); invert its x coordinate.
    let progress = progress.clamp(0., 1.);
    let mut low = 0.;
    let mut high = 1.;
    for _ in 0..18 {
        let t = (low + high) / 2.;
        let x = 3. * (1. - t) * (1. - t) * t * 0.42 + 3. * (1. - t) * t * t * 0.58 + t * t * t;
        if x < progress {
            low = t;
        } else {
            high = t;
        }
    }
    let t = (low + high) / 2.;
    3. * (1. - t) * t * t + t * t * t
}
impl Render for ProfileWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panels = Vec::new();
        let mut bottom = 0.;
        let scale = f32::from(window.rem_size()) / 16.;
        for item in self.notifications.clone() {
            let key = item.key();
            let motion = self.motion.get(&key);
            let (opacity, shift) = match motion {
                Some(Motion {
                    closing: Some(start),
                    ..
                }) => {
                    let p = (start.elapsed().as_secs_f32() / 0.2).min(1.);
                    if p < 1. {
                        window.request_animation_frame();
                    }
                    (1. - p, 15. * (1. - ease_in_out(p)))
                }
                Some(Motion {
                    entered: Some(start),
                    ..
                }) => {
                    let p = (start.elapsed().as_secs_f32() / 0.2).min(1.);
                    if p < 1. {
                        window.request_animation_frame();
                    }
                    (p, 15. * ease_in_out(p))
                }
                _ => (0., 0.),
            };
            let close_item = item.clone();
            let mut content = div()
                .flex()
                .flex_col()
                .pt(surface::css(20.))
                .pr(surface::css(20.))
                .pb(surface::css(20.))
                .pl(surface::css(50.))
                .child(
                    div()
                        .relative()
                        .text_color(Colors::accent())
                        .pb(surface::css(10.))
                        .mb(surface::css(11.))
                        .child(
                            img("synapse/notification-profile-switch-brand.svg")
                                .absolute()
                                .left(surface::css(-30.))
                                .top(surface::css(-3.))
                                .size(surface::css(20.)),
                        )
                        .child(razer_i18n::t("SWITCH_PROFILE").to_uppercase()),
                );
            if !item.title().is_empty() {
                content = content.child(
                    div()
                        .font_family("RazerF5")
                        .text_size(surface::css(20.))
                        .mb(surface::css(12.))
                        .whitespace_normal()
                        .child(item.title().to_owned()),
                );
            }
            content = content.child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .whitespace_normal()
                    .child(item.footer().to_owned()),
            );
            let panel = div()
                .id(SharedString::from(key.clone()))
                .absolute()
                .right(surface::css(shift))
                .bottom(px(bottom + 3. * scale))
                .w(surface::css(360.))
                .flex()
                .flex_col()
                .bg(Colors::surface())
                .text_color(Colors::text())
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(relative(1.22))
                .opacity(opacity)
                // Source outline does not change the measured 360px box.
                .child(
                    div()
                        .absolute()
                        .left(px(-1.))
                        .right(px(-1.))
                        .top(px(-1.))
                        .bottom(px(-1.))
                        .border_1()
                        .border_color(Colors::accent()),
                )
                .child(content)
                .child(
                    Button::new(SharedString::from(format!("{key}-close")))
                        .absolute()
                        .top_0()
                        .right_0()
                        .size(surface::css(32.))
                        .focusable(false)
                        .tab_stop(false)
                        // Source switch-profile close span is intentionally blank.
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.dismiss(close_item.clone(), window, cx);
                        })),
                )
                .into_any_element();
            bottom += self
                .measured
                .get(&key)
                .map(|(_, height)| *height + 9. * scale)
                .unwrap_or(0.);
            panels.push(panel);
        }
        let keys = self
            .notifications
            .iter()
            .map(Notification::key)
            .collect::<Vec<_>>();
        let owner = cx.entity().downgrade();
        let content = div()
            .size_full()
            .on_children_prepainted(move |bounds, window, cx| {
                let sizes = keys
                    .iter()
                    .zip(bounds)
                    .map(|(key, bounds)| {
                        (
                            key.clone(),
                            (
                                f32::from(bounds.size.width).round(),
                                f32::from(bounds.size.height).round(),
                            ),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let owner = owner.clone();
                window.defer(cx, move |_, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.measured != sizes || this.measured_scale != scale {
                            this.measured = sizes;
                            this.measured_scale = scale;
                            cx.emit(ProfileWindowEvent::Measured);
                            cx.notify();
                        }
                    });
                });
            })
            .children(panels);
        content
    }
}

/// SysUtilsNative.isFullScreenMode RVA79e10 compares all four rcMonitor edges.
fn is_fullscreen() -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::{
            Foundation::{POINT, RECT},
            Graphics::Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
            },
            UI::WindowsAndMessaging::{
                GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowRect,
            },
        };
        let foreground = unsafe { GetForegroundWindow() };
        let mut bounds = RECT::default();
        if unsafe { GetWindowRect(foreground, &mut bounds) } == 0 {
            return Ok(false);
        }
        if foreground == unsafe { GetDesktopWindow() } || foreground == unsafe { GetShellWindow() }
        {
            return Ok(false);
        }
        let handle = unsafe {
            MonitorFromPoint(
                POINT {
                    x: bounds.left,
                    y: bounds.top,
                },
                MONITOR_DEFAULTTONEAREST,
            )
        };
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };
        if unsafe { GetMonitorInfoW(handle, &mut monitor) } == 0 {
            return Ok(false);
        }
        Ok(bounds.left == monitor.rcMonitor.left
            && bounds.top == monitor.rcMonitor.top
            && bounds.right == monitor.rcMonitor.right
            && bounds.bottom == monitor.rcMonitor.bottom)
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("source fullscreen notification detection is unavailable on this platform")
    }
}

fn position_and_show(
    window: &mut Window,
    dimensions: Size<Pixels>,
    point: (f64, f64),
    visible: bool,
    topmost: bool,
) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::{
            Foundation::{POINT, RECT},
            Graphics::Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
            },
            UI::WindowsAndMessaging::{
                GWL_EXSTYLE, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, SW_HIDE,
                SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetWindowLongPtrW, SetWindowPos, ShowWindow,
                WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            },
        };
        let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("{error}"))?
            .as_raw()
        else {
            anyhow::bail!("profile notification requires a Win32 window");
        };
        let hwnd = handle.hwnd.get() as _;
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                (style & !(WS_EX_APPWINDOW as isize))
                    | WS_EX_TOOLWINDOW as isize
                    | WS_EX_NOACTIVATE as isize,
            );
        }
        if !visible {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            return Ok(());
        }
        let monitor_handle = unsafe {
            MonitorFromPoint(
                POINT {
                    x: point.0.round() as i32,
                    y: point.1.round() as i32,
                },
                MONITOR_DEFAULTTONEAREST,
            )
        };
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };
        anyhow::ensure!(
            unsafe { GetMonitorInfoW(monitor_handle, &mut monitor) } != 0,
            "profile notification display lookup failed: {}",
            std::io::Error::last_os_error()
        );
        let scale = window.scale_factor();
        let width = (f32::from(dimensions.width) * scale).round() as i32;
        let height = (f32::from(dimensions.height) * scale).round() as i32;
        let x = monitor.rcMonitor.right - width - (20. * scale).round() as i32;
        let y = monitor.rcMonitor.bottom - height - (80. * scale).round() as i32;
        // Source setBounds is issued twice, before and after exitSizeMove.
        for _ in 0..2 {
            anyhow::ensure!(
                unsafe {
                    SetWindowPos(
                        hwnd,
                        if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST },
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE,
                    )
                } != 0,
                "profile notification bounds update failed: {}",
                std::io::Error::last_os_error()
            );
        }
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, dimensions, point, visible, topmost);
        anyhow::bail!("resident source notification windows are unavailable on this platform")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HostStorage, HostStorageView, NOTIFICATION_VIEW, STORAGE_KEY, call, ease_in_out, json,
        register_notification_host,
    };
    use crate::shell::Location;
    use razer_pages::nav::Tab;
    #[test]
    fn profile_notification_preference_mount_follows_the_shared_settings_page() {
        let product = Location::ProductSettings(false);
        let main = Location::Main(Tab::Setting);
        let home = Location::Main(Tab::Home);
        for settings in [&product, &main] {
            assert_eq!(home.synapse_settings_mount_change(settings), Some(true));
            assert_eq!(settings.synapse_settings_mount_change(&home), Some(false));
            for same_owner in [&product, &main] {
                assert_eq!(settings.synapse_settings_mount_change(same_owner), None);
            }
            for other in [
                Location::ProductSettings(true),
                Location::ProductSettingsMigration,
                Location::ProfileMigration,
                Location::Device("source-device".into()),
            ] {
                assert_eq!(home.synapse_settings_mount_change(&other), None);
                assert_eq!(other.synapse_settings_mount_change(settings), Some(true));
                assert_eq!(settings.synapse_settings_mount_change(&other), Some(false));
            }
        }
    }

    #[test]
    fn profile_notification_exit_resume_restores_actual_host_subscription_once() {
        let mut storage = HostStorage::default();
        storage.register_view(HostStorageView {
            id: 7,
            url: "razer-local:stream-mixer-test".into(),
            remote: false,
            destroyed: false,
            crashed: false,
            disposed: false,
            tracked_url: true,
        });
        register_notification_host(&mut storage).unwrap();
        storage
            .call(call(
                7,
                "setWindowStorageItem",
                json!({"key":STORAGE_KEY,"value":"first"}),
            ))
            .unwrap();
        assert_eq!(storage.drain_events(NOTIFICATION_VIEW).unwrap().len(), 1);
        storage
            .call(call(
                NOTIFICATION_VIEW,
                "unRegisterWindowStorageEvent",
                json!({}),
            ))
            .unwrap();
        storage.close_view(NOTIFICATION_VIEW);
        storage
            .call(call(
                7,
                "setWindowStorageItem",
                json!({"key":STORAGE_KEY,"value":"during-cleanup"}),
            ))
            .unwrap();
        assert!(storage.drain_events(NOTIFICATION_VIEW).is_err());

        register_notification_host(&mut storage).unwrap();
        register_notification_host(&mut storage).unwrap();
        let aggregate = storage
            .call(call(
                NOTIFICATION_VIEW,
                "getWindowStorageItem",
                json!({"key":STORAGE_KEY}),
            ))
            .unwrap();
        assert!(aggregate.value.as_str().unwrap().contains("during-cleanup"));
        assert!(storage.drain_events(NOTIFICATION_VIEW).unwrap().is_empty());
        storage
            .call(call(
                7,
                "setWindowStorageItem",
                json!({"key":STORAGE_KEY,"value":"resumed"}),
            ))
            .unwrap();
        let events = storage.drain_events(NOTIFICATION_VIEW).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["newValue"], "resumed");
    }
    #[test]
    fn source_css_ease_has_exact_endpoints_and_midpoint() {
        assert!(ease_in_out(0.) < 0.00001);
        assert!((ease_in_out(0.5) - 0.5).abs() < 0.00001);
        assert!((ease_in_out(1.) - 1.).abs() < 0.00001);
    }
}
