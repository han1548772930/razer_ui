//! One OS observer feeds the retained controller and keyboard lighting owners.
use super::*;
use razer_device::controller_lighting::power_policy::Event as ControllerEvent;
use razer_platform::power_idle;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Default)]
pub(super) struct State {
    task: Option<Task<()>>,
    stop: Arc<AtomicBool>,
    leases: Arc<AtomicUsize>,
}
impl State {
    pub(super) fn pending(&self) -> bool {
        self.task.is_some() || self.leases.load(Ordering::Acquire) > 0
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
struct Accepted(Arc<AtomicUsize>);
impl Accepted {
    fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(counter.clone())
    }
}
impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct MonitorLease {
    monitor: Option<power_idle::Monitor>,
    accepted: Option<Accepted>,
    executor: BackgroundExecutor,
}
impl Drop for MonitorLease {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            let accepted = self.accepted.take();
            self.executor
                .spawn(async move {
                    let _accepted = accepted;
                    if let Err(error) = monitor.stop() {
                        eprintln!("[power observation cleanup] {error}");
                    }
                })
                .detach();
        }
    }
}

impl AppShell {
    pub(super) fn install_power_observation_cleanup(&mut self, cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, _| {
            shell.power_observation.stop.store(true, Ordering::Release);
            let delivery = shell.power_observation.task.take();
            async move {
                if let Some(delivery) = delivery {
                    delivery.await;
                }
            }
        });
        self.register_exit_resume(|shell, cx| shell.ensure_power_observation(cx));
        self.ensure_power_observation(cx);
    }

    pub(super) fn ensure_power_observation(&mut self, cx: &mut Context<Self>) {
        if self.power_observation.task.is_some() || self.close_requested {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.power_observation.stop = stop.clone();
        let accepted = Accepted::new(&self.power_observation.leases);
        self.power_observation.task = Some(cx.spawn(async move |this, cx| {
            let started = cx
                .background_spawn(async move { (power_idle::Monitor::start(), accepted) })
                .await;
            let result = match started {
                (Err(error), _accepted) => Err(error),
                (Ok((monitor, events)), accepted) => {
                    let mut lease = MonitorLease {
                        monitor: Some(monitor),
                        accepted: Some(accepted),
                        executor: cx.background_executor().clone(),
                    };
                    let mut failure = None;
                    while !stop.load(Ordering::Acquire) {
                        cx.background_executor()
                            .timer(Duration::from_millis(200))
                            .await;
                        let mut batch = Vec::new();
                        while let Ok(event) = events.try_recv() {
                            batch.push(event);
                        }
                        if batch.is_empty() && events.is_closed() {
                            failure = Some("System power observation stream closed".to_owned());
                            break;
                        }
                        let current = this.update(cx, |this, cx| {
                            if this.close_requested
                                || !Arc::ptr_eq(&this.power_observation.stop, &stop)
                            {
                                return None;
                            }
                            this.refresh_macro_recorder_storage();
                            this.sync_function_primary_recorder(cx);
                            if this.keyboard_power.has_pending_recorder() {
                                this.sync_keyboard_power_owners(cx);
                            }
                            this.refresh_controller_power_configuration(cx);
                            let owners = this
                                .controller_lighting_owners
                                .iter()
                                .filter_map(|(identity, session)| {
                                    let workspace = this.devices.iter().find(|workspace| {
                                        workspace.read(cx).identity(cx) == *identity
                                    })?;
                                    let owner = session.owner();
                                    this.device_observations
                                        .iter()
                                        .any(|route| {
                                            route.matches(workspace.read(cx).device(cx))
                                                && owner.matches_route(route)
                                        })
                                        .then_some(owner)
                                })
                                .collect::<Vec<_>>();
                            let mut actions = Vec::new();
                            for event in &batch {
                                this.keyboard_power_event(event, cx);
                                match event {
                                    power_idle::Event::DisplayOff => {
                                        actions.push(ControllerEvent::DisplayOff)
                                    }
                                    power_idle::Event::DisplayOn => {
                                        actions.push(ControllerEvent::DisplayOn)
                                    }
                                    power_idle::Event::DisplayDimmed => {
                                        actions.push(ControllerEvent::DisplayDimmed)
                                    }
                                    power_idle::Event::Idle { milliseconds } => {
                                        actions.push(ControllerEvent::Idle {
                                            milliseconds: *milliseconds,
                                        })
                                    }
                                    power_idle::Event::Shutdown => {
                                        actions.push(ControllerEvent::Shutdown)
                                    }
                                    power_idle::Event::Resume
                                    | power_idle::Event::ScreenUnlocked => {}
                                    power_idle::Event::Failed(error) => {
                                        this.status = format!("显示器／闲置状态未读取：{error}");
                                        cx.notify();
                                    }
                                }
                            }
                            Some((owners, actions))
                        });
                        let Ok(Some((owners, actions))) = current else {
                            break;
                        };
                        if actions.is_empty() {
                            continue;
                        }
                        let signal = stop.clone();
                        let failures = cx
                            .background_spawn(async move {
                                let mut failures = Vec::new();
                                for action in actions {
                                    if signal.load(Ordering::Acquire) {
                                        break;
                                    }
                                    for owner in &owners {
                                        if let Err(error) = owner.apply_power_event(action) {
                                            failures.push(format!("{error:#}"));
                                        }
                                    }
                                }
                                failures
                            })
                            .await;
                        if !failures.is_empty() {
                            let _ = this.update(cx, |this, cx| {
                                if Arc::ptr_eq(&this.power_observation.stop, &stop) {
                                    this.status = format!(
                                        "手柄运行时亮度写入未确认：{}",
                                        failures.join("；")
                                    );
                                    cx.notify();
                                }
                            });
                        }
                    }
                    let monitor = lease.monitor.take().expect("Live monitor lease");
                    let accepted = lease.accepted.take();
                    let cleanup = cx
                        .background_spawn(async move {
                            let _accepted = accepted;
                            monitor.stop()
                        })
                        .await;
                    match (failure, cleanup) {
                        (None, result) => result,
                        (Some(error), Ok(())) => Err(error),
                        (Some(error), Err(cleanup)) => Err(format!("{error}; cleanup: {cleanup}")),
                    }
                }
            };
            let _ = this.update(cx, |this, cx| {
                let current = Arc::ptr_eq(&this.power_observation.stop, &stop);
                let succeeded = result.is_ok();
                let restart = current && succeeded && !this.close_requested;
                if current {
                    this.power_observation.task = None;
                }
                if let Err(error) = result {
                    if current {
                        this.status = format!("显示器／闲置观察停止：{error}");
                    } else {
                        eprintln!("[retired power observation] {error}");
                    }
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(succeeded, cx);
                }
                if restart {
                    this.ensure_power_observation(cx);
                }
                cx.notify();
            });
        }));
    }
}
