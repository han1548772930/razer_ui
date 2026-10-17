//! Resource identity and CPU codec checks without network or product payloads.
use super::*;
use gpui_kit::{Image, ImageFormat, SvgRenderer};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, sync::Arc};

#[test]
fn listed_product_aliases_have_complete_file_identities() {
    let aliases = SynapseAssets.list("synapse/").unwrap();
    let mut files = BTreeSet::new();
    for alias in &aliases {
        let (path, bytes, hash) = remote::identity(alias)
            .unwrap_or_else(|| panic!("Listed resource has no file identity: {alias}"));
        assert!(path.starts_with("assets/synapse/"), "{alias}: {path}");
        assert!(bytes > 0, "Empty product resource: {alias}");
        assert_eq!(hash.len(), 64, "{alias}");
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{alias}");
        files.insert(path);
    }
    assert!(aliases.len() > 1_000);
    assert!(files.len() > 1_000);
    assert_eq!(SynapseAssets.list("synapse/").unwrap(), aliases);
    assert!(remote::identity("synapse/absent-resource-for-identity-test").is_none());
}

#[test]
fn sprite_views_share_one_download_and_special_providers_keep_list_rules() {
    let base = "synapse/audio-volume-external-sprite.svg";
    let identity = remote::identity(base).unwrap();
    for view in ["link", "hover"] {
        assert_eq!(remote::identity(&format!("{base}#{view}")), Some(identity));
    }
    assert!(remote::identity(&format!("{base}#unknown")).is_none());
    let controls = [
        "synapse/audio-demo-control-play.svg",
        "synapse/audio-demo-control-pause.svg",
        "synapse/audio-demo-control-fullscreen.svg",
        "synapse/audio-demo-control-fullscreen-exit.svg",
    ];
    for prefix in ["", "synapse", "synapse/"] {
        let listed = SynapseAssets.list(prefix).unwrap();
        for control in controls {
            assert!(listed.iter().any(|key| key == control));
            assert!(remote::identity(control).is_some());
        }
    }
    assert!(
        SynapseAssets
            .list("synapse/audio-demo-control-")
            .unwrap()
            .is_empty()
    );
    let fallback = gpui_kit::assets::AllAssets.list("").unwrap();
    let listed = SynapseAssets.list("").unwrap();
    assert!(!fallback.is_empty());
    for key in fallback {
        assert!(listed.contains(&key), "GPUI fallback omitted: {key}");
    }
}

#[test]
fn current_gamepad_trigger_resources_have_source_identities() {
    let mut aliases = vec![
        "synapse/gamepad-trigger-device/2629/0.png".to_owned(),
        "synapse/gamepad-trigger-device/2629/128.png".to_owned(),
        "synapse/gamepad-trigger-device/2636/0.png".to_owned(),
        "synapse/gamepad-trigger-device/2636/128.png".to_owned(),
        "synapse/gamepad-trigger-device/2636/129.png".to_owned(),
        "synapse/gamepad-trigger-device/2647/0.png".to_owned(),
        "synapse/gamepad-trigger-device/2647/128.png".to_owned(),
        "synapse/gamepad-trigger-device/2676/0.svg".to_owned(),
        "synapse/gamepad-trigger-device/2684/0.svg".to_owned(),
        "synapse/gamepad-trigger-device/4133/0.png".to_owned(),
        "synapse/gamepad-trigger-device/4133/128.png".to_owned(),
        "synapse/gamepad-trigger-device/4133/129.png".to_owned(),
        "synapse/gamepad-trigger-device/4144/0.svg".to_owned(),
    ];
    aliases.extend(
        [2629, 2636, 2647, 2676, 2684, 4133, 4144]
            .map(|pid| format!("synapse/gamepad-trigger-reset/{pid}.svg")),
    );
    let mut payloads = BTreeSet::new();
    for alias in aliases {
        let (_, bytes, hash) = remote::identity(&alias).expect(&alias);
        assert!(bytes > 0, "{alias}");
        payloads.insert((bytes, hash));
    }
    assert_eq!(payloads.len(), 13);
}

#[test]
fn inline_svg_codec_preserves_clips_and_rejects_invalid_input() {
    let renderer = SvgRenderer::new(Arc::new(gpui_kit::assets::AllAssets));
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2" viewBox="0 0 2 2"><defs><clipPath id="clip"><rect width="1" height="2"/></clipPath></defs><rect width="2" height="2" fill="#44d62c" clip-path="url(#clip)"/></svg>"##;
    let rendered = renderer.render_single_frame(svg, 1.).unwrap();
    assert_eq!(rendered.frame_count(), 1);
    let size = rendered.size(0);
    // GPUI rasterizes scaled SVGs at twice the requested density for smoothing.
    assert_eq!((size.width.0, size.height.0), (4, 4));
    let pixels = rendered.as_bytes(0).unwrap();
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 0));
    assert!(renderer.parse_svg(b"not an SVG").is_err());
}

#[test]
fn pid229_cycle_up_svg_matches_manifest_and_rasterizes_on_cpu() {
    const KEY: &str = "synapse/mouse-customize/8bd13c4f3a94a3834d2a05c6.svg";
    const SVG: &[u8] = include_bytes!("../testdata/mouse_customize/cycle-229.svg");
    let (path, bytes, hash) = remote::identity(KEY).expect(KEY);
    assert_eq!(
        path,
        "assets/synapse/shared/products/8bd13c4f3a94a383-8bd13c4f3a94a3834d2a05c6.svg"
    );
    assert_eq!(bytes, SVG.len() as u64);
    assert_eq!(hash, format!("{:x}", Sha256::digest(SVG)));

    // Keep this a pure codec check: no application/window or resource task is
    // involved. The fixture is the exact 102px PID229 cycle-up artwork.
    let renderer = SvgRenderer::new(Arc::new(SynapseAssets));
    let rendered = renderer
        .render_single_frame(SVG, 1.)
        .expect("PID229 cycle-up SVG should rasterize");
    assert_eq!(rendered.frame_count(), 1);
    let size = rendered.size(0);
    assert_eq!((size.width.0, size.height.0), (204, 204));
    let pixels = rendered.as_bytes(0).expect("rasterized frame bytes");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel == [17, 17, 17, 255])
    );
    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel == [102, 102, 102, 255])
    );
}

#[test]
fn inline_bitmap_codec_checks_pixels_and_rejects_truncation() {
    let renderer = SvgRenderer::new(Arc::new(gpui_kit::assets::AllAssets));
    // A synthetic 1x1 RGBA PNG with one opaque green pixel.
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 112, 185, 166, 243,
        31, 0, 4, 238, 2, 70, 90, 206, 191, 50, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let decoded = Image::from_bytes(ImageFormat::Png, PNG.to_vec())
        .to_image_data(renderer.clone())
        .unwrap();
    let size = decoded.size(0);
    assert_eq!((size.width.0, size.height.0), (1, 1));
    assert_eq!(decoded.frame_count(), 1);
    assert_eq!(decoded.as_bytes(0).unwrap()[3], 255);
    assert!(
        Image::from_bytes(ImageFormat::Png, PNG[..30].to_vec())
            .to_image_data(renderer)
            .is_err()
    );
}
