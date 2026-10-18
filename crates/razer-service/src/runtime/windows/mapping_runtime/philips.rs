//! PID 70 calibration owns real Feature commands, HID observations and shared
//! mapping/redirect leases until the source restoration sequence completes.
mod stream;
use super::capture::Target;
use anyhow::{Context, Result, ensure};
use razer_device::{
    device_reads::DeviceReadTarget, mapping_native_session::hooks::RedirectKind,
    mouse_philips::measurement::*,
};
use razer_ipc::ServiceRequest;
use serde::Deserialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) trait Host {
    fn acquire(&mut self, scope: Uuid, target: &Target) -> Result<()>;
    fn remove(&mut self, scope: Uuid, target: &Target) -> Result<()>;
    fn redirect(
        &mut self,
        scope: Uuid,
        target: &Target,
        kind: RedirectKind,
        enabled: bool,
    ) -> Result<()>;
    fn activity(&mut self, scope: Uuid, target: &Target) -> Result<(usize, usize)>;
    fn mapping(&mut self, scope: Uuid, enabled: bool) -> Result<()>;
    fn escape(&mut self, scope: Uuid, owner: &str, enabled: bool) -> Result<()>;
    fn take_escape(&mut self, scope: Uuid, owner: &str) -> Result<bool>;
}
pub(super) enum DriverEvent {
    LeftDown,
    Redirect,
    MouseMove,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Physical {
    path: String,
    device_instance_id: String,
    device_container_id: String,
    vendor_id: u32,
    product_id: u32,
}
impl Physical {
    fn current(target: &DeviceReadTarget) -> Result<Self> {
        ensure!(
            target.product_id == 70
                && target.physical_product_id == 70
                && target.peer_product_id.is_none(),
            "Philips measurement requires current direct PID70"
        );
        let (_, physical) = super::super::native_target(&target.device_container_id, Some(70))?;
        Ok(serde_json::from_value(physical)?)
    }
}
struct Session {
    scope: Uuid,
    id: String,
    target: DeviceReadTarget,
    physical: Physical,
    registration: Target,
    core: Machine,
    commands: Vec<CommandReport>,
    initialization_error: Option<String>,
    driver_ready: bool,
    redirect_callbacks: u64,
    mouse_move_callbacks: u64,
    connection: Option<super::super::device_reads::MeasurementConnection>,
    stream: Option<stream::Owner>,
    added: bool,
    redirects: Vec<RedirectKind>,
    mappings_suspended: bool,
    escape: bool,
    began: Instant,
    last_validation: Instant,
}
impl Session {
    fn released(&self) -> bool {
        self.core.cleanup_confirmed()
            && !self.added
            && self.redirects.is_empty()
            && !self.mappings_suspended
            && !self.escape
            && self.connection.is_none()
            && self.stream.is_none()
    }
    fn initialize(&mut self, host: &mut impl Host) -> Result<()> {
        self.added = true;
        host.acquire(self.scope, &self.registration)?;
        self.driver_ready = true;
        self.stream = Some(stream::Owner::start(
            self.physical.product_id.try_into()?,
            self.physical.device_container_id.clone(),
            self.began,
        )?);
        let (connection, initialization_error) =
            super::super::device_reads::MeasurementConnection::open(&self.target)?;
        self.connection = Some(connection);
        self.initialization_error = initialization_error;
        Ok(())
    }
    fn redirect(&mut self, enabled: bool, host: &mut impl Host) -> Result<()> {
        if enabled {
            let (macros, keys) = host.activity(self.scope, &self.registration)?;
            ensure!(
                macros == 0 && keys == 0,
                "Calibration redirect has active macros or pressed keys"
            );
            self.mappings_suspended = true;
            host.mapping(self.scope, false)?;
            // 6CFD0 enables mouse then keyboard; movement has its own lease.
            for kind in [
                RedirectKind::Mouse,
                RedirectKind::Keyboard,
                RedirectKind::MouseMove,
            ] {
                if !self.redirects.contains(&kind) {
                    self.redirects.push(kind);
                }
                host.redirect(self.scope, &self.registration, kind, true)?;
            }
            return Ok(());
        }
        let mut errors = Vec::new();
        if self.mappings_suspended {
            match host.mapping(self.scope, true) {
                Ok(()) => self.mappings_suspended = false,
                Err(error) => errors.push(format!("Restore mapping: {error:#}")),
            }
        }
        let pending = self.redirects.clone();
        for kind in pending {
            match host.redirect(self.scope, &self.registration, kind, false) {
                Ok(()) => self.redirects.retain(|value| *value != kind),
                Err(error) => errors.push(format!("Restore {kind:?}: {error:#}")),
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn release(&mut self, host: &mut impl Host) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(error) = self.redirect(false, host) {
            errors.push(format!("{error:#}"));
        }
        if self.escape {
            match host.escape(self.scope, &self.id, false) {
                Ok(()) => self.escape = false,
                Err(error) => errors.push(format!("Release Escape: {error:#}")),
            }
        }
        if let Some(stream) = &mut self.stream {
            match stream.stop() {
                Ok(()) => self.stream = None,
                Err(error) => errors.push(format!("Stop hardware input: {error:#}")),
            }
        }
        if self.added {
            match host.remove(self.scope, &self.registration) {
                Ok(()) => {
                    self.added = false;
                    self.driver_ready = false;
                    self.redirects.clear();
                }
                Err(error) => errors.push(format!("Release driver: {error:#}")),
            }
        }
        if !self.added && self.stream.is_none() {
            self.connection = None;
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn advance(&mut self, host: &mut impl Host) -> Result<()> {
        while let Some(effect) = self.core.next_effect() {
            let result = match effect {
                Effect::Device(command) => {
                    let mut sent = false;
                    let result = self
                        .connection
                        .as_ref()
                        .context("Philips Feature connection is not open")
                        .and_then(|connection| connection.exchange(command, &mut || sent = true));
                    self.commands.push(CommandReport {
                        command,
                        acknowledged: result.as_ref().ok().cloned(),
                        command_may_have_reached_device: sent,
                        error: result.as_ref().err().map(|e| format!("{e:#}")),
                    });
                    result.map(Some)
                }
                Effect::Escape(enabled) => {
                    if enabled {
                        self.escape = true;
                    }
                    let result = host.escape(self.scope, &self.id, enabled);
                    if result.is_ok() {
                        self.escape = enabled;
                    }
                    result.map(|_| None)
                }
                Effect::Redirect(enabled) => self.redirect(enabled, host).map(|_| None),
                Effect::Release => self.release(host).map(|_| None),
            };
            self.core
                .complete(effect, result, self.began.elapsed().as_millis() as u64)?;
        }
        Ok(())
    }
    fn progress(&self) -> Result<Value> {
        let mut progress = self
            .core
            .progress(&self.id, &self.target, self.commands.clone());
        progress.initialization_error = self.initialization_error.clone();
        progress.driver_ready = self.driver_ready;
        progress.redirect_callbacks = self.redirect_callbacks;
        progress.mouse_move_callbacks = self.mouse_move_callbacks;
        Ok(serde_json::to_value(progress)?)
    }
    fn poll(&mut self, host: &mut impl Host) -> Result<()> {
        if self.core.phase() == Phase::Finished {
            return Ok(());
        }
        let observed = (|| -> Result<()> {
            if self.last_validation.elapsed() >= Duration::from_millis(250) {
                ensure!(
                    Physical::current(&self.target)? == self.physical,
                    "Philips physical USB identity changed"
                );
                self.connection
                    .as_ref()
                    .context("Philips connection closed during measurement")?
                    .validate()?;
                self.last_validation = Instant::now();
            }
            if host.take_escape(self.scope, &self.id)? {
                self.core.cancel();
            }
            if let Some(stream) = &self.stream {
                for (input, time) in stream.events()? {
                    self.core.observe(input, time);
                }
            }
            self.core.tick(self.began.elapsed().as_millis() as u64);
            Ok(())
        })();
        if let Err(error) = observed {
            self.core.abort(format!("{error:#}"));
        }
        self.advance(host)
    }
}

#[derive(Default)]
pub(super) struct State {
    session: Option<Session>,
}
impl State {
    pub fn handle(
        &mut self,
        scope: Uuid,
        request: &ServiceRequest,
        host: &mut impl Host,
    ) -> Option<Result<Value>> {
        Some(match request {
            ServiceRequest::MousePhilipsMeasurementStart {
                operation_id,
                target,
                kind,
            } => self.start(scope, operation_id.clone(), target.clone(), *kind, host),
            ServiceRequest::MousePhilipsMeasurementPoll { operation_id } => {
                self.current(scope, operation_id).and_then(|row| {
                    row.poll(host)?;
                    row.progress()
                })
            }
            ServiceRequest::MousePhilipsMeasurementCancel { operation_id } => {
                self.current(scope, operation_id).and_then(|row| {
                    row.core.cancel();
                    row.core.retry_cleanup();
                    row.advance(host)?;
                    row.progress()
                })
            }
            _ => return None,
        })
    }
    fn current(&mut self, scope: Uuid, id: &str) -> Result<&mut Session> {
        let row = self
            .session
            .as_mut()
            .context("Philips measurement has no owner")?;
        ensure!(
            row.scope == scope && row.id == id,
            "Philips measurement owner changed"
        );
        Ok(row)
    }
    fn start(
        &mut self,
        scope: Uuid,
        id: String,
        target: DeviceReadTarget,
        kind: Kind,
        host: &mut impl Host,
    ) -> Result<Value> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Invalid Philips owner"
        );
        ensure!(
            self.session.as_ref().is_none_or(Session::released),
            "Previous Philips calibration cleanup is not confirmed"
        );
        let physical = Physical::current(&target)?;
        let registration = Target {
            vendor_id: physical.vendor_id,
            product_id: physical.product_id,
            container_id: physical.device_container_id.clone(),
            usb_instance_id: Some(physical.device_instance_id.clone()),
            guid: Uuid::new_v4(),
            keyboard_only: false,
        };
        self.session = Some(Session {
            scope,
            id,
            target,
            physical,
            registration,
            core: Machine::new(kind),
            commands: Vec::new(),
            initialization_error: None,
            driver_ready: false,
            redirect_callbacks: 0,
            mouse_move_callbacks: 0,
            connection: None,
            stream: None,
            added: false,
            redirects: Vec::new(),
            mappings_suspended: false,
            escape: false,
            began: Instant::now(),
            last_validation: Instant::now(),
        });
        let row = self.session.as_mut().unwrap();
        if let Err(error) = row.initialize(host) {
            row.core.abort(format!("Register Philips input: {error:#}"));
        }
        row.advance(host)?;
        row.progress()
    }
    pub fn input(&mut self, scope: Uuid, guid: Uuid, event: DriverEvent) {
        let Some(row) = &mut self.session else {
            return;
        };
        if row.scope != scope
            || row.registration.guid != guid
            || row.core.phase() == Phase::Finished
        {
            return;
        }
        match event {
            DriverEvent::LeftDown => row
                .core
                .observe(Input::LeftDown, row.began.elapsed().as_millis() as u64),
            DriverEvent::Redirect => row.redirect_callbacks += 1,
            DriverEvent::MouseMove => row.mouse_move_callbacks += 1,
        }
    }
    pub fn input_failed(&mut self, scope: Uuid, guid: Uuid, error: String) {
        if let Some(row) = &mut self.session {
            if row.scope == scope && row.registration.guid == guid {
                row.core.abort(error);
            }
        }
    }
    pub fn poll(&mut self, host: &mut impl Host) -> Result<()> {
        if let Some(row) = &mut self.session {
            row.poll(host)?;
        }
        Ok(())
    }
    pub fn release(&mut self, scope: Uuid, host: &mut impl Host) -> Result<()> {
        if let Some(row) = &mut self.session {
            if row.scope == scope {
                row.core.cancel();
                row.core.retry_cleanup();
                row.advance(host)?;
                ensure!(row.released(), "Philips restoration is unconfirmed");
                self.session = None;
            }
        }
        Ok(())
    }
}
