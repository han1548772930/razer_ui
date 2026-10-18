use super::{Input, PendingIo, ProfileQueue, Queued};
use razer_storage::linked_profiles::{
    Association, Effect, ProfileStack, RunningGameDetail, Runtime,
};
use serde_json::json;
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

fn runtime() -> Runtime {
    let mut runtime = Runtime::new(ProfileStack::new("base"));
    runtime.configure(
        vec![Association::new("game", "game-profile")],
        BTreeSet::from(["game".into()]),
    );
    runtime
}

fn admit_game_save(queue: &mut ProfileQueue, runtime: &mut Runtime) -> ProfileStack {
    queue
        .effects
        .extend(runtime.game_started("game", Some("base"), 10));
    assert_eq!(queue.next_effect(), Some(Effect::GameActive(true)));
    assert_eq!(
        queue.next_effect(),
        Some(Effect::RuntimeProfile("game-profile".into()))
    );
    let Some(Effect::SaveStack(stack)) = queue.next_effect() else {
        panic!("current source must persist before its system profile switch")
    };
    queue.saving = true;
    stack
}

#[test]
fn accepted_save_finishes_while_exit_holds_the_following_source_switch() {
    let mut queue = ProfileQueue::default();
    let mut runtime = runtime();
    let saved = admit_game_save(&mut queue, &mut runtime);
    assert_eq!(saved.running_games()[0].profile_id(), "game-profile");
    assert!(queue.prepare_exit().is_empty());
    assert!(queue.next_effect().is_none());

    queue.acknowledge_save(&Ok(()));
    assert!(!queue.saving);
    assert!(queue.next_effect().is_none());
    assert_eq!(runtime.active_game(), Some("game"));
    queue.cancel_exit();
    assert_eq!(
        queue.next_effect(),
        Some(Effect::SwitchProfile("game-profile".into()))
    );
    assert!(queue.next_effect().is_none());
    assert!(!queue.saving);
}

#[test]
fn failed_admitted_save_drops_the_system_task_without_undoing_optimistic_state() {
    let mut queue = ProfileQueue::default();
    let mut runtime = runtime();
    admit_game_save(&mut queue, &mut runtime);
    queue.prepare_exit();
    queue.acknowledge_save(&Err("local commit failed".into()));
    queue.cancel_exit();
    assert!(queue.next_effect().is_none());
    assert_eq!(runtime.active_game(), Some("game"));
    assert_eq!(runtime.stack().running_games()[0].game_id(), "game");
}

#[test]
fn exit_extracts_manual_writes_in_order_and_preserves_automatic_observations() {
    let mut queue = ProfileQueue::default();
    assert!(queue.select_base("first".into()).is_none());
    queue.inputs.push_back(Input::Started("game".into()));
    assert!(queue.select_base("second".into()).is_none());
    queue.inputs.push_back(Input::Foreground("Game.exe".into()));
    queue.inputs.push_back(Input::Stopped("game".into()));

    assert_eq!(queue.prepare_exit(), ["first", "second"]);
    assert_eq!(queue.select_base("third".into()), Some("third".into()));
    assert!(!queue.automatic());
    assert_eq!(
        queue.inputs.iter().collect::<Vec<_>>(),
        [
            &Input::Started("game".into()),
            &Input::Foreground("Game.exe".into()),
            &Input::Stopped("game".into()),
        ]
    );
    assert!(queue.prepare_exit().is_empty());
    queue.cancel_exit();
    assert!(queue.automatic());
    assert_eq!(queue.inputs.len(), 3);
}

#[test]
fn resumed_stack_snapshots_preserve_the_latest_explicit_base_and_extension_fields() {
    let initial = json!({
        "activeProfile":"base",
        "runningGames":[{"profileId":"old-game-profile","gameId":"old-game","timerTick":7,"futureEntry":19}],
        "futureStack":{"version":4}
    });
    let stack: ProfileStack = serde_json::from_value(initial.clone()).unwrap();
    let mut queue = ProfileQueue::default();
    queue.effects.push_back(Effect::SaveStack(stack));
    queue.prepare_exit();
    for guid in ["first-manual", "latest-manual"] {
        queue.rebase_pending_stacks(guid);
    }
    assert!(queue.next_effect().is_none());
    queue.cancel_exit();
    let Some(Effect::SaveStack(stack)) = queue.next_effect() else {
        panic!("queued source persistence must resume after exit cancellation")
    };
    let retained = serde_json::to_value(stack).unwrap();
    assert_eq!(retained["activeProfile"], "latest-manual");
    assert_eq!(retained["runningGames"], initial["runningGames"]);
    assert_eq!(retained["futureStack"], initial["futureStack"]);
}

#[test]
fn load_accepted_before_exit_cannot_restore_a_base_which_was_committed_during_exit() {
    let mut queue = ProfileQueue::default();
    queue.select_base("manual-while-loading".into());
    let commits = queue.prepare_exit();
    assert_eq!(commits, ["manual-while-loading"]);
    for guid in commits {
        queue.rebase_pending_stacks(&guid);
    }
    queue.cancel_exit();
    let loaded = queue.reconcile_loaded_stack(ProfileStack::new("old-file-base"));
    assert_eq!(loaded.active_profile(), "manual-while-loading");
    assert!(queue.inputs.is_empty());
    assert!(queue.next_effect().is_none());
}

#[test]
fn source_debounce_is_held_during_exit_and_keeps_its_original_due_time_on_resume() {
    let mut runtime = runtime();
    let mut queue = ProfileQueue::default();
    let details = [RunningGameDetail::new("game", vec!["Game.exe".into()])];
    assert!(
        runtime
            .foreground("Game.exe", &details, Some("base"), 100)
            .is_empty()
    );
    queue.prepare_exit();
    if queue.automatic() {
        queue.effects.extend(runtime.tick(900, Some("base")));
    }
    assert!(queue.effects.is_empty());
    assert!(queue.next_effect().is_none());
    queue.cancel_exit();
    queue.effects.extend(runtime.tick(900, Some("base")));
    assert_eq!(queue.next_effect(), Some(Effect::GameActive(true)));
    assert_eq!(
        queue.next_effect(),
        Some(Effect::RuntimeProfile("game-profile".into()))
    );
}

#[test]
fn accepted_io_remains_pending_through_write_receipt_and_transport_cleanup() {
    struct Transport(Arc<AtomicBool>);
    impl Drop for Transport {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let pending = PendingIo::default();
    let (commands, receiver) = mpsc::channel();
    let (receipts, responses) = mpsc::channel();
    let entered = Arc::new(Barrier::new(2));
    let finish_write = Arc::new(Barrier::new(2));
    let finish_cleanup = Arc::new(Barrier::new(2));
    let destroyed = Arc::new(AtomicBool::new(false));
    commands.send(pending.admit("write")).unwrap();
    commands.send(pending.admit("cleanup")).unwrap();
    drop(commands);
    let worker = {
        let entered = entered.clone();
        let finish_write = finish_write.clone();
        let finish_cleanup = finish_cleanup.clone();
        let destroyed = destroyed.clone();
        thread::spawn(move || {
            let transport = Transport(destroyed);
            let Queued { command, _pending } = receiver.recv().unwrap();
            assert_eq!(command, "write");
            entered.wait();
            finish_write.wait();
            receipts.send(_pending).unwrap();
            let Queued { command, _pending } = receiver.recv().unwrap();
            assert_eq!(command, "cleanup");
            finish_cleanup.wait();
            drop(transport);
            receipts.send(_pending).unwrap();
        })
    };
    entered.wait();
    assert!(pending.is_pending());
    finish_write.wait();
    let write_receipt = responses.recv().unwrap();
    assert!(pending.is_pending());
    assert!(!destroyed.load(Ordering::Acquire));
    drop(write_receipt);
    assert!(pending.is_pending());
    finish_cleanup.wait();
    let cleanup_receipt = responses.recv().unwrap();
    worker.join().unwrap();
    assert!(destroyed.load(Ordering::Acquire));
    assert!(pending.is_pending());
    drop(cleanup_receipt);
    assert!(!pending.is_pending());
}

#[test]
fn rejected_or_abandoned_commands_release_their_pending_admission() {
    let pending = PendingIo::default();
    let (commands, receiver) = mpsc::channel();
    commands.send(pending.admit("queued-write")).unwrap();
    assert!(pending.is_pending());
    drop(receiver);
    assert!(!pending.is_pending());
    assert!(commands.send(pending.admit("rejected-write")).is_err());
    assert!(!pending.is_pending());
}
