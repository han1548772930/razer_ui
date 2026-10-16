//! Current keyboard artwork selection and mounted 679 dial/media hover surfaces.
//! The caller supplies observed edition/layout/wrist state; unavailable source
//! assets are absent rather than substituted with a guessed layout or color.
use super::KeyboardProductWorkspace;
use gpui_kit::base::{Button as BaseButton, ElementExt as _};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_assets::KeyboardKey;
use razer_i18n::t;
use razer_widgets::surface;
use serde::Deserialize;
use std::sync::OnceLock;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceArtworkObservation {
    pub product_id: u32,
    pub edition_id: u32,
    pub layout_id: u32,
    /// UI source event MW_ATTACHED_THUMB_MODULE supplies this payload.
    pub wristrest_connected: Option<bool>,
}

impl SourceArtworkObservation {
    /// The formal page and all its key regions share the same presentation
    /// variant. Only the native legacy preview boundary may supply the verified
    /// MW default; source image selectors keep their original layout semantics.
    pub fn for_device(device: &razer_model::model::Device) -> Self {
        let (edition_id, layout_id) = razer_model::product_preview::presentation_variant(device);
        Self {
            product_id: device.product_id,
            edition_id,
            layout_id,
            wristrest_connected: device.dashboard.wristrest_connected,
        }
    }
}

#[derive(Deserialize)]
struct Image {
    role: String,
    edition_id: u32,
    layout_id: Option<u32>,
    resolution: Option<u32>,
    asset: String,
}
#[derive(Deserialize)]
struct Layout {
    layout_id: u32,
    keys: Vec<KeyboardKey>,
}
#[derive(Deserialize)]
struct Wrist {
    asset: String,
    bounds: [f32; 4],
    transform_y: f32,
}
#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    viewbox: [f32; 2],
    image_css_size: [f32; 2],
    images: Vec<Image>,
    default_keys: Vec<KeyboardKey>,
    layouts: Vec<Layout>,
    wrist: Wrist,
}
fn spec(pid: u32) -> Option<&'static Spec> {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    let source = SPEC.get_or_init(|| read_keyboard_source_artwork_catalog());
    (source.product_id == pid).then_some(source)
}

#[derive(Deserialize)]
struct FamilyImage {
    edition_id: u32,
    layout_id: u32,
    resolution: u32,
    svg: bool,
    asset: String,
}
#[derive(Deserialize)]
struct FamilyLayout {
    layout_id: u32,
    edition_id: u32,
    customize: bool,
    geometry: usize,
}
#[derive(Deserialize)]
struct CustomizeLayoutNormalization {
    ansi: Vec<u32>,
    iso: Vec<u32>,
    japanese: u32,
    united_states: u32,
    united_kingdom: u32,
}
#[derive(Deserialize)]
struct SourceVariant {
    edition_id: u32,
    layout_id: u32,
}
#[derive(Deserialize)]
struct FamilyProduct {
    product_id: u32,
    image_css_size: [f32; 2],
    image_mode_svg: bool,
    images: Vec<FamilyImage>,
    customize_images: Vec<FamilyImage>,
    #[serde(default)]
    customize_layout_normalization: Option<CustomizeLayoutNormalization>,
    #[serde(default)]
    source_supported_layout_ids: Vec<u32>,
    #[serde(default)]
    source_variants: Vec<SourceVariant>,
    #[serde(default)]
    source_one_x_only_variants: Vec<SourceVariant>,
    default_geometry: usize,
    layouts: Vec<FamilyLayout>,
}
impl FamilyProduct {
    fn customize_layout_id(&self, requested: u32) -> u32 {
        if self
            .customize_images
            .iter()
            .any(|image| image.layout_id == requested)
        {
            return requested;
        }
        let Some(normalization) = &self.customize_layout_normalization else {
            return requested;
        };
        if normalization.ansi.contains(&requested) {
            normalization.united_states
        } else if normalization.iso.contains(&requested) {
            normalization.united_kingdom
        } else if requested == normalization.japanese {
            requested
        } else {
            normalization.united_states
        }
    }
}
#[derive(Deserialize)]
struct FamilyGeometry {
    viewbox: [f32; 2],
    keys: Vec<KeyboardKey>,
    buttons: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct FamilyAtlas {
    products: Vec<FamilyProduct>,
    geometries: Vec<FamilyGeometry>,
    #[serde(default)]
    preview_products: Vec<PreviewProduct>,
}
#[derive(Deserialize)]
struct PreviewProduct {
    product_id: u32,
    source_variants: Vec<SourceVariant>,
    #[serde(default)]
    images: Vec<FamilyImage>,
    #[serde(default)]
    edition_zero_retry: bool,
}
#[derive(Deserialize)]
struct PreviewAtlas {
    products: Vec<PreviewProduct>,
}

fn preview_products() -> &'static [PreviewProduct] {
    static PREVIEW: OnceLock<PreviewAtlas> = OnceLock::new();
    &PREVIEW
        .get_or_init(|| read_keyboard_preview_current_catalog())
        .products
}
fn atlas() -> &'static FamilyAtlas {
    static ATLAS: OnceLock<FamilyAtlas> = OnceLock::new();
    ATLAS.get_or_init(|| read_keyboard_artwork_current_catalog())
}
fn family_product(pid: u32) -> Option<&'static FamilyProduct> {
    atlas()
        .products
        .iter()
        .find(|product| product.product_id == pid)
}

pub fn source_preview_layouts(pid: u32) -> Vec<u32> {
    if let Some(product) = preview_products()
        .iter()
        .chain(&atlas().preview_products)
        .filter(|product| family_product(product.product_id).is_none())
        .find(|product| product.product_id == pid)
    {
        return product
            .source_variants
            .iter()
            .map(|variant| variant.layout_id)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    let layouts = if let Some(product) = family_product(pid) {
        if !product.source_supported_layout_ids.is_empty() {
            return product.source_supported_layout_ids.clone();
        }
        product
            .images
            .iter()
            .map(|image| image.layout_id)
            .collect::<std::collections::BTreeSet<_>>()
    } else if let Some(product) = spec(pid) {
        product
            .images
            .iter()
            .filter(|image| image.role == "base")
            .filter_map(|image| image.layout_id)
            .collect()
    } else {
        return Vec::new();
    };
    layouts.into_iter().collect()
}
pub fn source_preview_layouts_for_edition(pid: u32, edition: u32) -> Option<Vec<u32>> {
    if let Some(product) = preview_products()
        .iter()
        .chain(&atlas().preview_products)
        .filter(|product| family_product(product.product_id).is_none())
        .find(|product| product.product_id == pid)
    {
        if product.source_variants.is_empty() {
            return None;
        }
        return Some(
            product
                .source_variants
                .iter()
                .filter(|variant| {
                    variant.edition_id == edition
                        || variant.edition_id == 0 && product.edition_zero_retry
                })
                .map(|variant| variant.layout_id)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
        );
    }
    let layouts = if let Some(product) = family_product(pid) {
        if product.source_variants.is_empty() {
            product
                .images
                .iter()
                .filter(|image| {
                    image.svg == product.image_mode_svg
                        && (image.edition_id == edition || image.edition_id == 0)
                })
                .map(|image| image.layout_id)
                .collect::<std::collections::BTreeSet<_>>()
        } else {
            product
                .source_variants
                .iter()
                .filter(|variant| variant.edition_id == edition || variant.edition_id == 0)
                .map(|variant| variant.layout_id)
                .collect()
        }
    } else if let Some(product) = spec(pid) {
        product
            .images
            .iter()
            .filter(|image| {
                image.role == "base" && (image.edition_id == edition || image.edition_id == 0)
            })
            .filter_map(|image| image.layout_id)
            .collect()
    } else {
        return None;
    };
    Some(layouts.into_iter().collect())
}
fn family_geometry(
    o: SourceArtworkObservation,
    customize: bool,
) -> Option<&'static FamilyGeometry> {
    let product = family_product(o.product_id)?;
    let customize = customize && product.image_mode_svg;
    let layout_id = if customize {
        product.customize_layout_id(o.layout_id)
    } else {
        o.layout_id
    };
    let layout = product
        .layouts
        .iter()
        .find(|layout| {
            layout.layout_id == layout_id
                && layout.edition_id == o.edition_id
                && layout.customize == customize
        })
        .or_else(|| {
            product.layouts.iter().find(|layout| {
                layout.layout_id == layout_id
                    && layout.edition_id == 0
                    && layout.customize == customize
            })
        });
    atlas().geometries.get(
        layout
            .map(|layout| layout.geometry)
            .unwrap_or(product.default_geometry),
    )
}

pub fn has_source(pid: u32) -> bool {
    family_product(pid).is_some() || spec(pid).is_some()
}

#[derive(Clone, Copy, Debug)]
pub struct ArtworkImage {
    pub asset: &'static str,
    pub css_size: [f32; 2],
}

#[derive(Clone, Copy)]
pub struct ArtworkGeometry {
    pub keys: &'static [KeyboardKey],
    pub viewbox: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct WristArtwork {
    pub asset: &'static str,
    pub bounds: [f32; 4],
    pub transform_y: f32,
    pub visible: bool,
}

/// Base image loader retries edition zero for the same requested layout only.
/// Layout zero is not rewritten to one: the current caller passes it unchanged.
pub fn base_image(o: SourceArtworkObservation) -> Option<ArtworkImage> {
    base_image_for_scale(o, 1.)
}

/// Original img uses src=1x and srcset=(1x,3x); the native caller supplies its
/// actual display scale. Both density candidates have independent edition retry.
pub fn base_image_for_scale(o: SourceArtworkObservation, scale: f32) -> Option<ArtworkImage> {
    image_for_scale(o, scale, false)
}

pub(super) fn customize_image_for_scale(
    o: SourceArtworkObservation,
    scale: f32,
) -> Option<ArtworkImage> {
    image_for_scale(o, scale, true)
}

fn image_for_scale(
    o: SourceArtworkObservation,
    scale: f32,
    customize: bool,
) -> Option<ArtworkImage> {
    if let Some(product) = family_product(o.product_id) {
        let density = if scale > 1. { 3 } else { 1 };
        let layout_id = if customize && product.image_mode_svg {
            product.customize_layout_id(o.layout_id)
        } else {
            o.layout_id
        };
        let images = if customize && product.image_mode_svg {
            &product.customize_images
        } else {
            &product.images
        };
        for edition in [o.edition_id, 0] {
            if let Some(image) = images.iter().find(|image| {
                image.edition_id == edition
                    && image.layout_id == layout_id
                    && image.svg == product.image_mode_svg
                    && (image.svg || image.resolution == density)
            }) {
                return Some(ArtworkImage {
                    asset: &image.asset,
                    css_size: product.image_css_size,
                });
            }
            if o.edition_id == 0 {
                break;
            }
        }
        if density == 3
            && !product.image_mode_svg
            && product.source_one_x_only_variants.iter().any(|variant| {
                variant.layout_id == layout_id
                    && (variant.edition_id == o.edition_id || variant.edition_id == 0)
            })
        {
            // The verified ProductImage emits "url 1x,  3x" for this
            // source context. HTML parses the final URL as another 1x
            // candidate and keeps the earlier real URL (sections 4.8.4.3.7,
            // 4.8.4.3.10, 4.8.4.3.12). No synthetic 3x resource is added.
            return image_for_scale(o, 1., customize);
        }
        return None;
    }
    let s = spec(o.product_id)?;
    let density = if scale > 1. { 3 } else { 1 };
    for edition in [o.edition_id, 0] {
        let image = s.images.iter().find(|i| {
            i.role == "base"
                && i.edition_id == edition
                && i.layout_id == Some(o.layout_id)
                && i.resolution == Some(density)
        });
        if let Some(image) = image {
            return Some(ArtworkImage {
                asset: &image.asset,
                css_size: s.image_css_size,
            });
        }
        if o.edition_id == 0 {
            break;
        }
    }
    None
}

pub fn source_preview_image_for_scale(
    o: SourceArtworkObservation,
    scale: f32,
) -> Option<&'static str> {
    if let Some(image) = base_image_for_scale(o, scale) {
        return Some(image.asset);
    }
    if let Some(image) = super::source_keypad_product_image(
        o.product_id,
        o.edition_id,
        if scale > 1. { 3 } else { 1 },
    ) {
        return Some(image);
    }
    let product = preview_products()
        .iter()
        .find(|product| product.product_id == o.product_id)?;
    let density = if scale > 1. { 3 } else { 1 };
    [o.edition_id, 0]
        .into_iter()
        .filter(|edition| *edition == o.edition_id || product.edition_zero_retry)
        .find_map(|edition| {
            product
                .images
                .iter()
                .find(|image| {
                    image.edition_id == edition
                        && image.layout_id == o.layout_id
                        && (image.svg || image.resolution == density)
                })
                .map(|image| image.asset.as_str())
        })
}

#[cfg(test)]
fn geometry(o: SourceArtworkObservation) -> Option<ArtworkGeometry> {
    geometry_for_surface(o, true)
}

pub(super) fn geometry_for_surface(
    o: SourceArtworkObservation,
    customize: bool,
) -> Option<ArtworkGeometry> {
    if let Some(source) = family_geometry(o, customize) {
        return Some(ArtworkGeometry {
            keys: &source.keys,
            viewbox: source.viewbox,
        });
    }
    let s = spec(o.product_id)?;
    let layout = s.layouts.iter().find(|l| l.layout_id == o.layout_id);
    Some(ArtworkGeometry {
        keys: layout.map(|l| l.keys.as_slice()).unwrap_or(&s.default_keys),
        viewbox: s.viewbox,
    })
}

pub(super) fn buttons(
    o: SourceArtworkObservation,
    customize: bool,
) -> Option<&'static [serde_json::Value]> {
    family_geometry(o, customize).map(|geometry| geometry.buttons.as_slice())
}

/// These two artworks belong to the source special popovers, not the base image.
pub fn dial_image(o: SourceArtworkObservation) -> Option<&'static str> {
    special_image(o, "dial")
}
pub fn media_image(o: SourceArtworkObservation) -> Option<&'static str> {
    special_image(o, "media")
}
fn special_image(o: SourceArtworkObservation, role: &str) -> Option<&'static str> {
    let s = spec(o.product_id)?;
    let edition = if o.edition_id == 128 { 128 } else { 0 };
    s.images
        .iter()
        .find(|i| i.role == role && i.edition_id == edition)
        .map(|i| i.asset.as_str())
}

#[derive(Default)]
pub struct ArtworkState {
    observation: Option<SourceArtworkObservation>,
    wrist_mounted: bool,
    special_shown: [bool; 2],
    special_bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    pressed_special: Option<String>,
}
impl ArtworkState {
    pub(super) fn dismiss_special(&mut self) {
        self.special_shown = [false; 2];
        self.pressed_special = None;
        for bounds in &self.special_bounds {
            bounds.set(Bounds::default());
        }
    }
    pub fn observation(&self) -> Option<SourceArtworkObservation> {
        self.observation
    }
    pub fn observe(&mut self, o: SourceArtworkObservation) {
        if self
            .observation
            .is_some_and(|previous| previous.product_id != o.product_id)
        {
            self.wrist_mounted = false;
        }
        // Sh mounts no wrist node when initially disconnected; after connection
        // its node stays mounted and the source show/hide class changes.
        if o.wristrest_connected == Some(true) {
            self.wrist_mounted = true;
        }
        if o.wristrest_connected.is_none() {
            self.wrist_mounted = false;
        }
        self.observation = Some(o);
    }
    pub fn wrist(&self) -> Option<WristArtwork> {
        let o = self.observation?;
        let visible = o.wristrest_connected?;
        if !self.wrist_mounted {
            return None;
        }
        let s = spec(o.product_id)?;
        Some(WristArtwork {
            asset: &s.wrist.asset,
            bounds: s.wrist.bounds,
            transform_y: s.wrist.transform_y,
            visible,
        })
    }
}

// Current 679 main.194d8a7b.js RM/DM/SM (288393..288643) and Sh
// (291323..291575), main.1c5a651a.css .tip-special. These are source
// hover surfaces; clicking a dial row enters the existing staged mapping flow.
const SPECIAL_ANCHORS: [&str; 2] = ["DIAL_CLICK", "DKM_KBMK_02"];
const DIAL_KEYS: [&str; 3] = ["DIAL_LEFT_ROTATION", "DIAL_CLICK", "DIAL_RIGHT_ROTATION"];
fn special_icons(default: &str) -> Option<(&'static str, &'static str)> {
    Some(match default {
        "VOLUME_DOWN" => (
            "icon_scrolldown.96292d1b.svg",
            "icon_scrolldown_a.4224bc6a.svg",
        ),
        "MUTE_VOLUME" => (
            "icon_mutevolume.00860e32.svg",
            "icon_mutevolume_a.e5e1d147.svg",
        ),
        "VOLUME_UP" => ("icon_scrollup.d4ceff30.svg", "icon_scrollup_a.b399c705.svg"),
        "PLAY_PAUSE" => (
            "icon_playandpause.30c146c6.svg",
            "icon_playandpause.30c146c6.svg",
        ),
        "NEXT_TRACK" => ("icon_tapdouble.e5332a5c.svg", "icon_tapdouble.e5332a5c.svg"),
        "PREVIOUS_TRACK" => ("icon_taptriple.b652d4b5.svg", "icon_taptriple.b652d4b5.svg"),
        _ => return None,
    })
}
fn special_label(name: &str) -> String {
    // Exact SM.getLocalizationMapping dictionary; custom mapping names stay literal.
    let key = name.replace([' ', '/'], "_").to_uppercase();
    const LOCALIZED: &[&str] = &[
        "ACTUATION_POINT_ADJUSTMENT_MODE",
        "CYCLE_APPS",
        "CYCLE_UP_PROFILE",
        "DELETE",
        "DOWN",
        "LED_TOGGLE_INDICATORS",
        "LEFT",
        "MAIL",
        "MENU",
        "MUTE_VOLUME",
        "NEXT_TRACK",
        "OPEN_XBOX_GAME_BAR",
        "PAUSE",
        "PLAY_PAUSE",
        "PREVIOUS_TRACK",
        "PRINT_SCREEN",
        "RAZER_HYPERSHIFT",
        "RIGHT",
        "RIGHT_ALT",
        "RIGHT_CTRL",
        "RIGHT_SHIFT",
        "SLEEP",
        "SNAP_TAP_TOGGLE",
        "THIS_PC",
        "TOGGLE_BATTERY_CHARGE_OVERRIDE",
        "TOGGLE_FAN_SPEED_MODE",
        "TOGGLE_GAME_MODE",
        "TOGGLE_MACRO_MODE",
        "TOGGLE_OUTPUT",
        "TOGGLE_PERFORMANCE_MODE",
        "TOGGLE_REFRESH_RATE",
        "TOGGLE_TRACKPAD",
        "UP",
        "VOLUME_DOWN",
        "VOLUME_UP",
    ];
    if LOCALIZED.contains(&key.as_str()) {
        return t(&key);
    }
    match name {
        "Decrease Brightness" => t("BRIGHTNESS_DOWN"),
        "Increase Brightness" => t("BRIGHTNESS_UP"),
        _ => name.to_owned(),
    }
}
impl KeyboardProductWorkspace {
    pub(super) fn hover_source_special(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.spec.product_id != 679 || self.page != "TAB_CUSTOMIZE" {
            return;
        }
        if let Some(index) = SPECIAL_ANCHORS.iter().position(|anchor| *anchor == key) {
            if !self.source_artwork.special_shown[index] {
                self.source_artwork.special_shown[index] = true;
                cx.notify();
            }
        }
    }
    pub(super) fn source_special_pointer(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 679 {
            return;
        }
        let selected_dial = self.selected_key.as_deref().is_some_and(|key| {
            DIAL_KEYS.contains(&key) || ["ScrollDown", "ScrollUp", "DKM_KBMK_04"].contains(&key)
        });
        let regions = self.analog_gamepad.floating.key_bounds.borrow();
        let mut changed = false;
        for (index, anchor) in SPECIAL_ANCHORS.iter().enumerate() {
            let over_anchor = regions
                .values()
                .any(|(key, bounds)| key.id == *anchor && bounds.contains(&position));
            let over_popup = self.source_artwork.special_bounds[index]
                .get()
                .contains(&position);
            if self.source_artwork.special_shown[index]
                && !over_anchor
                && !over_popup
                && !(index == 0 && selected_dial)
            {
                self.source_artwork.special_shown[index] = false;
                self.source_artwork.pressed_special = None;
                changed = true;
            }
        }
        drop(regions);
        if changed {
            cx.notify();
        }
    }
    pub(super) fn source_special_panels(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        use razer_widgets::theme::KeyboardSpecialColors as C;
        if self.spec.product_id != 679 || self.page != "TAB_CUSTOMIZE" {
            return Vec::new();
        }
        let Some(observation) = self.source_artwork.observation() else {
            return Vec::new();
        };
        let selected_dial = self.selected_key.as_deref().is_some_and(|key| {
            DIAL_KEYS.contains(&key) || ["ScrollDown", "ScrollUp", "DKM_KBMK_04"].contains(&key)
        });
        let mut panels = Vec::new();
        for (index, anchor) in SPECIAL_ANCHORS.iter().enumerate() {
            if !self.source_artwork.special_shown[index] && !(index == 0 && selected_dial) {
                continue;
            }
            let Some(key) = self.keyboard_shapes().iter().find(|key| key.id == *anchor) else {
                continue;
            };
            let Some(artwork) = (if index == 0 {
                dial_image(observation)
            } else {
                media_image(observation)
            }) else {
                continue;
            };
            let bounds = self.source_artwork.special_bounds[index].clone();
            let [x, y, _, _] = key.bounds;
            let mut rows = v_flex()
                .absolute()
                .top(surface::css(if index == 0 { 55. } else { 50. }))
                .left(surface::css(10.));
            for raw in self.mapping_buttons() {
                let default = raw["defaultValue"].as_str().unwrap_or_default();
                let is_media = matches!(default, "PLAY_PAUSE" | "NEXT_TRACK" | "PREVIOUS_TRACK");
                if if index == 0 {
                    !raw["display"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty())
                } else {
                    !is_media
                } {
                    continue;
                }
                let Some((normal_icon, pressed_icon)) = special_icons(default) else {
                    continue;
                };
                let button = self.mapping_active_button(raw);
                let id = button["buttonKey"].as_str().unwrap_or_default().to_owned();
                let mapped = button
                    .get("assignmentGroup")
                    .is_some_and(|value| !value.is_null());
                let name = button["assignmentValue"].as_str().unwrap_or(default);
                let label = special_label(name);
                let disabled_color = name == "DISABLE";
                let color = if mapped {
                    if self.hypershift { C::hyper() } else { C::accent() }
                } else if disabled_color {
                    C::disabled()
                } else {
                    C::text()
                };
                let icon = if self.source_artwork.pressed_special.as_deref() == Some(&id) {
                    pressed_icon
                } else {
                    normal_icon
                };
                let pressed_id = id.clone();
                let selected = self.selected_key.as_deref() == Some(&id);
                let row =
                    BaseButton::new(SharedString::from(format!("keyboard-special-{index}-{id}")))
                        .relative()
                        .h(surface::css(30.))
                        .ml(surface::css(if index == 0 { 40. } else { 35. }))
                        .mb(surface::css(10.))
                        .px(surface::css(10.))
                        .border_1()
                        .border_color(if selected { C::accent() } else { C::transparent() })
                        .rounded(surface::css(3.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .text_color(color)
                        .hover(|style| style.bg(C::hover()))
                        .active(|style| style.bg(C::background()).border_color(C::accent()))
                        .focus_visible(|style| style.border_color(C::accent()))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.source_artwork.pressed_special = Some(pressed_id.clone());
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.source_artwork.pressed_special = None;
                                cx.notify();
                            }),
                        )
                        .child(
                            img(SharedString::from(format!(
                                "synapse/keyboard679-current/{icon}"
                            )))
                            .absolute()
                            .left(surface::css(if index == 0 { -30. } else { -35. }))
                            .top(surface::css(5.))
                            .w(surface::css(if index == 0 { 20. } else { 33. }))
                            .h(surface::css(20.)),
                        )
                        .child(
                            h_flex()
                                .h_full()
                                .items_center()
                                .max_w(surface::css(155.))
                                .when(index == 1, |row| row.ml(surface::css(10.)))
                                .overflow_hidden()
                                .child(label),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            // Sh explicitly excludes the three immutable media-tap keys.
                            if index == 0 {
                                this.analog_mapping_next(
                                    super::analog_gamepad::MappingNext::Key(id.clone(), false),
                                    window,
                                    cx,
                                );
                            }
                        }));
                rows = rows.child(row);
            }
            let border = if index == 0 {
                if self.hypershift { C::hyper() } else { C::accent() }
            } else {
                C::border()
            };
            let popup = div()
                .id(SharedString::from(format!("keyboard-special-{index}")))
                .absolute()
                .left(surface::css(x - if index == 0 { 15. } else { 25. }))
                .top(surface::css(y - if index == 0 { 20. } else { 25. }))
                .w(surface::css(234.))
                .h(surface::css(175.))
                .p(surface::css(10.))
                .rounded(surface::css(5.))
                .bg(C::background())
                .shadow(vec![BoxShadow {
                    color: C::shadow(),
                    inset: false,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(20.),
                    spread_radius: px(0.),
                }])
                .on_prepaint(move |rect, _, _| bounds.set(rect))
                .child(
                    img(artwork)
                        .absolute()
                        .left(surface::css(if index == 0 { 10. } else { 21. }))
                        .top(surface::css(if index == 0 { 7. } else { 12. }))
                        .size(surface::css(if index == 0 { 38. } else { 20. })),
                )
                .child(
                    div()
                        .absolute()
                        .left(surface::css(if index == 0 { 11. } else { 21. }))
                        .top(surface::css(if index == 0 { 7. } else { 12. }))
                        .size(surface::css(if index == 0 { 36. } else { 20. }))
                        .rounded_full()
                        .border(surface::css(2.))
                        .border_color(border),
                )
                .child(rows);
            panels.push(deferred(popup).with_priority(102).into_any_element());
        }
        panels
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtworkState, SourceArtworkObservation, base_image_for_scale, geometry};

    #[::core::prelude::v1::test]
    fn formal_legacy_keyboard_pages_decode_the_mounted_artwork_in_its_source_frame() {
        use super::super::{SourceBaseArtwork, layout::KeyboardImageFrame, source_product};
        use gpui_kit::{
            AssetSource as _, Bounds, Image, ImageFormat, ObjectFit, SvgRenderer, point, px, size,
        };
        use razer_model::{model::Device, product_preview::registered_preview};
        use std::{collections::BTreeMap, sync::Arc};
        let renderer = SvgRenderer::new(Arc::new(razer_assets::SynapseAssets));
        let mut decoded_assets = BTreeMap::new();
        for pid in [579, 689, 720] {
            let legacy = registered_preview(pid).unwrap();
            let mut restored: Device =
                serde_json::from_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
            restored.begin_local_session();
            let persisted = serde_json::to_value(&restored).unwrap();
            let observation = SourceArtworkObservation::for_device(&restored);
            assert_eq!((observation.edition_id, observation.layout_id), (0, 1));
            // These are the three actual image branches: normal Customize,
            // factory Customize, and the other pages' base image.
            for (customize, factory) in [(true, false), (true, true), (false, false)] {
                let frame = KeyboardImageFrame::new(
                    source_product(pid).unwrap(),
                    Some(observation),
                    customize,
                    factory,
                );
                let mounted = SourceBaseArtwork::new(observation, &frame);
                let geometry = super::geometry_for_surface(observation, customize).unwrap();
                let reference = SourceArtworkObservation {
                    layout_id: 1,
                    ..observation
                };
                assert!(std::ptr::eq(
                    geometry.keys,
                    super::geometry_for_surface(reference, customize)
                        .unwrap()
                        .keys
                ));
                assert!(std::ptr::eq(
                    super::buttons(observation, customize).unwrap(),
                    super::buttons(reference, customize).unwrap()
                ));
                for scale in [1., 1.5, 2., 3.] {
                    let artwork = mounted
                        .artwork_for_scale(scale)
                        .unwrap_or_else(|| panic!("PID {pid} formal page lost artwork"));
                    let (dimensions, visible) =
                        decoded_assets.entry(artwork.asset).or_insert_with(|| {
                            let bytes = razer_assets::SynapseAssets
                                .load(artwork.asset)
                                .unwrap()
                                .unwrap();
                            let format = if artwork.asset.ends_with(".svg") {
                                ImageFormat::Svg
                            } else {
                                ImageFormat::Png
                            };
                            let decoded = Image::from_bytes(format, bytes.into_owned())
                                .to_image_data(renderer.clone())
                                .unwrap();
                            let dimensions = decoded.size(0);
                            let mut visible = [dimensions.width.0, dimensions.height.0, 0, 0];
                            for (index, pixel) in
                                decoded.as_bytes(0).unwrap().chunks_exact(4).enumerate()
                            {
                                if pixel[3] == 0 {
                                    continue;
                                }
                                let x = (index % dimensions.width.0 as usize) as i32;
                                let y = (index / dimensions.width.0 as usize) as i32;
                                visible[0] = visible[0].min(x);
                                visible[1] = visible[1].min(y);
                                visible[2] = visible[2].max(x + 1);
                                visible[3] = visible[3].max(y + 1);
                            }
                            assert!(
                                visible[2] > visible[0] && visible[3] > visible[1],
                                "{}",
                                artwork.asset
                            );
                            (dimensions, visible)
                        });
                    let slot = Bounds::new(
                        point(px(frame.image_origin[0]), px(frame.image_origin[1])),
                        size(px(frame.image_size[0]), px(frame.image_size[1])),
                    );
                    let fitted = ObjectFit::Contain.get_bounds(slot, *dimensions);
                    let visible_width = fitted.size.width
                        * ((visible[2] - visible[0]) as f32 / dimensions.width.0 as f32);
                    let visible_height = fitted.size.height
                        * ((visible[3] - visible[1]) as f32 / dimensions.height.0 as f32);
                    assert!(
                        visible_width > px(500.) && visible_height > px(100.),
                        "PID {pid}: {visible_width:?} x {visible_height:?}"
                    );
                    assert!(
                        fitted.size.width <= slot.size.width
                            && fitted.size.height <= slot.size.height
                    );
                    assert!(geometry.keys.iter().all(|key| {
                        let [x, y, width, height] = key.bounds;
                        x.is_finite()
                            && y.is_finite()
                            && width > 0.
                            && height > 0.
                            && x * frame.source_scale < frame.size[0]
                            && y * frame.source_scale < frame.size[1]
                    }));
                }
            }
            assert_eq!(serde_json::to_value(&restored).unwrap(), persisted);
        }
    }

    #[::core::prelude::v1::test]
    fn formal_keyboard_preview_variants_share_image_and_key_region_identity() {
        use super::super::{SourceBaseArtwork, layout::KeyboardImageFrame, source_product};
        use razer_model::product_preview::registered_preview;
        let mut variants = 0;
        for product in &super::atlas().products {
            let mut device = registered_preview(product.product_id).unwrap();
            for variant in std::iter::once(None).chain(product.source_variants.iter().map(Some)) {
                if let Some(variant) = variant {
                    device.edition_id = variant.edition_id;
                    device.layout_id = variant.layout_id;
                    device.serial_number = format!(
                        "PREVIEW-{}-{}-{}",
                        product.product_id, variant.edition_id, variant.layout_id
                    );
                    device.device_container_id = format!(
                        "preview-{}-{}-{}",
                        product.product_id, variant.edition_id, variant.layout_id
                    );
                }
                let observation = SourceArtworkObservation::for_device(&device);
                for (customize, factory) in [(true, false), (true, true), (false, false)] {
                    let frame = KeyboardImageFrame::new(
                        source_product(product.product_id).unwrap(),
                        Some(observation),
                        customize,
                        factory,
                    );
                    let mounted = SourceBaseArtwork::new(observation, &frame);
                    assert!(
                        [1., 3.]
                            .into_iter()
                            .all(|scale| mounted.artwork_for_scale(scale).is_some()),
                        "PID {}: {:?}",
                        product.product_id,
                        observation
                    );
                    let geometry = super::geometry_for_surface(observation, customize).unwrap();
                    assert!(!geometry.keys.is_empty());
                    let buttons = super::buttons(observation, customize).unwrap();
                    assert!(
                        geometry
                            .keys
                            .iter()
                            .all(|key| buttons.iter().any(|button| button["inputID"] == key.id))
                    );
                }
                variants += 1;
            }
        }
        assert_eq!(variants, 1164);
    }

    #[::core::prelude::v1::test]
    fn formal_keyboard_preview_compatibility_never_rewrites_observed_or_unknown_identity() {
        use razer_model::{
            model::DeviceConnectionObservation, product_preview::registered_preview,
        };
        let original = registered_preview(579).unwrap();
        for invalid in 0..7 {
            let mut device = original.clone();
            match invalid {
                0 => device.dashboard.local_snapshot = false,
                1 => {
                    device.dashboard.connection_observation =
                        Some(DeviceConnectionObservation::UsbPresent)
                }
                2 => device.serial_number = "physical-device".into(),
                3 => device.device_container_id = "physical-container".into(),
                4 => device.layout_id = u32::MAX,
                5 => device.edition_id = u32::MAX,
                _ => {
                    device.dashboard.readonly_keyboard_metadata =
                        Some(razer_device::keyboard_metadata::KeyboardMetadataReading {
                            keyboard_layout: 0,
                            edition_id: 0,
                            firmware_id: None,
                        })
                }
            }
            let observation = SourceArtworkObservation::for_device(&device);
            assert_eq!(
                (observation.edition_id, observation.layout_id),
                (device.edition_id, device.layout_id)
            );
            assert!(base_image_for_scale(observation, 1.).is_none());
        }
        let mut selected = original;
        selected.layout_id = 12;
        let observation = SourceArtworkObservation::for_device(&selected);
        assert_eq!(observation.layout_id, 12);
        assert!(base_image_for_scale(observation, 1.).is_some());
    }

    #[::core::prelude::v1::test]
    fn formal_keyboard_single_density_context_keeps_the_real_img_candidate() {
        use super::super::{SourceBaseArtwork, layout::KeyboardImageFrame, source_product};
        let mut selected = razer_model::product_preview::registered_preview(605).unwrap();
        selected.serial_number = "PREVIEW-605-0-0".into();
        selected.device_container_id = "preview-605-0-0".into();
        let observation = SourceArtworkObservation::for_device(&selected);
        assert_eq!(observation.layout_id, 0);
        let product = super::family_product(605).unwrap();
        assert!(product.images.iter().all(|image| {
            image.edition_id != 0 || image.layout_id != 0 || image.resolution != 3
        }));
        for (customize, factory) in [(true, false), (true, true), (false, false)] {
            let frame = KeyboardImageFrame::new(
                source_product(605).unwrap(),
                Some(observation),
                customize,
                factory,
            );
            let mounted = SourceBaseArtwork::new(observation, &frame);
            for scale in [1., 1.5, 2., 3.] {
                assert_eq!(
                    mounted.artwork_for_scale(scale).unwrap().asset,
                    "synapse/keyboard-current/605/0-1x.3ed8699b.png"
                );
            }
        }
        for scale in [1., 3.] {
            assert!(
                base_image_for_scale(
                    SourceArtworkObservation {
                        layout_id: u32::MAX,
                        ..observation
                    },
                    scale
                )
                .is_none()
            );
        }
    }

    #[::core::prelude::v1::test]
    fn formal_keyboard_physical_regions_build_visible_paths_and_hit_their_interiors() {
        use crate::keyboard_geometry::KeyboardKeyExt as _;
        use gpui_kit::{Bounds, point, px, size};
        let mut checked = 0;
        for product in &super::atlas().products {
            for layout in &product.layouts {
                let geometry = &super::atlas().geometries[layout.geometry];
                for key in &geometry.keys {
                    for scale in [1., 1.25, 1.5, 2.] {
                        let [x, y, width, height] = key.bounds;
                        // Same source coordinates and viewBox scaling used by
                        // keyboard_image; offset retains nonzero window space.
                        let bounds = Bounds::new(
                            point(px(300. + x * scale), px(180. + y * scale)),
                            size(px(width * scale), px(height * scale)),
                        );
                        for stroke in [false, true] {
                            let path = key.path(bounds, stroke).build().unwrap_or_else(|error| panic!("PID {} layout {} key {} scale {scale} stroke {stroke}: {error}", product.product_id, layout.layout_id, key.id));
                            assert!(
                                !path.vertices.is_empty(),
                                "PID {} layout {} key {} scale {scale} stroke {stroke}: no painted triangles",
                                product.product_id,
                                layout.layout_id,
                                key.id
                            );
                            let mut painted = [
                                f32::INFINITY,
                                f32::INFINITY,
                                f32::NEG_INFINITY,
                                f32::NEG_INFINITY,
                            ];
                            for vertex in &path.vertices {
                                let x = f32::from(vertex.xy_position.x);
                                let y = f32::from(vertex.xy_position.y);
                                assert!(
                                    x.is_finite() && y.is_finite(),
                                    "PID {} layout {} key {} scale {scale} stroke {stroke}: nonfinite painted vertex",
                                    product.product_id,
                                    layout.layout_id,
                                    key.id
                                );
                                painted[0] = painted[0].min(x);
                                painted[1] = painted[1].min(y);
                                painted[2] = painted[2].max(x);
                                painted[3] = painted[3].max(y);
                            }
                            let expected = [
                                f32::from(bounds.left()),
                                f32::from(bounds.top()),
                                f32::from(bounds.right()),
                                f32::from(bounds.bottom()),
                            ];
                            // The locked tessellator's default stroke miter
                            // limit is four half-widths. Fill geometry stays
                            // within the original SVG bounds, up to its curve
                            // flattening tolerance and f32 window projection.
                            let tolerance = if stroke { 4.1 * scale } else { 0.2 };
                            assert!(
                                painted[0] >= expected[0] - tolerance
                                    && painted[1] >= expected[1] - tolerance
                                    && painted[2] <= expected[2] + tolerance
                                    && painted[3] <= expected[3] + tolerance,
                                "PID {} layout {} key {} scale {scale} stroke {stroke}: painted {painted:?}, expected {expected:?}",
                                product.product_id,
                                layout.layout_id,
                                key.id
                            );
                            for (actual, expected) in painted.into_iter().zip(expected) {
                                assert!(
                                    (actual - expected).abs() <= tolerance.max(0.5),
                                    "PID {} layout {} key {} scale {scale} stroke {stroke}: shifted/shortened painted edge {actual}, expected {expected}",
                                    product.product_id,
                                    layout.layout_id,
                                    key.id
                                );
                            }
                        }
                        // A concave ISO Enter may exclude the bounding center;
                        // every physical shape still needs an interior hit.
                        assert!(
                            [0.2, 0.5, 0.8]
                                .into_iter()
                                .any(|x| [0.2, 0.5, 0.8].into_iter().any(|y| key
                                    .contains_window_point(
                                        bounds.origin
                                            + point(bounds.size.width * x, bounds.size.height * y),
                                        bounds
                                    ))),
                            "PID {} layout {} key {} has no interior hit",
                            product.product_id,
                            layout.layout_id,
                            key.id
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 100_000);
    }

    fn observed() -> SourceArtworkObservation {
        SourceArtworkObservation {
            product_id: 679,
            edition_id: 128,
            layout_id: 1,
            wristrest_connected: None,
        }
    }

    #[::core::prelude::v1::test]
    fn source_artwork_retries_edition_without_substituting_layout() {
        let mut observation = observed();
        observation.edition_id = 9999;
        let image = base_image_for_scale(observation, 1.).unwrap();
        let original = base_image_for_scale(
            SourceArtworkObservation {
                edition_id: 0,
                ..observation
            },
            1.,
        )
        .unwrap();
        assert_eq!(image.asset, original.asset);
        observation.layout_id = 9999;
        assert!(base_image_for_scale(observation, 1.).is_none());
        let product = super::family_product(679).unwrap();
        assert_eq!(
            geometry(observation).unwrap().keys.len(),
            super::atlas().geometries[product.default_geometry]
                .keys
                .len()
        );
    }

    #[::core::prelude::v1::test]
    fn source_artwork_selects_original_density_candidates() {
        let low = base_image_for_scale(observed(), 1.).unwrap();
        let high = base_image_for_scale(observed(), 2.).unwrap();
        assert!(low.asset.contains("-1x."));
        assert!(high.asset.contains("-3x."));
        assert_eq!(low.css_size, high.css_size);
        let expected = super::family_product(679)
            .unwrap()
            .images
            .iter()
            .find(|image| {
                image.edition_id == 128
                    && image.layout_id == 1
                    && image.resolution == 3
                    && !image.svg
            })
            .unwrap();
        assert_eq!(high.asset, expected.asset);
    }

    #[::core::prelude::v1::test]
    fn current_family_covers_keyboard_layouts_with_complete_button_metadata() {
        let atlas = super::atlas();
        assert_eq!(atlas.products.len(), 69);
        for product in &atlas.products {
            assert!(!product.images.is_empty(), "{}", product.product_id);
            for layout in &product.layouts {
                let geometry = &atlas.geometries[layout.geometry];
                assert!(!geometry.keys.is_empty());
                for key in &geometry.keys {
                    assert!(
                        geometry
                            .buttons
                            .iter()
                            .any(|button| button["inputID"] == key.id)
                    );
                    assert!(key.bounds.iter().all(|value| value.is_finite()));
                    assert!(key.bounds[2] > 0. && key.bounds[3] > 0.);
                }
            }
            assert!(!atlas.geometries[product.default_geometry].keys.is_empty());
        }
    }

    #[::core::prelude::v1::test]
    fn current_family_retries_only_edition_for_the_same_base_layout() {
        for pid in [515, 550, 654, 679, 691, 708, 721, 724, 741, 756] {
            let observation = SourceArtworkObservation {
                product_id: pid,
                edition_id: 9999,
                layout_id: 1,
                wristrest_connected: None,
            };
            let zero = SourceArtworkObservation {
                edition_id: 0,
                ..observation
            };
            for scale in [1., 2.] {
                assert_eq!(
                    base_image_for_scale(observation, scale).unwrap().asset,
                    base_image_for_scale(zero, scale).unwrap().asset
                );
                assert!(
                    base_image_for_scale(
                        SourceArtworkObservation {
                            layout_id: 9999,
                            ..observation
                        },
                        scale
                    )
                    .is_none()
                );
            }
        }
    }

    #[::core::prelude::v1::test]
    fn current_family_keeps_factory_svg_separate_from_customize_filter() {
        for pid in [691, 720, 721, 741] {
            let observation = SourceArtworkObservation {
                product_id: pid,
                edition_id: 0,
                layout_id: 1,
                wristrest_connected: None,
            };
            let base = base_image_for_scale(observation, 2.).unwrap();
            let customize = super::customize_image_for_scale(observation, 2.).unwrap();
            assert!(base.asset.ends_with(".svg"));
            assert!(customize.asset.ends_with(".svg"));
            assert!(!base.asset.contains("-customize"));
            assert!(customize.asset.contains("-customize"), "{pid}");
            assert_ne!(base.asset, customize.asset);
        }
        let unchanged = SourceArtworkObservation {
            product_id: 756,
            edition_id: 0,
            layout_id: 1,
            wristrest_connected: None,
        };
        assert_eq!(
            base_image_for_scale(unchanged, 1.).unwrap().asset,
            super::customize_image_for_scale(unchanged, 1.)
                .unwrap()
                .asset
        );
    }

    #[::core::prelude::v1::test]
    fn current_family_loads_every_registered_base_and_customize_image_without_a_window() {
        use gpui_kit::AssetSource as _;
        let mut checked = std::collections::BTreeSet::new();
        for product in &super::atlas().products {
            for image in product.images.iter().chain(&product.customize_images) {
                if !checked.insert(image.asset.as_str()) {
                    continue;
                }
                let bytes = razer_assets::SynapseAssets
                    .load(&image.asset)
                    .unwrap()
                    .unwrap_or_else(|| panic!("Missing current keyboard artwork: {}", image.asset));
                assert!(!bytes.is_empty(), "{}", image.asset);
                if image.svg {
                    let xml = std::str::from_utf8(&bytes).unwrap();
                    assert!(
                        xml.contains("<svg") && xml.contains("</svg>"),
                        "{}",
                        image.asset
                    );
                } else {
                    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{}", image.asset);
                    assert!(
                        bytes.len() >= 33 && &bytes[12..16] == b"IHDR",
                        "{}",
                        image.asset
                    );
                    assert!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()) > 0);
                    assert!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()) > 0);
                }
            }
        }
        assert!(checked.len() > 1000);
    }

    #[::core::prelude::v1::test]
    fn current_family_uses_real_japanese_keys_and_derived_french_german_labels() {
        let japanese = SourceArtworkObservation {
            product_id: 515,
            edition_id: 0,
            // Current 515 layout enum module 13254: Japanese=12,
            // TraditionalChinese=8; selector 14551 chooses the literal U list.
            layout_id: 12,
            wristrest_connected: None,
        };
        let chinese = SourceArtworkObservation {
            layout_id: 8,
            ..japanese
        };
        assert!(base_image_for_scale(japanese, 1.).is_some());
        for (id, label) in [
            ("KEY_BEFORE_BACKSPACE", "¥"),
            ("KEY_NON_US_POUND", "@"),
            ("KEY_BEFORE_LEFT_SHIFT", "\\"),
            ("KEY_BEFORE_SPACE", "無変換"),
            ("KEY_AFTER_SPACE", "変換"),
            ("KEY_BEFORE_RIGHT_ALT", "ひらがな"),
        ] {
            let key = geometry(japanese)
                .unwrap()
                .keys
                .iter()
                .find(|key| key.id == id)
                .unwrap_or_else(|| panic!("Missing current Japanese physical key: {id}"));
            assert_eq!(key.label, label);
            let button = super::buttons(japanese, true)
                .unwrap()
                .iter()
                .find(|button| button["inputID"] == id)
                .unwrap_or_else(|| panic!("Missing current Japanese mapping metadata: {id}"));
            assert_eq!(button["counter"], label);
            assert!(
                geometry(chinese)
                    .unwrap()
                    .keys
                    .iter()
                    .all(|key| key.id != id)
            );
        }
        for (layout, input, expected) in [(3, "KEY_Y", "Z"), (4, "KEY_A", "Q")] {
            let observation = SourceArtworkObservation {
                product_id: 708,
                layout_id: layout,
                ..japanese
            };
            let buttons = super::buttons(observation, true).unwrap();
            assert_eq!(
                buttons
                    .iter()
                    .find(|button| button["inputID"] == input)
                    .unwrap()["counter"],
                expected
            );
            assert_eq!(
                buttons
                    .iter()
                    .filter(|button| button["buttonKey"] == "DIAL_CLICK")
                    .count(),
                1
            );
            assert!(
                geometry(observation)
                    .unwrap()
                    .keys
                    .iter()
                    .any(|key| key.id == input)
            );
        }
    }

    #[::core::prelude::v1::test]
    fn source_preview_layouts_preserve_declared_variants_and_prepared_density_identity() {
        use gpui_kit::AssetSource as _;
        for product in &super::atlas().products {
            let layouts = super::source_preview_layouts(product.product_id);
            assert!(!layouts.is_empty(), "{}", product.product_id);
            for source in product
                .images
                .iter()
                .filter(|image| image.svg == product.image_mode_svg)
            {
                assert!(layouts.contains(&source.layout_id));
                let edition_id = source.edition_id;
                let layout_id = source.layout_id;
                assert!(
                    super::source_preview_layouts_for_edition(product.product_id, edition_id)
                        .unwrap()
                        .contains(&layout_id)
                );
                let observation = SourceArtworkObservation {
                    product_id: product.product_id,
                    edition_id,
                    layout_id,
                    wristrest_connected: None,
                };
                for scale in [source.resolution as f32] {
                    for image in [
                        base_image_for_scale(observation, scale),
                        super::customize_image_for_scale(observation, scale),
                    ] {
                        let image = image.unwrap_or_else(|| {
                            panic!(
                                "Preview image missing for {}:{edition_id}:{layout_id}@{scale}",
                                product.product_id
                            )
                        });
                        assert!(
                            razer_assets::SynapseAssets
                                .load(image.asset)
                                .unwrap()
                                .is_some(),
                            "{}",
                            image.asset
                        );
                    }
                }
            }
            for variant in &product.source_variants {
                assert!(
                    super::source_preview_layouts_for_edition(
                        product.product_id,
                        variant.edition_id
                    )
                    .unwrap()
                    .contains(&variant.layout_id)
                );
            }
        }
        assert!(super::source_preview_layouts(u32::MAX).is_empty());
        assert_eq!(super::source_preview_layouts(756), vec![1]);
        assert!(!super::source_preview_layouts(574).contains(&6));
    }

    #[::core::prelude::v1::test]
    fn named_product_images_keep_metadata_domains_and_verified_edition_retry() {
        for (product_id, edition_id) in [(221, 128), (224, 130), (777, 129), (3942, 128)] {
            assert_eq!(
                super::source_preview_layouts_for_edition(product_id, edition_id),
                None
            );
            let observation = SourceArtworkObservation {
                product_id,
                edition_id,
                layout_id: 0,
                wristrest_connected: None,
            };
            let fallback = super::source_preview_image_for_scale(observation, 1.);
            assert!(fallback.is_some(), "PID {product_id} edition {edition_id}");
            assert_eq!(
                fallback,
                super::source_preview_image_for_scale(
                    SourceArtworkObservation {
                        edition_id: 0,
                        ..observation
                    },
                    1.,
                )
            );
        }
        for (product_id, edition_id) in [(711, 129), (736, 131), (737, 129), (737, 131)] {
            assert_eq!(
                super::source_preview_layouts_for_edition(product_id, edition_id),
                super::source_preview_layouts_for_edition(product_id, 0),
            );
        }
        assert_eq!(super::source_preview_layouts_for_edition(769, 0), None);
        let hue = SourceArtworkObservation {
            product_id: 769,
            edition_id: 0,
            layout_id: 0,
            wristrest_connected: None,
        };
        assert_eq!(
            super::source_preview_image_for_scale(hue, 1.),
            Some("synapse/preview-source-769-0-0-1x.svg"),
        );
    }

    #[::core::prelude::v1::test]
    fn source_customize_normalizes_unknown_layout_only_in_the_verified_svg_branch() {
        let mut normalized_products = 0;
        for product in &super::atlas().products {
            let observation = SourceArtworkObservation {
                product_id: product.product_id,
                edition_id: 0,
                layout_id: u32::MAX,
                wristrest_connected: None,
            };
            assert!(base_image_for_scale(observation, 1.).is_none());
            let actual = super::customize_image_for_scale(observation, 1.);
            if let Some(normalization) = &product.customize_layout_normalization {
                normalized_products += 1;
                let expected = SourceArtworkObservation {
                    layout_id: normalization.united_states,
                    ..observation
                };
                assert_eq!(
                    actual.unwrap().asset,
                    super::customize_image_for_scale(expected, 1.)
                        .unwrap()
                        .asset,
                    "{}",
                    product.product_id
                );
                assert!(std::ptr::eq(
                    super::geometry_for_surface(observation, true).unwrap().keys,
                    super::geometry_for_surface(expected, true).unwrap().keys,
                ));
            } else {
                assert!(actual.is_none(), "{}", product.product_id);
            }
        }
        assert!(normalized_products > 0);
    }

    #[::core::prelude::v1::test]
    fn current_svg_normalization_preserves_requested_language_and_raw_actuation_layout() {
        let observation = SourceArtworkObservation {
            product_id: 720,
            edition_id: 0,
            layout_id: 3,
            wristrest_connected: None,
        };
        let raw = base_image_for_scale(observation, 1.).unwrap();
        let customize = super::customize_image_for_scale(observation, 1.).unwrap();
        let product = super::family_product(720).unwrap();
        let selected = product
            .customize_images
            .iter()
            .find(|image| image.edition_id == 0 && image.layout_id == 3)
            .unwrap();
        assert_eq!(selected.asset, customize.asset);
        assert!(customize.asset.contains("customize-layout-3"));
        assert_ne!(raw.asset, customize.asset);
        let raw_geometry = super::geometry_for_surface(observation, false).unwrap();
        let customize_geometry = super::geometry_for_surface(observation, true).unwrap();
        assert!(raw_geometry.keys.iter().any(|key| key.id == "KEY_Y"));
        assert!(customize_geometry.keys.iter().any(|key| key.id == "KEY_Y"));
        let unnormalized = SourceArtworkObservation {
            product_id: 691,
            layout_id: 9999,
            ..observation
        };
        assert!(super::customize_image_for_scale(unnormalized, 1.).is_none());
    }

    #[::core::prelude::v1::test]
    fn source_artwork_wrist_requires_observation_and_retains_hidden_node() {
        let mut state = ArtworkState::default();
        let mut observation = observed();
        state.observe(observation);
        assert!(state.wrist().is_none());
        observation.wristrest_connected = Some(false);
        state.observe(observation);
        assert!(state.wrist().is_none());
        observation.wristrest_connected = Some(true);
        state.observe(observation);
        assert!(state.wrist().unwrap().visible);
        observation.wristrest_connected = Some(false);
        state.observe(observation);
        assert!(!state.wrist().unwrap().visible);
        state = ArtworkState::default();
        assert!(state.wrist().is_none());
    }
}

// Generated catalog reader: razer-pages/keyboard_artwork_current_data.
fn read_keyboard_artwork_current_catalog() -> FamilyAtlas {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FamilyAtlas> {
        let value = match r.tag()? {
            0 => FamilyAtlas {
                products: r.child(decode_1)?,
                geometries: r.child(decode_18)?,
                preview_products: r.child(decode_34)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<FamilyProduct>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FamilyProduct> {
        let value = match r.tag()? {
            0 => FamilyProduct {
                product_id: r.child(decode_3)?,
                image_css_size: r.child(decode_4)?,
                image_mode_svg: r.child(decode_6)?,
                images: r.child(decode_7)?,
                customize_images: r.child(decode_7)?,
                customize_layout_normalization: None,
                source_supported_layout_ids: r.child(decode_10)?,
                source_variants: r.child(decode_11)?,
                source_one_x_only_variants: Default::default(),
                default_geometry: r.child(decode_13)?,
                layouts: r.child(decode_14)?,
            },
            1 => FamilyProduct {
                product_id: r.child(decode_3)?,
                image_css_size: r.child(decode_4)?,
                image_mode_svg: r.child(decode_6)?,
                images: r.child(decode_7)?,
                customize_images: r.child(decode_7)?,
                customize_layout_normalization: None,
                source_supported_layout_ids: r.child(decode_10)?,
                source_variants: r.child(decode_11)?,
                source_one_x_only_variants: r.child(decode_11)?,
                default_geometry: r.child(decode_13)?,
                layouts: r.child(decode_14)?,
            },
            2 => FamilyProduct {
                product_id: r.child(decode_3)?,
                image_css_size: r.child(decode_4)?,
                image_mode_svg: r.child(decode_6)?,
                images: r.child(decode_7)?,
                customize_images: r.child(decode_7)?,
                customize_layout_normalization: r.child(decode_16)?,
                source_supported_layout_ids: r.child(decode_10)?,
                source_variants: r.child(decode_11)?,
                source_one_x_only_variants: Default::default(),
                default_geometry: r.child(decode_13)?,
                layouts: r.child(decode_14)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 2]> {
        let value = match r.tag()? {
            0 => r.array::<_, 2>(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<FamilyImage>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FamilyImage> {
        let value = match r.tag()? {
            0 => FamilyImage {
                edition_id: r.child(decode_3)?,
                layout_id: r.child(decode_3)?,
                resolution: r.child(decode_3)?,
                svg: r.child(decode_6)?,
                asset: r.child(decode_9)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SourceVariant>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceVariant> {
        let value = match r.tag()? {
            0 => SourceVariant {
                edition_id: r.child(decode_3)?,
                layout_id: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<FamilyLayout>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_15)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FamilyLayout> {
        let value = match r.tag()? {
            0 => FamilyLayout {
                layout_id: r.child(decode_3)?,
                edition_id: r.child(decode_3)?,
                customize: r.child(decode_6)?,
                geometry: r.child(decode_13)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<CustomizeLayoutNormalization>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_17)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<CustomizeLayoutNormalization> {
        let value = match r.tag()? {
            0 => CustomizeLayoutNormalization {
                ansi: r.child(decode_10)?,
                iso: r.child(decode_10)?,
                japanese: r.child(decode_3)?,
                united_states: r.child(decode_3)?,
                united_kingdom: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<FamilyGeometry>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_19)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FamilyGeometry> {
        let value = match r.tag()? {
            0 => FamilyGeometry {
                viewbox: r.child(decode_4)?,
                keys: r.child(decode_20)?,
                buttons: r.child(decode_32)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<razer_assets::KeyboardKey>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_21)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardKey> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardKey {
                id: r.child(decode_22)?,
                label: r.child(decode_22)?,
                enabled: r.child(decode_23)?,
                functions: r.child(decode_24)?,
                bounds: r.child(decode_25)?,
                geometry: r.child(decode_27)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_22(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_23(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_24(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_22)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_25(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 4]> {
        let value = match r.tag()? {
            0 => r.array::<_, 4>(decode_26)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_26(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_27(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardGeometry> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardGeometry::Path {
                commands: r.child(decode_28)?,
            },
            1 => razer_assets::KeyboardGeometry::Circle {
                center: r.child(decode_30)?,
                radius: r.child(decode_26)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_28(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<razer_assets::KeyboardPathCommand>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_29)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_29(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardPathCommand> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardPathCommand::Move(r.child(decode_30)?),
            1 => razer_assets::KeyboardPathCommand::Line(r.child(decode_30)?),
            2 => razer_assets::KeyboardPathCommand::Curve(r.child(decode_31)?),
            3 => razer_assets::KeyboardPathCommand::Close,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_30(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 2]> {
        let value = match r.tag()? {
            0 => r.array::<_, 2>(decode_26)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_31(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[[f32; 2]; 3]> {
        let value = match r.tag()? {
            0 => r.array::<_, 3>(decode_30)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_32(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_33)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_33(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_34(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<PreviewProduct>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_35)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_35(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PreviewProduct> {
        let value = match r.tag()? {
            0 => PreviewProduct {
                product_id: r.child(decode_3)?,
                source_variants: r.child(decode_11)?,
                images: Default::default(),
                edition_zero_retry: Default::default(),
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_ARTWORK_CURRENT_DATA,
        0,
        "57ea1313da1cec978327281951ea59a454cb8db722e9488e0bacb14ba40a1b67",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_artwork_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_artwork_current_data_decodes() {
        let _ = super::read_keyboard_artwork_current_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_artwork_current_data.

// Generated catalog reader: razer-pages/keyboard_preview_current_data.
fn read_keyboard_preview_current_catalog() -> PreviewAtlas {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PreviewAtlas> {
        let value = match r.tag()? {
            0 => PreviewAtlas {
                products: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<PreviewProduct>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PreviewProduct> {
        let value = match r.tag()? {
            0 => PreviewProduct {
                product_id: r.child(decode_3)?,
                source_variants: r.child(decode_4)?,
                images: r.child(decode_5)?,
                edition_zero_retry: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SourceVariant>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<FamilyImage>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<FamilyImage> {
        let value = match r.tag()? {
            0 => FamilyImage {
                edition_id: r.child(decode_3)?,
                layout_id: r.child(decode_3)?,
                resolution: r.child(decode_3)?,
                svg: r.child(decode_7)?,
                asset: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceVariant> {
        let value = match r.tag()? {
            0 => SourceVariant {
                edition_id: r.child(decode_3)?,
                layout_id: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_PREVIEW_CURRENT_DATA,
        0,
        "d2e9249fdf984f1f5540061f9ec4d6374ba87407bd306a241d63c1640add8406",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_preview_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_preview_current_data_decodes() {
        let _ = super::read_keyboard_preview_current_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_preview_current_data.

// Generated catalog reader: razer-pages/keyboard_source_artwork_data.
fn read_keyboard_source_artwork_catalog() -> Spec {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Spec> {
        let value = match r.tag()? {
            0 => Spec {
                product_id: r.child(decode_1)?,
                viewbox: r.child(decode_2)?,
                image_css_size: r.child(decode_2)?,
                images: r.child(decode_4)?,
                default_keys: r.child(decode_8)?,
                layouts: r.child(decode_20)?,
                wrist: r.child(decode_22)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 2]> {
        let value = match r.tag()? {
            0 => r.array::<_, 2>(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Image>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Image> {
        let value = match r.tag()? {
            0 => Image {
                role: r.child(decode_6)?,
                edition_id: r.child(decode_1)?,
                layout_id: r.child(decode_7)?,
                resolution: r.child(decode_7)?,
                asset: r.child(decode_6)?,
            },
            1 => Image {
                role: r.child(decode_6)?,
                edition_id: r.child(decode_1)?,
                layout_id: None,
                resolution: None,
                asset: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_1)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<razer_assets::KeyboardKey>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardKey> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardKey {
                id: r.child(decode_10)?,
                label: r.child(decode_10)?,
                enabled: r.child(decode_11)?,
                functions: r.child(decode_12)?,
                bounds: r.child(decode_13)?,
                geometry: r.child(decode_15)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_10)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 4]> {
        let value = match r.tag()? {
            0 => r.array::<_, 4>(decode_14)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardGeometry> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardGeometry::Circle {
                center: r.child(decode_16)?,
                radius: r.child(decode_14)?,
            },
            1 => razer_assets::KeyboardGeometry::Path {
                commands: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 2]> {
        let value = match r.tag()? {
            0 => r.array::<_, 2>(decode_14)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<razer_assets::KeyboardPathCommand>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_18)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<razer_assets::KeyboardPathCommand> {
        let value = match r.tag()? {
            0 => razer_assets::KeyboardPathCommand::Move(r.child(decode_16)?),
            1 => razer_assets::KeyboardPathCommand::Line(r.child(decode_16)?),
            2 => razer_assets::KeyboardPathCommand::Curve(r.child(decode_19)?),
            3 => razer_assets::KeyboardPathCommand::Close,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[[f32; 2]; 3]> {
        let value = match r.tag()? {
            0 => r.array::<_, 3>(decode_16)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Layout>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_21)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Layout> {
        let value = match r.tag()? {
            0 => Layout {
                layout_id: r.child(decode_1)?,
                keys: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_22(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Wrist> {
        let value = match r.tag()? {
            0 => Wrist {
                asset: r.child(decode_6)?,
                bounds: r.child(decode_23)?,
                transform_y: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_23(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<[f32; 4]> {
        let value = match r.tag()? {
            0 => r.array::<_, 4>(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_SOURCE_ARTWORK_DATA,
        0,
        "3f68729b25358144567570ff9ddc3eeb66002dbdfa0e7409585ad296e2218bbe",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_source_artwork_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_source_artwork_data_decodes() {
        let _ = super::read_keyboard_source_artwork_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_source_artwork_data.
