use super::*;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

fn cursor_fixture() -> Vec<u8> {
    let mut bytes = vec![0; 22 + 64];
    bytes[2..4].copy_from_slice(&2_u16.to_le_bytes());
    bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
    bytes[6] = 2;
    bytes[7] = 2;
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
    bytes[14..18].copy_from_slice(&64_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&22_u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&40_u32.to_le_bytes());
    bytes[26..30].copy_from_slice(&2_u32.to_le_bytes());
    bytes[30..34].copy_from_slice(&4_u32.to_le_bytes());
    bytes[34..36].copy_from_slice(&1_u16.to_le_bytes());
    bytes[36..38].copy_from_slice(&32_u16.to_le_bytes());
    bytes
}

#[test]
fn native_colorref_becomes_the_source_event_rgb_without_inventing_failed_samples() {
    assert_eq!(colorref_rgb(0x0056_3412).unwrap(), 0x0012_3456);
    assert_eq!(colorref_rgb(0x00ff_ffff).unwrap(), 0x00ff_ffff);
    assert_eq!(colorref_rgb(0).unwrap(), 0);
    assert!(colorref_rgb(u32::MAX).is_err());
}

#[test]
fn cursor_preserves_native_hotspots_and_rejects_truncated_or_invalid_payloads() {
    let bytes = cursor_fixture();
    let images = cursor_images(&bytes).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!((images[0].width, images[0].height), (2, 2));
    assert_eq!(images[0].hotspot, [1, 1]);
    assert_eq!(images[0].dib, &bytes[22..]);
    assert!(cursor_images(&bytes[..bytes.len() - 1]).is_err());
    let mut invalid = bytes.clone();
    invalid[10..12].copy_from_slice(&2_u16.to_le_bytes());
    assert!(cursor_images(&invalid).is_err());
    let mut invalid = bytes.clone();
    invalid[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(cursor_images(&invalid).is_err());
    let mut invalid = bytes;
    invalid[36..38].copy_from_slice(&24_u16.to_le_bytes());
    assert!(cursor_images(&invalid).is_err());
}

fn terminal(picker: &ScreenColorPicker) -> Vec<ScreenColorEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut result = Vec::new();
    loop {
        let events = picker.drain().unwrap();
        let finished = events
            .iter()
            .any(|event| !matches!(event, ScreenColorEvent::Color(_)));
        result.extend(events);
        if finished {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "Simulated screen color owner failed to stop"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn simulated_picker_replacement_waits_for_cleanup_and_cancel_never_commits() {
    let (started, ready) = mpsc::channel();
    let (release, waiting) = mpsc::channel();
    let cleaned = Arc::new(AtomicBool::new(false));
    let first_cleanup = cleaned.clone();
    let first = ScreenColorPicker::spawn(move |worker| {
        worker.color(0x11_22_33)?;
        worker.color(0x44_55_66)?;
        started.send(()).unwrap();
        waiting.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(worker.canceled());
        worker.color(0xaa_bb_cc)?;
        first_cleanup.store(true, Ordering::Release);
        Ok(Completion::Canceled)
    })
    .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    let second = ScreenColorPicker::spawn(move |worker| {
        assert!(cleaned.load(Ordering::Acquire));
        worker.color(0x12_34_56)?;
        Ok(Completion::Done)
    })
    .unwrap();
    release.send(()).unwrap();
    assert_eq!(
        terminal(&first),
        vec![
            ScreenColorEvent::Color(0x44_55_66),
            ScreenColorEvent::Canceled
        ]
    );
    assert_eq!(
        terminal(&second),
        vec![ScreenColorEvent::Color(0x12_34_56), ScreenColorEvent::Done]
    );

    let failed =
        ScreenColorPicker::spawn(|_| anyhow::bail!("Simulated screen read failure")).unwrap();
    assert_eq!(
        terminal(&failed),
        vec![ScreenColorEvent::Failed(
            "Simulated screen read failure".into()
        )]
    );

    let (started, ready) = mpsc::channel();
    let (release, waiting) = mpsc::channel();
    let (finished, cleanup) = mpsc::channel();
    let dropped = ScreenColorPicker::spawn(move |worker| {
        started.send(()).unwrap();
        waiting.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(worker.canceled());
        finished.send(()).unwrap();
        Ok(Completion::Canceled)
    })
    .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    drop(dropped);
    release.send(()).unwrap();
    cleanup.recv_timeout(Duration::from_secs(3)).unwrap();
}
