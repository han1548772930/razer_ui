//! Retained dock monitor: discovery, source getters, charging peer engines,
//! hardware notifications and exit restoration share the dock worker lifetime.
use super::{Engine, hid, hid_transport, receiver};
use crate::runtime::lighting_engine_session::{DeviceEngines, SharedEngine};
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::FeatureTransport,
    lighting_engine::{EngineEffect, EngineFrame},
    receiver_capabilities::ReceiverCapability,
    receiver_lighting::{
        ChromaAttributeReceipt, ChromaAttributeRequest, DockMouseSelection, PeerDeviceFactory,
        PeerManifest, QuickEffect,
        battery_monitor::{self, HardwareObservation, MonitorStatus, NoWriteReason},
        peer_identities, peer_manifest,
        peer_protocol::PeerHid,
        peer_support,
    },
    receiver_pairing::HardwareEvents,
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Candidate {
    product_id: u16,
    path: String,
    container: String,
    instance: String,
    bytes: usize,
    relay: bool,
}
impl Candidate {
    fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.product_id,
            self.container.to_ascii_lowercase(),
            self.path.to_ascii_lowercase()
        )
    }
    fn validate(&self) -> anyhow::Result<()> {
        let snapshot = hid::enumerate_metadata()?;
        ensure!(
            snapshot["complete"] == true,
            "Battery peer HID enumeration is incomplete"
        );
        let rows = snapshot["interfaces"]
            .as_array()
            .context("Battery peer interfaces are absent")?;
        let exact = rows
            .iter()
            .filter(|row| {
                row["path"]
                    .as_str()
                    .is_some_and(|path| path.eq_ignore_ascii_case(&self.path))
            })
            .collect::<Vec<_>>();
        ensure!(
            exact.len() == 1
                && exact[0]["device_instance_id"].as_str() == Some(self.instance.as_str())
                && exact[0]["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&self.container)),
            "Battery peer physical identity changed"
        );
        Ok(())
    }
}

struct Peer {
    candidate: Candidate,
    manifest: PeerManifest,
    io: Option<PeerHid>,
    powered: bool,
    charging: bool,
    lighting: DeviceEngines<Engine>,
    normal_effect: Option<EngineEffect>,
}
impl Peer {
    fn transport<T>(
        &mut self,
        dock: &ReceiverCapability,
        validate: &impl Fn() -> anyhow::Result<()>,
        operation: impl FnOnce(
            &mut PeerHid,
            &dyn FeatureTransport,
            &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let candidate = &self.candidate;
        let _lock = receiver::ReceiverLock::acquire(&candidate.container)?;
        validate()?;
        candidate.validate()?;
        let device = hid_transport::open(&candidate.path, candidate.bytes)?;
        let guard = || {
            validate()?;
            candidate.validate()
        };
        let peer_check = || -> anyhow::Result<()> {
            if candidate.relay {
                let peers = razer_device::device_query::read_receiver(
                    &device,
                    dock,
                    receiver::next_transaction(&candidate.path, &candidate.container, dock)?,
                    &guard,
                )?;
                let matches = peers
                    .iter()
                    .filter(|(pid, _)| *pid == candidate.product_id)
                    .collect::<Vec<_>>();
                ensure!(
                    matches.len() == 1 && matches[0].1 == 1,
                    "Dock peer disconnected or became ambiguous"
                );
            }
            Ok(())
        };
        peer_check()?;
        let result = operation(
            self.io
                .as_mut()
                .context("BLE monitor peer has no Feature sender")?,
            &device,
            &guard,
        )?;
        peer_check()?;
        guard()?;
        Ok(result)
    }
    fn normal_effect(&self) -> anyhow::Result<EngineEffect> {
        let workspace = razer_storage::workspace::read_workspace(&razer_storage::store_path())?;
        let owners = workspace
            .as_ref()
            .map(|workspace| {
                workspace
                    .devices
                    .iter()
                    .filter(|device| {
                        (device.product_id == u32::from(self.manifest.support.wired_product_id)
                            || device.product_id == u32::from(self.candidate.product_id))
                            && device
                                .device_container_id
                                .eq_ignore_ascii_case(&self.candidate.container)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ensure!(
            owners.len() <= 1,
            "Battery peer has ambiguous persisted profile owners"
        );
        let effect = if let Some(device) = owners.first() {
            let profiles = device
                .profiles
                .iter()
                .filter(|profile| profile.id == device.active_profile)
                .collect::<Vec<_>>();
            ensure!(
                profiles.len() == 1,
                "Battery peer persisted active profile is absent or ambiguous"
            );
            match profiles[0].source_settings.as_ref() {
                Some(settings) => QuickEffect::parse(
                    settings
                        .get("quickEffects")
                        .context("Battery peer source profile has no quick effects")?,
                )?,
                None => self.manifest.support.default_quick_effect(),
            }
        } else {
            self.manifest.support.default_quick_effect()
        };
        effect.engine_effect_for_geometry(&self.manifest.led)
    }
    fn start_charging(
        &mut self,
        effect: &EngineEffect,
        dock: &ReceiverCapability,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        // Source marks the peer charging before querying/applying effects;
        // a failed effect must not hide it from the later removal query.
        self.charging = true;
        let mut acknowledgements = 0;
        if !self.io.as_ref().is_some_and(PeerHid::custom_selected) {
            self.candidate.validate()?;
            validate()?;
            acknowledgements = self.transport(dock, validate, |io, device, guard| {
                io.select_custom(device, &|| guard())
            })?;
        }
        if self.normal_effect.is_none() {
            let normal = self.normal_effect()?;
            self.lighting.apply_normal(&normal)?;
            self.normal_effect = Some(normal);
        }
        self.lighting.apply_special(effect)?;
        Ok(acknowledgements)
    }
    fn stop_charging(&mut self) -> anyhow::Result<()> {
        if !self.lighting.is_special() {
            self.charging = false;
            return Ok(());
        }
        let normal = self.normal_effect()?;
        self.lighting.remove_special(Some(&normal))?;
        self.normal_effect = Some(normal);
        self.charging = false;
        Ok(())
    }
    fn cleanup(
        &mut self,
        dock: &ReceiverCapability,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> Vec<String> {
        let mut errors = self.lighting.cleanup();
        if self.io.as_ref().is_some_and(PeerHid::restoration_required) {
            if let Err(error) = self.transport(dock, validate, |io, device, guard| {
                io.restore(device, &|| guard())
            }) {
                errors.push(format!(
                    "Peer {} exit restoration: {error:#}",
                    self.candidate.product_id
                ));
            }
        }
        errors
    }
}

pub(super) struct Effects {
    pub normal: Option<EngineEffect>,
    pub special: Option<EngineEffect>,
    pub no_write: Option<NoWriteReason>,
    pub acknowledgements: u32,
}
impl Effects {
    fn empty() -> Self {
        Self {
            normal: None,
            special: None,
            no_write: None,
            acknowledgements: 0,
        }
    }
}

pub(super) struct Monitor {
    runtime: SharedEngine<Engine>,
    dock: ReceiverCapability,
    path: String,
    container: String,
    before: Value,
    peers: Vec<Peer>,
    events: super::super::receiver_events::Events,
    pub status: MonitorStatus,
    last_scan: Option<Instant>,
    last_battery: Option<Instant>,
    dirty: bool,
    refresh_power: bool,
    charging_event: Option<(bool, u16)>,
    worker_id: uuid::Uuid,
    attribute_generation: u64,
    selection: Option<DockMouseSelection>,
}
impl Monitor {
    pub(super) fn new(
        runtime: SharedEngine<Engine>,
        dock: ReceiverCapability,
        path: &str,
        container: &str,
        before: Value,
    ) -> anyhow::Result<Self> {
        let mut events = super::super::receiver_events::Events::new(dock.product_id, container);
        events.begin()?;
        Ok(Self {
            runtime,
            dock,
            path: path.into(),
            container: container.into(),
            before,
            peers: Vec::new(),
            events,
            status: MonitorStatus::default(),
            last_scan: None,
            last_battery: None,
            dirty: true,
            refresh_power: false,
            charging_event: None,
            worker_id: uuid::Uuid::new_v4(),
            attribute_generation: 0,
            selection: None,
        })
    }
    fn candidates(&self) -> anyhow::Result<Vec<Candidate>> {
        let snapshot = hid::enumerate_metadata()?;
        ensure!(
            snapshot["complete"] == true,
            "Battery monitoring discovery is incomplete"
        );
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();
        for row in snapshot["interfaces"]
            .as_array()
            .context("Battery discovery interfaces are absent")?
        {
            let Some(pid) = row["product_id"]
                .as_u64()
                .and_then(|pid| u16::try_from(pid).ok())
            else {
                continue;
            };
            if !peer_identities()
                .iter()
                .any(|(candidate, _)| *candidate == pid)
            {
                continue;
            }
            let support = peer_support(pid, 0)?;
            if row["vendor_id"] != support.vendor_id
                || row["claim_interface"] != support.claim_interface
            {
                continue;
            }
            let candidate = Candidate {
                product_id: pid,
                path: row["path"]
                    .as_str()
                    .context("Battery interface path is absent")?
                    .into(),
                container: row["device_container_id"]
                    .as_str()
                    .context("Battery interface container is absent")?
                    .into(),
                instance: row["device_instance_id"]
                    .as_str()
                    .context("Battery interface instance is absent")?
                    .into(),
                bytes: row["feature_report_bytes"]
                    .as_u64()
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    .context("Battery Feature length is absent")?,
                relay: false,
            };
            ensure!(
                seen.insert(candidate.key()),
                "Battery peer collection identity is ambiguous"
            );
            candidates.push(candidate);
        }
        let observed = receiver::query(&self.path, &self.container, None)?;
        for (pid, state) in razer_device::receiver_protocol::receiver_peers(&observed)? {
            let pid = u16::try_from(pid)?;
            if state != 1
                || !peer_identities()
                    .iter()
                    .any(|(candidate, _)| *candidate == pid)
            {
                continue;
            }
            let candidate = Candidate {
                product_id: pid,
                path: self.path.clone(),
                container: self.container.clone(),
                instance: self.before["device_instance_id"]
                    .as_str()
                    .context("Dock physical instance is absent")?
                    .into(),
                bytes: self.before["feature_report_bytes"]
                    .as_u64()
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    .context("Dock Feature length is absent")?,
                relay: true,
            };
            ensure!(
                seen.insert(candidate.key()),
                "Dock peer collection identity is ambiguous"
            );
            candidates.push(candidate);
        }
        Ok(candidates)
    }
    fn observe(&mut self, validate: &impl Fn() -> anyhow::Result<()>) -> anyhow::Result<bool> {
        let mut candidates = self.candidates()?;
        candidates.retain(|candidate| {
            peer_support(candidate.product_id, 0)
                .is_ok_and(|support| battery_monitor::selected(self.selection.as_ref(), &support))
        });
        let changed = self.peers.len() != candidates.len()
            || candidates
                .iter()
                .any(|candidate| !self.peers.iter().any(|peer| peer.candidate == *candidate));
        let mut retained = Vec::new();
        for mut peer in self.peers.drain(..) {
            if self.refresh_power
                || !candidates
                    .iter()
                    .any(|candidate| *candidate == peer.candidate)
            {
                self.status
                    .cleanup_errors
                    .extend(peer.cleanup(&self.dock, validate));
            } else {
                retained.push(peer);
            }
        }
        self.peers = retained;
        for candidate in candidates {
            if self
                .peers
                .iter()
                .any(|peer| peer.candidate.key() == candidate.key())
            {
                continue;
            }
            let mut manifest = peer_manifest(candidate.product_id, 0, 0)?;
            if candidate.relay {
                manifest.support.claim_interface = self.dock.claim_interface;
            }
            let io = (manifest.support.rz_device_name != PeerDeviceFactory::RzDevice30)
                .then(|| PeerHid::new(manifest.support))
                .transpose()?;
            let mut peer = Peer {
                candidate,
                lighting: DeviceEngines::new(self.runtime.clone(), manifest.led.clone()),
                manifest,
                io,
                powered: false,
                charging: false,
                normal_effect: None,
            };
            if peer.io.is_some()
                && peer
                    .candidate
                    .container
                    .eq_ignore_ascii_case(&self.container)
            {
                let query = peer.transport(&self.dock, validate, |io, device, guard| {
                    io.edition(device, &|| guard())
                });
                match query {
                    Ok(edition) => {
                        let mut manifest = peer_manifest(
                            peer.candidate.product_id,
                            edition.edition,
                            edition.layout,
                        )?;
                        if peer.candidate.relay {
                            manifest.support.claim_interface = self.dock.claim_interface;
                        }
                        peer.io = Some(PeerHid::new(manifest.support)?);
                        peer.lighting =
                            DeviceEngines::new(self.runtime.clone(), manifest.led.clone());
                        peer.manifest = manifest;
                        peer.powered = true;
                        if peer.manifest.support.support_docking_v2 {
                            match peer.transport(&self.dock, validate, |io, device, guard| {
                                io.on_dock(device, &|| guard())
                            }) {
                                Ok(charging)
                                    if peer
                                        .candidate
                                        .container
                                        .eq_ignore_ascii_case(&self.container) =>
                                {
                                    peer.charging = charging;
                                    self.status.charging = charging;
                                    if charging {
                                        self.status.attached_product_id =
                                            Some(peer.candidate.product_id);
                                    }
                                }
                                Ok(_) => {}
                                Err(error) => self.status.diagnostics.push(format!(
                                    "Peer {} initial dock-charge query: {error:#}",
                                    peer.candidate.product_id
                                )),
                            }
                        }
                    }
                    Err(error) => self.status.diagnostics.push(format!(
                        "Peer {} power/edition query: {error:#}",
                        peer.candidate.product_id
                    )),
                }
            }
            self.peers.push(peer);
        }
        self.status.monitored_peers = self.peers.len();
        self.status.eligible_peers = self
            .peers
            .iter()
            .filter(|peer| {
                battery_monitor::eligible_v2(
                    &peer.manifest.support,
                    peer.candidate
                        .container
                        .eq_ignore_ascii_case(&self.container),
                    peer.manifest.support.rz_device_name == PeerDeviceFactory::RzDevice30,
                    self.status.charging,
                )
            })
            .count();
        self.status.observations += 1;
        self.refresh_power = false;
        self.last_scan = Some(Instant::now());
        Ok(changed)
    }
    pub(super) fn request(&mut self, selection: Option<DockMouseSelection>) {
        if self.selection != selection {
            self.selection = selection;
            self.last_scan = None;
        }
        self.dirty = true;
    }
    pub(super) fn chroma_attribute_result(
        &mut self,
        worker_id: uuid::Uuid,
        id: u64,
        receipt: ChromaAttributeReceipt,
    ) -> anyhow::Result<()> {
        self.status
            .accept_chroma_attribute(worker_id, id, receipt)?;
        self.dirty = true;
        Ok(())
    }
    pub(super) fn tick(
        &mut self,
        battery_ui: bool,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Effects> {
        if self.status.pending_chroma_attribute.is_some() {
            return Ok(Effects::empty());
        }
        for _ in 0..32 {
            let Some(bytes) = self.events.receive(Duration::ZERO)? else {
                break;
            };
            match battery_monitor::hardware_observation(&bytes)? {
                Some(HardwareObservation::Charging {
                    connected,
                    product_id,
                    ..
                }) => {
                    self.charging_event = Some((connected, product_id));
                    self.last_scan = None;
                    self.dirty = true;
                    break;
                }
                Some(HardwareObservation::Wireless { .. } | HardwareObservation::Power { .. }) => {
                    self.last_scan = None;
                    self.refresh_power = true;
                    self.dirty = true;
                }
                Some(HardwareObservation::Battery { .. }) => {}
                None => {}
            }
        }
        if self
            .last_scan
            .is_none_or(|time| time.elapsed() >= Duration::from_secs(2))
        {
            self.dirty |= self.observe(validate)?;
        }
        if let Some((connected, product_id)) = self.charging_event.take() {
            let selected_peer = self.peers.iter().any(|peer| {
                peer.candidate.product_id == product_id
                    && peer.manifest.support.support_docking_v2
                    && peer
                        .candidate
                        .container
                        .eq_ignore_ascii_case(&self.container)
            });
            let mut attribute_pid = None;
            if connected {
                if selected_peer {
                    for peer in self.peers.iter_mut().filter(|peer| {
                        peer.candidate.product_id == product_id
                            && peer
                                .candidate
                                .container
                                .eq_ignore_ascii_case(&self.container)
                    }) {
                        peer.charging = true;
                    }
                    self.status.charging = true;
                    self.status.attached_product_id = Some(product_id);
                    attribute_pid = Some(product_id);
                }
            } else {
                for peer in self.peers.iter_mut().filter(|peer| peer.charging) {
                    match peer.transport(&self.dock, validate, |io, device, guard| {
                        io.on_dock(device, &|| guard())
                    }) {
                        Ok(false) => {
                            peer.charging = false;
                            self.status.charging = false;
                            self.status.attached_product_id = None;
                            attribute_pid = Some(peer.candidate.product_id);
                            break;
                        }
                        Ok(true) => {}
                        Err(error) => self
                            .status
                            .diagnostics
                            .push(format!("Peer docking-charge removal query: {error:#}")),
                    }
                }
            }
            if let Some(pid) = attribute_pid {
                self.attribute_generation = self
                    .attribute_generation
                    .checked_add(1)
                    .context("Chroma attribute generation overflow")?;
                self.status.pending_chroma_attribute = Some(ChromaAttributeRequest::charging(
                    self.worker_id,
                    self.attribute_generation,
                    pid,
                )?);
            }
        }
        if self.status.pending_chroma_attribute.is_some() {
            return Ok(Effects::empty());
        }
        if !self.dirty
            && self
                .last_battery
                .is_some_and(|time| time.elapsed() < battery_monitor::MONITOR_INTERVAL)
        {
            return Ok(Effects::empty());
        }
        self.dirty = false;
        let mut effects = Effects::empty();
        if !self.status.charging {
            for peer in self.peers.iter_mut() {
                if let Err(error) = peer.stop_charging() {
                    self.status
                        .diagnostics
                        .push(format!("Peer charging restoration: {error:#}"));
                }
            }
        }
        if !battery_ui && !self.status.charging {
            return Ok(effects);
        }
        if self.peers.is_empty() {
            if battery_ui {
                effects.normal = Some(battery_monitor::no_device_effect());
            }
            self.last_battery = Some(Instant::now());
            return Ok(effects);
        }
        let mut powered = 0;
        let mut read = 0;
        for peer in self.peers.iter_mut() {
            if !battery_monitor::eligible_v2(
                &peer.manifest.support,
                peer.candidate
                    .container
                    .eq_ignore_ascii_case(&self.container),
                peer.manifest.support.rz_device_name == PeerDeviceFactory::RzDevice30,
                self.status.charging,
            ) {
                continue;
            }
            if !peer.powered {
                continue;
            }
            powered += 1;
            match peer.transport(&self.dock, validate, |io, device, guard| {
                io.battery(device, &|| guard())
            }) {
                Ok(level) => {
                    read += 1;
                    self.status.battery_reads += 1;
                    if self.status.charging {
                        let effect = battery_monitor::effect(level, true)?;
                        match peer.start_charging(&effect, &self.dock, validate) {
                            Ok(acknowledgements) => {
                                effects.acknowledgements += acknowledgements;
                                effects.special = Some(effect);
                            }
                            Err(error) => self.status.diagnostics.push(format!(
                                "Peer {} charging effect: {error:#}",
                                peer.candidate.product_id
                            )),
                        }
                    } else if battery_ui {
                        effects.normal = Some(battery_monitor::effect(level, false)?);
                    }
                }
                Err(error) => self.status.diagnostics.push(format!(
                    "Peer {} battery observation: {error:#}",
                    peer.candidate.product_id
                )),
            }
        }
        if battery_ui && effects.normal.is_none() && effects.special.is_none() {
            effects.no_write = Some(if self.status.eligible_peers == 0 {
                NoWriteReason::NoEligiblePeer
            } else if powered == 0 {
                NoWriteReason::NoPoweredEligiblePeer
            } else {
                NoWriteReason::NoBatteryObservation
            });
        }
        if read > 0 || self.last_battery.is_none() {
            self.last_battery = Some(Instant::now());
        }
        Ok(effects)
    }
    pub(super) fn send_frame(
        &mut self,
        frame: &EngineFrame,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<u32>> {
        let Some(peer) = self
            .peers
            .iter_mut()
            .find(|peer| peer.lighting.device_handle == Some(frame.device_handle))
        else {
            return Ok(None);
        };
        let (rows, columns) = if frame.region_id == 0 {
            (
                peer.manifest.led.device_max_row,
                peer.manifest.led.device_max_col,
            )
        } else {
            let regions = peer
                .manifest
                .led
                .led_regions
                .iter()
                .filter(|region| region.region_id == frame.region_id)
                .collect::<Vec<_>>();
            ensure!(
                regions.len() == 1,
                "Peer native frame region is absent or ambiguous in its manifest"
            );
            (regions[0].max_row, regions[0].max_col)
        };
        // Streaming rows follow source checkIN=false. Identity and the current
        // physical collection are re-observed without inventing a frame ACK.
        validate()?;
        peer.candidate.validate()?;
        let _lock = receiver::ReceiverLock::acquire(&peer.candidate.container)?;
        let device = hid_transport::open(&peer.candidate.path, peer.candidate.bytes)?;
        let submitted = peer
            .io
            .as_mut()
            .context("Peer frame has no Feature sender")?
            .send_frame(&device, frame, frame.device_handle, rows, columns, validate)?;
        Ok(Some(submitted))
    }
    pub(super) fn cleanup(&mut self, validate: &impl Fn() -> anyhow::Result<()>) -> Vec<String> {
        let mut errors = self.status.cleanup_errors.clone();
        if let Err(error) = self.events.finish() {
            errors.push(format!("Dock event release: {error:#}"));
        }
        for peer in self.peers.iter_mut() {
            errors.extend(peer.cleanup(&self.dock, validate));
        }
        self.peers.clear();
        errors
    }
}
