use super::*;
use razer_storage::{
    game_catalog::decode_game_catalog,
    host::{HostStorage, HostStorageCall, HostStorageView, HostStoreKind},
};

fn records() -> Vec<GameSignature> {
    decode_game_catalog(include_bytes!("../../testdata/games/matching.xml"))
        .unwrap()
        .into_signatures()
}
fn process(pid: u32, path: &str) -> RunningProcess {
    RunningProcess::new(pid, path)
}
fn master() -> GameLibrary {
    GameLibrary::new(vec![], vec![], records(), vec![])
}

#[test]
fn installed_matching_preserves_local_priority_default_basename_and_path_branches() {
    let signatures = records();
    let local = GameLibrary::new(signatures.clone(), vec![], vec![], vec![]);
    assert_eq!(
        local
            .match_process(r"c:\synthetic\PRIMARY.exe", true)
            .unwrap()
            .display_name(),
        "Local Game"
    );
    assert!(
        local
            .match_process(r"C:\Different\primary.exe", true)
            .is_none()
    );
    // MGL's initial basename candidate still passes the installed-folder
    // resolution in AddDetectedGame before becoming a running game.
    assert!(
        master()
            .match_process(r"C:\Different\primary.exe", true)
            .is_none()
    );
    assert_eq!(
        master()
            .match_process(r"C:\Synthetic\primary.exe", true)
            .unwrap()
            .display_name(),
        "Local Game"
    );
    assert!(
        local
            .match_process(r"C:\Different\alternate64.exe", true)
            .is_some()
    );
    assert!(
        local
            .match_process(r"C:\Synthetic\client.exe", true)
            .is_some()
    );
    assert!(
        local
            .match_process(r"C:\Synthetic\client.exe.bak", true)
            .is_none()
    );
    assert!(
        local
            .match_process(r"C:\Synthetic32\client32.exe", false)
            .is_some()
    );
    assert!(
        local
            .match_process(r"C:\Synthetic32\client32.exe", true)
            .is_none()
    );
    assert!(
        local
            .match_process(r"C:\Synthetic\running.exe", true)
            .is_some()
    );
    assert!(
        local
            .match_process(r"C:\Synthetic\overlay.exe", true)
            .is_some()
    );
    // Current '*' branch checks all fragments, including out-of-order ones.
    assert!(
        local
            .match_process(r"C:\pieces\wild\engine.exe", true)
            .is_some()
    );
    assert!(local.match_process(r"C:\Other\ignored.exe", true).is_none());
    assert!(local.match_process(r"C:\Other\actual.exe", true).is_some());
    let library = GameLibrary::new(
        vec![GameSignature::linked("LGL", r"C:\Synthetic\primary.exe").unwrap()],
        vec![GameSignature::linked("UGL", r"C:\Synthetic\primary.exe").unwrap()],
        records(),
        vec![],
    );
    assert_eq!(
        library
            .match_process(r"C:\Synthetic\primary.exe", true)
            .unwrap()
            .display_name(),
        "LGL"
    );
}

#[test]
fn candidate_filter_covers_source_exe_groups_and_exact_helper_exclusions() {
    let names = master().candidate_names();
    for name in [
        "PRIMARY",
        "PRIMARY32",
        "PRIMARY64",
        "ALTERNATE",
        "ALTERNATE64",
        "CLIENT",
        "CLIENT32",
        "RUNNING",
        "LAUNCHER",
        "CASE",
        "JAVAW",
    ] {
        assert!(names.contains(name), "Missing candidate {name}");
    }
    assert!(!names.contains("EXCLUDED"));
    // Current GetAllPossibleFilenames excludes exact helper text, case-sensitive.
    assert!(names.contains("CASE"));
    assert!(!names.contains("OVERLAY"));
}

#[test]
fn duplicate_pids_share_one_guid_and_stop_after_the_last_observed_process() {
    let library = GameLibrary::new(records(), vec![], vec![], vec![]);
    let mut running = RunningGames::default();
    let first = running.observe(
        &library,
        &[
            process(10, r"C:\Synthetic\primary.exe"),
            process(11, r"C:\G\alternate64.exe"),
            process(12, r"C:\Synthetic\empty.exe"),
            process(0, r"C:\G\other.exe"),
        ],
        true,
    );
    assert_eq!(first.count(), 1);
    assert_eq!(first.messages().len(), 1);
    assert_eq!(first.messages()[0]["action"], "GMS_onGameStartedEvent");
    assert_eq!(
        first.messages()[0]["payload"]["runningGamesDetail"][0]["exe_name"],
        json!(["primary.exe", "primary32.exe", "primary64.exe"])
    );
    assert!(
        running
            .observe(&library, &[process(11, r"C:\G\alternate64.exe")], true)
            .messages()
            .is_empty()
    );
    let last = running.observe(&library, &[], true);
    assert_eq!(last.count(), 0);
    assert_eq!(last.messages()[0]["action"], "GMS_onGameStoppedEvent");
    assert_eq!(last.messages()[0]["payload"]["runningGames"], json!([]));
    assert!(running.observe(&library, &[], true).messages().is_empty());
}

#[test]
fn observation_recovery_republishes_current_starts_without_inventing_stops() {
    let library = master();
    let processes = [process(10, r"C:\Synthetic\primary.exe")];
    let mut running = RunningGames::default();
    running.observe(&library, &processes, true);
    running.invalidate();
    let recovered = running.observe(&library, &processes, true);
    assert_eq!(recovered.messages().len(), 1);
    assert_eq!(recovered.messages()[0]["action"], "GMS_onGameStartedEvent");
    running.invalidate();
    assert!(running.observe(&library, &[], true).messages().is_empty());
}

#[test]
fn source_message_round_trips_through_real_host_storage_and_nested_consumer() {
    let mut storage = HostStorage::default();
    storage.register_view(HostStorageView {
        id: 1,
        url: "razer-local:game-observer".into(),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: true,
    });
    let call = |action: &str, payload| HostStorageCall {
        store: HostStoreKind::Memory,
        sender_id: 1,
        action: action.into(),
        payload,
        target_url_array: vec![],
    };
    let absent = storage
        .call(call("getMemoryStorageItem", json!({"key":STORAGE_KEY})))
        .unwrap();
    assert_eq!(running_count_from_memory(&absent.value), None);
    let mut running = RunningGames::default();
    let update = running.observe(
        &master(),
        &[
            process(10, r"C:\Synthetic\primary.exe"),
            process(20, r"C:\Other\other.exe"),
        ],
        true,
    );
    for message in update.messages() {
        storage
            .call(call(
                "setMemoryStorageItem",
                json!({"key":STORAGE_KEY,"value":message.to_string()}),
            ))
            .unwrap();
    }
    let reply = storage
        .call(call("getMemoryStorageItem", json!({"key":STORAGE_KEY})))
        .unwrap();
    assert_eq!(running_count_from_memory(&reply.value), Some(2));
    let update = running.observe(&master(), &[], true);
    for message in update.messages() {
        storage
            .call(call(
                "setMemoryStorageItem",
                json!({"key":STORAGE_KEY,"value":message.to_string()}),
            ))
            .unwrap();
    }
    let reply = storage
        .call(call("getMemoryStorageItem", json!({"key":STORAGE_KEY})))
        .unwrap();
    assert_eq!(running_count_from_memory(&reply.value), Some(0));
    storage
        .call(call("removeMemoryStorageItem", json!({"key":STORAGE_KEY})))
        .unwrap();
    let reply = storage
        .call(call("getMemoryStorageItem", json!({"key":STORAGE_KEY})))
        .unwrap();
    assert_eq!(running_count_from_memory(&reply.value), None);
    assert_eq!(running_count_from_memory(&json!("invalid")), None);
    assert_eq!(
        running_count_from_memory(&json!("[{\"value\":\"bad json\"}]")),
        None
    );
}

#[test]
fn linked_executable_identity_never_matches_an_unrelated_same_named_program() {
    let library = GameLibrary::new(
        vec![],
        vec![],
        vec![],
        vec![GameSignature::linked("Linked", r"C:\Games\Program.exe").unwrap()],
    );
    assert!(
        library
            .match_process(r"c:\games\program.EXE", true)
            .is_some()
    );
    assert!(
        library
            .match_process(r"C:\Elsewhere\Program.exe", true)
            .is_none()
    );
}

#[test]
fn linked_profile_identity_uses_the_real_local_catalog_match_and_running_order() {
    let linked = GameSignature::linked("App link", r"C:\Synthetic\primary.exe").unwrap();
    let app_id = linked.entry_guid().to_owned();
    let library = GameLibrary::new(records(), vec![], vec![], vec![linked]);
    let source_id = library
        .match_process(r"C:\Synthetic\primary.exe", true)
        .unwrap()
        .entry_guid();
    assert_ne!(source_id, app_id);
    let mut running = RunningGames::default();
    let update = running.observe(&library, &[process(10, r"C:\Synthetic\primary.exe")], true);
    assert_eq!(update.linked_identities()[0].game_id(), source_id);
    assert_eq!(
        update.linked_identities()[0].executable(),
        r"C:\Synthetic\primary.exe"
    );
    assert!(update.available_game_ids().contains(source_id));
    assert!(!update.available_game_ids().contains(&app_id));
    assert_eq!(update.running_games()[0].entry_guid(), source_id);
}

#[test]
fn local_catalog_cache_reloads_changed_files_and_reports_invalid_updates() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "razer-game-monitor-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("MasterGameList.xml");
    let data = include_bytes!("../../testdata/games/matching.xml");
    std::fs::write(&path, data).unwrap();
    let mut cache = CatalogCache::new(CatalogLocations::new(root.clone(), None, "SYNTHETIC-PC"));
    assert!(
        cache
            .refresh()
            .unwrap()
            .match_process(r"C:\Synthetic\primary.exe", true)
            .is_some()
    );
    std::fs::write(&path, "corrupt encrypted XML").unwrap();
    assert!(cache.refresh().is_err());
    std::fs::remove_file(&path).unwrap();
    // Actual absence removes the old installed catalog; it is not a failed read.
    assert!(
        cache
            .refresh()
            .unwrap()
            .match_process(r"C:\Synthetic\primary.exe", true)
            .is_none()
    );
    std::fs::remove_dir(root).unwrap();
}
