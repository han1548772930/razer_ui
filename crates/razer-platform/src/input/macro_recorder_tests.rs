use super::*;
use serde_json::{json, to_value};

#[test]
fn shared_keyboard_mouse_delta_preserves_signed_fields_and_pending_moves() {
    let mut batch = Batch::default();
    assert!(batch.mouse(0x200, -1200, -20, 0, 100_000).is_none());
    let inputs = batch.keyboard(0x104, 0x8000_0041, 30, 0x8000_0010, 101_234);
    assert_eq!(
        to_value(inputs).unwrap(),
        json!([
            {"action":"mousemove","time":0,"x":-1200,"y":-20,"data":0},
            {"action":"keydown","time":1234,"vkCode":-2147483583i32,"scanCode":30,"flags":-2147483632i32}
        ])
    );
    let wheel = batch.mouse(0x20a, -1, 2, 0xff88_0000, 101_900).unwrap();
    assert_eq!(
        to_value(wheel).unwrap(),
        json!([
            {"action":"mousewheel","time":666,"x":-1,"y":2,"data":-7864320}
        ])
    );
}

#[test]
fn mousemove_flushes_only_after_1000_items_without_a_time_threshold() {
    let mut batch = Batch::default();
    for n in 0..1000 {
        assert!(
            batch
                .mouse(0x200, n, 0, 0, 100_000 + i64::from(n) * 60_000_000)
                .is_none()
        );
    }
    let items = batch.mouse(0x200, 1000, 0, 0, 60_000_100_000).unwrap();
    assert_eq!(items.len(), 1001);
    assert!(batch.flush().is_empty());
}

#[test]
fn unknown_mouse_messages_and_keyboard_flags_are_not_normalized_or_filtered() {
    let mut batch = Batch::default();
    let extra_button = batch.mouse(0x20b, 4, 5, 0x10000, 100).unwrap();
    assert_eq!(
        to_value(extra_button).unwrap(),
        json!([
            {"action":523,"time":0,"x":4,"y":5,"data":65536}
        ])
    );
    let injected = batch.keyboard(0x100, 65, 30, 0x10, 200);
    let repeat = batch.keyboard(0x100, 65, 30, 0x10, 300);
    assert_eq!(injected, repeat);
    let unknown = batch.keyboard(0xffff_ffff, 65, 30, 0, 400);
    assert_eq!(to_value(unknown).unwrap()[0]["action"], -1);
}

#[test]
fn wall_clock_adjustment_and_long_delta_keep_native_signed_truncation() {
    let mut batch = Batch::default();
    batch.keyboard(0x100, 65, 30, 0, 1_000);
    assert_eq!(
        to_value(batch.keyboard(0x101, 65, 30, 0, 900)).unwrap()[0]["time"],
        -100
    );
    assert_eq!(
        to_value(batch.keyboard(0x101, 65, 30, 0, 900 + 0x8000_0001)).unwrap()[0]["time"],
        -2147483647i32
    );
}

#[test]
fn stopping_flushes_moves_before_stopped_using_the_lifecycle_tick() {
    let mut batch = Batch::default();
    batch.mouse(0x200, 7, 8, 0, 133_000_000_000_000);
    let events = batch
        .finish(Mode::Otfm)
        .into_iter()
        .map(|kind| Event::new(kind, 99_000, 123))
        .collect::<Vec<_>>();
    assert_eq!(
        to_value(events).unwrap(),
        json!([
            {"kind":"item","event_type":2,"time_tick":99000,"received_at_ms":123,
             "event":[{"action":"mousemove","time":0,"x":7,"y":8,"data":0}]},
            {"kind":"stopped","event_type":1,"time_tick":99000,"received_at_ms":123,"mode":"kOTFM"}
        ])
    );
    assert!(matches!(
        batch.finish(Mode::Software).as_slice(),
        [EventKind::Stopped {
            mode: Mode::Software
        }]
    ));
}

#[test]
fn start_envelope_retains_source_mode_and_event_type() {
    let event = Event::new(
        EventKind::Started {
            mode: Mode::Software,
        },
        42,
        100,
    );
    assert_eq!(
        to_value(event).unwrap(),
        json!({"kind":"started","mode":"kSoftware","event_type":0,"time_tick":42,"received_at_ms":100})
    );
}

#[test]
fn duplicate_start_preserves_mode_and_pending_input_and_stop_clears_observation() {
    let mut session = Session::default();
    assert_eq!(session.mode(), None);
    assert!(session.stop().is_err());
    session.start(Mode::Software).unwrap();
    session.batch.mouse(0x200, 15, 16, 0, 500);
    assert!(session.start(Mode::Otfm).is_err());
    assert_eq!(session.mode(), Some(Mode::Software));
    let events = session.stop().unwrap();
    assert!(
        matches!(&events[..], [EventKind::Item { event }, EventKind::Stopped { mode: Mode::Software }] if event.len() == 1)
    );
    assert_eq!(session.mode(), None);
    assert!(session.stop().is_err());
}

#[test]
fn new_recording_resets_delta_and_has_no_previous_session_inputs() {
    let mut session = Session::default();
    session.start(Mode::Software).unwrap();
    session.batch.keyboard(0x100, 65, 30, 0, 500);
    session.stop().unwrap();
    session.start(Mode::Otfm).unwrap();
    let input = to_value(session.batch.keyboard(0x101, 66, 48, 0, 800)).unwrap();
    assert_eq!(
        input,
        json!([{"action":"keyup","time":0,"vkCode":66,"scanCode":48,"flags":0}])
    );
    assert_eq!(session.mode(), Some(Mode::Otfm));
}
