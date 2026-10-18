//! Independent current Studio root (xt → 4264), separate from Chroma Dashboard.
//! Device/LED state intentionally starts empty: a product catalogue is not a
//! device enumeration. Layer edits are retained locally and never call a DLL.
use gpui_kit::{
    base::Button as BaseButton,
    component::{
        button::Button,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenuItem},
    },
    prelude::FluentBuilder as _,
    *,
};
use razer_model::chroma_studio::{Document, Layer};
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};
#[path = "chroma_studio_canvas.rs"]
mod studio_canvas;
#[path = "chroma_studio_checkbox.rs"]
mod studio_checkbox;
#[path = "chroma_studio_color.rs"]
mod studio_color;
#[path = "chroma_studio_color_dropdown.rs"]
mod studio_color_dropdown;
#[path = "chroma_studio_dropdown.rs"]
mod studio_dropdown;
#[path = "chroma_studio_duration.rs"]
mod studio_duration;
#[path = "chroma_studio_gradient.rs"]
mod studio_gradient;
#[path = "chroma_studio_gradient_data.rs"]
mod studio_gradient_data;
#[path = "chroma_studio_gradient_preset.rs"]
mod studio_gradient_preset;
#[path = "chroma_studio_layers.rs"]
mod studio_layers;
#[path = "chroma_studio_playback.rs"]
mod studio_playback;
#[path = "chroma_studio_properties.rs"]
mod studio_properties;
#[path = "chroma_studio_region_preset.rs"]
mod studio_region_preset;
#[path = "chroma_studio_slider.rs"]
mod studio_slider;
#[path = "chroma_studio_theme.rs"]
mod theme;
use studio_gradient_data::{GradientDefinition, GradientPoint};
use studio_playback::PlaybackSource;
use theme::Colors;

#[derive(Deserialize)]
struct Source {
    effects: Vec<Effect>,
    gradients: BTreeMap<String, GradientDefinition>,
    playback: PlaybackSource,
    empty_color: u32,
    tools: Vec<String>,
    zoom_levels: Vec<u32>,
    locales: BTreeMap<String, BTreeMap<String, String>>,
    assets: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Effect {
    name: String,
    label: String,
    value: u32,
    params: Value,
    paint_params: Value,
    duration_values: Vec<u64>,
}
fn source() -> &'static Source {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE.get_or_init(|| read_chroma_studio_catalog())
}
fn label(key: &str) -> String {
    let source = source();
    source
        .locales
        .get(&razer_i18n::locale())
        .and_then(|locale| locale.get(key))
        .or_else(|| source.locales.get("en").and_then(|locale| locale.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}
fn icon(name: &str, size: f32) -> impl IntoElement + use<> {
    img(SharedString::from(source().assets[name].clone())).size(surface::css(size))
}
fn button(id: impl Into<ElementId>, key: &str) -> BaseButton {
    button_with_hover(id, key, |style| style.bg(Colors::hover()))
}
fn button_with_hover(
    id: impl Into<ElementId>,
    key: &str,
    hover: impl FnOnce(StyleRefinement) -> StyleRefinement,
) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(label(key))
        .flex()
        .items_center()
        .justify_center()
        .p_0()
        .text_color(Colors::text())
        .bg(Colors::panel())
        .hover(hover)
        .focus_visible(|style| style.border_1().border_color(Colors::selected()))
        .styles(|style| style.disabled(|style| style.opacity(0.3)))
}

/// Local draft serialization; deliberately distinct from vendor profile files.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChromaStudioSnapshot {
    document: Document,
}
impl ChromaStudioSnapshot {
    pub fn default_profile_name() -> String {
        format!("{} 1", label("TEXT_CHROMA_EFFECT"))
    }
    pub fn default_profile() -> anyhow::Result<Self> {
        let effect = source()
            .effects
            .iter()
            .find(|effect| effect.name == "spectrum")
            .ok_or_else(|| anyhow::anyhow!("Current Studio spectrum default is missing"))?;
        Self::from_document(Document {
            layers: vec![Layer {
                id: 1,
                name: effect.name.clone(),
                title: effect.label.clone(),
                value: effect.value,
                params: effect.params.clone(),
                params2: serde_json::json!({}),
                paint_params: effect.paint_params.clone(),
                visible: true,
                group: false,
                extensions: Default::default(),
            }],
            extensions: Default::default(),
        })
    }
    pub fn from_document(document: Document) -> anyhow::Result<Self> {
        let snapshot = Self { document };
        snapshot.validate()?;
        Ok(snapshot)
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut ids = std::collections::BTreeSet::new();
        anyhow::ensure!(
            self.document.layers.len() <= 10000,
            "Studio draft contains too many layers"
        );
        for layer in &self.document.layers {
            anyhow::ensure!(
                layer.id > 0 && layer.id < u64::MAX && ids.insert(layer.id),
                "Invalid or duplicate Studio layer identity"
            );
            anyhow::ensure!(
                !layer.title.trim().is_empty() && layer.title.len() <= 4096,
                "Invalid Studio layer title"
            );
            if layer.group {
                anyhow::ensure!(
                    layer.name == "group"
                        && layer.value == 0
                        && layer.params.is_null()
                        && layer.paint_params.is_null(),
                    "Invalid Studio group"
                );
            } else {
                let effect = source()
                    .effects
                    .iter()
                    .find(|effect| effect.name == layer.name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown Studio effect"))?;
                anyhow::ensure!(
                    layer.value == effect.value,
                    "Studio effect identity mismatch"
                );
                anyhow::ensure!(
                    layer.params.is_object() && layer.paint_params.is_object(),
                    "Studio effect parameters must be objects"
                );
                anyhow::ensure!(
                    layer.params2.is_object(),
                    "Studio effect params2 must be an object"
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
pub enum ChromaStudioEvent {
    Save(ChromaStudioSnapshot),
    Profile(StudioProfileCommand),
}
#[derive(Clone, Debug)]
pub enum StudioProfileCommand {
    Add { name: String },
    Select { id: String },
    Rename { id: String, name: String },
    Duplicate { id: String, name: String },
    Delete { id: String },
}
#[derive(Clone, Debug)]
pub struct StudioProfileItem {
    pub id: String,
    pub name: String,
}
#[path = "chroma_studio_profiles.rs"]
mod studio_profiles;

pub struct ChromaStudio {
    document: Document,
    saved: Document,
    save_pending: bool,
    past: Vec<Document>,
    future: Vec<Document>,
    next_id: u64,
    current: Option<u64>,
    tool: String,
    zoom_ix: usize,
    show_device_name: bool,
    is_preview: bool,
    slow_preview: bool,
    renaming: Option<(u64, Entity<InputState>)>,
    rename_subscription: Option<Subscription>,
    focus: FocusHandle,
    properties: Entity<studio_properties::StudioProperties>,
    // Owns property change delivery for the lifetime of this studio entity.
    _property_subscription: Subscription,
    profiles: Vec<StudioProfileItem>,
    selected_profile: Option<String>,
    profiles_pending: bool,
    profile_dropdown: Entity<studio_dropdown::DropdownState>,
    profile_actions: Entity<studio_dropdown::DropdownState>,
    profile_rename: Option<(String, Entity<InputState>)>,
    profile_rename_subscription: Option<Subscription>,
    profile_delete_confirm: bool,
}
impl EventEmitter<ChromaStudioEvent> for ChromaStudio {}
impl ChromaStudio {
    /// Release transient native input when this retained document loses its host.
    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        self.properties
            .update(cx, |properties, cx| properties.deactivate(cx));
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A StudioSession survives its host window. Drop window-bound input
        // state before a newly attached host focuses this retained document.
        self.renaming = None;
        self.rename_subscription = None;
        self.profile_rename = None;
        self.profile_rename_subscription = None;
        self.properties
            .update(cx, |properties, cx| properties.attach(window, cx));
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub fn new(cx: &mut Context<Self>) -> Self {
        let owner = cx.entity();
        let properties = cx.new(|cx| studio_properties::StudioProperties::new(&owner, cx));
        let property_subscription = cx.subscribe(
            &properties,
            |this, _, event: &studio_properties::StudioPropertiesChanged, cx| {
                let Some(layer) = this
                    .document
                    .layers
                    .iter()
                    .find(|layer| layer.id == event.layer_id && !layer.group)
                else {
                    return;
                };
                if layer.params == event.params
                    && layer.params2 == event.params2
                    && layer.paint_params == event.paint_params
                {
                    return;
                }
                this.checkpoint();
                if let Some(layer) = this
                    .document
                    .layers
                    .iter_mut()
                    .find(|layer| layer.id == event.layer_id)
                {
                    layer.params = event.params.clone();
                    layer.params2 = event.params2.clone();
                    layer.paint_params = event.paint_params.clone();
                }
                cx.notify();
            },
        );
        Self {
            document: Document::default(),
            saved: Document::default(),
            save_pending: false,
            past: vec![],
            future: vec![],
            next_id: 1,
            current: None,
            tool: "select".into(),
            zoom_ix: 3,
            show_device_name: false,
            is_preview: true,
            slow_preview: false,
            renaming: None,
            rename_subscription: None,
            focus: cx.focus_handle(),
            properties,
            _property_subscription: property_subscription,
            profiles: Vec::new(),
            selected_profile: None,
            profiles_pending: true,
            profile_dropdown: cx.new(|_| studio_dropdown::DropdownState::new()),
            profile_actions: cx.new(|_| studio_dropdown::DropdownState::new()),
            profile_rename: None,
            profile_rename_subscription: None,
            profile_delete_confirm: false,
        }
    }
    pub fn restore(&mut self, snapshot: ChromaStudioSnapshot, cx: &mut Context<Self>) {
        if snapshot.validate().is_err() {
            return;
        }
        // Accept only current source effect names; never hydrate devices from a draft.
        self.document = snapshot.document;
        self.document.layers.retain(|layer| {
            layer.group
                || source()
                    .effects
                    .iter()
                    .any(|effect| effect.name == layer.name)
        });
        self.next_id = self
            .document
            .layers
            .iter()
            .map(|layer| layer.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.saved = self.document.clone();
        self.current = None;
        self.past.clear();
        self.future.clear();
        cx.notify();
    }
    /// Migrating the former draft does not publish a Studio profile. Its
    /// contents remain editable until the user explicitly saves the library.
    pub fn restore_draft(&mut self, snapshot: ChromaStudioSnapshot, cx: &mut Context<Self>) {
        let saved = self.saved.clone();
        self.restore(snapshot, cx);
        self.saved = saved;
    }
    fn checkpoint(&mut self) {
        self.past.push(self.document.clone());
        self.future.clear();
    }
    fn add_effect(&mut self, name: &str, cx: &mut Context<Self>) {
        // J.ce: these two effects require an observed mouse/keyboard/keypad/system.
        if matches!(name, "reactive" | "ripple") {
            return;
        }
        let Some(effect) = source().effects.iter().find(|effect| effect.name == name) else {
            return;
        };
        self.checkpoint();
        let id = self.next_id;
        self.next_id += 1;
        self.document.layers.insert(
            0,
            Layer {
                id,
                name: effect.name.clone(),
                title: effect.label.clone(),
                value: effect.value,
                params: effect.params.clone(),
                params2: if matches!(effect.name.as_str(), "wheel" | "tidal") {
                    serde_json::json!({"counterclockwise": false})
                } else {
                    serde_json::json!({})
                },
                paint_params: effect.paint_params.clone(),
                visible: true,
                group: false,
                extensions: Default::default(),
            },
        );
        self.current = Some(id);
        cx.notify();
    }
    fn add_group(&mut self, cx: &mut Context<Self>) {
        self.checkpoint();
        let id = self.next_id;
        self.next_id += 1;
        let title = studio_layers::unique_title(None, true, &self.document.layers);
        self.document.layers.insert(
            0,
            Layer {
                id,
                name: "group".into(),
                title,
                value: 0,
                params: Value::Null,
                params2: Value::Null,
                paint_params: Value::Null,
                visible: true,
                group: true,
                extensions: Default::default(),
            },
        );
        cx.notify();
    }
    fn duplicate(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(mut layer) = self
            .document
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .cloned()
        else {
            return;
        };
        self.checkpoint();
        let default_title = source()
            .effects
            .iter()
            .find(|effect| effect.name == layer.name)
            .map(|effect| effect.label.as_str());
        if default_title != Some(layer.title.as_str()) {
            layer.title =
                studio_layers::unique_title(Some(&layer.title), layer.group, &self.document.layers);
        }
        layer.id = self.next_id;
        self.next_id += 1;
        self.document.layers.insert(0, layer);
        self.current = self.first_visible_layer();
        cx.notify();
    }
    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.can_remove_layer(id) || !self.document.layers.iter().any(|layer| layer.id == id) {
            return;
        }
        self.checkpoint();
        self.document.layers.retain(|layer| layer.id != id);
        if self.current == Some(id) {
            self.current = self
                .document
                .layers
                .iter()
                .find(|layer| !layer.group)
                .map(|layer| layer.id);
        }
        cx.notify();
    }
    fn visibility(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.document.layers.iter().any(|layer| layer.id == id) {
            return;
        }
        self.checkpoint();
        if let Some(layer) = self.document.layers.iter_mut().find(|layer| layer.id == id) {
            layer.visible = !layer.visible;
        }
        self.current = self.first_visible_layer();
        cx.notify();
    }
    fn begin_rename(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(title) = self
            .document
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .map(|layer| label(&layer.title))
        else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(title, window, cx);
            input
        });
        self.rename_subscription = Some(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                this.finish_rename(cx);
            }
        }));
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.renaming = Some((id, input));
        cx.notify();
    }
    fn finish_rename(&mut self, cx: &mut Context<Self>) {
        let Some((id, input)) = self.renaming.take() else {
            return;
        };
        self.rename_subscription = None;
        let entered = input.read(cx).value();
        // f.TH resolves to 32 in the production environment; HTML maxLength
        // counts UTF-16 units, not UTF-8 bytes or Unicode scalar values.
        let mut units = 0;
        let title: String = entered
            .trim()
            .chars()
            .take_while(|ch| {
                units += ch.len_utf16();
                units <= 32
            })
            .collect();
        if !title.is_empty() {
            if let Some(layer) = self.document.layers.iter().find(|layer| layer.id == id) {
                let group = layer.group;
                if !self
                    .document
                    .layers
                    .iter()
                    .any(|layer| layer.title == title && layer.group == group)
                {
                    self.checkpoint();
                    if let Some(layer) =
                        self.document.layers.iter_mut().find(|layer| layer.id == id)
                    {
                        layer.title = title;
                    }
                }
            }
        }
        cx.notify();
    }
    fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        let target = if redo { self.future.pop() } else { self.past.pop() };
        if let Some(target) = target {
            let previous = std::mem::replace(&mut self.document, target);
            if redo {
                self.past.push(previous);
            } else {
                self.future.push(previous);
            }
            if !self
                .document
                .layers
                .iter()
                .any(|layer| Some(layer.id) == self.current)
            {
                // 5303:m / 9286:G retain the current layer if present,
                // otherwise select the first remaining effect (not a group).
                self.current = self
                    .document
                    .layers
                    .iter()
                    .find(|layer| !layer.group)
                    .map(|layer| layer.id);
            }
            cx.notify();
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.document == self.saved && !self.save_pending {
            return;
        }
        cx.emit(ChromaStudioEvent::Save(self.snapshot()));
    }
    /// Called by the persistence owner only after a successful local write.
    pub fn saved(&mut self, snapshot: ChromaStudioSnapshot, cx: &mut Context<Self>) {
        self.saved = snapshot.document;
        cx.notify();
    }
    pub fn snapshot(&self) -> ChromaStudioSnapshot {
        ChromaStudioSnapshot {
            document: self.document.clone(),
        }
    }
    pub fn set_save_pending(&mut self, pending: bool, cx: &mut Context<Self>) {
        self.save_pending = pending;
        cx.notify();
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if self.profile_rename.is_some() {
            if key == "escape" {
                self.profile_rename = None;
                self.profile_rename_subscription = None;
                self.focus.focus(window, cx);
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }
        if self.renaming.is_some() {
            if key == "escape" {
                self.renaming = None;
                self.rename_subscription = None;
                self.focus.focus(window, cx);
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }
        let control = event.keystroke.modifiers.control;
        if control {
            match key {
                "z" => self.history(event.keystroke.modifiers.shift, cx),
                "s" => self.save(cx),
                "l" => {
                    self.show_device_name = !self.show_device_name;
                    cx.notify();
                }
                "d" => {
                    if let Some(id) = self.current.filter(|id| {
                        self.document
                            .layers
                            .iter()
                            .any(|layer| layer.id == *id && !layer.group)
                    }) {
                        self.duplicate(id, cx);
                    }
                }
                "e" => {
                    if let Some(id) = self.current {
                        self.begin_rename(id, window, cx);
                    }
                }
                "f" => {
                    self.zoom_ix = 3;
                    cx.notify();
                }
                _ => return,
            }
        } else {
            let tool = match key {
                "s" => "select",
                "p" => "pen",
                "b" => "bucket",
                "m" => "move",
                _ => return,
            };
            self.tool = tool.into();
            cx.notify();
        }
        cx.stop_propagation();
    }
    fn layer_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut layers = div().id("studio-layers").flex_1().min_h_0().scrollable_y();
        for layer in &self.document.layers {
            let id = layer.id;
            let menu_owner = cx.entity();
            let selected = self.current == Some(id);
            layers = layers.child(
                div()
                    .flex()
                    .items_center()
                    .h(surface::css(40.))
                    .child(
                        if let Some((_, input)) = self
                            .renaming
                            .as_ref()
                            .filter(|(renaming_id, _)| *renaming_id == id)
                        {
                            div()
                                .flex_1()
                                .min_w_0()
                                .px(surface::css(10.))
                                .child(Input::new(input).appearance(false))
                                .into_any_element()
                        } else {
                            button(("studio-layer", id), "EFFECTS_LAYERS")
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .px(surface::css(10.))
                                .justify_start()
                                .gap(surface::css(5.))
                                .text_color(if selected {
                                    Colors::selected()
                                } else {
                                    Colors::text()
                                })
                                .opacity(if layer.visible { 1. } else { 0.3 })
                                .child(icon(
                                    if layer.group {
                                        "folder-gray"
                                    } else {
                                        source()
                                            .assets
                                            .keys()
                                            .find(|key| **key == format!("{}-white", layer.name))
                                            .expect("effect icon")
                                    },
                                    20.,
                                ))
                                .child(div().truncate().child(label(&layer.title)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.current = if this
                                        .document
                                        .layers
                                        .iter()
                                        .any(|layer| layer.id == id && layer.group)
                                    {
                                        this.first_visible_layer()
                                    } else {
                                        Some(id)
                                    };
                                    cx.notify();
                                }))
                                .into_any_element()
                        },
                    )
                    .child(
                        button(("studio-visibility", id), "LIVE_PREVIEW")
                            .w(surface::css(29.5))
                            .h_full()
                            .child(icon(
                                if layer.visible {
                                    "eye-gray"
                                } else {
                                    "eye-disabled-white"
                                },
                                20.,
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| this.visibility(id, cx))),
                    )
                    .child(
                        Button::new(("studio-layer-menu", id))
                            .label("⋯")
                            .w(surface::css(29.5))
                            .h(surface::css(27.))
                            .dropdown_menu(move |menu, window, cx| {
                                let rename = menu_owner.clone();
                                let duplicate = menu_owner.clone();
                                let remove = menu_owner.clone();
                                let is_group = menu_owner
                                    .read(cx)
                                    .document
                                    .layers
                                    .iter()
                                    .find(|layer| layer.id == id)
                                    .is_none_or(|layer| layer.group);
                                let can_remove = menu_owner.read(cx).can_remove_layer(id);
                                let menu = menu.item(PopupMenuItem::new(label("RENAME")).on_click(
                                    move |_, window, cx| {
                                        rename.update(cx, |this, cx| {
                                            this.begin_rename(id, window, cx)
                                        })
                                    },
                                ));
                                let menu = if is_group {
                                    menu
                                } else {
                                    let owner = menu_owner.clone();
                                    menu.submenu(
                                        label("CHANGE_EFFECT"),
                                        window,
                                        cx,
                                        move |mut menu, _, cx| {
                                            let current = owner
                                                .read(cx)
                                                .document
                                                .layers
                                                .iter()
                                                .find(|layer| layer.id == id)
                                                .map(|layer| layer.name.clone());
                                            for effect in &source().effects {
                                                if current.as_deref() == Some(&effect.name)
                                                    || matches!(
                                                        effect.name.as_str(),
                                                        "reactive" | "ripple"
                                                    )
                                                {
                                                    continue;
                                                }
                                                let name = effect.name.clone();
                                                let owner = owner.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(label(&effect.label))
                                                        .on_click(move |_, _, cx| {
                                                            owner.update(cx, |this, cx| {
                                                                this.change_layer_effect(
                                                                    id, &name, cx,
                                                                )
                                                            });
                                                        }),
                                                );
                                            }
                                            menu
                                        },
                                    )
                                };
                                menu.item(PopupMenuItem::new(label("DUPLICATE")).on_click(
                                    move |_, _, cx| {
                                        duplicate.update(cx, |this, cx| this.duplicate(id, cx))
                                    },
                                ))
                                .separator()
                                .item(
                                    PopupMenuItem::new(label("DELETE"))
                                        .disabled(!can_remove)
                                        .on_click(move |_, _, cx| {
                                            remove.update(cx, |this, cx| this.remove(id, cx))
                                        }),
                                )
                            }),
                    ),
            );
        }
        let mut effects = div().flex().flex_wrap().w_full().justify_center();
        for effect in &source().effects {
            let name = effect.name.clone();
            effects = effects.child(
                button(
                    SharedString::from(format!("studio-add-{name}")),
                    &effect.label,
                )
                .w(surface::css(76.))
                .h(surface::css(72.))
                .flex_col()
                .gap(surface::css(2.))
                .disabled(matches!(name.as_str(), "reactive" | "ripple"))
                .child(icon(&format!("{name}-white"), 30.))
                .child(
                    div()
                        .text_size(surface::css(10.))
                        .child(label(&effect.label)),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.add_effect(&name, cx))),
            );
        }
        div()
            .id("studio-layer-panel")
            .test_support()
            .flex()
            .flex_col()
            .w(surface::css(250.))
            .flex_shrink_0()
            .h_full()
            .bg(Colors::panel())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(surface::css(40.))
                            .px(surface::css(10.))
                            .child(div().flex_1().child(label("EFFECTS_LAYERS")))
                            .child(
                                button("studio-add-group", "TOOLTIP_ADD_GROUP")
                                    .size(surface::css(27.))
                                    .child(icon("folder-add-gray", 20.))
                                    .on_click(cx.listener(|this, _, _, cx| this.add_group(cx))),
                            ),
                    )
                    .child(layers),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .max_h(relative(0.5))
                    .min_h_0()
                    .child(
                        div()
                            .h(surface::css(40.))
                            .flex()
                            .items_center()
                            .px(surface::css(10.))
                            .border_t_1()
                            .border_color(Colors::border())
                            .child(label("TEXT_ONBOARD_ADD_EFFECT")),
                    )
                    .child(
                        div()
                            .id("studio-effects")
                            .min_h_0()
                            .scrollable_y()
                            .child(effects),
                    ),
            )
            .into_any_element()
    }
    fn editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut toolbar = div()
            .flex()
            .items_center()
            .justify_center()
            .h(surface::css(40.))
            .flex_shrink_0()
            .gap(surface::css(14.))
            .bg(Colors::panel());
        for tool in &source().tools {
            let name = tool.clone();
            let key = match tool.as_str() {
                "select" => "SELECTOR",
                "pen" => "PEN",
                "bucket" => "PAINT_BUCKET",
                _ => "MOVE",
            };
            let asset = if tool == "select" { "cursor" } else { tool };
            toolbar = toolbar.child(
                button(SharedString::from(format!("studio-tool-{tool}")), key)
                    .size(surface::css(27.))
                    .child(icon(
                        &format!(
                            "{asset}-{}",
                            if self.tool == *tool { "green" } else { "white" }
                        ),
                        20.,
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tool = name.clone();
                        cx.notify();
                    })),
            );
        }
        toolbar = toolbar
            .child(
                button("studio-clear-leds", "DELETE")
                    .size(surface::css(27.))
                    .disabled(true)
                    .child(icon("trash-white", 20.)),
            )
            .child(
                button("studio-undo", "UNDO")
                    .size(surface::css(27.))
                    .disabled(self.past.is_empty())
                    .child(icon("undo-white", 20.))
                    .on_click(cx.listener(|this, _, _, cx| this.history(false, cx))),
            )
            .child(
                button("studio-redo", "REDO")
                    .size(surface::css(27.))
                    .disabled(self.future.is_empty())
                    .child(icon("redo-white", 20.))
                    .on_click(cx.listener(|this, _, _, cx| this.history(true, cx))),
            );
        let zoom_owner = cx.entity();
        let preview_owner = cx.entity();
        let zoom_ix = self.zoom_ix;
        let zoom = Button::new("studio-zoom")
            .label(format!("{}%", source().zoom_levels[zoom_ix]))
            .w(surface::css(84.))
            .h(surface::css(30.))
            .dropdown_menu(move |mut menu, _, _| {
                for (ix, level) in source().zoom_levels.iter().enumerate() {
                    let owner = zoom_owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{level}%"))
                            .checked(ix == zoom_ix)
                            .on_click(move |_, _, cx| {
                                owner.update(cx, |this, cx| {
                                    this.zoom_ix = ix;
                                    cx.notify();
                                })
                            }),
                    );
                }
                menu
            });
        let preview = self.is_preview;
        let slow = self.slow_preview;
        let controls = div()
            .absolute()
            .bottom(surface::css(14.))
            .right(surface::css(14.))
            .flex()
            .items_center()
            .gap(surface::css(2.))
            .child(
                button("studio-device-names", "DEVICE")
                    .size(surface::css(30.))
                    .child(icon(
                        if self.show_device_name {
                            "icon-label"
                        } else {
                            "icon-label-disable"
                        },
                        20.,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_device_name = !this.show_device_name;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("studio-preview")
                    .label(label("STUDIO_PREVIEW"))
                    .h(surface::css(30.))
                    .dropdown_menu(move |menu, _, _| {
                        let enabled_owner = preview_owner.clone();
                        let speed_owner = preview_owner.clone();
                        menu.item(
                            PopupMenuItem::new(label("STUDIO_PREVIEW"))
                                .checked(preview)
                                .on_click(move |_, _, cx| {
                                    enabled_owner.update(cx, |this, cx| {
                                        this.is_preview = !this.is_preview;
                                        cx.notify();
                                    })
                                }),
                        )
                        .item(
                            PopupMenuItem::new(label(if slow { "SLOW" } else { "NORMAL" }))
                                .disabled(!preview)
                                .on_click(move |_, _, cx| {
                                    speed_owner.update(cx, |this, cx| {
                                        this.slow_preview = !this.slow_preview;
                                        cx.notify();
                                    })
                                }),
                        )
                    }),
            )
            .child(
                button("studio-minus", "ZOOM_OUT")
                    .size(surface::css(30.))
                    .disabled(zoom_ix + 1 == source().zoom_levels.len())
                    .child(icon("minus-white", 20.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_ix = (this.zoom_ix + 1).min(source().zoom_levels.len() - 1);
                        cx.notify();
                    })),
            )
            .child(
                button("studio-plus", "ZOOM_IN")
                    .size(surface::css(30.))
                    .disabled(zoom_ix == 0)
                    .child(icon("plus-white", 20.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_ix = this.zoom_ix.saturating_sub(1);
                        cx.notify();
                    })),
            )
            .child(zoom);
        div()
            .id("studio-editor")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .overflow_hidden()
            .child(toolbar)
            // vt renders no device or LED without an actual enumerated item.
            .child(
                div()
                    .id("studio-canvas")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .bg(Colors::canvas())
                    .child(studio_canvas::render(
                        source().zoom_levels[self.zoom_ix] as f32 / 100.,
                        self.tool == "move",
                    )),
            )
            .child(controls)
            .into_any_element()
    }
    fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self
            .document
            .layers
            .iter()
            .find(|layer| Some(layer.id) == self.current && !layer.group);
        let title = current
            .map(|layer| {
                source()
                    .effects
                    .iter()
                    .find(|effect| effect.name == layer.name)
                    .map(|effect| label(&effect.label))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        div()
            .id("studio-inspector")
            .test_support()
            .flex()
            .flex_col()
            .w(surface::css(250.))
            .flex_shrink_0()
            .h_full()
            .bg(Colors::panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(surface::css(40.))
                    .px(surface::css(10.))
                    .when_some(current, |row, layer| {
                        row.child(
                            div()
                                .mr(surface::css(10.))
                                .child(icon(&format!("{}-white", layer.name), 20.)),
                        )
                    })
                    .child(div().flex_1().child(title))
                    .child(
                        button_with_hover("studio-reset-properties", "RESET", |style| {
                            style.bg(Colors::panel()).text_color(Colors::selected())
                        })
                        .accessibility_label(label("RESET"))
                        .size(surface::css(20.))
                        .mt(surface::css(-3.))
                        .active(|style| style.opacity(0.7))
                        .disabled(
                            current.is_none() || !matches!(self.tool.as_str(), "pen" | "bucket"),
                        )
                        .child(
                            razer_assets::resource_svg()
                                .path(SharedString::from(source().assets["icon_reset"].clone()))
                                .size_full(),
                        )
                        .on_click({
                            let properties = self.properties.clone();
                            move |_, _, cx| {
                                properties.update(cx, |properties, cx| properties.reset(cx))
                            }
                        }),
                    ),
            )
            .child(self.properties.clone())
            .child(
                div()
                    .flex()
                    .justify_center()
                    .py(surface::css(7.))
                    .border_t_1()
                    .border_color(Colors::border())
                    .child(
                        button("studio-save", "SAVE")
                            .w(surface::css(100.))
                            .h(surface::css(27.))
                            .bg(Colors::selected())
                            .text_color(Colors::panel())
                            .disabled(self.document == self.saved && !self.save_pending)
                            .child(label("SAVE"))
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ),
            )
            .into_any_element()
    }
}
impl Render for ChromaStudio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chroma-studio-root")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(Colors::panel())
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .on_key_down(cx.listener(Self::key_down))
            .child(
                div()
                    .h(surface::css(48.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(surface::css(10.))
                    .gap(surface::css(10.))
                    .child(
                        div()
                            .text_color(Colors::selected())
                            .child(label("TEXT_STUDIO")),
                    )
                    .child(self.profile_header(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.layer_panel(cx))
                    .child(self.editor(cx))
                    .child(self.inspector(cx)),
            )
    }
}

// Generated catalog reader: razer-pages/chroma_studio_data.
fn read_chroma_studio_catalog() -> Source {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                effects: r.child(decode_1)?,
                gradients: r.child(decode_8)?,
                playback: r.child(decode_18)?,
                empty_color: r.child(decode_4)?,
                tools: r.child(decode_23)?,
                zoom_levels: r.child(decode_24)?,
                locales: r.child(decode_25)?,
                assets: r.child(decode_26)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Effect>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Effect> {
        let value = match r.tag()? {
            0 => Effect {
                name: r.child(decode_3)?,
                label: r.child(decode_3)?,
                value: r.child(decode_4)?,
                params: r.child(decode_5)?,
                paint_params: r.child(decode_5)?,
                duration_values: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u64>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, GradientDefinition>> {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<GradientDefinition> {
        let value = match r.tag()? {
            0 => GradientDefinition {
                presets: r.child(decode_10)?,
                min_stops: r.child(decode_17)?,
                max_stops: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<std::vec::Vec<GradientPoint>>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_11)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<GradientPoint>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<GradientPoint> {
        let value = match r.tag()? {
            0 => GradientPoint {
                stop: r.child(decode_13)?,
                rgb: r.child(decode_14)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<[u8; 3]>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_15)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[u8; 3]> {
        let value = match r.tag()? {
            0 => r.array::<_, 3>(decode_16)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PlaybackSource> {
        let value = match r.tag()? {
            0 => PlaybackSource {
                start: r.child(decode_19)?,
                end: r.child(decode_19)?,
                labels: r.child(decode_21)?,
                min: r.child(decode_22)?,
                max: r.child(decode_22)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_20)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_20, decode_20)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_22(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<i64> {
        let value = match r.tag()? {
            0 => r.i64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_23(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_24(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_25(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_26)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_26(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_CHROMA_STUDIO_DATA,
        0,
        "7eb6a71f78e6ec99a72c0fea45d05d226786c1b74a33c0bf0033add965141fac",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod chroma_studio_data_catalog_tests {
    #[test]
    fn packaged_catalog_chroma_studio_data_decodes() {
        let _ = super::read_chroma_studio_catalog();
    }
}
// End generated catalog reader: razer-pages/chroma_studio_data.

#[cfg(test)]
mod button_tests {
    #[test]
    fn studio_button_hover_override_constructs_without_duplicate_registration() {
        let mut applied = 0;
        let _ = super::button_with_hover("studio-hover-test", "RESET", |style| {
            applied += 1;
            style
        });
        assert_eq!(applied, 1);
    }
}
