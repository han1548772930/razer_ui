//! 1383 `xx -> Dv -> Hp/Bp`, from its own current manifest. Source defaults are
//! local preview data, never connected-device telemetry or a transport reply.
use super::*;
use gpui_kit::base::Button as BaseButton;
use std::time::Instant;
#[path = "audio_oled_home_theme.rs"]
mod theme;
use theme::Colors;
#[path = "audio_oled_artwork.rs"]
mod artwork;
#[path = "audio_oled_banner.rs"]
mod banner;
#[path = "audio_oled_dialog.rs"]
mod dialog;
#[path = "audio_oled_media.rs"]
mod media;
#[path = "audio_oled_runtime.rs"]
mod runtime;
#[path = "audio_oled_system.rs"]
mod system;
#[path = "audio_oled_tooltip.rs"]
mod tooltip;
pub use runtime::{OledRuntimeObservation, OledRuntimeRequested};

#[derive(Deserialize)]
struct Spec {
    labels: BTreeMap<String, String>,
    home: Value,
    media: Value,
    visualizers: Vec<String>,
    headset: Vec<String>,
    battery_default: u32,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("audio_oled_home_data.json"))
            .expect("validated 1383 OLED home data")
    })
}
fn label(symbol: &str) -> String {
    t(&spec().labels[symbol])
}
pub(super) struct OledHomeState {
    started: Instant,
    editor: Option<AnyView>,
    runtime: runtime::OledRuntimeState,
}
impl OledHomeState {
    pub(super) fn new(pid: u32) -> Option<Self> {
        (pid == 1383).then(|| Self {
            started: Instant::now(),
            editor: None,
            runtime: runtime::OledRuntimeState::new(),
        })
    }
}
impl AudioProductWorkspace {
    pub(super) fn initialize_oled_home(&mut self, window: &Window) {
        if self.spec.product_id == 1383 {
            self.draft["oledHome"] = json!({
                "home":spec().home,"media":spec().media,
                "animation": artwork::default_value(0),
                "image": artwork::default_value(1),
                "emote": artwork::default_value(4),
                "system": system::default_value(),
                "banner": banner::default_value(),
            });
            let duration = banner::measure_duration_ms(&self.draft["oledHome"]["banner"], window);
            self.draft["oledHome"]["banner"]["local_duration_ms"] = json!(duration);
            if let Some(state) = &mut self.oled_home {
                state.started = Instant::now();
                state.editor = None;
            }
        }
    }
    pub(super) fn restore_oled_home_saved(&mut self, saved: &Value) {
        // Empty System Info slots are null. The general scalar-only merge
        // cannot restore them; this module's normalizer rebuilds known tags.
        if self.spec.product_id == 1383 {
            // Banner's optional font attributes also transition through null.
            for key in ["system", "banner"] {
                if saved["oledHome"][key].is_object() {
                    self.draft["oledHome"][key] = saved["oledHome"][key].clone();
                }
            }
        }
    }
    pub(super) fn normalize_oled_home(&mut self, window: &Window) {
        if self.spec.product_id != 1383 {
            return;
        }
        let home = &mut self.draft["oledHome"]["home"];
        if !home["selected"].as_u64().is_some_and(|v| v <= 6) {
            home["selected"] = spec().home["selected"].clone();
        }
        let media = &mut self.draft["oledHome"]["media"];
        if !matches!(media["info"]["selected"].as_str(), Some("top" | "bottom")) {
            media["info"]["selected"] = json!("top");
        }
        if !media["visualizer"]["selected"]
            .as_u64()
            .is_some_and(|v| v < 3)
        {
            media["visualizer"]["selected"] = json!(0);
        }
        if media["info"]["enabled"] == false && media["visualizer"]["enabled"] == false {
            media["info"]["enabled"] = json!(true);
        }
        for (mode, key) in [(0, "animation"), (1, "image"), (4, "emote")] {
            artwork::normalize(mode, &mut self.draft["oledHome"][key]);
        }
        system::normalize(&mut self.draft["oledHome"]["system"]);
        banner::normalize(&mut self.draft["oledHome"]["banner"]);
        let duration = banner::measure_duration_ms(&self.draft["oledHome"]["banner"], window);
        self.draft["oledHome"]["banner"]["local_duration_ms"] = json!(duration);
    }
    fn oled_home_apply(&mut self, mode: u32, cx: &mut Context<Self>) {
        if self.draft["oledHome"]["home"]["enabled"] != true
            || self.oled_is_loading()
            || (self.oled_is_ble() && matches!(mode, 5 | 6))
        {
            return;
        }
        self.draft["oledHome"]["home"]["selected"] = json!(mode);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    pub(super) fn oled_home_dialog(&self) -> Option<AnyElement> {
        self.oled_home
            .as_ref()?
            .editor
            .as_ref()
            .map(|editor| editor.clone().into_any_element())
    }
    pub(super) fn kraken_oled_home(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = self.draft["oledHome"]["home"]["enabled"] == true;
        let loading = self.oled_is_loading();
        let is_ble = self.oled_is_ble();
        let mut ble_tooltip = false;
        let selected = self.draft["oledHome"]["home"]["selected"]
            .as_u64()
            .unwrap_or(0) as u32;
        let narrow = window.viewport_size().width <= window.rem_size() * (1279. / 16.);
        window.request_animation_frame();
        let mut cards = h_flex()
            .relative()
            .flex_wrap()
            .gap(surface::css(10.))
            .mt(surface::css(20.));
        for (mode, name, title) in [
            (0, "animation", "RG"),
            (1, "image", "KTY"),
            (4, "emote", "Nj5"),
            (2, "banner", "$Mz"),
            (5, "media", "rwQ"),
            (6, "system", "FSz"),
            (3, "headset", "w23"),
        ] {
            let blocked = is_ble && matches!(mode, 5 | 6);
            let active = selected == mode;
            let hover = window.use_keyed_state(
                (ElementId::from(format!("1383-oled-{name}")), "hover"),
                cx,
                |_, _| false,
            );
            let hovered = enabled && !loading && !blocked && *hover.read(cx);
            let ble_hover = window.use_keyed_state(
                (ElementId::from(format!("1383-oled-{name}")), "ble-hover"),
                cx,
                |_, _| false,
            );
            ble_tooltip |= enabled && !loading && blocked && *ble_hover.read(cx);
            let mut header = h_flex()
                .h(surface::css(19.))
                .p(surface::css(2.))
                .gap(surface::css(7.))
                .text_size(surface::css(14.))
                .text_color(if hovered {
                    Colors::hovered()
                } else if active {
                    Colors::selected()
                } else {
                    Colors::text()
                })
                .child(label(title).to_uppercase());
            if matches!(mode, 5 | 6) {
                header = header.child(if blocked {
                    img("synapse/audio-oled-1383-requires-synapse.svg")
                        .size(surface::css(15.))
                        .into_any_element()
                } else {
                    tooltip::requires_synapse(format!("1383-oled-requires-{name}"), window, cx)
                });
            }

            let mut overlay = h_flex()
                .absolute()
                .left_0()
                // Header is 19px; the source overlay begins at card y=18px.
                .top(surface::css(-1.))
                .w(surface::css(236.))
                .h(surface::css(68.))
                .justify_around()
                .bg(Colors::overlay())
                .border_2()
                .border_color(Colors::hovered())
                .occlude();
            if mode != 3 {
                overlay = overlay.child(
                    card_action(
                        format!("1383-oled-edit-{name}"),
                        "zxY",
                        "synapse/audio-oled-1383-edit.svg",
                        false,
                        false,
                        is_ble,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if mode == 5 {
                            let draft = this.draft["oledHome"]["media"].clone();
                            let editor = media::MediaEditor::open(
                                cx.entity().downgrade(),
                                draft,
                                window,
                                cx,
                            );
                            if let Some(state) = &mut this.oled_home {
                                state.editor = Some(editor.into());
                            }
                            cx.notify();
                        } else if mode == 2 {
                            let editor = banner::BannerEditor::open(
                                cx.entity().downgrade(),
                                this.draft["oledHome"]["banner"].clone(),
                                window,
                                cx,
                            );
                            if let Some(state) = &mut this.oled_home {
                                state.editor = Some(editor.into());
                            }
                            cx.notify();
                        } else if mode == 6 {
                            let editor = system::SystemEditor::open(
                                cx.entity().downgrade(),
                                this.draft["oledHome"]["system"].clone(),
                                window,
                                cx,
                            );
                            if let Some(state) = &mut this.oled_home {
                                state.editor = Some(editor.into());
                            }
                            cx.notify();
                        } else if matches!(mode, 0 | 1 | 4) {
                            let draft = this.draft["oledHome"][name].clone();
                            let editor = artwork::ArtworkEditor::open(
                                cx.entity().downgrade(),
                                mode,
                                draft,
                                window,
                                cx,
                            );
                            if let Some(state) = &mut this.oled_home {
                                state.editor = Some(editor.into());
                            }
                            cx.notify();
                        }
                    })),
                );
            }
            overlay = overlay.child(
                card_action(
                    format!("1383-oled-apply-{name}"),
                    "pJk",
                    "synapse/audio-oled-1383-apply.svg",
                    true,
                    active,
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.oled_home_apply(mode, cx))),
            );
            let border = if hovered {
                Colors::hovered()
            } else if active {
                Colors::selected()
            } else {
                Colors::border()
            };
            let body = v_flex()
                .w(surface::css(if active || hovered { 236. } else { 234. }))
                .h(surface::css(if active || hovered { 68. } else { 66. }))
                .m(surface::css(if active || hovered { 0. } else { 1. }))
                .border_1()
                .when(active || hovered, |el| el.border_2())
                .border_color(border)
                .bg(Colors::black())
                .justify_center()
                .child(self.oled_preview(mode, window));
            cards = cards.child(
                v_flex()
                    .id(SharedString::from(format!("1383-oled-card-{name}")))
                    .relative()
                    .w(surface::css(236.))
                    .flex_shrink_0()
                    .when(blocked, |el| el.opacity(0.5))
                    .on_hover(window.listener_for(&ble_hover, |state, hovered, _, cx| {
                        *state = *hovered;
                        cx.notify();
                    }))
                    .child(header)
                    .child(
                        div()
                            .id(SharedString::from(format!("1383-oled-body-{name}")))
                            .relative()
                            .w(surface::css(236.))
                            .h(surface::css(68.))
                            .on_hover(window.listener_for(&hover, |state, hovered, _, cx| {
                                *state = *hovered;
                                cx.notify();
                            }))
                            .child(body)
                            .when(hovered, |el| el.child(overlay)),
                    ),
            );
        }
        if !enabled {
            cards = cards.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(Colors::panel())
                    .opacity(0.8)
                    .occlude(),
            );
        }
        surface::panel_with_title_switch(
            label("JQR"),
            surface::SynapseSwitch::new("1383-oled-home-enabled")
                .accessibility_label(label("JQR"))
                .checked(enabled)
                .on_change(cx.listener(|this, next: &bool, _, cx| {
                    if this.oled_is_loading() {
                        return;
                    }
                    this.draft["oledHome"]["home"]["enabled"] = json!(*next);
                    cx.emit(AudioProductChanged);
                    cx.notify();
                })),
            div(),
            cx,
        )
        .relative()
        .w(surface::css(if narrow { 600. } else { 1220. }))
        .min_w(surface::css(if narrow { 600. } else { 1220. }))
        .max_w(surface::css(if narrow { 600. } else { 1220. }))
        .mx_auto()
        .when(narrow, |panel| panel.px(surface::css(25.)))
        .child(super::oled::kraken_oled_help("home", &spec().labels["O0W"]))
        .child(div().child(label("Tcb")))
        .child(cards)
        .when(ble_tooltip, |panel| {
            panel.child(
                deferred(
                    div()
                        .absolute()
                        .left(surface::css(20.))
                        .top(surface::css(185.))
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .bg(Colors::black())
                        .border_1()
                        .border_color(Colors::border())
                        .text_color(Colors::text())
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .child(runtime::ble_text())
                        .with_animation(
                            "1383-oled-ble-card-tip",
                            Animation::new(std::time::Duration::from_millis(300)),
                            |el, delta| el.opacity(delta),
                        ),
                )
                .with_priority(107),
            )
        })
        .when(loading, |panel| {
            panel
                .opacity(0.3)
                .child(div().absolute().inset_0().occlude())
        })
        .into_any_element()
    }
    fn oled_preview(&self, mode: u32, window: &Window) -> AnyElement {
        match mode {
            0 => artwork::preview(mode, &self.draft["oledHome"]["animation"]),
            1 => artwork::preview(mode, &self.draft["oledHome"]["image"]),
            4 => artwork::preview(mode, &self.draft["oledHome"]["emote"]),
            2 => banner::preview(
                &self.draft["oledHome"]["banner"],
                self.oled_home
                    .as_ref()
                    .map_or(0., |state| state.started.elapsed().as_secs_f32()),
                window,
            ),
            5 => media_preview(&self.draft["oledHome"]["media"]),
            6 => system::preview(
                &self.draft["oledHome"]["system"],
                self.oled_home
                    .as_ref()
                    .map_or(0., |state| state.started.elapsed().as_secs_f32()),
                self.draft["device"]["oledLanguage"].as_u64().unwrap_or(0),
            ),
            3 => {
                let volume = self.draft["device"]["volume"]["value"]
                    .as_u64()
                    .unwrap_or(50);
                h_flex()
                    .mt(surface::css(15.))
                    .children(
                        [
                            format!("{volume}%"),
                            "PC".into(),
                            format!("{}%", spec().battery_default),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(ix, text)| {
                            h_flex()
                                .flex_1()
                                .px(surface::css(7.))
                                .justify_center()
                                .when(ix == 0, |el| el.justify_start())
                                .when(ix == 2, |el| el.justify_end())
                                .child(
                                    v_flex()
                                        .items_center()
                                        .child(
                                            img(SharedString::from(spec().headset[ix].clone()))
                                                .size(surface::css(24.)),
                                        )
                                        .child(
                                            div()
                                                .font_family("RazerF5")
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(text),
                                        ),
                                )
                        }),
                    )
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}

fn preview_image(asset: &str, height: f32) -> AnyElement {
    img(SharedString::from(asset.to_owned()))
        .w(surface::css(232.))
        .h(surface::css(height))
        .flex_shrink_0()
        .into_any_element()
}
fn media_preview(value: &Value) -> AnyElement {
    let info = value["info"]["enabled"] == true;
    let text = || div().child("(Track Title - Artiste Name)");
    v_flex()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .justify_center()
        .when(info && value["info"]["selected"] == "top", |el| {
            el.child(text())
        })
        .when(value["visualizer"]["enabled"] == true, |el| {
            el.child(preview_image(
                &spec().visualizers
                    [value["visualizer"]["selected"].as_u64().unwrap_or(0).min(2) as usize],
                44.,
            ))
        })
        .when(info && value["info"]["selected"] == "bottom", |el| {
            el.child(text())
        })
        .into_any_element()
}
fn card_action(
    id: String,
    label_symbol: &str,
    icon: &'static str,
    apply: bool,
    disabled: bool,
    muted: bool,
) -> BaseButton {
    let color = if disabled || muted {
        Colors::disabled()
    } else if apply {
        Colors::selected()
    } else {
        Colors::white()
    };
    BaseButton::new(SharedString::from(id))
        .accessibility_label(label(label_symbol))
        .disabled(disabled)
        .styles(|s| s.disabled(|s| s.opacity(1.)))
        .flex()
        .flex_col()
        .items_center()
        .p_0()
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .text_color(color)
        .child(
            svg()
                .path(icon)
                .size(surface::css(24.))
                .m(surface::css(8.))
                .text_color(color),
        )
        .child(label(label_symbol).to_uppercase())
}
