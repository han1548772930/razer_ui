//! The four current mounted video-react players share the same audible MOV.
//! The media clock comes from consumed native PCM frames, never a UI timer.
//! Full caller bodies and CSS are in the current audio-demo-audit query.
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use gpui_kit::component::{
    WindowExt as _, h_flex,
    slider::{SliderEvent, SliderState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_platform::media::{Clip, PlaybackRate, Session, Status};
use razer_widgets::{surface, theme::AudioDemoColors as Colors};
use std::{sync::Arc, time::Duration};

pub(super) struct MediaPresentationChanged;

pub(super) struct AudioMedia {
    product: u32,
    poster: SharedString,
    object_fit: ObjectFit,
    button_width: f32,
    glyph_size: f32,
    width: f32,
    height: f32,
    clip: Option<Arc<Clip>>,
    session: Option<Session>,
    status: Status,
    shown: bool,
    loading: bool,
    start_requested: bool,
    has_started: bool,
    requested_volume: f32,
    requested_rate: PlaybackRate,
    scrubbing: Option<bool>,
    generation: u64,
    seek_revision: u64,
    frame_ix: Option<usize>,
    frame: Option<Arc<RenderImage>>,
    progress: Entity<SliderState>,
    focus: FocusHandle,
    progress_focus: FocusHandle,
    fullscreen: bool,
    window_was_fullscreen: bool,
    _progress: Subscription,
    _resources: Subscription,
    _release: Subscription,
    _window: Subscription,
    resource_ready: async_channel::Receiver<()>,
    cleanup: Option<Task<()>>,
    error: Option<String>,
    reported_error: Option<String>,
    task: Option<Task<()>>,
}
impl EventEmitter<MediaPresentationChanged> for AudioMedia {}

impl AudioMedia {
    pub(super) fn new(
        product: u32,
        poster: impl Into<SharedString>,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let poster = poster.into();
        cx.new(|cx: &mut Context<Self>| {
            let progress = cx.new(|_| SliderState::new().max(100.).step(0.01));
            let subscription = cx.subscribe_in(&progress, window, |this, _, event, window, cx| {
                if matches!(event, SliderEvent::Change(_)) && this.scrubbing.is_none() {
                    this.scrubbing = Some(this.status.is_playing());
                    if let Some(session) = &this.session {
                        if let Err(error) = session.pause() {
                            this.error = Some(format!("{error:#}"));
                        }
                    }
                }
                if let SliderEvent::Release(value) = event {
                    if let Some(clip) = &this.clip {
                        let target = (clip.sample_count() as f64
                            * f64::from(value.start().clamp(0., 100.))
                            / 100.)
                            .round() as u64;
                        if let Some(session) = &this.session {
                            match session.seek(target.min(clip.sample_count())) {
                                Ok(revision) => {
                                    this.seek_revision = revision;
                                    this.frame_ix = None;
                                    if this.scrubbing.take() == Some(true) {
                                        if let Err(error) = session.play() {
                                            this.error = Some(format!("{error:#}"));
                                        }
                                    }
                                }
                                Err(error) => {
                                    this.error = Some(format!("{error:#}"));
                                }
                            }
                        }
                    }
                }
                this.report_error(window, cx);
                cx.notify();
            });
            let (sender, resource_ready) = async_channel::bounded(1);
            let resources = razer_assets::observe_resources(cx, move |_, event, _| {
                if event.path() == "assets/synapse/products/1352/audio-mode.rzav" {
                    let _ = sender.try_send(());
                }
            });
            let release = cx.on_release_in(window, |this, window, _| {
                this.exit_fullscreen(window);
                if let Some(session) = &this.session {
                    session.cancel();
                }
            });
            let window_state = cx.observe_window_bounds(window, |this, window, cx| {
                if this.fullscreen && !window.is_fullscreen() {
                    this.fullscreen = false;
                    cx.emit(MediaPresentationChanged);
                    cx.notify();
                }
            });
            Self {
                product,
                poster,
                object_fit: ObjectFit::Fill,
                button_width: 40.,
                glyph_size: 18.,
                width,
                height,
                clip: None,
                session: None,
                status: Status::default(),
                shown: false,
                loading: false,
                start_requested: false,
                has_started: false,
                requested_volume: 1.,
                requested_rate: PlaybackRate::Normal,
                scrubbing: None,
                generation: 0,
                frame_ix: None,
                frame: None,
                progress,
                focus: cx.focus_handle().tab_stop(false),
                progress_focus: cx.focus_handle().tab_stop(true),
                fullscreen: false,
                window_was_fullscreen: false,
                _progress: subscription,
                task: None,
                seek_revision: 0,
                _resources: resources,
                _release: release,
                _window: window_state,
                resource_ready,
                cleanup: None,
                error: None,
                reported_error: None,
            }
        })
    }

    pub(super) fn has_position(&self) -> bool {
        self.status.cursor() > 0
    }
    pub(super) fn is_playing(&self) -> bool {
        self.status.is_playing()
    }
    pub(super) fn remount_presentation(
        &mut self,
        shown: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shown = shown;
        self.has_started = self.status.is_playing();
        if !shown {
            self.pause(window, cx);
        }
        cx.notify();
    }
    pub(super) fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }
    pub(super) fn set_size(&mut self, width: f32, height: f32, cx: &mut Context<Self>) {
        self.width = width;
        self.height = height;
        cx.notify();
    }
    pub(super) fn pause(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_requested = false;
        self.exit_fullscreen(window);
        if let Some(session) = &self.session {
            if let Err(error) = session.pause() {
                self.error = Some(format!("{error:#}"));
            }
        }
        self.report_error(window, cx);
        cx.notify();
    }
    pub(super) fn set_presentation(&mut self, fit: ObjectFit, button_width: f32, glyph_size: f32) {
        self.object_fit = fit;
        self.button_width = button_width;
        self.glyph_size = glyph_size;
    }
    pub(super) fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        self.exit_fullscreen(window);
        if let Some(mut session) = self.session.take() {
            session.cancel();
            self.cleanup = Some(cx.background_spawn(async move {
                if let Err(error) = session.stop() {
                    eprintln!("Audible demo cleanup: {error:#}");
                }
            }));
        }
        self.shown = false;
        self.loading = false;
        self.start_requested = false;
        self.scrubbing = None;
        self.has_started = false;
        self.requested_volume = 1.;
        self.requested_rate = PlaybackRate::Normal;
        self.status = Status::default();
        self.seek_revision = 0;
        self.error = None;
        self.reported_error = None;
        self.frame = None;
        self.frame_ix = None;
        cx.notify();
    }

    pub(super) fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            self.start_requested = true;
            self.shown = true;
            cx.notify();
            return;
        }
        if self.error.is_some() || self.status.error().is_some() {
            self.stop(window, cx);
            if let Err(error) = razer_assets::retry_resources() {
                self.error = Some(format!("{error:#}"));
            }
        }
        if let Some(session) = &self.session {
            let result = session.toggle();
            if let Err(error) = result {
                self.error = Some(format!("{error:#}"));
                self.report_error(window, cx);
                cx.notify();
            }
            return;
        }
        self.start_requested = true;
        self.shown = true;
        self.load(window, cx);
        self.focus.focus(window, cx);
    }

    pub(super) fn preload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clip.is_none() && !self.loading && self.error.is_none() {
            self.load(window, cx);
        }
    }

    pub(super) fn leave_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.exit_fullscreen(window);
        cx.notify();
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let assets = cx.asset_source().clone();
        let retained = self.clip.clone();
        let resource_ready = self.resource_ready.clone();
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let clip = match retained {
                        Some(clip) => clip,
                        None => {
                            let bytes = loop {
                                match assets.load("synapse/audio-mode.rzav") {
                                    Err(error) if razer_assets::is_resource_pending(&error) => {
                                        resource_ready.recv().await.map_err(|_| {
                                            anyhow::anyhow!("Audible resource loading cancelled")
                                        })?;
                                    }
                                    result => {
                                        break result?.ok_or_else(|| {
                                            anyhow::anyhow!("Packaged audible demo is missing")
                                        })?;
                                    }
                                }
                            };
                            Arc::new(Clip::parse(Arc::from(bytes.into_owned()))?)
                        }
                    };
                    Ok::<_, anyhow::Error>(clip)
                })
                .await;
            let started = this.update_in(cx, |this, window, cx| {
                if this.generation != generation {
                    return None;
                }
                match result {
                    Ok(clip) => {
                        this.clip = Some(clip);
                    }
                    Err(error) => {
                        this.error = Some(format!("{error:#}"));
                        this.shown = false;
                    }
                }
                let start = this.start_requested;
                this.loading = start && this.clip.is_some();
                this.report_error(window, cx);
                cx.notify();
                start.then(|| this.clip.clone()).flatten()
            });
            let Ok(Some(clip)) = started else {
                return;
            };
            let output = cx
                .background_spawn(async move { Session::prepare(clip) })
                .await;
            let started = this.update_in(cx, |this, window, cx| {
                if this.generation != generation {
                    return false;
                }
                this.loading = false;
                match output {
                    Ok(session) => {
                        if let Err(error) = session
                            .set_volume(this.requested_volume)
                            .and_then(|_| session.set_playback_rate(this.requested_rate))
                        {
                            this.error = Some(format!("{error:#}"));
                        }
                        if this.start_requested {
                            if let Err(error) = session.play() {
                                this.error = Some(format!("{error:#}"));
                            }
                        }
                        this.session = Some(session);
                    }
                    Err(error) => {
                        this.error = Some(format!("{error:#}"));
                        this.shown = false;
                    }
                }
                this.start_requested = false;
                this.report_error(window, cx);
                cx.notify();
                this.session.is_some()
            });
            if !matches!(started, Ok(true)) {
                return;
            }
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
                let next = this.update_in(cx, |this, window, cx| {
                    if this.generation != generation {
                        return None;
                    }
                    let clip = this.clip.clone()?;
                    let status = match this.session.as_ref()?.status() {
                        Ok(status) => status,
                        Err(error) => {
                            this.error = Some(format!("{error:#}"));
                            this.report_error(window, cx);
                            cx.notify();
                            return None;
                        }
                    };
                    if let Some(error) = status.error() {
                        if this.status.error() != Some(error) {
                            eprintln!("Audible demo output: {error}");
                        }
                        this.error = Some(error.to_owned());
                        this.report_error(window, cx);
                    }
                    // 1392/1442/3942 onEnded explicitly restarts; 1352 does not.
                    if status.is_ended() && !this.status.is_ended() && this.product != 1352 {
                        if let Err(error) = this.session.as_ref()?.play() {
                            this.error = Some(format!("{error:#}"));
                        }
                    }
                    let changed = status.cursor() != this.status.cursor()
                        || status.is_playing() != this.status.is_playing()
                        || status.is_ended() != this.status.is_ended()
                        || status.error() != this.status.error()
                        || status.volume() != this.status.volume()
                        || status.playback_rate() != this.status.playback_rate();
                    this.has_started |= status.is_playing();
                    if status.seek_revision() < this.seek_revision || this.scrubbing.is_some() {
                        return Some((clip.clone(), 0, false, this.seek_revision));
                    }
                    let ix = clip.frame_index(status.cursor());
                    if changed {
                        this.progress.update(cx, |progress, cx| {
                            progress.set_value(
                                (status.cursor() as f64 / clip.sample_count() as f64 * 100.) as f32,
                                window,
                                cx,
                            )
                        });
                    }
                    this.status = status;
                    this.report_error(window, cx);
                    if changed {
                        cx.notify();
                    }
                    Some((
                        clip.clone(),
                        ix,
                        this.frame_ix != Some(ix),
                        this.seek_revision,
                    ))
                });
                let Ok(Some((clip, ix, decode, revision))) = next else {
                    break;
                };
                if !decode {
                    continue;
                }
                let image = cx
                    .background_spawn(async move {
                        let raw = clip
                            .jpeg(ix)
                            .ok_or_else(|| anyhow::anyhow!("Demo frame is absent"))?;
                        let mut rgba =
                            image::load_from_memory_with_format(raw, image::ImageFormat::Jpeg)?
                                .to_rgba8();
                        anyhow::ensure!(
                            rgba.width() == clip.width() && rgba.height() == clip.height(),
                            "Demo frame dimensions differ"
                        );
                        // GPUI uploads BGRA, including on the camera preview seam.
                        for pixel in rgba.pixels_mut() {
                            pixel.0.swap(0, 2);
                        }
                        Ok::<_, anyhow::Error>(Arc::new(RenderImage::new(vec![image::Frame::new(
                            rgba,
                        )])))
                    })
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.generation != generation
                        || this.seek_revision != revision
                        || this.scrubbing.is_some()
                    {
                        return;
                    }
                    match image {
                        Ok(image) => {
                            this.frame_ix = Some(ix);
                            this.frame = Some(image);
                            cx.notify();
                        }
                        Err(error) => {
                            this.error = Some(format!("{error:#}"));
                            if let Some(session) = &this.session {
                                let _ = session.pause();
                            }
                            this.report_error(window, cx);
                            cx.notify();
                        }
                    }
                });
            }
        }));
        cx.notify();
    }

    fn exit_fullscreen(&mut self, window: &mut Window) {
        if self.fullscreen && !self.window_was_fullscreen && window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        self.fullscreen = false;
    }

    fn report_error(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let error = self.error.as_deref().or_else(|| self.status.error());
        if error != self.reported_error.as_deref() {
            if let Some(error) = error {
                window.push_notification(format!("音频演示：{error}"), cx);
            }
            self.reported_error = error.map(str::to_owned);
        }
    }

    fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fullscreen {
            self.exit_fullscreen(window);
        } else {
            self.window_was_fullscreen = window.is_fullscreen();
            if !self.window_was_fullscreen {
                window.toggle_fullscreen();
            }
            self.fullscreen = true;
        }
        self.focus.focus(window, cx);
        cx.emit(MediaPresentationChanged);
        cx.notify();
    }

    fn seek_to(&mut self, cursor: u64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_started {
            return;
        }
        if let (Some(clip), Some(session)) = (&self.clip, &self.session) {
            match session.seek(cursor.min(clip.sample_count())) {
                Ok(revision) => {
                    self.seek_revision = revision;
                    self.frame_ix = None;
                }
                Err(error) => self.error = Some(format!("{error:#}")),
            }
        }
        self.report_error(window, cx);
        cx.notify();
    }

    fn seek_by(&mut self, seconds: i64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(clip) = &self.clip {
            let delta = seconds * i64::from(clip.sample_rate());
            self.seek_to(
                self.status.cursor().saturating_add_signed(delta),
                window,
                cx,
            );
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && self.fullscreen {
            self.toggle_fullscreen(window, cx);
            cx.stop_propagation();
            return;
        }
        if !self.shown || !self.focus.is_focused(window) {
            return;
        }
        let mut handled = true;
        match event.keystroke.key.to_lowercase().as_str() {
            "space" | "k" => self.toggle(window, cx),
            "f" => self.toggle_fullscreen(window, cx),
            "left" => self.seek_by(-5, window, cx),
            "j" => self.seek_by(-10, window, cx),
            "right" => self.seek_by(5, window, cx),
            "l" => self.seek_by(10, window, cx),
            "home" => self.seek_to(0, window, cx),
            "end" => {
                if let Some(clip) = &self.clip {
                    self.seek_to(clip.sample_count(), window, cx);
                }
            }
            "up" | "down" => {
                let step = if event.keystroke.key == "up" { 0.05 } else { -0.05 };
                let volume = (self.requested_volume + step).clamp(0., 1.);
                let result = self
                    .session
                    .as_ref()
                    .map_or(Ok(()), |session| session.set_volume(volume));
                match result {
                    Ok(()) => self.requested_volume = volume,
                    Err(error) => self.error = Some(format!("{error:#}")),
                }
            }
            "." | ">" | "," | "<" if event.keystroke.modifiers.shift => {
                let rate = if matches!(event.keystroke.key.as_str(), "." | ">") {
                    self.requested_rate.faster()
                } else {
                    self.requested_rate.slower()
                };
                let result = self
                    .session
                    .as_ref()
                    .map_or(Ok(()), |session| session.set_playback_rate(rate));
                match result {
                    Ok(()) => self.requested_rate = rate,
                    Err(error) => self.error = Some(format!("{error:#}")),
                }
            }
            _ => handled = false,
        }
        if handled {
            self.report_error(window, cx);
            cx.stop_propagation();
        }
    }

    fn player(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let custom = self.product != 1352;
        div()
            .id("audible-source-player")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(Colors::video_background())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::handle_key))
            .child(
                div()
                    .id("audible-demo-video")
                    .size_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.focus.focus(window, cx);
                        }),
                    )
                    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                        if this.shown && this.frame.is_some() && this.focus.is_focused(window) {
                            this.toggle(window, cx);
                            if event.click_count() == 2 {
                                this.toggle_fullscreen(window, cx);
                            }
                        }
                    }))
                    .child(match self.frame.as_ref().filter(|_| self.shown) {
                        Some(frame) => img(frame.clone())
                            .size_full()
                            .object_fit(ObjectFit::Contain)
                            .into_any_element(),
                        None => img(self.poster.clone())
                            .size_full()
                            .object_fit(match self.object_fit {
                                ObjectFit::Fill => ObjectFit::Fill,
                                ObjectFit::Contain => ObjectFit::Contain,
                                ObjectFit::Cover => ObjectFit::Cover,
                                ObjectFit::ScaleDown => ObjectFit::ScaleDown,
                                ObjectFit::None => ObjectFit::None,
                            })
                            .into_any_element(),
                    }),
            )
            .when(!self.shown, |player| {
                player.child(
                    gpui_kit::base::Button::new("audible-demo-start")
                        .absolute()
                        .inset_0()
                        .p_0()
                        .accessibility_label("Play demo video")
                        .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .pb(surface::css(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .size(surface::css(100.))
                                        .rounded_full()
                                        .border_3()
                                        .border_color(Colors::play_foreground())
                                        .bg(Colors::play_background())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            img(if custom {
                                                "synapse/audio-demo-play.svg"
                                            } else {
                                                "synapse/audio-1352-play.svg"
                                            })
                                            .w(surface::css(if custom { 48. } else { 25. }))
                                            .when(custom, |icon| icon.h(surface::css(48.)))
                                            .ml(surface::css(2.)),
                                        ),
                                ),
                        ),
                )
            })
            .when(self.shown && self.status.error().is_none(), |player| {
                player.child(
                    h_flex()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(surface::css(if custom { 40. } else { 30. }))
                        .justify_between()
                        .bg(if custom {
                            Colors::control_background()
                        } else {
                            Colors::default_control_background()
                        })
                        .when(custom, |bar| bar.rounded(surface::css(5.)))
                        .child(
                            div()
                                .id("audible-demo-progress-wrapper")
                                .track_focus(&self.progress_focus)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.progress_focus.focus(window, cx);
                                    }),
                                )
                                .on_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, window, cx| {
                                        match event.keystroke.key.as_str() {
                                            "left" | "down" => this.seek_by(-5, window, cx),
                                            "right" | "up" => this.seek_by(5, window, cx),
                                            _ => return,
                                        }
                                        cx.stop_propagation();
                                    },
                                ))
                                .absolute()
                                .left_0()
                                .right_0()
                                .when(custom, |progress| progress.bottom(surface::css(16.)))
                                .when(!custom, |progress| progress.top(surface::css(-15.)))
                                .group("audible-demo-progress")
                                .h(surface::css(8.))
                                .flex()
                                .items_center()
                                .child(
                                    BaseSlider::new(&self.progress)
                                        .size_full()
                                        .disabled(self.session.is_none())
                                        .child(
                                            SliderTrack::new(&self.progress)
                                                .disabled(self.session.is_none())
                                                .w_full()
                                                .h_full()
                                                .flex()
                                                .items_center()
                                                .child(
                                                    SliderIndicator::new(&self.progress)
                                                        .relative()
                                                        .w_full()
                                                        .h(surface::css(2.))
                                                        .bg(rgb(0x555555))
                                                        .group_hover(
                                                            "audible-demo-progress",
                                                            |track| track.h(surface::css(4.)),
                                                        )
                                                        .child(
                                                            div()
                                                                .absolute()
                                                                .left_0()
                                                                .top_0()
                                                                .h_full()
                                                                .w(relative(
                                                                    self.progress
                                                                        .read(cx)
                                                                        .value()
                                                                        .start()
                                                                        / 100.,
                                                                ))
                                                                .bg(rgb(0x44d62c)),
                                                        ),
                                                ),
                                        ),
                                ),
                        )
                        .child(
                            gpui_kit::base::Button::new("audible-demo-toggle")
                                .w(surface::css(self.button_width))
                                .h_full()
                                .p_0()
                                .disabled(self.loading)
                                .accessibility_label(if self.status.is_playing() {
                                    "Pause"
                                } else {
                                    "Play"
                                })
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.toggle(window, cx)),
                                )
                                .child(
                                    img(if self.status.is_playing() {
                                        "synapse/audio-demo-control-pause.svg"
                                    } else {
                                        "synapse/audio-demo-control-play.svg"
                                    })
                                    .size(surface::css(self.glyph_size)),
                                ),
                        )
                        .child(
                            gpui_kit::base::Button::new("audible-demo-fullscreen")
                                .w(surface::css(self.button_width))
                                .h_full()
                                .p_0()
                                .accessibility_label(if self.fullscreen {
                                    "Exit fullscreen"
                                } else {
                                    "Fullscreen"
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_fullscreen(window, cx)
                                }))
                                .child(
                                    img(if !custom && !self.fullscreen {
                                        "synapse/audio-1352-fullscreen.svg"
                                    } else if self.fullscreen {
                                        "synapse/audio-demo-control-fullscreen-exit.svg"
                                    } else {
                                        "synapse/audio-demo-control-fullscreen.svg"
                                    })
                                    .size(surface::css(
                                        if !custom && !self.fullscreen {
                                            14.
                                        } else {
                                            self.glyph_size
                                        },
                                    )),
                                ),
                        ),
                )
            })
            .when(self.shown && self.loading, |player| {
                player.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(gpui_kit::component::spinner::Spinner::new()),
                )
            })
            .into_any_element()
    }
}

impl Render for AudioMedia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let inline = div()
            .id("audible-demo-frame")
            .relative()
            .w(surface::css(self.width))
            .h(surface::css(self.height));
        if self.fullscreen {
            inline
                .child(
                    razer_assets::deferred_resource(
                        gpui_kit::base::Positioner::corner(Anchor::TopLeft, point(px(0.), px(0.)))
                            .margin(px(0.))
                            .child(
                                div()
                                    .w(window.viewport_size().width)
                                    .h(window.viewport_size().height)
                                    .bg(Colors::video_background())
                                    .child(self.player(window, cx)),
                            ),
                    )
                    .with_priority(250),
                )
                .into_any_element()
        } else {
            inline.child(self.player(window, cx)).into_any_element()
        }
    }
}
