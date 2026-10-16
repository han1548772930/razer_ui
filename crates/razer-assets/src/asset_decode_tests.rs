//! CPU-only checks of the actual embedded bytes through the locked GPUI codecs.
//! These tests create no App, Window, platform service, or device connection.
use super::*;
use gpui_kit::{Image, ImageFormat, SvgRenderer};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

fn image_format(alias: &str) -> Option<ImageFormat> {
    match alias.split('#').next().unwrap().rsplit('.').next()? {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::Webp),
        "gif" => Some(ImageFormat::Gif),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "tiff" | "tif" => Some(ImageFormat::Tiff),
        "ico" => Some(ImageFormat::Ico),
        "pnm" | "pbm" | "pgm" | "ppm" => Some(ImageFormat::Pnm),
        "avif" => panic!("AVIF must be prepared as PNG for the locked GPUI decoder: {alias}"),
        _ => None,
    }
}

#[test]
fn listed_aliases_load_and_duplicate_entries_keep_identical_content() {
    let mut aliases = BTreeMap::new();
    for alias in SynapseAssets.list("").unwrap() {
        let bytes = SynapseAssets.load(&alias).unwrap().expect(&alias);
        let hash: [u8; 32] = Sha256::digest(bytes.as_ref()).into();
        if let Some(previous) = aliases.insert(alias.clone(), (bytes.len(), hash)) {
            assert_eq!(
                previous,
                (bytes.len(), hash),
                "Duplicate alias changed source output: {alias}"
            );
        }
    }
    assert!(aliases.len() > 1_000);
}

#[test]
fn every_embedded_image_decodes_with_locked_gpui_codecs() {
    let renderer = SvgRenderer::new(Arc::new(SynapseAssets));
    let mut verified = HashSet::new();
    let mut images = 0;
    for alias in SynapseAssets.list("synapse/").unwrap() {
        let Some(format) = image_format(&alias) else {
            continue;
        };
        let bytes = SynapseAssets.load(&alias).unwrap().expect(&alias);
        let hash: [u8; 32] = Sha256::digest(bytes.as_ref()).into();
        if !verified.insert((hash, format)) {
            continue;
        }
        if format == ImageFormat::Svg {
            renderer
                .parse_svg(&bytes)
                .unwrap_or_else(|error| panic!("{alias}: {error}"));
        } else {
            let decoded = Image::from_bytes(format, bytes.into_owned())
                .to_image_data(renderer.clone())
                .unwrap_or_else(|error| panic!("{alias}: {error:#}"));
            assert!(decoded.frame_count() > 0, "no decoded frames: {alias}");
            let size = decoded.size(0);
            assert!(
                size.width.0 > 0 && size.height.0 > 0,
                "empty image: {alias}"
            );
        }
        images += 1;
    }
    assert!(
        images > 1_000,
        "the complete embedded image tables must be checked"
    );
}

#[test]
fn current_mouse_inline_svg_clips_retain_visible_pixels() {
    let renderer = SvgRenderer::new(Arc::new(SynapseAssets));
    for alias in [
        "synapse/mouse-customize/930024c1c6c7047513f8f644.svg",
        "synapse/mouse-customize/eba716ff296fcdb4441672f7.svg",
    ] {
        let bytes = SynapseAssets.load(alias).unwrap().expect(alias);
        let image = renderer.render_single_frame(&bytes, 1.).expect(alias);
        assert!(
            image
                .as_bytes(0)
                .unwrap()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0),
            "SVG clipping removed the complete image: {alias}"
        );
    }
}

#[test]
fn current_gamepad_trigger_resources_decode_with_source_dimensions() {
    // Seven independent current product receipts select these aliases. The
    // PNG dimensions come from the original AVIF images; SVGs retain their
    // original root geometry. No analysis paths are used by this test.
    let renderer = SvgRenderer::new(Arc::new(SynapseAssets));
    let mut identities = HashSet::new();
    let mut resources = vec![
        (
            "synapse/gamepad-trigger-device/2629/0.png".to_owned(),
            (1500, 690),
        ),
        (
            "synapse/gamepad-trigger-device/2629/128.png".to_owned(),
            (1500, 670),
        ),
        (
            "synapse/gamepad-trigger-device/2636/0.png".to_owned(),
            (1500, 690),
        ),
        (
            "synapse/gamepad-trigger-device/2636/128.png".to_owned(),
            (2000, 894),
        ),
        (
            "synapse/gamepad-trigger-device/2636/129.png".to_owned(),
            (1500, 690),
        ),
        (
            "synapse/gamepad-trigger-device/2647/0.png".to_owned(),
            (1500, 690),
        ),
        (
            "synapse/gamepad-trigger-device/2647/128.png".to_owned(),
            (1500, 690),
        ),
        (
            "synapse/gamepad-trigger-device/2676/0.svg".to_owned(),
            (371, 156),
        ),
        (
            "synapse/gamepad-trigger-device/2684/0.svg".to_owned(),
            (372, 156),
        ),
        (
            "synapse/gamepad-trigger-device/4133/0.png".to_owned(),
            (1390, 580),
        ),
        (
            "synapse/gamepad-trigger-device/4133/128.png".to_owned(),
            (1389, 580),
        ),
        (
            "synapse/gamepad-trigger-device/4133/129.png".to_owned(),
            (1390, 580),
        ),
        (
            "synapse/gamepad-trigger-device/4144/0.svg".to_owned(),
            (479, 185),
        ),
    ];
    resources.extend(
        [2629, 2636, 2647, 2676, 2684, 4133, 4144]
            .map(|pid| (format!("synapse/gamepad-trigger-reset/{pid}.svg"), (20, 20))),
    );
    for (alias, dimensions) in resources {
        let bytes = SynapseAssets.load(&alias).unwrap().expect(&alias);
        let identity: [u8; 32] = Sha256::digest(bytes.as_ref()).into();
        identities.insert(identity);
        let decoded = if alias.ends_with(".svg") {
            renderer.parse_svg(&bytes).expect(&alias);
            renderer.render_single_frame(&bytes, 1.).expect(&alias)
        } else {
            Image::from_bytes(ImageFormat::Png, bytes.into_owned())
                .to_image_data(renderer.clone())
                .unwrap_or_else(|error| panic!("{alias}: {error:#}"))
        };
        assert_eq!(decoded.frame_count(), 1, "{alias}");
        let size = decoded.size(0);
        assert_eq!((size.width.0, size.height.0), dimensions, "{alias}");
        assert!(
            decoded
                .as_bytes(0)
                .unwrap()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0),
            "resource has no visible pixels: {alias}"
        );
    }
    // Twelve artwork payloads plus the reset icon; equal bytes may be shared
    // while all twenty product aliases remain independently resolvable.
    assert_eq!(identities.len(), 13);
}
