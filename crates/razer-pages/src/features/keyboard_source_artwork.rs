//! Current 679 source artwork selectors. Pure state selection, no device reads.
//! The caller supplies observed edition/layout/wrist state; unavailable source
//! assets are absent rather than substituted with a guessed layout or color.
use razer_assets::KeyboardKey;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceArtworkObservation {
    pub product_id: u32,
    pub edition_id: u32,
    pub layout_id: u32,
    /// UI source event MW_ATTACHED_THUMB_MODULE supplies this payload.
    pub wristrest_connected: Option<bool>,
}

#[derive(Deserialize)]
struct Image {
    role: String,
    edition_id: u32,
    layout_id: Option<u32>,
    resolution: Option<u32>,
    asset: String,
    pixel_size: [f32; 2],
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
    let source = SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_source_artwork_data.json"))
            .expect("validated current artwork source literals")
    });
    (source.product_id == pid).then_some(source)
}

pub fn has_source(pid: u32) -> bool {
    spec(pid).is_some()
}

#[derive(Clone, Copy, Debug)]
pub struct ArtworkImage {
    pub asset: &'static str,
    pub pixel_size: [f32; 2],
    pub css_size: [f32; 2],
    pub actual_edition_id: u32,
    pub source_resolution: u32,
    pub retried_edition_zero: bool,
}

#[derive(Clone, Copy)]
pub struct ArtworkGeometry {
    pub keys: &'static [KeyboardKey],
    pub viewbox: [f32; 2],
    /// Source updatedProductWithLayoutAndEdition defaults to US for other IDs.
    pub used_source_default_group: bool,
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
                pixel_size: image.pixel_size,
                css_size: s.image_css_size,
                actual_edition_id: edition,
                source_resolution: image.resolution?,
                retried_edition_zero: edition != o.edition_id,
            });
        }
        if o.edition_id == 0 {
            break;
        }
    }
    None
}

pub fn geometry(o: SourceArtworkObservation) -> Option<ArtworkGeometry> {
    let s = spec(o.product_id)?;
    let layout = s.layouts.iter().find(|l| l.layout_id == o.layout_id);
    Some(ArtworkGeometry {
        keys: layout.map(|l| l.keys.as_slice()).unwrap_or(&s.default_keys),
        viewbox: s.viewbox,
        used_source_default_group: layout.is_none(),
    })
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
}
impl ArtworkState {
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
    pub fn clear(&mut self) {
        self.observation = None;
        self.wrist_mounted = false;
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

#[cfg(test)]
mod tests {
    use super::{ArtworkState, SourceArtworkObservation, base_image_for_scale, geometry};

    fn observed() -> SourceArtworkObservation {
        SourceArtworkObservation {
            product_id: 679,
            edition_id: 128,
            layout_id: 1,
            wristrest_connected: None,
        }
    }

    #[test]
    fn source_artwork_retries_edition_without_substituting_layout() {
        let mut observation = observed();
        observation.edition_id = 9999;
        let image = base_image_for_scale(observation, 1.).unwrap();
        assert_eq!(image.actual_edition_id, 0);
        assert!(image.retried_edition_zero);
        observation.layout_id = 9999;
        assert!(base_image_for_scale(observation, 1.).is_none());
        assert!(geometry(observation).unwrap().used_source_default_group);
    }

    #[test]
    fn source_artwork_selects_original_density_candidates() {
        let low = base_image_for_scale(observed(), 1.).unwrap();
        let high = base_image_for_scale(observed(), 2.).unwrap();
        assert_eq!(low.source_resolution, 1);
        assert_eq!(high.source_resolution, 3);
        assert_eq!(low.css_size, high.css_size);
        assert_eq!(high.pixel_size, low.pixel_size.map(|n| n * 3.));
        assert_eq!(high.actual_edition_id, 128);
    }

    #[test]
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
        state.clear();
        assert!(state.wrist().is_none());
    }
}
