//! Current product 784's device layout and IoT card. Local edits are independent
//! of the unavailable device transport; only the explicit preview supplies observations.
use crate::{
    features::Choice,
    i18n,
    model::Device,
    ui::{surface, theme::AetherStripColors as Colors},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    select::SelectState,
    slider::Slider,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

mod device;
mod editor;
mod presentation;
mod preview;
mod source_tip;
mod state;
use device::{DeviceDialog, Peer};
pub(crate) use preview::open_preview;
use state::{Bend, Observation, configured};

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    page: String,
    translations: BTreeMap<String, BTreeMap<String, String>>,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("aether_strip_data.json"))
            .expect("audited Aether Strip source")
    })
}
fn text(key: &str) -> String {
    spec()
        .translations
        .get(&i18n::locale())
        .and_then(|m| m.get(key))
        .or_else(|| spec().translations.get("en").and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| i18n::t(key))
}
fn asset(name: &str) -> SharedString {
    format!("synapse/aether-{name}.svg").into()
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    pid == spec().product_id && key == spec().page
}
pub(crate) struct AetherStripChanged;
pub(crate) struct AetherStrip {
    edition: u32,
    peers: Vec<Peer>,
    selected: String,
    observation: Observation,
    bends: Vec<Bend>,
    inputs: Vec<Entity<InputState>>,
    rename_input: Entity<InputState>,
    renaming: bool,
    syncing: bool,
    preview: bool,
    alert: Option<String>,
    last_request: Option<Value>,
    modal: Option<Entity<DeviceDialog>>,
    /// Source `ol` keeps the identify badge disabled for 500 ms after power
    /// turns on. The task is replaced when a newer power observation arrives,
    /// matching the source effect's clearTimeout path.
    identify_ready: bool,
    identify_task: Option<Task<()>>,
    /// 轮播滚动句柄：源码 `H()` 在挂载与切换时把
    /// `scrollLeft` 设为 `carousel.scrollWidth / 5 * index`。
    carousel_scroll: ScrollHandle,
    subscriptions: Vec<Subscription>,
}
impl EventEmitter<AetherStripChanged> for AetherStrip {}
impl AetherStrip {
    pub(crate) fn new(device: &Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::create(
            device.edition_id,
            device.device_container_id.clone(),
            device.display_name(),
            window,
            cx,
        )
    }
    fn create(
        edition: u32,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let inputs: Vec<_> = (0..4)
            .map(|_| {
                cx.new(|cx| {
                    InputState::new(window, cx).validate(|text, _| {
                        text.len() <= 2 && text.bytes().all(|b| b.is_ascii_digit())
                    })
                })
            })
            .collect();
        let rename_input =
            cx.new(|cx| InputState::new(window, cx).validate(|text, _| text.chars().count() <= 32));
        let mut this = Self {
            edition,
            selected: id.clone(),
            peers: vec![Peer::new(id, name)],
            observation: Observation::default(),
            bends: vec![],
            inputs,
            rename_input,
            renaming: false,
            syncing: false,
            preview: false,
            alert: None,
            last_request: None,
            modal: None,
            identify_ready: false,
            identify_task: None,
            carousel_scroll: ScrollHandle::default(),
            subscriptions: vec![],
        };
        for id in 0..4u32 {
            this.subscriptions.push(cx.subscribe_in(
                &this.inputs[id as usize],
                window,
                move |this, _, event, window, cx| {
                    if !this.syncing
                        && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    {
                        this.commit_side(id, window, cx);
                    }
                },
            ));
        }
        this.subscriptions.push(cx.subscribe_in(
            &this.rename_input,
            window,
            |this, _, event, window, cx| {
                if !this.syncing
                    && this.renaming
                    && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                {
                    this.commit_name(window, cx);
                }
            },
        ));
        this
    }
    pub(crate) fn snapshot(&self) -> Value {
        json!({"bendData":self.bends})
    }
    pub(crate) fn restore(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss(window, cx);
        self.bends = state::restored(saved, self.observation.detected);
        self.sync_inputs(window, cx);
        cx.notify();
    }
    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = false;
        self.identify_task = None;
        self.identify_ready = false;
        if let Some(modal) = self.modal.take() {
            modal.update(cx, |modal, cx| modal.close(window, cx));
        }
        self.alert = None;
        cx.notify();
    }
    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (id, input) in self.inputs.iter().enumerate() {
            let value = self
                .bends
                .iter()
                .find(|b| b.id == id as u32)
                .map(|b| b.value.to_string())
                .unwrap_or_default();
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.syncing = false;
    }
    fn request(&mut self, action: &str, payload: Value, cx: &mut Context<Self>) {
        if self.preview {
            self.last_request = Some(json!({"type":action,"payload":payload}));
            self.alert = None;
        } else {
            self.alert = Some(i18n::t_or(
                "AETHER_SERVICE_UNAVAILABLE",
                "暂时无法连接灯带，请稍后重试。",
            ));
        }
        cx.notify();
    }
    fn choose_shape(&mut self, sides: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.observation.enabled() {
            return;
        }
        let bends = state::distribute(self.observation.detected.unwrap_or(0), sides);
        if bends.is_empty() {
            return;
        }
        self.bends = bends;
        self.sync_inputs(window, cx);
        self.request(
            "ON_SET_CHROMA_LED_NUMBER",
            json!({"bendData":self.bends,"configuredLedCount":configured(&self.bends)}),
            cx,
        );
        cx.emit(AetherStripChanged);
    }
    fn identify_side(&mut self, id: u32, cx: &mut Context<Self>) {
        if !self.observation.enabled() {
            return;
        }
        if let Some(payload) =
            state::identify_payload(&self.bends, id, self.observation.detected.unwrap_or(0))
        {
            self.request("ON_AETHER_IDENTIFY_LEDS", payload, cx);
        }
    }
    fn layout(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.observation.enabled();
        let count = configured(&self.bends);
        let detected = self.observation.detected.unwrap_or(0);
        let help = text("DEVICE_LAYOUT_TIP");
        let mut content = v_flex()
            .gap_0()
            .when(!enabled, |v| v.opacity(0.5))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .children((1..=4u32).map(|sides| {
                        let selected = self.bends.len().max(1) == sides as usize;
                        presentation::ShapeChoice {
                            sides,
                            selected,
                            enabled: enabled && detected >= sides,
                            owner: cx.entity().downgrade(),
                        }
                    })),
            )
            .child(div().my(surface::css(14.)).child(text("BEND_LED_STRIP")));
        if count > 0 {
            for bend in &self.bends {
                let id = bend.id;
                content = content.child(
                    h_flex()
                        .items_start()
                        .gap(surface::css(20.))
                        .mt(surface::css(5.))
                        .child(
                            img(asset(&format!("side-{}", id + 1)))
                                .w(surface::css(24.))
                                .h(surface::css(25.)),
                        )
                        .child(editor::LedInput {
                            input: self.inputs[id as usize].clone(),
                            owner: cx.entity().downgrade(),
                            id,
                            enabled,
                            value: bend.value,
                            max: bend.value.saturating_add(detected.saturating_sub(count)),
                        })
                        .child(
                            icon_button(
                                ("aether-identify-side", id),
                                "identify",
                                text("IDENTIFY_BEND"),
                                !enabled,
                                cx,
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.identify_side(id, cx)),
                            ),
                        ),
                );
            }
        }
        content = content
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .mt(surface::css(20.))
                    .when(count > 0, |v| v.child(div().w(surface::css(24.))))
                    .child(text("LEDSCONFIGURED").replace("{{number}}", &count.to_string())),
            )
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .mt(surface::css(5.))
                    .when(detected > 0, |v| v.child(div().w(surface::css(24.))))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                text("LEDSDETECTED").replace("{{number}}", &detected.to_string()),
                            )
                            .child(if self.observation.refreshing {
                                div()
                                    .size(surface::css(27.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(presentation::SourceSpinner)
                                    .into_any_element()
                            } else {
                                icon_button(
                                    "aether-refresh",
                                    "refresh",
                                    text("REFRESH_STRIP"),
                                    !enabled,
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request("ON_GET_HARDWARE_LED_INFO", json!({}), cx)
                                }))
                                .into_any_element()
                            }),
                    ),
            );
        let mut panel = v_flex()
            .relative()
            .w_full()
            .py(surface::css(30.))
            .px(surface::css(40.))
            .bg(cx.theme().group_box)
            .rounded(surface::css(5.))
            .text_size(surface::css(14.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(20.))
                    .when(!enabled, |v| v.opacity(0.5))
                    .child(text("DEVICE_LAYOUT").to_uppercase()),
            )
            .child(
                source_tip::TipCommand::help(help)
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.)),
            )
            .child(content);
        if self.observation.external_control() {
            panel = panel.child(
                div()
                    .mt(surface::css(14.))
                    .p(surface::css(20.))
                    .border_1()
                    .border_color(Colors::warning())
                    .rounded(surface::css(3.))
                    .child(text("SMART_HOME_APP_CONTROLLING")),
            );
        }
        if self.observation.unavailable() {
            panel = panel.child(
                surface::note(
                    i18n::t_or(
                        "AETHER_SERVICE_UNAVAILABLE",
                        "暂时无法连接灯带，请稍后重试。",
                    ),
                    cx,
                )
                .mt(surface::css(14.)),
            );
        }
        panel
            .when_some(self.alert.clone(), |v, alert| {
                v.child(surface::note(alert, cx).mt(surface::css(14.)))
            })
            .into_any_element()
    }
}
impl Render for AetherStrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .relative()
            .w_full()
            .child(self.device_carousel(cx))
            .child(surface::page_columns().child(surface::page_column(self.layout(cx))))
            .when(
                self.observation.locked == Some(true) || self.observation.online == Some(false),
                |v| {
                    v.child(
                        h_flex()
                            .w_full()
                            .justify_center()
                            .mt(surface::css(24.))
                            .mb(surface::css(50.))
                            .child(
                                div()
                                    .bg(Colors::background())
                                    .border_1()
                                    .border_color(Colors::warning())
                                    .rounded(surface::css(3.))
                                    .py(surface::css(20.))
                                    .px(surface::css(95.))
                                    .child(text(if self.observation.locked == Some(true) {
                                        "IOT_LOCKED_DESC"
                                    } else {
                                        "IOT_OFFLINE_DESC"
                                    })),
                            ),
                    )
                },
            )
            .children(self.modal.clone())
    }
}

pub(crate) struct AetherLightingPage {
    quick_effects: Entity<SelectState<Vec<Choice>>>,
    chroma_profiles: Entity<SelectState<Vec<Choice>>>,
    /// The source Effects widget is a two tab surface.  Keep the tab choice
    /// local while device/service state is unavailable; it must not be
    /// represented by two simultaneously mounted panels.
    advanced_mode: bool,
}

impl AetherLightingPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            quick_effects: cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx)),
            chroma_profiles: cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx)),
            advanced_mode: false,
        }
    }
}

impl Render for AetherLightingPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let offline = crate::i18n::t_or("DEVICE_OFFLINE", "Device status unavailable");
        let unavailable = crate::i18n::t_or(
            "AETHER_SERVICE_UNAVAILABLE",
            "Lighting controls are unavailable until device status is observed.",
        );
        let unknown = crate::i18n::t_or("UNKNOWN", "Unknown");
        let quick = crate::i18n::t_or("QUICK_EFFECTS", "Quick Effects");
        let advanced = crate::i18n::t_or("ADVANCED_EFFECTS", "Advanced Effects");
        let synapse_override = surface::SynapseSwitch::new("aether-lighting-override-switch")
            .checked(false)
            .disabled(true)
            .accessibility_label(crate::i18n::t_or("DYNAMIC_LIGHTING", "Synapse Override"));
        let brightness_switch = surface::SynapseSwitch::new("aether-lighting-brightness-switch")
            .checked(false)
            .disabled(true)
            .accessibility_label(crate::i18n::t_or("BRIGHTNESS_HEADER", "Brightness"));
        div().id("aether-lighting-page").test_support().child(
            surface::page_columns()
                .child(surface::page_column(
                    v_flex()
                        .gap_5()
                        .child(
                            surface::panel_with_title_switch(
                                crate::i18n::t_or("DYNAMIC_LIGHTING", "Synapse Override"),
                                synapse_override,
                                div(),
                                cx,
                            )
                            .id("aether-lighting-override")
                            .test_support()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child(offline.clone())
                                    .child(surface::note(unavailable.clone(), cx)),
                            ),
                        )
                        .child(
                            surface::panel_with_title_switch(
                                crate::i18n::t_or("BRIGHTNESS_HEADER", "Brightness"),
                                brightness_switch,
                                div(),
                                cx,
                            )
                            .id("aether-lighting-brightness")
                            .test_support()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(crate::i18n::t_or("BRIGHTNESS", "Brightness"))
                                    .child(unknown.clone()),
                            )
                            .child(
                                div()
                                    .id("aether-lighting-brightness-control")
                                    .test_support()
                                    .child(
                                        Slider::new(&cx.new(|_| {
                                            gpui_kit::component::slider::SliderState::new()
                                                .min(0.)
                                                .max(100.)
                                                .step(1.)
                                        }))
                                        .disabled(true),
                                    ),
                            ),
                        ),
                ))
                .child(surface::page_column(
                    surface::panel(crate::i18n::t_or("EFFECTS", "Effects"), cx)
                        .id("aether-lighting-effects")
                        .test_support()
                        .child(
                            h_flex()
                                .id("aether-lighting-effect-tabs")
                                .test_support()
                                .child(
                                    BaseButton::new("aether-lighting-quick-tab")
                                        .role(Role::Tab)
                                        .selected(!self.advanced_mode)
                                        .child(quick.clone())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.advanced_mode = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    BaseButton::new("aether-lighting-advanced-tab")
                                        .role(Role::Tab)
                                        .selected(self.advanced_mode)
                                        .child(advanced.clone())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.advanced_mode = true;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .child(crate::i18n::t_or(
                                    "AETHER_MODE_UNKNOWN",
                                    "Effect mode is unknown",
                                )),
                        )
                        .child(surface::note(unavailable, cx))
                        .when(!self.advanced_mode, |effects| {
                            effects.child(
                                v_flex()
                                    .id("aether-lighting-quick-effects")
                                    .test_support()
                                    .gap_3()
                                    .child(crate::i18n::t_or("QUICK_EFFECTS_MSG", "Quick effect"))
                                    .child(
                                        surface::select(&self.quick_effects)
                                            .id("aether-lighting-effect-selector")
                                            .accessibility_label(quick)
                                            .placeholder(unknown.clone())
                                            .disabled(true),
                                    )
                                    .child(
                                        Button::new("aether-lighting-sync")
                                            .label(crate::i18n::t_or(
                                                "APPLY_TO_ALL_CHROMA_DEVICES",
                                                "Apply to all Chroma devices",
                                            ))
                                            .disabled(true),
                                    ),
                            )
                        })
                        .when(self.advanced_mode, |effects| {
                            effects.child(
                                v_flex()
                                    .id("aether-lighting-advanced-effects")
                                    .test_support()
                                    .gap_3()
                                    .child(crate::i18n::t_or(
                                        "ADVANCED_EFFECTS_DETAIL",
                                        "Chroma Studio profiles",
                                    ))
                                    .child(
                                        surface::select(&self.chroma_profiles)
                                            .id("aether-lighting-profile-selector")
                                            .accessibility_label(advanced)
                                            .placeholder(unknown)
                                            .disabled(true),
                                    )
                                    .child(
                                        Button::new("aether-lighting-chroma-studio")
                                            .label(crate::i18n::t_or(
                                                "CHROMA_STUDIO",
                                                "Chroma Studio",
                                            ))
                                            .disabled(true),
                                    ),
                            )
                        }),
                )),
        )
    }
}

#[cfg(test)]
#[path = "aether_strip_lighting_tests.rs"]
mod lighting_tests;
fn icon_button(
    id: impl Into<ElementId>,
    name: &'static str,
    label: String,
    disabled: bool,
    _: &App,
) -> source_tip::TipCommand {
    source_tip::TipCommand::icon(id, name, label, disabled)
}
