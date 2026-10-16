//! OTFM lifecycle and input reduction. These cases drive the pure state
//! machine and the assignment-input validator; no transport, service or
//! persistence is involved, and no recorded macro is treated as saved.
use super::*;

fn keyboard_keydown(scancode: u64, flag: u64) -> Value {
    json!({"type":"keyboard","action":"keydown","scancode":scancode,"flag":flag,"vkCode":32})
}
fn keyboard_keyup(scancode: u64, flag: u64) -> Value {
    json!({"type":"keyboard","action":"keyup","scancode":scancode,"flag":flag,"vkCode":32})
}
/// Drives the lifecycle up to `Recording` (the first state that collects input).
fn recording_machine(now: u64) -> Machine {
    let mut machine = Machine::default();
    machine.press("container-1", None, now);
    let _ = machine.effect();
    machine.resolved(Effect::QueryRecording, true, Some(false), now);
    machine.resolved(Effect::SuspendMappings, true, None, now);
    machine.resolved(Effect::MonitorBefore, true, None, now);
    machine.resolved(Effect::StartRecorder, true, Some(true), now);
    machine.resolved(Effect::MacroLedOn, true, None, now);
    machine.resolved(Effect::ReactiveShort, true, None, now);
    assert_eq!(machine.state, State::Recording);
    machine
}

#[test]
fn otfm_press_enters_querying_with_its_source_effect() {
    let mut machine = Machine::default();
    machine.press("container-1", Some(41), 10);
    assert_eq!(machine.state, State::Querying);
    assert_eq!(machine.keyboard_started["containerId"], "container-1");
    assert_eq!(machine.keyboard_started["timerTick"], 41);
    // The effect is consumed once.
    assert_eq!(machine.effect(), Some(Effect::QueryRecording));
    assert_eq!(machine.effect(), None);
}

#[test]
fn otfm_query_error_keeps_querying_and_a_running_recorder_is_an_error() {
    let mut machine = Machine::default();
    machine.press("container-1", None, 0);
    let _ = machine.effect();
    // Querying has no source onError edge: it keeps awaiting a real event.
    machine.resolved(Effect::QueryRecording, false, None, 1);
    assert_eq!(machine.state, State::Querying);
    machine.resolved(Effect::QueryRecording, true, Some(true), 2);
    assert_eq!(machine.state, State::ErrorLighting);
    assert_eq!(machine.effect(), Some(Effect::ErrorLighting));
}

#[test]
fn otfm_recorder_start_without_a_started_answer_enters_error_lighting() {
    let mut machine = Machine::default();
    machine.enter(State::StartingRecorder, 0);
    let _ = machine.effect();
    machine.resolved(Effect::StartRecorder, true, None, 0);
    assert_eq!(machine.state, State::ErrorLighting);
}

#[test]
fn otfm_full_recording_lifecycle_completes_through_enable_mappings() {
    let mut machine = recording_machine(0);
    // A second press stops the recording and starts the long reactive phase.
    machine.press("container-1", Some(9), 1);
    assert_eq!(machine.state, State::MacroLedBlinking);
    assert_eq!(machine.keyboard_ended["containerId"], "container-1");
    assert_eq!(machine.keyboard_ended["timerTick"], 9);
    machine.resolved(Effect::MacroLedBlinking, true, None, 1);
    assert_eq!(machine.state, State::ReactiveLong);
    machine.resolved(Effect::ReactiveLong, true, None, 1);
    assert_eq!(machine.state, State::AssigningKey);
    assert!(machine.assigning);
    // The assigned key is a source keyup, not the Escape or the Windows key.
    machine
        .item(json!({"type":"keyboard","action":"keyup","vkCode":32}), 2)
        .unwrap();
    assert_eq!(machine.assigned.len(), 1);
    assert_eq!(machine.state, State::AcceptedLedOff);
    machine.resolved(Effect::AcceptedLedOff, true, None, 2);
    assert_eq!(machine.state, State::MonitorAfter);
    machine.resolved(Effect::MonitorAfter, true, None, 2);
    assert_eq!(machine.state, State::Processing);
    assert!(!machine.assigning);
    machine.resolved(Effect::PostRecording, true, None, 2);
    assert_eq!(machine.state, State::Stopping);
    machine.resolved(Effect::StopRecorder, true, None, 2);
    assert_eq!(machine.state, State::Restore);
    machine.resolved(Effect::RestoreLighting, true, None, 2);
    assert_eq!(machine.state, State::EnablingMapping);
    assert!(!machine.completed);
    machine.resolved(Effect::EnableMappings, true, None, 2);
    assert_eq!(machine.state, State::Idle);
    assert!(machine.completed);
}

#[test]
fn otfm_escape_aborts_recording_and_stops_the_recorder() {
    let mut machine = recording_machine(0);
    machine
        .item(json!({"type":"keyboard","action":"keydown","vkCode":27}), 5)
        .unwrap();
    // A held Escape only aborts on its keyup.
    assert_eq!(machine.state, State::Recording);
    machine
        .item(json!({"type":"keyboard","action":"keyup","vkCode":27}), 6)
        .unwrap();
    assert_eq!(machine.abort, Some(Abort::Escape));
    assert_eq!(machine.abort.unwrap().source_reason(), "esc");
    assert_eq!(machine.state, State::AbortedLedOff);
    assert_eq!(machine.effect(), Some(Effect::MacroLedOff));
    machine.resolved(Effect::MacroLedOff, true, None, 6);
    assert_eq!(machine.state, State::AbortedStop);
    machine.resolved(Effect::StopRecorder, true, None, 6);
    assert_eq!(machine.state, State::Restore);
}

#[test]
fn otfm_recording_timeout_aborts_with_the_timeout_reason() {
    let mut machine = recording_machine(1_000);
    // Real input extends the recording deadline to its own tick plus 120s.
    machine
        .item(
            json!({"type":"keyboard","action":"keydown","vkCode":65}),
            2_000,
        )
        .unwrap();
    machine.tick(121_999);
    assert_eq!(machine.state, State::Recording);
    machine.tick(122_000);
    assert_eq!(machine.abort, Some(Abort::Timeout));
    assert_eq!(machine.abort.unwrap().source_reason(), "timeout");
    assert_eq!(machine.state, State::AbortedLedOff);
}

#[test]
fn otfm_recording_keeps_real_input_and_normalizes_numpad_keys() {
    let mut machine = recording_machine(0);
    machine
        .item(
            json!({"type":"keyboard","action":"keydown","vkCode":45,"flags":0}),
            1,
        )
        .unwrap();
    assert_eq!(machine.items.last().unwrap()["vkCode"], 96);
    machine
        .item(json!({"type":"keyboard","action":"mousemove"}), 2)
        .unwrap();
    assert!(machine.has_mouse_move);
    // The native reader stamps its own tick on each accepted item.
    machine
        .native_items(&[json!({"action":"keydown","vkCode":65,"flags":0})], 7, 3)
        .unwrap();
    assert_eq!(machine.items.last().unwrap()["timerTick"], 7);
    // Source-suppressed media keys are never recorded.
    machine
        .native_items(&[json!({"action":"keydown","vkCode":173})], 7, 3)
        .unwrap();
    assert_eq!(machine.items.len(), 3);
}

#[test]
fn otfm_mouse_move_choice_reads_the_source_code_and_times_out_to_monitor_after() {
    let mut machine = Machine::default();
    machine.has_mouse_move = true;
    machine.enter(State::AcceptedLedOff, 500);
    machine.resolved(Effect::AcceptedLedOff, true, None, 500);
    assert_eq!(machine.state, State::MouseMoveChoice);
    // An unrelated key keeps the choice open.
    machine
        .item(json!({"type":"keyboard","action":"keyup","vkCode":65}), 501)
        .unwrap();
    assert_eq!(machine.state, State::MouseMoveChoice);
    assert_eq!(machine.mouse_move_type, "none");
    machine
        .item(json!({"type":"keyboard","action":"keyup","vkCode":50}), 502)
        .unwrap();
    assert_eq!(machine.mouse_move_type, "absolute");
    assert_eq!(machine.state, State::MonitorAfter);
    // The choice window is 10s from its own entry.
    let mut timed_out = Machine::default();
    timed_out.has_mouse_move = true;
    timed_out.enter(State::MouseMoveChoice, 0);
    timed_out.tick(10_000);
    assert_eq!(timed_out.state, State::MonitorAfter);
}

#[test]
fn otfm_post_recording_without_an_observed_geometry_change_keeps_processing() {
    let mut machine = Machine::default();
    machine.enter(State::Processing, 0);
    let _ = machine.effect();
    // Persistence exceptions do not fabricate the machine's rejection event.
    machine.resolved(Effect::PostRecording, false, None, 0);
    assert_eq!(machine.state, State::Processing);
}

#[test]
fn otfm_geometry_change_requires_a_mouse_move_and_a_screen_shift() {
    let mut machine = Machine::default();
    machine.geometry(
        true,
        json!([{"name":"a"}]),
        json!({"top":0,"left":0,"width":100,"height":100}),
    );
    machine.geometry(
        false,
        json!([{"name":"b"}]),
        json!({"top":0,"left":0,"width":100,"height":100}),
    );
    assert!(!machine.geometry_changed());
    // The mouse moved, but the observed screen geometry did not: the monitor
    // list alone is not the geometry change the source rejects on.
    machine.has_mouse_move = true;
    assert!(!machine.geometry_changed());
    machine.geometry(
        false,
        json!([{"name":"b"}]),
        json!({"top":0,"left":0,"width":200,"height":100}),
    );
    assert!(machine.geometry_changed());
    machine.reject_geometry(9);
    assert_eq!(machine.state, State::RejectedRestore);
}

#[test]
fn otfm_normalize_key_remaps_only_unflagged_numpad_keys() {
    let mut item = json!({"action":"keydown","vkCode":45,"flags":0});
    normalize_key(&mut item);
    assert_eq!(item["vkCode"], 96);
    // An extended (flagged) key keeps its own virtual key.
    let mut flagged = json!({"action":"keyup","vkCode":45,"flags":1});
    normalize_key(&mut flagged);
    assert_eq!(flagged["vkCode"], 45);
    // Only the current table's numpad entries are remapped.
    let mut unknown = json!({"action":"keydown","vkCode":8,"flags":0});
    normalize_key(&mut unknown);
    assert_eq!(unknown["vkCode"], 8);
    // Non-key actions are left untouched.
    let mut mouse = json!({"action":"mousemove","vkCode":45,"flags":0});
    normalize_key(&mut mouse);
    assert_eq!(mouse["vkCode"], 45);
}

/// A table entry whose keyup flag is zero, i.e. the second half of a source pair.
fn zero_flag_key() -> (u64, &'static str) {
    let keys = crate::mapping_engine::source_keys(70).expect("current macro key table");
    let key = keys
        .iter()
        .find(|key| key.output_flag.unwrap_or(key.flag) == 0)
        .expect("the current table retains a zero output flag key");
    (u64::from(key.scancode), key.virtual_key)
}

#[test]
fn otfm_assignment_input_requires_a_complete_source_pair() {
    let (scancode, virtual_key) = zero_flag_key();
    let mut input = AssignmentInput::new(u64::MAX);
    // The keydown alone is retained but produces no assigned action.
    assert_eq!(
        input
            .push(keyboard_keydown(scancode, 0), 1_000, 1, &[])
            .unwrap(),
        None
    );
    assert_eq!(input.items.len(), 1);
    let assigned = input
        .push(keyboard_keyup(scancode, 1), 1_001, 1, &[])
        .unwrap()
        .expect("a paired keyup becomes the assigned action");
    assert_eq!(assigned["action"], "keyup");
    assert_eq!(assigned["scanCode"], scancode);
    assert_eq!(assigned["flag"], 0);
    assert_eq!(assigned["timerTick"], 1_001);
    assert_eq!(
        assigned["vkCode"].as_u64(),
        virtual_key.parse::<u32>().ok().map(u64::from)
    );
}

#[test]
fn otfm_assignment_input_ignores_the_trigger_scancode_inside_its_grace_period() {
    let mut input = AssignmentInput::new(7);
    assert_eq!(
        input.push(keyboard_keydown(7, 1), 100, 1, &[]).unwrap(),
        None
    );
    assert!(input.items.is_empty());
    assert_eq!(
        input.push(keyboard_keydown(9, 1), 100, 1, &[]).unwrap(),
        None
    );
    assert_eq!(input.items.len(), 1);
    input.reset();
    assert!(input.items.is_empty());
    assert!(input.first_at.is_none());
    input.set_trigger(9);
    assert_eq!(
        input.push(keyboard_keydown(9, 1), 200, 1, &[]).unwrap(),
        None
    );
    assert!(input.items.is_empty());
}

#[test]
fn otfm_assignment_input_reports_a_missing_flag_and_the_source_fn_state() {
    let mut missing = AssignmentInput::new(u64::MAX);
    let error = missing
        .push(
            json!({"type":"keyboard","action":"keydown","scancode":1234}),
            0,
            1,
            &[],
        )
        .unwrap_err();
    assert!(error.to_string().contains("flag"));
    // A held Fn suppresses the assignment and survives a collection reset.
    let mut input = AssignmentInput::new(u64::MAX);
    assert_eq!(
        input
            .push(json!({"type":"razerKey","key":1,"flag":0}), 0, 1, &[])
            .unwrap(),
        None
    );
    assert!(input.fn_held);
    input.reset();
    assert!(input.fn_held);
    assert_eq!(
        input
            .push(json!({"type":"razerKey","key":1,"flag":1}), 1, 1, &[])
            .unwrap(),
        None
    );
    assert!(!input.fn_held);
}
