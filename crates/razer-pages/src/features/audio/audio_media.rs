//! The four current mounted video-react players share the same audible MOV.
//! The media clock comes from consumed native PCM frames, never a UI timer.
//! Full caller bodies and CSS are in the current audio-demo-audit query.
use gpui_kit::component::{ActiveTheme as _, Disableable as _, h_flex, slider::{Slider, SliderEvent, SliderState}};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_platform::media::{Clip, Session, Status};
use razer_widgets::{surface, theme::AudioDemoColors as Colors};
use std::{sync::Arc, time::Duration};

pub(super) struct AudioMedia {
    product: u32,
    poster: SharedString,
    width: f32,
    height: f32,
    clip: Option<Arc<Clip>>,
    session: Option<Session>,
    status: Status,
    shown: bool,
    loading: bool,
    generation: u64,
    frame_ix: Option<usize>,
    frame: Option<Arc<RenderImage>>,
    progress: Entity<SliderState>,
    focus: FocusHandle,
    fullscreen: bool,
    window_was_fullscreen: bool,
    _progress: Subscription,
    task: Option<Task<()>>,
}

impl AudioMedia {
    pub(super) fn new(product: u32, poster: impl Into<SharedString>, width: f32, height: f32, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let poster = poster.into();
        cx.new(|cx| {
            let progress = cx.new(|_| SliderState::new().max(100.).step(0.01));
            let subscription = cx.subscribe_in(&progress, window, |this, _, event, _, cx| {
                if let SliderEvent::Change(value) = event {
                    if let Some(clip) = &this.clip {
                        let target = (clip.sample_count() as f64 * f64::from(value.start().clamp(0.,100.)) / 100.).round() as u64;
                        if let Some(session) = &this.session {
                            if let Err(error) = session.seek(target.min(clip.sample_count())) {
                                eprintln!("Audible demo seek: {error:#}");
                            }
                        }
                    }
                }
                cx.notify();
            });
            Self { product, poster, width, height, clip: None, session: None, status: Status::default(),
                shown: false, loading: false, generation: 0, frame_ix: None, frame: None,
                progress, focus: cx.focus_handle().tab_stop(true), fullscreen: false,
                window_was_fullscreen: false, _progress: subscription, task: None }
        })
    }

    pub(super) fn is_shown(&self) -> bool { self.shown }
    pub(super) fn stop(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        if let Some(mut session) = self.session.take() {
            if let Err(error) = session.stop() { eprintln!("Audible demo cleanup: {error:#}"); }
        }
        self.shown = false;
        self.loading = false;
        self.status = Status::default();
        self.frame = None;
        self.frame_ix = None;
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading { return; }
        if let Some(session) = &self.session {
            let result = if self.status.is_playing() { session.pause() } else { session.play() };
            if let Err(error) = result { eprintln!("Audible demo command: {error:#}"); }
            return;
        }
        self.shown = true;
        self.loading = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let assets = cx.asset_source().clone();
        let retained = self.clip.clone();
        self.focus.focus(window);
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                let clip = match retained {
                    Some(clip) => clip,
                    None => {
                        let bytes = assets.load("synapse/audio-mode.rzav")?
                            .ok_or_else(||anyhow::anyhow!("Packaged audible demo is missing"))?;
                        Arc::new(Clip::parse(Arc::from(bytes.into_owned()))?)
                    }
                };
                let session = Session::start(clip.clone())?;
                Ok::<_,anyhow::Error>((clip,session))
            }).await;
            let started = this.update_in(cx, |this, _, cx| {
                if this.generation != generation { return false; }
                this.loading = false;
                match result {
                    Ok((clip,session)) => { this.clip=Some(clip); this.session=Some(session); },
                    Err(error) => { eprintln!("Audible demo load: {error:#}"); this.shown=false; }
                }
                cx.notify();
                this.session.is_some()
            });
            if !matches!(started,Ok(true)) { return; }
            loop {
                cx.background_executor().timer(Duration::from_millis(10)).await;
                let next = this.update_in(cx, |this, window, cx| {
                    if this.generation != generation { return None; }
                    let clip = this.clip.as_ref()?;
                    let status = match this.session.as_ref()?.status() {
                        Ok(status) => status,
                        Err(error) => { eprintln!("Audible demo state: {error:#}"); return None; }
                    };
                    if let Some(error) = status.error() {
                        if this.status.error() != Some(error) { eprintln!("Audible demo output: {error}"); }
                    }
                    // 1392/1442/3942 onEnded explicitly restarts; 1352 does not.
                    if status.is_ended() && !this.status.is_ended() && this.product != 1352 {
                        if let Err(error) = this.session.as_ref()?.play() { eprintln!("Audible demo loop: {error:#}"); }
                    }
                    let changed = status.cursor() != this.status.cursor()
                        || status.is_playing() != this.status.is_playing()
                        || status.is_ended() != this.status.is_ended()
                        || status.error() != this.status.error();
                    let ix = clip.frame_index(status.cursor());
                    this.progress.update(cx, |progress,cx|progress.set_value((status.cursor() as f64 / clip.sample_count() as f64 * 100.) as f32,window,cx));
                    this.status = status;
                    if changed { cx.notify(); }
                    Some((clip.clone(), ix, this.frame_ix != Some(ix)))
                });
                let Ok(Some((clip,ix,decode))) = next else { break; };
                if !decode { continue; }
                let image = cx.background_spawn(async move {
                    let raw = clip.jpeg(ix).ok_or_else(||anyhow::anyhow!("Demo frame is absent"))?;
                    let mut rgba = image::load_from_memory_with_format(raw,image::ImageFormat::Jpeg)?.to_rgba8();
                    anyhow::ensure!(rgba.width()==clip.width() && rgba.height()==clip.height(),"Demo frame dimensions differ");
                    // GPUI uploads BGRA, including on the camera preview seam.
                    for pixel in rgba.as_mut().chunks_exact_mut(4) { pixel.swap(0,2); }
                    Ok::<_,anyhow::Error>(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
                }).await;
                let _ = this.update_in(cx, |this, _, cx| {
                    if this.generation != generation { return; }
                    match image {
                        Ok(image) => { this.frame_ix=Some(ix); this.frame=Some(image); cx.notify(); },
                        Err(error) => eprintln!("Audible demo frame: {error:#}"),
                    }
                });
            }
        }));
        cx.notify();
    }

    fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fullscreen {
            if !self.window_was_fullscreen && window.is_fullscreen() { window.toggle_fullscreen(); }
            self.fullscreen=false;
        } else {
            self.window_was_fullscreen=window.is_fullscreen();
            if !self.window_was_fullscreen { window.toggle_fullscreen(); }
            self.fullscreen=true;
        }
        self.focus.focus(window);
        cx.notify();
    }

    fn player(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let custom = self.product != 1352;
        div().id("audible-source-player").relative().size_full().overflow_hidden()
            .bg(Colors::video_background())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this,event:&KeyDownEvent,window,cx| {
                match event.keystroke.key.as_str() {
                    "escape" if this.fullscreen => {this.toggle_fullscreen(window,cx);cx.stop_propagation();},
                    "space" if this.focus.is_focused(window) => {this.toggle(window,cx);cx.stop_propagation();},
                    _=>{}
                }
            }))
            .child(match &self.frame {
                Some(frame) => img(frame.clone()).size_full().object_fit(ObjectFit::Contain).into_any_element(),
                None => img(self.poster.clone()).size_full().object_fit(if self.product==3942 {ObjectFit::Cover}else{ObjectFit::Fill}).into_any_element(),
            })
            .when(!self.shown, |player|player.child(
                gpui_kit::base::Button::new("audible-demo-start").absolute().inset_0().p_0()
                    .accessibility_label("Play demo video")
                    .on_click(cx.listener(|this,_,window,cx|this.toggle(window,cx)))
                    .child(div().absolute().inset_0().pb(surface::css(28.)).flex().items_center().justify_center()
                        .child(div().size(surface::css(100.)).rounded_full().border_3()
                            .border_color(Colors::play_foreground()).bg(Colors::play_background())
                            .flex().items_center().justify_center()
                            .child(img("synapse/audio-demo-play.svg").size(surface::css(48.)).ml(surface::css(2.)))))
            ))
            .when(self.shown && self.status.error().is_none(), |player|player.child(
                h_flex().absolute().left_0().right_0().bottom_0().h(surface::css(if custom {40.}else{30.}))
                    .justify_between().bg(if custom {Colors::control_background()} else {Colors::default_control_background()})
                    .when(custom,|bar|bar.rounded(surface::css(5.)))
                    .child(Slider::new(&self.progress).absolute().left_0().right_0()
                        .top(surface::css(if custom {-2.}else{-15.})).h(surface::css(8.))
                        .bg(Colors::play_foreground()).text_color(Colors::play_foreground()).disabled(self.session.is_none()))
                    .child(gpui_kit::base::Button::new("audible-demo-toggle").w(surface::css(40.)).h_full().p_0()
                        .disabled(self.loading).accessibility_label(if self.status.is_playing(){"Pause"}else{"Play"})
                        .on_click(cx.listener(|this,_,window,cx|this.toggle(window,cx)))
                        .child(img(if self.status.is_playing(){"synapse/audio-demo-control-pause.svg"}else{"synapse/audio-demo-control-play.svg"}).size(surface::css(18.))))
                    .child(gpui_kit::base::Button::new("audible-demo-fullscreen").w(surface::css(40.)).h_full().p_0()
                        .accessibility_label(if self.fullscreen {"Exit fullscreen"} else {"Fullscreen"})
                        .on_click(cx.listener(|this,_,window,cx|this.toggle_fullscreen(window,cx)))
                        .child(img(if self.fullscreen{"synapse/audio-demo-control-fullscreen-exit.svg"}else{"synapse/audio-demo-control-fullscreen.svg"}).size(surface::css(18.))))
            )).into_any_element()
    }
}

impl Render for AudioMedia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let inline = div().id("audible-demo-frame").relative().w(surface::css(self.width)).h(surface::css(self.height));
        if self.fullscreen {
            inline.child(deferred(gpui_kit::base::Positioner::corner(Anchor::TopLeft,point(px(0.),px(0.)))
                .margin(px(0.)).child(div().w(window.viewport_size().width).h(window.viewport_size().height)
                    .bg(Colors::video_background()).child(self.player(window,cx)))).with_priority(250)).into_any_element()
        } else { inline.child(self.player(window,cx)).into_any_element() }
    }
}
