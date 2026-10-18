//! Pure ownership checks; no GPUI application, window, renderer or executor.
use super::{DecoderLifecycle, discard_image_entries};
use razer_storage::file_transfer::Transfer;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

struct DecodeEntry {
    name: &'static str,
    image: Option<Result<u32, ()>>,
    task: Transfer,
    events: Rc<RefCell<Vec<String>>>,
}

impl Drop for DecodeEntry {
    fn drop(&mut self) {
        self.task.cancel();
        self.events.borrow_mut().push(format!("drop {}", self.name));
    }
}

fn entry(
    name: &'static str,
    image: Option<Result<u32, ()>>,
    events: &Rc<RefCell<Vec<String>>>,
) -> (DecodeEntry, Transfer) {
    let task = Transfer::new();
    (
        DecodeEntry {
            name,
            image,
            task: task.clone(),
            events: events.clone(),
        },
        task,
    )
}

fn discard(
    entries: &mut HashMap<&'static str, DecodeEntry>,
    lifecycle: &DecoderLifecycle,
    events: &Rc<RefCell<Vec<String>>>,
) {
    discard_image_entries(
        entries,
        |entry| entry.image.and_then(Result::ok),
        |image| {
            assert!(lifecycle.check().is_err(), "stop admission before release");
            events.borrow_mut().push(format!("release {image}"));
        },
    );
}

#[::core::prelude::v1::test]
fn decoder_lifecycle_shutdown_rejects_work_even_before_a_window_cache_exists() {
    let mut lifecycle = DecoderLifecycle::default();
    assert!(lifecycle.check().is_ok());
    lifecycle.stop();
    assert_eq!(
        lifecycle.check().unwrap_err().to_string(),
        "asset error: Resource image cache is stopping"
    );
    lifecycle.stop();
    assert!(
        lifecycle.check().is_err(),
        "repeated cleanup cannot reopen work"
    );
}

#[::core::prelude::v1::test]
fn decoder_lifecycle_shutdown_discards_both_pending_kinds_and_releases_ready_images() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (normal_pending, normal_task) = entry("normal pending", None, &events);
    let (filtered_pending, filtered_task) = entry("filtered pending", None, &events);
    let (normal_ready, _) = entry("normal ready", Some(Ok(1)), &events);
    let (filtered_ready, _) = entry("filtered ready", Some(Ok(2)), &events);
    let (normal_failed, _) = entry("normal failed", Some(Err(())), &events);
    let mut normal = HashMap::from([
        ("pending", normal_pending),
        ("ready", normal_ready),
        ("failed", normal_failed),
    ]);
    let mut filtered = HashMap::from([("pending", filtered_pending), ("ready", filtered_ready)]);
    let mut lifecycle = DecoderLifecycle::default();
    lifecycle.stop();
    discard(&mut normal, &lifecycle, &events);
    discard(&mut filtered, &lifecycle, &events);
    assert!(normal.is_empty());
    assert!(filtered.is_empty());
    assert!(normal_task.check().is_err());
    assert!(filtered_task.check().is_err());
    let events = events.borrow();
    assert_eq!(events.len(), 7);
    for (image, name) in [(1, "normal ready"), (2, "filtered ready")] {
        let released = events
            .iter()
            .position(|event| *event == format!("release {image}"))
            .unwrap();
        let dropped = events
            .iter()
            .position(|event| *event == format!("drop {name}"))
            .unwrap();
        assert!(released < dropped);
    }
}

#[::core::prelude::v1::test]
fn decoder_lifecycle_window_release_discards_once_without_stopping_other_windows() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (first, first_task) = entry("first", Some(Ok(1)), &events);
    let (other, other_task) = entry("other", Some(Ok(2)), &events);
    let mut first_window = HashMap::from([("image", first)]);
    let mut other_window = HashMap::from([("image", other)]);
    let lifecycle = DecoderLifecycle::default();
    let mut released = Vec::new();
    discard_image_entries(
        &mut first_window,
        |entry| entry.image.and_then(Result::ok),
        |image| released.push(image),
    );
    discard_image_entries(
        &mut first_window,
        |entry| entry.image.and_then(Result::ok),
        |image| released.push(image),
    );
    assert_eq!(released, [1]);
    assert!(first_task.check().is_err());
    assert!(other_task.check().is_ok());
    assert_eq!(other_window.len(), 1);
    assert!(lifecycle.check().is_ok());
    discard_image_entries(
        &mut other_window,
        |entry| entry.image.and_then(Result::ok),
        |image| released.push(image),
    );
    assert_eq!(released, [1, 2]);
}
