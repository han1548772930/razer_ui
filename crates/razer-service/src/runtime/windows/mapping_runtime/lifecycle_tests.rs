use super::*;

#[test]
fn caller_drain_cancels_and_joins_real_workers_despite_scheduler_mismatch() {
    let mut state = State::new();
    let caller = Uuid::new_v4();
    let mut reserve = |guid: &str| match state
        .scheduler
        .submit(
            Request {
                caller,
                guid: guid.into(),
                payload: Parameters {
                    repeat: 1,
                    program: Program::Macro,
                    driver_route: None,
                    mapped_key: None,
                },
            },
            Mode::Start,
        )
        .unwrap()
    {
        Transition::Start(dispatch) => dispatch.id,
        _ => panic!("Expected a reserved execution"),
    };
    let missing_worker = reserve("a_missing_worker");
    let scheduled_worker = reserve("z_existing_worker");
    let unscheduled_worker = Uuid::new_v4();
    let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut cancellations = Vec::new();
    for id in [scheduled_worker, unscheduled_worker] {
        let cancellation = Arc::new(Cancellation::default());
        let signal = cancellation.clone();
        let completed = completed.clone();
        let thread = thread::spawn(move || {
            signal.wait(Duration::from_secs(2));
            ensure!(
                signal.is_cancelled(),
                "Worker was not cancelled before join"
            );
            completed.fetch_add(1, Ordering::SeqCst);
            Ok(Finished {
                id,
                completion: Completion {
                    outcome: razer_device::macro_runtime::Outcome::Cancelled,
                    execution_error: None,
                    operation_errors: Vec::new(),
                    omitted_operation_errors: 0,
                    cleanup_errors: Vec::new(),
                },
                last_input: None,
                completed_inputs: 0,
                audio_observations: Vec::new(),
            })
        });
        cancellations.push(cancellation.clone());
        state.running.insert(
            id,
            Running {
                caller,
                cancellation,
                thread,
            },
        );
    }
    let error = state.drain_caller(caller).unwrap_err().to_string();
    assert!(error.contains(&missing_worker.to_string()));
    assert!(error.contains("Macro scheduler completion"));
    assert!(cancellations.iter().all(|signal| signal.is_cancelled()));
    assert_eq!(completed.load(Ordering::SeqCst), 2);
    assert!(state.running.is_empty());
    assert_eq!(state.pending_completions.len(), 2);
    assert!(
        state
            .pending_completions
            .iter()
            .all(|(_, result)| result.is_ok())
    );
}
