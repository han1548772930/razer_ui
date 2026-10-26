use super::*;
use std::{
    cell::RefCell,
    sync::mpsc,
    time::{Duration, Instant},
};

static WORKER_TESTS: Mutex<()> = Mutex::new(());

struct ThreadExitGate {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}
impl Drop for ThreadExitGate {
    fn drop(&mut self) {
        if self.started.send(()).is_ok() {
            let _ = self.release.recv();
        }
    }
}
thread_local! {
    static THREAD_EXIT_GATE: RefCell<Option<ThreadExitGate>> = const { RefCell::new(None) };
}

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
    let completion = picker.completion();
    let (finished, result) = mpsc::channel();
    let waiter = thread::spawn(move || finished.send(completion.wait()).unwrap());
    result.recv_timeout(Duration::from_secs(3)).unwrap();
    waiter.join().unwrap();
    picker.drain().unwrap()
}

#[test]
fn simulated_picker_replacement_waits_for_cleanup_and_cancel_never_commits() {
    let _serial = WORKER_TESTS.lock().unwrap();
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
    let (second_started, second_ready) = mpsc::channel();
    let second = ScreenColorPicker::spawn(move |worker| {
        assert!(cleaned.load(Ordering::Acquire));
        second_started.send(()).unwrap();
        worker.color(0x12_34_56)?;
        Ok(Completion::Done)
    })
    .unwrap();
    assert_eq!(active_workers(), 2);
    assert_eq!(second_ready.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(
        first.drain().unwrap(),
        vec![ScreenColorEvent::Color(0x44_55_66)]
    );
    release.send(()).unwrap();
    assert_eq!(terminal(&first), vec![ScreenColorEvent::Canceled]);
    second_ready.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(active_workers(), 1);
    assert_eq!(
        terminal(&second),
        vec![ScreenColorEvent::Color(0x12_34_56), ScreenColorEvent::Done]
    );
    assert_eq!(active_workers(), 0);

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
    let completion = dropped.completion();
    drop(dropped);
    assert_eq!(active_workers(), 1);
    release.send(()).unwrap();
    cleanup.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(completion.wait(), ScreenColorEvent::Canceled);
    assert_eq!(active_workers(), 0);
}

#[test]
fn simulated_picker_terminal_and_replacement_wait_for_thread_exit_not_just_run_return() {
    let _serial = WORKER_TESTS.lock().unwrap();
    let (exiting, at_exit) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let first = ScreenColorPicker::spawn(move |worker| {
        THREAD_EXIT_GATE.set(Some(ThreadExitGate {
            started: exiting,
            release: blocked,
        }));
        worker.color(0x12_34_56)?;
        Ok(Completion::Done)
    })
    .unwrap();
    at_exit.recv_timeout(Duration::from_secs(3)).unwrap();
    let (started, ready) = mpsc::channel();
    let second = ScreenColorPicker::spawn(move |_| {
        started.send(()).unwrap();
        Ok(Completion::Done)
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while first.session.join.lock().unwrap().thread.is_some() {
        assert!(
            Instant::now() < deadline,
            "Replacement did not claim its predecessor's join"
        );
        thread::yield_now();
    }
    assert_eq!(active_workers(), 2);
    assert_eq!(
        first.drain().unwrap(),
        vec![ScreenColorEvent::Color(0x12_34_56)]
    );
    assert_eq!(ready.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert!(second.drain().unwrap().is_empty());
    release.send(()).unwrap();
    assert_eq!(terminal(&first), vec![ScreenColorEvent::Canceled]);
    assert_eq!(terminal(&second), vec![ScreenColorEvent::Done]);
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(active_workers(), 0);
}

#[test]
fn simulated_picker_exit_retains_dropped_workers_rejects_new_work_and_preserves_cleanup_failure() {
    let _serial = WORKER_TESTS.lock().unwrap();
    let (started, ready) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let picker = ScreenColorPicker::spawn(move |worker| {
        started.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(worker.canceled());
        anyhow::bail!("Simulated screen cleanup failure")
    })
    .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    let id = picker.completion().id();
    drop(picker);
    let completions = begin_exit();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].id(), id);
    assert_eq!(active_workers(), 1);
    assert!(ScreenColorPicker::spawn(|_| Ok(Completion::Done)).is_err());
    release.send(()).unwrap();
    assert_eq!(
        completions.into_iter().next().unwrap().wait(),
        ScreenColorEvent::Failed("Simulated screen cleanup failure".into())
    );
    assert_eq!(active_workers(), 0);
    cancel_exit();
    let panicked = ScreenColorPicker::spawn(|_| panic!("Simulated screen worker panic")).unwrap();
    assert_eq!(
        terminal(&panicked),
        vec![ScreenColorEvent::Failed(
            "Screen color worker panicked".into()
        )]
    );
    let resumed = ScreenColorPicker::spawn(|_| Ok(Completion::Done)).unwrap();
    assert_eq!(terminal(&resumed), vec![ScreenColorEvent::Done]);
    assert_eq!(active_workers(), 0);
}
