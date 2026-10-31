//! Camera task ownership through cancellation, native release and thread join.
//! The registry retains lifecycle resources; camera state stays with callers.
use anyhow::{Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

type Work = Box<dyn FnOnce() -> std::result::Result<(), String> + Send>;
type Outcome = std::result::Result<(), String>;

struct Group {
    id: u64,
    stopped: AtomicBool,
}

enum Worker {
    Starting,
    Deferred,
    Running(JoinHandle<()>),
    Joined(Outcome),
}

struct Entry {
    id: u64,
    group: Arc<Group>,
    name: String,
    canceled: Arc<AtomicBool>,
    worker: Mutex<Worker>,
    work: Mutex<Option<Work>>,
    outcome: Mutex<Option<Outcome>>,
}

fn panic_message(payload: Box<dyn std::any::Any + Send>, name: &str) -> String {
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic payload");
    format!("Camera task {name} panicked: {message}")
}

impl Entry {
    fn run(&self) {
        let work = self
            .work
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let result = match work {
            Some(work) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|payload| Err(panic_message(payload, &self.name))),
            None => Err(format!("Camera task {} has no retained work", self.name)),
        };
        *self
            .outcome
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result);
    }

    fn finish(&self, worker: &mut Worker) -> Outcome {
        let previous = std::mem::replace(worker, Worker::Starting);
        let result = match previous {
            Worker::Running(thread) => thread
                .join()
                .map_err(|payload| panic_message(payload, &self.name))
                .and_then(|()| self.outcome()),
            Worker::Deferred => {
                self.run();
                self.outcome()
            }
            Worker::Joined(result) => result,
            Worker::Starting => Err(format!("Camera task {} has no retained thread", self.name)),
        };
        *worker = Worker::Joined(result.clone());
        result
    }

    fn outcome(&self) -> Outcome {
        self.outcome
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(|| Err(format!("Camera task {} did not finish cleanup", self.name)))
    }

    fn reap_finished(&self) -> Option<Outcome> {
        let mut worker = match self.worker.try_lock() {
            Ok(worker) => worker,
            Err(std::sync::TryLockError::WouldBlock) => return None,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        };
        match &*worker {
            Worker::Running(thread) if thread.is_finished() => Some(self.finish(&mut worker)),
            Worker::Joined(result) => Some(result.clone()),
            _ => None,
        }
    }

    fn join(&self) -> Outcome {
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.finish(&mut worker)
    }
}

#[derive(Default)]
struct State {
    next_group: u64,
    next_task: u64,
    groups: BTreeMap<u64, Weak<Group>>,
    tasks: BTreeMap<u64, Arc<Entry>>,
    failures: BTreeMap<u64, Vec<String>>,
}

#[derive(Default)]
struct Registry {
    state: Mutex<State>,
    poison_reported: AtomicBool,
}

impl Registry {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| {
            // Recover the retained native cleanup resources after a Rust unwind.
            if !self.poison_reported.swap(true, Ordering::AcqRel) {
                eprintln!("Camera task registry lock poisoned; retaining cleanup ownership");
            }
            poisoned.into_inner()
        })
    }

    fn group(&self) -> Arc<Group> {
        let mut state = self.state();
        state.groups.retain(|_, group| group.strong_count() > 0);
        state.next_group = state
            .next_group
            .checked_add(1)
            .expect("Camera task group identity exhausted");
        let group = Arc::new(Group {
            id: state.next_group,
            stopped: AtomicBool::new(false),
        });
        state.groups.insert(group.id, Arc::downgrade(&group));
        group
    }

    fn register(
        &self,
        group: &Arc<Group>,
        name: &str,
        canceled: Arc<AtomicBool>,
        work: Work,
        cleanup: bool,
    ) -> Result<()> {
        self.register_with(group, name, canceled, work, cleanup, |name, run| {
            thread::Builder::new().name(name).spawn(run)
        })
    }

    fn register_with(
        &self,
        group: &Arc<Group>,
        name: &str,
        canceled: Arc<AtomicBool>,
        work: Work,
        cleanup: bool,
        spawn: impl FnOnce(String, Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> Result<()> {
        self.reap_finished(None);
        let mut state = self.state();
        ensure!(
            cleanup || !group.stopped.load(Ordering::Acquire),
            "Camera task owner is stopped"
        );
        state.next_task = state
            .next_task
            .checked_add(1)
            .expect("Camera task identity exhausted");
        let entry = Arc::new(Entry {
            id: state.next_task,
            group: group.clone(),
            name: name.into(),
            canceled,
            worker: Mutex::new(Worker::Starting),
            work: Mutex::new(Some(work)),
            outcome: Mutex::new(None),
        });
        // Count and retain accepted work before creating its OS thread. The
        // same lock orders a group stop against this reservation.
        state.groups.insert(group.id, Arc::downgrade(group));
        state.tasks.insert(entry.id, entry.clone());
        let worker_entry = entry.clone();
        let spawned = if name.contains('\0') {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Camera task name contains NUL",
            ))
        } else {
            spawn(name.into(), Box::new(move || worker_entry.run()))
        };
        match spawned {
            Ok(thread) => {
                *entry
                    .worker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Worker::Running(thread);
                Ok(())
            }
            Err(error) if cleanup => {
                *entry
                    .worker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Worker::Deferred;
                let error =
                    format!("Camera cleanup {name} deferred after thread creation failed: {error}");
                eprintln!("{error}");
                state.failures.entry(group.id).or_default().push(error);
                Ok(())
            }
            Err(error) => {
                state.tasks.remove(&entry.id);
                Err(error.into())
            }
        }
    }

    fn stop(&self, group: &Group) {
        group.stopped.store(true, Ordering::Release);
        let state = self.state();
        for entry in state
            .tasks
            .values()
            .filter(|entry| entry.group.id == group.id)
        {
            entry.canceled.store(true, Ordering::Release);
        }
    }

    fn settle(&self, entry: &Arc<Entry>, result: Outcome) {
        let mut state = self.state();
        if !state
            .tasks
            .get(&entry.id)
            .is_some_and(|current| Arc::ptr_eq(current, entry))
        {
            return;
        }
        state.tasks.remove(&entry.id);
        if let Err(error) = result {
            eprintln!("{error}");
            state
                .failures
                .entry(entry.group.id)
                .or_default()
                .push(error);
        }
    }

    fn reap_finished(&self, group: Option<u64>) {
        let entries: Vec<_> = self
            .state()
            .tasks
            .values()
            .filter(|entry| group.is_none_or(|group| entry.group.id == group))
            .cloned()
            .collect();
        for entry in entries {
            if let Some(result) = entry.reap_finished() {
                self.settle(&entry, result);
            }
        }
    }

    fn take_failures(&self, groups: &BTreeSet<u64>) -> Result<()> {
        let mut state = self.state();
        let failures: Vec<_> = groups
            .iter()
            .filter_map(|group| state.failures.remove(group))
            .flatten()
            .collect();
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }

    fn active_workers(&self) -> usize {
        self.reap_finished(None);
        self.state().tasks.len()
    }

    fn shutdown_all(&self) -> Result<()> {
        let groups = {
            let state = self.state();
            let mut groups: BTreeSet<_> = state.groups.keys().copied().collect();
            groups.extend(state.failures.keys().copied());
            for group in state.groups.values().filter_map(Weak::upgrade) {
                group.stopped.store(true, Ordering::Release);
            }
            for entry in state.tasks.values() {
                groups.insert(entry.group.id);
                entry.canceled.store(true, Ordering::Release);
            }
            groups
        };
        self.drain_groups(&groups)
    }

    fn drain_groups(&self, groups: &BTreeSet<u64>) -> Result<()> {
        loop {
            let entries: Vec<_> = self
                .state()
                .tasks
                .values()
                .filter(|entry| groups.contains(&entry.group.id))
                .cloned()
                .collect();
            if entries.is_empty() {
                break;
            }
            // Never hold the registry lock across native cleanup or thread join.
            // Entries remain counted until their actual join has completed.
            for entry in entries {
                let result = entry.join();
                self.settle(&entry, result);
            }
        }
        self.state().groups.retain(|id, _| !groups.contains(id));
        self.take_failures(groups)
    }
}

fn registry() -> Arc<Registry> {
    static REGISTRY: OnceLock<Arc<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default).clone()
}

/// One retained page/session owner. Its producers share this admission scope.
pub struct Owner {
    spawner: Spawner,
}

impl Default for Owner {
    fn default() -> Self {
        Self::new()
    }
}

impl Owner {
    pub fn new() -> Self {
        Self::with_registry(registry())
    }

    fn with_registry(registry: Arc<Registry>) -> Self {
        let group = registry.group();
        Self {
            spawner: Spawner { registry, group },
        }
    }

    /// Dropping a producer leaves its owner's admission and sibling work intact.
    pub fn spawner(&self) -> Spawner {
        self.spawner.clone()
    }

    pub fn spawn<T: Send + 'static>(
        &self,
        name: &str,
        canceled: Arc<AtomicBool>,
        run: impl FnOnce() -> T + Send + 'static,
    ) -> Result<mpsc::Receiver<T>> {
        self.spawner.spawn(name, canceled, run)
    }

    pub fn is_stopped(&self) -> bool {
        self.spawner.group.stopped.load(Ordering::Acquire)
    }

    /// Close normal admission and signal accepted work without waiting for I/O.
    pub fn stop(&self) {
        self.spawner.registry.stop(&self.spawner.group);
    }

    /// Join only completed threads; never wait on a concurrent background join.
    pub fn reap_finished(&self) -> Result<()> {
        let id = self.spawner.group.id;
        self.spawner.registry.reap_finished(Some(id));
        self.spawner.registry.take_failures(&BTreeSet::from([id]))
    }

    /// Counts this owner's accepted tasks until their real join. Finished
    /// threads and deferred cleanup remain counted until explicitly reaped.
    pub fn active_workers(&self) -> usize {
        let id = self.spawner.group.id;
        self.spawner
            .registry
            .state()
            .tasks
            .values()
            .filter(|entry| entry.group.id == id)
            .count()
    }

    /// Run on a background thread. Stop and drain only this owner's group;
    /// unrelated camera owners retain their admission and pending results.
    pub fn shutdown(&self) -> Result<()> {
        self.stop();
        self.spawner
            .registry
            .drain_groups(&BTreeSet::from([self.spawner.group.id]))
    }

    /// Retain the release of already-owned native resources, including after
    /// stop. Failed thread creation leaves cleanup queued for background drain.
    pub fn cleanup(
        &self,
        name: &str,
        run: impl FnOnce() -> Result<()> + Send + 'static,
    ) -> Result<mpsc::Receiver<Outcome>> {
        let (sender, receiver) = mpsc::channel();
        self.spawner.registry.register(
            &self.spawner.group,
            name,
            Arc::new(AtomicBool::new(false)),
            Box::new(move || {
                let result = run().map_err(|error| format!("{error:#}"));
                let _ = sender.send(result.clone());
                result
            }),
            true,
        )?;
        Ok(receiver)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Child tasks retain their owner's stable group without taking its lifetime.
#[derive(Clone)]
pub struct Spawner {
    registry: Arc<Registry>,
    group: Arc<Group>,
}

impl Spawner {
    pub fn spawn<T: Send + 'static>(
        &self,
        name: &str,
        canceled: Arc<AtomicBool>,
        run: impl FnOnce() -> T + Send + 'static,
    ) -> Result<mpsc::Receiver<T>> {
        let (sender, receiver) = mpsc::channel();
        self.registry.register(
            &self.group,
            name,
            canceled,
            Box::new(move || {
                let result = run();
                // A closed delivery channel does not abandon task cleanup.
                let _ = sender.send(result);
                Ok(())
            }),
            false,
        )?;
        Ok(receiver)
    }
}

/// Counts accepted tasks until actual join, including deferred native cleanup.
/// Reaping never executes deferred cleanup or waits on running threads.
pub fn active_workers() -> usize {
    registry().active_workers()
}

/// Run on a background executor. Close captured groups, cancel their work and
/// join every owned task; later groups keep their independent admission.
pub fn shutdown_all() -> Result<()> {
    registry().shutdown_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    fn canceled() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn group_shutdown_joins_its_work_without_stopping_another_owner() {
        let registry = Arc::new(Registry::default());
        let first = Arc::new(Owner::with_registry(registry.clone()));
        let second = Owner::with_registry(registry.clone());
        let (first_release, first_wait) = mpsc::channel();
        let (second_release, second_wait) = mpsc::channel();
        let first_canceled = canceled();
        let second_canceled = canceled();
        let first_result = first
            .spawn("first-group", first_canceled.clone(), move || {
                first_wait.recv().unwrap()
            })
            .unwrap();
        let second_result = second
            .spawn("second-group", second_canceled.clone(), move || {
                second_wait.recv().unwrap()
            })
            .unwrap();
        assert_eq!(first.active_workers(), 1);
        assert_eq!(second.active_workers(), 1);
        let stopping = first.clone();
        let shutdown = thread::spawn(move || stopping.shutdown());
        while !first_canceled.load(Ordering::Acquire) {
            thread::yield_now();
        }
        assert_eq!(first.active_workers(), 1);
        assert!(!shutdown.is_finished());
        assert!(!second.is_stopped());
        assert!(!second_canceled.load(Ordering::Acquire));
        first_release.send(17).unwrap();
        assert_eq!(first_result.recv().unwrap(), 17);
        shutdown.join().unwrap().unwrap();
        assert_eq!(first.active_workers(), 0);
        assert_eq!(second.active_workers(), 1);
        assert!(first.spawn("closed-first", canceled(), || ()).is_err());
        let sibling = second.spawn("live-second", canceled(), || 23).unwrap();
        assert_eq!(sibling.recv().unwrap(), 23);
        second_release.send(31).unwrap();
        assert_eq!(second_result.recv().unwrap(), 31);
        second.shutdown().unwrap();
        assert_eq!(second.active_workers(), 0);
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn group_shutdown_executes_its_deferred_cleanup_and_preserves_other_failures() {
        let registry = Arc::new(Registry::default());
        let first = Owner::with_registry(registry.clone());
        let second = Owner::with_registry(registry.clone());
        let released = Arc::new(AtomicBool::new(false));
        let retained = released.clone();
        registry
            .register_with(
                &first.spawner.group,
                "deferred-first",
                canceled(),
                Box::new(move || {
                    retained.store(true, Ordering::Release);
                    Ok(())
                }),
                true,
                |_, _| Err(std::io::Error::other("cleanup thread unavailable")),
            )
            .unwrap();
        let other = second
            .cleanup("failed-second", || anyhow::bail!("second failure"))
            .unwrap();
        assert!(other.recv().unwrap().is_err());
        assert_eq!(first.active_workers(), 1);
        let error = first.shutdown().unwrap_err().to_string();
        assert!(error.contains("cleanup thread unavailable"));
        assert!(!error.contains("second failure"));
        assert!(released.load(Ordering::Acquire));
        assert_eq!(first.active_workers(), 0);
        assert!(!second.is_stopped());
        assert!(
            second
                .shutdown()
                .unwrap_err()
                .to_string()
                .contains("second failure")
        );
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn stop_closes_owner_and_child_admission_without_waiting_for_running_work() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        let producer = owner.spawner();
        let gate = Arc::new(Barrier::new(2));
        let worker_gate = gate.clone();
        let signal = canceled();
        let result = producer
            .spawn("held-camera", signal.clone(), move || {
                worker_gate.wait();
                42
            })
            .unwrap();
        owner.stop();
        owner.stop();
        assert!(signal.load(Ordering::Acquire));
        assert!(owner.is_stopped());
        owner.reap_finished().unwrap();
        assert_eq!(registry.active_workers(), 1);
        assert!(owner.spawn("rejected", canceled(), || ()).is_err());
        assert!(producer.spawn("rejected-child", canceled(), || ()).is_err());
        gate.wait();
        assert_eq!(result.recv().unwrap(), 42);
        registry.shutdown_all().unwrap();
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn dropping_delivery_and_owner_keeps_canceled_task_until_real_cleanup() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        let gate = Arc::new(Barrier::new(2));
        let worker_gate = gate.clone();
        let signal = canceled();
        let result = owner
            .spawn("lost-camera-page", signal.clone(), move || {
                worker_gate.wait();
            })
            .unwrap();
        drop(result);
        drop(owner);
        assert!(signal.load(Ordering::Acquire));
        assert_eq!(registry.active_workers(), 1);
        gate.wait();
        registry.shutdown_all().unwrap();
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn failed_thread_creation_releases_a_normal_reservation() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        let result = registry.register_with(
            &owner.spawner.group,
            "failed-camera",
            canceled(),
            Box::new(|| Ok(())),
            false,
            |_, _| Err(std::io::Error::other("simulated thread creation failure")),
        );
        assert!(result.unwrap_err().to_string().contains("simulated"));
        assert_eq!(registry.active_workers(), 0);
        assert!(!owner.is_stopped());
    }

    #[test]
    fn reaped_panics_remain_available_to_their_owner() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        let result = owner
            .spawn("panicking-camera", canceled(), || panic!("cleanup failed"))
            .unwrap();
        assert!(result.recv().is_err());
        let entry = registry.state().tasks.values().next().unwrap().clone();
        let result = entry.join();
        registry.settle(&entry, result);
        assert_eq!(registry.active_workers(), 0);
        let error = owner.reap_finished().unwrap_err().to_string();
        assert!(error.contains("panicking-camera"));
        assert!(error.contains("cleanup failed"));
        owner.reap_finished().unwrap();
    }

    #[test]
    fn child_getters_remain_owned_after_a_parent_reports_failure() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        let producer = owner.spawner();
        let gate = Arc::new(Barrier::new(2));
        let child_gate = gate.clone();
        let parent = owner
            .spawn("camera-query", canceled(), move || {
                let child = producer
                    .spawn("camera-getter", canceled(), move || child_gate.wait())
                    .unwrap();
                drop(child);
                Err::<(), _>("first getter rejected")
            })
            .unwrap();
        assert_eq!(parent.recv().unwrap(), Err("first getter rejected"));
        assert!(registry.active_workers() >= 1);
        assert!(!owner.is_stopped());
        gate.wait();
        registry.shutdown_all().unwrap();
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn failed_cleanup_spawn_retains_native_resources_for_background_drain() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        owner.stop();
        let executed = Arc::new(AtomicBool::new(false));
        let cleanup_executed = executed.clone();
        let (sender, receiver) = mpsc::channel();
        registry
            .register_with(
                &owner.spawner.group,
                "deferred-camera-cleanup",
                canceled(),
                Box::new(move || {
                    cleanup_executed.store(true, Ordering::Release);
                    sender.send(()).unwrap();
                    Ok(())
                }),
                true,
                |_, _| Err(std::io::Error::other("simulated cleanup spawn failure")),
            )
            .unwrap();
        owner.reap_finished().unwrap_err();
        assert!(!executed.load(Ordering::Acquire));
        assert_eq!(registry.active_workers(), 1);
        assert!(receiver.try_recv().is_err());
        registry.shutdown_all().unwrap();
        receiver.recv().unwrap();
        assert!(executed.load(Ordering::Acquire));
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn cleanup_after_stop_preserves_failure_until_it_is_reported() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        owner.stop();
        let result = owner
            .cleanup("failed-native-release", || {
                anyhow::bail!("native release failed")
            })
            .unwrap();
        assert!(
            result
                .recv()
                .unwrap()
                .unwrap_err()
                .contains("native release failed")
        );
        let error = registry.shutdown_all().unwrap_err().to_string();
        assert!(error.contains("native release failed"));
        assert_eq!(registry.active_workers(), 0);
    }

    #[test]
    fn shutdown_snapshot_keeps_successor_groups_and_joins_nested_cleanup() {
        let registry = Arc::new(Registry::default());
        let owner = Owner::with_registry(registry.clone());
        let first_signal = canceled();
        let first_gate = Arc::new(Barrier::new(2));
        let worker_gate = first_gate.clone();
        let worker_registry = registry.clone();
        let group = owner.spawner.group.clone();
        let (replacement_send, replacement_receive) = mpsc::channel();
        let nested_cleanup = Arc::new(AtomicBool::new(false));
        let completed_cleanup = nested_cleanup.clone();
        owner
            .spawn("old-camera", first_signal.clone(), move || {
                worker_gate.wait();
                worker_registry
                    .register(
                        &group,
                        "nested-camera-release",
                        canceled(),
                        Box::new(move || {
                            completed_cleanup.store(true, Ordering::Release);
                            Ok(())
                        }),
                        true,
                    )
                    .unwrap();
                let replacement = Owner::with_registry(worker_registry.clone());
                let gate = Arc::new(Barrier::new(2));
                let worker_gate = gate.clone();
                let signal = canceled();
                replacement
                    .spawn("replacement-camera", signal.clone(), move || {
                        worker_gate.wait()
                    })
                    .unwrap();
                replacement_send.send((replacement, gate, signal)).unwrap();
            })
            .unwrap();
        let shutdown_registry = registry.clone();
        let shutdown = thread::spawn(move || shutdown_registry.shutdown_all());
        while !first_signal.load(Ordering::Acquire) {
            thread::yield_now();
        }
        first_gate.wait();
        let (replacement, replacement_gate, replacement_signal) =
            replacement_receive.recv().unwrap();
        shutdown.join().unwrap().unwrap();
        assert!(owner.is_stopped());
        assert!(!replacement.is_stopped());
        assert!(!replacement_signal.load(Ordering::Acquire));
        assert!(nested_cleanup.load(Ordering::Acquire));
        assert_eq!(registry.active_workers(), 1);
        replacement_gate.wait();
        registry.shutdown_all().unwrap();
        assert_eq!(registry.active_workers(), 0);
    }
}
