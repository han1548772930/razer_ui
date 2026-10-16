use super::*;
use serde_json::json;

fn runtime() -> Runtime {
    let mut runtime = Runtime::new(ProfileStack::new("base"));
    runtime.configure(
        vec![
            Association::new("a", "profile-a"),
            Association::new("b", "profile-b"),
        ],
        BTreeSet::from(["a".into(), "b".into(), "unlinked".into()]),
    );
    runtime
}
fn switches(effects: &[Effect]) -> Vec<&str> {
    effects
        .iter()
        .filter_map(|effect| {
            if let Effect::SwitchProfile(guid) = effect {
                Some(guid.as_str())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn start_preserves_source_effect_order_existing_capture_and_manual_base() {
    let mut runtime = runtime();
    let effects = runtime.game_started("a", Some("base"), 10);
    assert!(
        matches!(effects.as_slice(), [Effect::GameActive(true), Effect::RuntimeProfile(profile), Effect::SaveStack(_), Effect::SwitchProfile(switch)] if profile == "profile-a" && switch == profile)
    );
    runtime.select_base("manual");
    runtime.configure(
        vec![Association::new("a", "new-link")],
        BTreeSet::from(["a".into()]),
    );
    assert_eq!(
        switches(&runtime.game_started("a", Some("manual"), 40)),
        ["new-link"]
    );
    assert_eq!(runtime.stack.active_profile(), "manual");
    assert_eq!(runtime.stack.running_games().len(), 1);
    assert_eq!(runtime.stack.running_games()[0].profile_id(), "profile-a");
    assert_eq!(runtime.stack.running_games()[0].timer_tick(), 40);
    assert!(
        runtime
            .game_started("unknown", Some("manual"), 50)
            .is_empty()
    );
    assert_eq!(runtime.active_game(), Some("unknown"));
}

#[test]
fn initialization_and_delayed_restore_follow_observed_array_order() {
    let mut runtime = runtime();
    runtime.observe_running(vec!["b".into(), "a".into()]);
    assert_eq!(switches(&runtime.initialize(10)), ["profile-a"]);
    assert_eq!(
        runtime
            .stack
            .running_games()
            .iter()
            .map(RunningGame::game_id)
            .collect::<Vec<_>>(),
        ["b", "a"]
    );
    let stopped = runtime.game_stopped("a", false, Some("profile-a"), 100);
    assert_eq!(switches(&stopped), ["base"]);
    assert!(runtime.tick(3099, Some("base")).is_empty());
    runtime.observe_running(vec!["b".into()]);
    assert_eq!(switches(&runtime.tick(3100, Some("base"))), ["profile-b"]);
    assert_eq!(runtime.stack.running_games()[0].game_id(), "b");
    assert_eq!(runtime.stack.active_profile(), "base");
}

#[test]
fn stop_captures_pre_switch_profile_and_stack_without_canceling_old_callbacks() {
    let mut runtime = runtime();
    runtime.game_started("a", Some("base"), 0);
    runtime.game_stopped("a", false, Some("base"), 10);
    runtime.select_base("later-manual");
    runtime.observe_running(vec!["b".into()]);
    // Captured curProfile equalled the old base, so this clears the captured
    // stack even though both the current profile/base and games have changed.
    let effects = runtime.tick(3010, Some("profile-b"));
    assert!(switches(&effects).is_empty());
    assert_eq!(runtime.stack.active_profile(), "base");
    assert!(runtime.stack.running_games().is_empty());
    runtime.game_stopped("a", false, Some("profile-a"), 4000);
    runtime.game_stopped("b", false, Some("profile-b"), 4050);
    assert_eq!(
        switches(&runtime.tick(7100, Some("base"))),
        ["profile-b", "profile-b"]
    );
}

#[test]
fn foreground_matches_only_first_exact_executable_and_keeps_start_debounce() {
    let mut runtime = runtime();
    let details = vec![RunningGameDetail::new(
        "a",
        vec!["Game.EXE".into(), "alias.exe".into()],
    )];
    runtime.game_started("a", Some("base"), 0);
    assert_eq!(
        switches(&runtime.foreground("game.exe", &details, Some("profile-a"), 10)),
        ["base"]
    );
    assert!(
        runtime
            .foreground("alias.exe", &details, Some("base"), 20)
            .is_empty()
    );
    runtime.foreground("Game.EXE", &details, Some("base"), 100);
    runtime.foreground("Game.EXE", &details, Some("base"), 200);
    // A later unmatched foreground event does not call debounce.cancel().
    runtime.foreground("Other.exe", &details, Some("base"), 300);
    assert!(runtime.tick(699, Some("base")).is_empty());
    assert_eq!(switches(&runtime.tick(700, Some("base"))), ["profile-a"]);
    assert!(
        runtime
            .foreground("Other.exe", &[], Some("profile-a"), 800)
            .is_empty()
    );
    assert_eq!(runtime.active_game(), Some("a"));
    let effects = runtime.foreground("Other.exe", &details, Some("profile-a"), 900);
    assert_eq!(effects, [Effect::SwitchProfile("base".into())]);
    assert!(runtime.tick(5000, Some("base")).is_empty());
}

#[test]
fn local_owner_stack_round_trip_preserves_foreign_owners_and_unknown_fields() {
    let path = std::env::temp_dir().join(format!(
        "razer-linked-stack-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let key = storage_key(515);
    let initial = json!({"unrelated-owner":{"retained":true},LOCAL_OWNER:{"activeProfile":"base","runningGames":[{"profileId":"p","gameId":"g","timerTick":1,"unknown":42}],"future":{"x":2}}});
    crate::source_local::set(&path, &key, &initial).unwrap();
    let mut stack = read(&path, 515).unwrap().unwrap();
    stack.select_base("manual");
    save(&path, 515, &stack).unwrap();
    let saved = crate::source_local::get(&path, &key).unwrap().unwrap();
    assert_eq!(saved["unrelated-owner"], initial["unrelated-owner"]);
    assert_eq!(saved[LOCAL_OWNER]["future"], initial[LOCAL_OWNER]["future"]);
    assert_eq!(
        saved[LOCAL_OWNER]["runningGames"],
        initial[LOCAL_OWNER]["runningGames"]
    );
    assert_eq!(saved[LOCAL_OWNER]["activeProfile"], "manual");
    crate::source_local::set(&path, &key, &json!(["invalid shape"])).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(read(&path, 515).is_err());
    assert!(save(&path, 515, &stack).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_file(path.with_extension("json.lock")).unwrap();
}
