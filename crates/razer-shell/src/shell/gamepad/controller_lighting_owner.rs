//! The current Kitsune quick-effect engine survives page and profile changes.
//! Its child process owns native frame delivery until physical removal or exit.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::controller_input::Event;
use razer_device::controller_lighting::{SoftwarePhase, SoftwareStatus};
use razer_device::controller_lighting::power_policy::{Event as PowerEvent, Policy};
use crate::shell::controller_power::Configuration as PowerConfiguration;
use razer_discovery::discovery::ObservedDevice;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

trait Connection: Send {
    fn request(&mut self, operation: &LightingOperation) -> anyhow::Result<LightingResult>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
}

struct LocalConnection {
    client: ServiceClient,
    node: HidNode,
    product_id: u32,
}
impl LocalConnection {
    fn open(route: &ObservedDevice) -> anyhow::Result<Self> {
        let mut client = ServiceClient::spawn()?;
        let node = razer_discovery::controller_lighting::resolve(&mut client, route)?;
        Ok(Self {
            client,
            node,
            product_id: route.product_id(),
        })
    }
}
impl Connection for LocalConnection {
    fn request(&mut self, operation: &LightingOperation) -> anyhow::Result<LightingResult> {
        let reply: LightingReply =
            serde_json::from_value(self.client.request(ServiceRequest::ControllerLighting {
                node: self.node.clone(),
                product_id: self.product_id,
                lighting_operation: operation.clone(),
            })?)?;
        reply.result(&self.node, self.product_id, operation)
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        if !self.client.is_stopped() {
            self.client.request(ServiceRequest::Shutdown)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct ClientState {
    connection: Option<Box<dyn Connection>>,
    software_started: bool,
    advanced: bool,
    advanced_applied: Option<bool>,
    power: Option<(String, Policy)>,
}
impl ClientState {
    fn request(&mut self, operation: &LightingOperation) -> anyhow::Result<LightingResult> {
        let connection = self
            .connection
            .as_mut()
            .context("Controller lighting connection was not initialized")?;
        if matches!(operation, LightingOperation::Software { .. }) {
            // A failed start can still own native handles and requires cleanup.
            self.software_started = true;
            self.advanced_applied=None;
        }
        let mut result = connection.request(operation)?;
        if self.advanced && matches!(operation, LightingOperation::Software { .. }) {
            let suspended = connection.request(&LightingOperation::SuspendSoftware { suspended: true })?;
            result.software = suspended.software;
            self.advanced_applied=Some(true);
        } else if matches!(operation, LightingOperation::Software {..}) {
            self.advanced_applied=Some(false);
        }
        if matches!(operation, LightingOperation::StopSoftware) {
            self.software_started = false;
            self.advanced_applied=None;
        }
        Ok(result)
    }
    fn set_advanced(&mut self,enabled:bool)->anyhow::Result<Option<LightingResult>> {
        self.advanced=enabled;
        if !self.software_started||self.advanced_applied==Some(enabled) {return Ok(None);}
        let result=self.request(&LightingOperation::SuspendSoftware {suspended:enabled})?;
        self.advanced_applied=Some(enabled);
        Ok(Some(result))
    }
    fn release(&mut self) -> anyhow::Result<()> {
        let Some(mut connection) = self.connection.take() else {
            return Ok(());
        };
        let mut failures = Vec::new();
        if self.software_started {
            match connection.request(&LightingOperation::StopSoftware) {
                Ok(result) => match result.software {
                    Some(status)
                        if status.phase == SoftwarePhase::Stopped
                            && status.cleanup_errors.is_empty()
                            && status.restore_acknowledged => {}
                    Some(status) => failures.push(format!(
                        "Controller software lighting cleanup was not confirmed: {status:?}"
                    )),
                    None => {
                        failures.push("Controller software lighting cleanup has no receipt".into())
                    }
                },
                Err(error) => failures.push(format!(
                    "Controller software lighting cleanup failed: {error:#}"
                )),
            }
        }
        self.software_started = false;
        self.advanced_applied=None;
        if let Err(error) = connection.shutdown() {
            failures.push(format!(
                "Controller lighting child shutdown failed: {error:#}"
            ));
        }
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
}

pub(crate) struct RetainedOwner {
    route: ObservedDevice,
    accepting: AtomicBool,
    state: Mutex<ClientState>,
    predecessor: Option<Arc<RetainedOwner>>,
    input: Mutex<InputQueue>,
    power_configuration: Mutex<Option<PublishedPowerConfiguration>>,
}
#[derive(Clone)]
struct PublishedPowerConfiguration {
    configuration:PowerConfiguration,
    guard:Arc<AtomicBool>,
}
#[derive(Default)]
struct InputQueue {
    pending: VecDeque<Event>,
    pumping: bool,
}
impl RetainedOwner {
    pub(in crate::shell) fn matches_route(&self,route:&ObservedDevice)->bool {same_route(&self.route,route)}
    pub(in crate::shell) fn configure_power(&self,configuration:PowerConfiguration)->anyhow::Result<()> {
        configuration.settings.validate()?;
        configuration.brightness.validate()?;
        let mut published=self.power_configuration.lock().map_err(|_|anyhow::anyhow!("Controller power configuration lock failed"))?;
        if published.as_ref().is_some_and(|previous|previous.configuration==configuration) {return Ok(());}
        if let Some(previous)=published.take() {previous.guard.store(false,Ordering::Release);}
        *published=Some(PublishedPowerConfiguration {configuration,guard:Arc::new(AtomicBool::new(true))});
        Ok(())
    }
    pub(in crate::shell) fn apply_power_event(&self,event:PowerEvent)->anyhow::Result<Option<LightingResult>> {
        let mut state=self.state.lock().map_err(|_|anyhow::anyhow!("Controller lighting owner lock failed"))?;
        if !self.accepting.load(Ordering::Acquire) {return Ok(None);}
        // Configuration publication never holds the I/O lock on the GUI thread.
        // Read it after taking that lock so a queued event uses the current profile.
        let Some(published)=self.power_configuration.lock().map_err(|_|anyhow::anyhow!("Controller power configuration lock failed"))?.clone() else {return Ok(None);};
        if !published.guard.load(Ordering::Acquire) {return Ok(None);}
        let configuration=published.configuration;
        match &mut state.power {
            Some((id,policy)) if *id==configuration.profile_id=>policy.update(configuration.settings,configuration.brightness)?,
            _=>state.power=Some((configuration.profile_id,Policy::new(configuration.settings,configuration.brightness)?)),
        }
        let Some(value)=state.power.as_mut().expect("power policy initialized").1.apply(event) else {return Ok(None);};
        if state.connection.is_none() {
            if let Some(predecessor)=&self.predecessor {predecessor.release()?;}
            state.connection=Some(Box::new(LocalConnection::open(&self.route)?));
        }
        ensure!(self.accepting.load(Ordering::Acquire)&&published.guard.load(Ordering::Acquire),"Controller runtime brightness profile was replaced");
        let result=state.request(&LightingOperation::Brightness {value})?;
        ensure!(self.accepting.load(Ordering::Acquire)&&published.guard.load(Ordering::Acquire),"Controller runtime brightness response belongs to a retired owner or profile");
        Ok(Some(result))
    }
    pub(in crate::shell) fn set_advanced(
        &self,
        enabled: bool,
        guard: &AtomicBool,
    ) -> anyhow::Result<Option<LightingResult>> {
        ensure!(self.accepting.load(Ordering::Acquire) && guard.load(Ordering::Acquire), "Controller lighting owner was retired");
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("Controller lighting owner lock failed"))?;
        ensure!(self.accepting.load(Ordering::Acquire) && guard.load(Ordering::Acquire), "Controller lighting owner was retired");
        let result = state.set_advanced(enabled)?;
        ensure!(self.accepting.load(Ordering::Acquire) && guard.load(Ordering::Acquire), "Controller lighting response belongs to a retired request");
        Ok(result)
    }
    pub(super) fn submit(
        &self,
        request: &ControllerLightingRequested,
    ) -> anyhow::Result<LightingResult> {
        self.submit_operation(request.operation(), &request.guard())
    }
    pub(in crate::shell) fn submit_operation(
        &self,
        operation: &LightingOperation,
        guard: &AtomicBool,
    ) -> anyhow::Result<LightingResult> {
        ensure!(
            guard.load(Ordering::Acquire),
            "Controller lighting profile was replaced"
        );
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Controller lighting owner lock failed"))?;
        ensure!(
            self.accepting.load(Ordering::Acquire) && guard.load(Ordering::Acquire),
            "Controller lighting owner was retired"
        );
        if state.connection.is_none() {
            if let Some(predecessor) = &self.predecessor {
                predecessor.release()?;
            }
            state.connection = Some(Box::new(LocalConnection::open(&self.route)?));
        }
        // A profile guard must not kill this retained child: it also owns the
        // native engine's stop, restore and handle-release chain.
        let result = state.request(operation)?;
        ensure!(
            self.accepting.load(Ordering::Acquire) && guard.load(Ordering::Acquire),
            "Controller lighting response belongs to a retired request"
        );
        Ok(result)
    }
    fn poll(&self) -> anyhow::Result<Option<SoftwareStatus>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Controller lighting owner lock failed"))?;
        if !self.accepting.load(Ordering::Acquire) || !state.software_started {
            return Ok(None);
        }
        let result = state.request(&LightingOperation::PollSoftware)?;
        Ok(result.software)
    }
    fn release(&self) -> anyhow::Result<()> {
        self.accepting.store(false, Ordering::Release);
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Controller lighting owner lock failed"))?
            .release()
    }
    fn enqueue_input(&self, events: &[Event]) -> anyhow::Result<bool> {
        let mut input = self
            .input
            .lock()
            .map_err(|_| anyhow::anyhow!("Controller lighting input lock failed"))?;
        if !self.accepting.load(Ordering::Acquire) {
            return Ok(false);
        }
        let buttons = events.iter().filter(|event| matches!(event, Event::Buttons { .. })).count();
        ensure!(input.pending.len().saturating_add(buttons) <= 4096, "Controller lighting input queue is full");
        input.pending.extend(
            events
                .iter()
                .filter(|event| matches!(event, Event::Buttons { .. }))
                .cloned(),
        );
        if input.pumping || input.pending.is_empty() {
            return Ok(false);
        }
        input.pumping = true;
        Ok(true)
    }
    fn pump_input(&self) -> anyhow::Result<()> {
        loop {
            let event = {
                let mut input = self
                    .input
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Controller lighting input lock failed"))?;
                if !self.accepting.load(Ordering::Acquire) {
                    input.pending.clear();
                }
                let event = input.pending.pop_front();
                if event.is_none() {
                    input.pumping = false;
                }
                event
            };
            let Some(event) = event else {
                return Ok(());
            };
            let result = (|| {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Controller lighting owner lock failed"))?;
                if self.accepting.load(Ordering::Acquire) && state.software_started {
                    state.request(&LightingOperation::SoftwareInput { event })?;
                }
                Ok(())
            })();
            if result.is_err() {
                if let Ok(mut input) = self.input.lock() {
                    input.pending.clear();
                    input.pumping = false;
                }
                return result;
            }
        }
    }
}

pub(crate) struct Session {
    owner: Arc<RetainedOwner>,
    _poll: Task<()>,
}
impl Session {
    pub(in crate::shell) fn owner(&self) -> Arc<RetainedOwner> {
        self.owner.clone()
    }
    fn retire(self) -> Arc<RetainedOwner> {
        self.owner.accepting.store(false, Ordering::Release);
        self.owner
    }
}

fn same_route(first: &ObservedDevice, second: &ObservedDevice) -> bool {
    first.product_id() == second.product_id()
        && first.physical_product_id() == second.physical_product_id()
        && first.container().eq_ignore_ascii_case(second.container())
        && first.hid_node() == second.hid_node()
}

impl AppShell {
    pub(in crate::shell) fn forward_controller_lighting_input(
        &mut self,
        identity: &str,
        events: &[Event],
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.controller_lighting_owners.get(identity) else {
            return;
        };
        let owner = session.owner.clone();
        match owner.enqueue_input(events) {
            Ok(true) => {
                cx.spawn(async move |this, cx| {
                    let result = cx.background_spawn(async move { owner.pump_input() }).await;
                    if let Err(error) = result {
                        let _ = this.update(cx, |this, cx| {
                            this.status = format!("手柄软件灯效输入未提交：{error:#}");
                            cx.notify();
                        });
                    }
                })
                .detach();
            }
            Ok(false) => {}
            Err(error) => {
                self.status = format!("手柄软件灯效输入未排队：{error:#}");
                cx.notify();
            }
        }
    }
    pub(in crate::shell) fn install_controller_lighting_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            this.controller_power.stop.store(true,Ordering::Release);
            let power=this.controller_power.task.take();
            let mut owners = std::mem::take(&mut this.controller_lighting_owners)
                .into_values()
                .map(Session::retire)
                .collect::<Vec<_>>();
            owners.extend(std::mem::take(&mut this.controller_lighting_cleanup));
            let cleanup = cx.background_executor().spawn(async move {
                for owner in owners {
                    if let Err(error) = owner.release() {
                        eprintln!("[controller-lighting] {error:#}");
                    }
                }
            });
            async move { if let Some(power)=power {power.await;} cleanup.await }
        }));
    }

    pub(in crate::shell) fn retire_controller_lighting_owners(&mut self, cx: &mut Context<Self>) {
        self.controller_power.stop.store(true,Ordering::Release);
        for (_, session) in std::mem::take(&mut self.controller_lighting_owners) {
            self.release_controller_lighting_owner(session.retire(), cx);
        }
    }

    fn release_controller_lighting_owner(
        &mut self,
        owner: Arc<RetainedOwner>,
        cx: &mut Context<Self>,
    ) {
        self.controller_lighting_cleanup.push(owner.clone());
        self.controller_lighting_workers += 1;
        cx.spawn(async move |this, cx| {
            let selected = owner.clone();
            let result = cx.background_spawn(async move { selected.release() }).await;
            let _ = this.update(cx, |this, cx| {
                this.controller_lighting_cleanup
                    .retain(|retired| !Arc::ptr_eq(retired, &owner));
                this.controller_lighting_workers =
                    this.controller_lighting_workers.saturating_sub(1);
                if let Err(error) = result {
                    this.status = format!("手柄软件灯效恢复或释放未确认：{error:#}");
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::shell) fn sync_controller_lighting_owners(
        &mut self,
        complete: bool,
        cx: &mut Context<Self>,
    ) {
        if !complete {
            return;
        }
        let stale = self
            .controller_lighting_owners
            .iter()
            .filter_map(|(identity, session)| {
                let current = self
                    .devices
                    .iter()
                    .find(|workspace| workspace.read(cx).identity(cx) == *identity);
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|route| {
                        current
                            .is_some_and(|workspace| route.matches(workspace.read(cx).device(cx)))
                    })
                    .collect::<Vec<_>>();
                (!matches!(routes.as_slice(), [route] if same_route(route, &session.owner.route)))
                    .then(|| identity.clone())
            })
            .collect::<Vec<_>>();
        for identity in stale {
            self.controller_software_status.remove(&identity);
            if let Some(session) = self.controller_lighting_owners.remove(&identity) {
                self.release_controller_lighting_owner(session.retire(), cx);
            }
        }
        self.refresh_controller_power_configuration(cx);
    }

    pub(in crate::shell) fn controller_lighting_owner(
        &mut self,
        identity: &str,
        route: ObservedDevice,
        cx: &mut Context<Self>,
    ) -> Arc<RetainedOwner> {
        if let Some(session) = self.controller_lighting_owners.get(identity) {
            if same_route(&session.owner.route, &route) {
                return session.owner.clone();
            }
        }
        let predecessor = self
            .controller_lighting_owners
            .remove(identity)
            .map(Session::retire)
            .or_else(|| {
                self.controller_lighting_cleanup
                    .iter()
                    .find(|owner| {
                        owner.route.product_id() == route.product_id()
                            && owner
                                .route
                                .container()
                                .eq_ignore_ascii_case(route.container())
                    })
                    .cloned()
            });
        if let Some(predecessor) = &predecessor {
            self.release_controller_lighting_owner(predecessor.clone(), cx);
        }
        let owner = Arc::new(RetainedOwner {
            route,
            accepting: AtomicBool::new(true),
            state: Mutex::new(ClientState {
                advanced: self.controller_chroma.enabled == Some(true),
                ..Default::default()
            }),
            predecessor,
            input: Mutex::new(InputQueue::default()),
            power_configuration: Mutex::new(None),
        });
        let retained = owner.clone();
        let key = identity.to_owned();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                // This observes diagnostics; the service owns source-timed
                // native polling and frame delivery independently of the GUI.
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if !retained.accepting.load(Ordering::Acquire) {
                    break;
                }
                let selected = retained.clone();
                let result = cx.background_spawn(async move { selected.poll() }).await;
                let failed = result.is_err();
                let current = this.update(cx, |this, cx| {
                    let current = this
                        .controller_lighting_owners
                        .get(&key)
                        .is_some_and(|session| Arc::ptr_eq(&session.owner, &retained));
                    if !current {
                        return false;
                    }
                    match result {
                        Ok(Some(status)) => {
                            if !status.errors.is_empty() || !status.cleanup_errors.is_empty() {
                                this.status = format!(
                                    "手柄软件灯效引擎报告错误：{}",
                                    status
                                        .errors
                                        .iter()
                                        .chain(&status.cleanup_errors)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join("；")
                                );
                                cx.notify();
                            }
                            // Device/service observations stay outside the
                            // source's optimistic profile and control state.
                            this.controller_software_status.insert(key.clone(), status);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            this.status = format!("手柄软件灯效状态未读取：{error:#}");
                            cx.notify();
                        }
                    }
                    true
                });
                if failed || !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.controller_lighting_owners.insert(
            identity.into(),
            Session {
                owner: owner.clone(),
                _poll: poll,
            },
        );
        self.start_controller_power_monitor(cx);
        owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::controller_lighting::SoftwareEffect;

    struct SimulatedConnection {
        calls: Arc<Mutex<Vec<&'static str>>>,
        fail_start: bool,
        fail_stop: bool,
    }
    impl Connection for SimulatedConnection {
        fn request(&mut self, operation: &LightingOperation) -> anyhow::Result<LightingResult> {
            let name = match operation {
                LightingOperation::Software { .. } => "effect",
                LightingOperation::PollSoftware => "poll",
                LightingOperation::SuspendSoftware { .. } => "suspend",
                LightingOperation::StopSoftware => "stop",
                _ => "hardware",
            };
            self.calls.lock().unwrap().push(name);
            ensure!(
                !(name == "effect" && self.fail_start),
                "simulated partial native start failure"
            );
            ensure!(
                !(name == "stop" && self.fail_stop),
                "simulated restore failure"
            );
            let mut status = SoftwareStatus::starting(1);
            status.phase = if name == "stop" {
                SoftwarePhase::Stopped
            } else {
                SoftwarePhase::Running
            };
            status.restore_acknowledged = name == "stop";
            Ok(LightingResult {
                software: Some(status),
                ..Default::default()
            })
        }
        fn shutdown(&mut self) -> anyhow::Result<()> {
            self.calls.lock().unwrap().push("shutdown");
            Ok(())
        }
    }
    fn state(fail_start: bool, fail_stop: bool) -> (ClientState, Arc<Mutex<Vec<&'static str>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            ClientState {
                connection: Some(Box::new(SimulatedConnection {
                    calls: calls.clone(),
                    fail_start,
                    fail_stop,
                })),
                advanced: false,
                advanced_applied:None,
                software_started: false,
                power: None,
            },
            calls,
        )
    }
    fn effect() -> LightingOperation {
        LightingOperation::Software {
            edition: 0,
            effect: SoftwareEffect::Static { color: [1, 2, 3] },
        }
    }

    #[core::prelude::v1::test]
    fn controller_lighting_profile_updates_and_suspend_keep_one_connection_until_release() {
        let (mut state, calls) = state(false, false);
        state.request(&effect()).unwrap();
        state.request(&effect()).unwrap();
        state
            .request(&LightingOperation::SuspendSoftware { suspended: true })
            .unwrap();
        state.request(&LightingOperation::PollSoftware).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            ["effect", "effect", "suspend", "poll"]
        );
        state.release().unwrap();
        state.release().unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            ["effect", "effect", "suspend", "poll", "stop", "shutdown"]
        );
    }

    #[core::prelude::v1::test]
    fn controller_lighting_partial_start_still_releases_native_owner_before_shutdown() {
        let (mut state, calls) = state(true, false);
        assert!(state.request(&effect()).is_err());
        state.release().unwrap();
        assert_eq!(*calls.lock().unwrap(), ["effect", "stop", "shutdown"]);
    }

    #[core::prelude::v1::test]
    fn controller_lighting_restore_failure_is_reported_and_does_not_skip_child_shutdown() {
        let (mut state, calls) = state(false, true);
        state.request(&effect()).unwrap();
        let error = state.release().unwrap_err().to_string();
        assert!(error.contains("simulated restore failure"));
        assert_eq!(*calls.lock().unwrap(), ["effect", "stop", "shutdown"]);
    }
    #[core::prelude::v1::test]
    fn controller_lighting_advanced_selection_before_start_suspends_each_new_quick_effect() {
        let (mut state,calls)=state(false,false);
        assert!(state.set_advanced(true).unwrap().is_none());
        assert!(calls.lock().unwrap().is_empty());
        state.request(&effect()).unwrap();
        assert_eq!(state.advanced_applied,Some(true));
        assert!(state.set_advanced(true).unwrap().is_none());
        state.request(&effect()).unwrap();
        assert_eq!(*calls.lock().unwrap(),["effect","suspend","effect","suspend"]);
        assert!(state.set_advanced(false).unwrap().is_some());
        assert_eq!(state.advanced_applied,Some(false));
        assert!(state.set_advanced(false).unwrap().is_none());
    }
    #[core::prelude::v1::test]
    fn controller_lighting_failed_suspend_is_retried_until_an_actual_service_response() {
        struct FailFirstSuspend {values:Arc<Mutex<Vec<bool>>>}
        impl Connection for FailFirstSuspend {
            fn request(&mut self,operation:&LightingOperation)->anyhow::Result<LightingResult> {
                if let LightingOperation::SuspendSoftware {suspended}=operation {
                    let mut values=self.values.lock().unwrap();
                    values.push(*suspended);
                    ensure!(values.len()>1,"simulated suspend failure");
                }
                Ok(LightingResult::default())
            }
            fn shutdown(&mut self)->anyhow::Result<()> {Ok(())}
        }
        let values=Arc::new(Mutex::new(Vec::new()));
        let mut state=ClientState {connection:Some(Box::new(FailFirstSuspend {values:values.clone()})),software_started:true,..Default::default()};
        assert!(state.set_advanced(true).is_err());
        assert!(state.advanced);
        assert_eq!(state.advanced_applied,None);
        assert!(state.set_advanced(true).unwrap().is_some());
        assert_eq!(state.advanced_applied,Some(true));
        assert!(state.set_advanced(true).unwrap().is_none());
        assert_eq!(*values.lock().unwrap(),[true,true]);
    }
}
