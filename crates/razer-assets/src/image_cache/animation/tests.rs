use super::*;
use std::{
    borrow::Cow,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

#[test]
fn animation_candidate_accepts_only_local_animated_extensions() {
    assert!(candidate(&Resource::Embedded("icons/spinner.GIF".into())));
    assert!(candidate(&Resource::Path(
        PathBuf::from("C:/cache/loop.WebP").into()
    )));
    assert!(!candidate(&Resource::Path(
        PathBuf::from("C:/cache/icon.svg").into()
    )));
    assert!(!candidate(&Resource::Uri(
        "https://example.invalid/loop.gif".into()
    )));
}

#[test]
fn gif_without_loop_extension_plays_once() {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&[0; 7]);
    gif.push(0x3b);
    assert_eq!(gif_passes(&gif, &operation).unwrap(), Some(1));
}

#[test]
fn malformed_gif_is_rejected_before_decoding() {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    assert!(gif_passes(b"GIF89a", &operation).is_err());
    assert!(gif_passes(b"not-a-gif", &operation).is_err());
}

#[test]
fn gif_loop_extension_distinguishes_finite_repetitions_from_infinite_playback() {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    for (repeats, expected) in [(0u16, None), (1, Some(2)), (u16::MAX, Some(65_536))] {
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0; 7]);
        gif.extend_from_slice(b"\x21\xff\x0bNETSCAPE2.0\x03\x01");
        gif.extend_from_slice(&repeats.to_le_bytes());
        gif.extend_from_slice(b"\x00\x3b");
        assert_eq!(gif_passes(&gif, &operation).unwrap(), expected);
    }
}

struct ReadAssets {
    loads: AtomicUsize,
    cancel_during_load: Option<Transfer>,
}

impl AssetSource for ReadAssets {
    fn load(&self, _: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if let Some(transfer) = &self.cancel_during_load {
            transfer.cancel();
        }
        Ok(Some(Cow::Borrowed(b"resource bytes")))
    }

    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[test]
fn cancelled_animation_does_not_start_resource_loading() {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    let assets = Arc::new(ReadAssets {
        loads: AtomicUsize::new(0),
        cancel_during_load: None,
    });
    transfer.cancel();
    assert!(
        read_source(
            Resource::Embedded("test/cancelled.gif".into()),
            assets.clone(),
            &operation,
        )
        .is_err()
    );
    assert_eq!(assets.loads.load(Ordering::SeqCst), 0);
    assert!(gif_passes(b"GIF89a\0\0\0\0\0\0\0\x3b", &operation).is_err());
}

#[test]
fn cancellation_during_resource_loading_rejects_bytes_before_decoder_initialization() {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    let assets = Arc::new(ReadAssets {
        loads: AtomicUsize::new(0),
        cancel_during_load: Some(transfer),
    });
    assert!(
        read_source(
            Resource::Embedded("test/cancelled.gif".into()),
            assets.clone(),
            &operation,
        )
        .is_err()
    );
    assert_eq!(assets.loads.load(Ordering::SeqCst), 1);
}

#[test]
fn playback_uses_frame_delays_and_keeps_the_final_frame_after_decoder_end() {
    let mut playback = Playback::default();
    assert!(playback.receive(Some(1)));
    assert!(!playback.receive(Some(2)));
    let tick = |playback: &mut Playback<u8>, millis| {
        playback.advance(Duration::from_millis(millis), true, |_| {
            Duration::from_millis(10)
        })
    };
    assert_eq!(tick(&mut playback, 0), None);
    assert_eq!(tick(&mut playback, 9), None);
    assert_eq!(tick(&mut playback, 10), Some(1));
    assert_eq!(playback.current, Some(2));
    assert_eq!(playback.next, None);
    assert!(!playback.receive(Some(3)));
    assert_eq!(tick(&mut playback, 19), None);
    assert_eq!(tick(&mut playback, 20), Some(2));
    assert!(!playback.receive(None));
    assert!(!playback.completed);
    assert_eq!(tick(&mut playback, 29), None);
    assert_eq!(tick(&mut playback, 30), None);
    assert!(playback.completed);
    assert_eq!(playback.current, Some(3));
    assert_eq!(tick(&mut playback, 100), None);
    assert_eq!(playback.current, Some(3));
}

#[test]
fn static_webp_keeps_exif_orientation_and_bgra_pixels_on_the_animation_path() {
    use image::ImageEncoder as _;

    // A two-pixel horizontal strip, with TIFF orientation 6 (90 degrees clockwise).
    // The pixel expectation below is independent of the frame-sequence decoder.
    let exif = b"II\x2a\x00\x08\x00\x00\x00\x01\x00\x12\x01\x03\x00\x01\x00\x00\x00\x06\x00\x00\x00\x00\x00\x00\x00";
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
    encoder.set_exif_metadata(exif.to_vec()).unwrap();
    encoder
        .encode(
            &[255, 0, 0, 255, 0, 0, 255, 255],
            2,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    let mut sequence = FrameSequence::new(bytes.into(), &operation).unwrap();
    let frame = sequence.next(&operation).unwrap().unwrap();
    assert_eq!(frame.image.frame_count(), 1);
    assert_eq!(
        (frame.image.size(0).width.0, frame.image.size(0).height.0),
        (1, 2)
    );
    assert_eq!(
        frame.image.as_bytes(0).unwrap(),
        &[0, 0, 255, 255, 255, 0, 0, 255]
    );
    assert!(sequence.next(&operation).unwrap().is_none());
    transfer.cancel();
    assert!(sequence.next(&operation).is_err());
}
