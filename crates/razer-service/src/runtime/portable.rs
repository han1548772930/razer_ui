//! Portable worker routes: native nodes and source-proved Razer queries.
//! Platform-specific audio/driver calls delegate to isolated OS adapters.
//! No guessed peer routing, downloaded DLL or direct OS API in this layer.
use super::{KeyboardRead, ServiceRequest};
use anyhow::{Context as _, bail, ensure};
use razer_device::audio_mixer::{MatrixRoute, MixerRoute, MixerSession, MixerTarget, MixerValue};
use razer_device::backend::HidNode;
use razer_device::device_identity;
use razer_device::device_identity::IdentityLookup;
use razer_device::device_query;
use razer_device::device_reads;
use razer_device::receiver_capabilities;
use razer_hid::with_backend;

enum HanboExchange<'a> {
    Command(&'a razer_device::hanbo_cooler::CoolerCommand),
    Telemetry,
    Startup(razer_device::hanbo_cooler::StartupQuery),
}
#[path = "hyperpolling_receiver.rs"]
mod hyperpolling_receiver;
#[path = "keyboard_indicator.rs"]
mod keyboard_indicator;
#[path = "keyboard_latest_restore.rs"]
mod keyboard_latest_restore;
#[path = "keyboard_legacy_calibration.rs"]
mod keyboard_legacy_calibration;
#[path = "leviathan.rs"]
mod leviathan;
#[path = "mouse_brightness.rs"]
mod mouse_brightness;
#[path = "mouse_current_dpi.rs"]
mod mouse_current_dpi;
#[path = "mouse_current_polling.rs"]
mod mouse_current_polling;
#[path = "mouse_current_power.rs"]
mod mouse_current_power;
#[path = "mouse_dynamic.rs"]
mod mouse_dynamic;
#[path = "mouse_indicator_led.rs"]
mod mouse_indicator_led;
#[path = "mouse_philips.rs"]
mod mouse_philips;
#[path = "mouse_receiver_legacy.rs"]
mod mouse_receiver_legacy;
#[path = "mouse_rotation.rs"]
mod mouse_rotation;
#[path = "mouse_scroll.rs"]
mod mouse_scroll;
#[path = "mouse_tracking.rs"]
mod mouse_tracking;
#[path = "seiren_eq.rs"]
mod seiren_eq;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct PortableRuntime {
    cleanup: PortableCleanup,
    latest_keyboard_events: keyboard_latest_restore::EventsState,
    transactions: HashMap<String, u8>,
    audio_notifications: crate::audio_notification::AudioNotifications,
    audio_router: crate::audio_router::AudioRouter,
    foreground_monitor: razer_platform::foreground_monitor::ForegroundMonitor,
    controller_calibration: super::controller_calibration::Controller,
    controller_lighting: super::controller_lighting::Controller,
    marci_lighting: super::marci_lighting::Controller,
    keyboard_submission: super::keyboard_submission_controller::Controller,
    keyboard_wristrest: super::keyboard_wristrest_controller::Controller,
    mouse_runtime_artwork: super::mouse_runtime_artwork_controller::Controller,
    mouse_indicator_led: super::mouse_indicator_led_controller::Controller,
    mouse_tracking: super::mouse_tracking_controller::Controller,
    mouse_philips: super::mouse_philips_controller::Controller,
    leviathan_events: super::leviathan_events::Controller,
    seiren_audio_capture: super::seiren_audio_capture_controller::Controller,
    camera: super::camera_controller::Controller,
    thx: super::thx_controller::Controller,
}

#[derive(Default)]
struct PortableCleanup {
    joined: super::shutdown::OwnerCleanup,
    keyboard_events: super::shutdown::OwnerCleanup,
    audio_notifications: super::shutdown::OwnerCleanup,
    foreground_monitor: super::shutdown::OwnerCleanup,
}

impl Drop for PortableRuntime {
    fn drop(&mut self) {
        // EOF, malformed IPC and fatal native errors still cancel and join
        // owned tasks; the client retains these diagnostics on worker failure.
        self.shutdown().log_failures();
    }
}

impl PortableRuntime {
    #[cfg(test)]
    pub(in crate::runtime) fn simulated_pending_events(
        &mut self,
        events: Box<dyn razer_device::receiver_pairing::HardwareEvents>,
    ) {
        self.latest_keyboard_events = keyboard_latest_restore::simulated_events_state(
            events,
            json!({"observed_owner": "simulated"}),
        );
    }

    pub(in crate::runtime) fn shutdown(&mut self) -> super::shutdown::Cleanup {
        let mut cleanup = self.cleanup.joined.run(|| {
            let mut cleanup = super::shutdown::Cleanup::default();
            cleanup.record_submission("mouse_tracking_cleanup", self.mouse_tracking.shutdown());
            cleanup.record_submission("mouse_philips_cleanup", self.mouse_philips.shutdown());
            self.camera.shutdown(&mut cleanup);
            self.thx.shutdown(&mut cleanup);
            self.controller_calibration.shutdown(&mut cleanup);
            self.controller_lighting.shutdown(&mut cleanup);
            self.marci_lighting.shutdown(&mut cleanup);
            self.keyboard_submission.shutdown(&mut cleanup);
            self.keyboard_wristrest.shutdown(&mut cleanup);
            self.mouse_runtime_artwork.shutdown(&mut cleanup);
            self.mouse_indicator_led.shutdown(&mut cleanup);
            self.leviathan_events.shutdown(&mut cleanup);
            self.seiren_audio_capture.shutdown(&mut cleanup);
            cleanup.record("audio_router", self.audio_router.shutdown());
            cleanup
        });
        cleanup.merge(self.cleanup.keyboard_events.run(|| {
            let mut cleanup = super::shutdown::Cleanup::default();
            cleanup.record(
                "latest_keyboard_events",
                self.latest_keyboard_events.shutdown(),
            );
            if self.latest_keyboard_events.cleanup_pending() {
                cleanup.retain_resources();
            }
            cleanup
        }));
        cleanup.merge(self.cleanup.audio_notifications.run(|| {
            let mut cleanup = super::shutdown::Cleanup::default();
            cleanup.record("audio_notifications", self.audio_notifications.shutdown());
            if self.audio_notifications.cleanup_pending() {
                cleanup.retain_resources();
            }
            cleanup
        }));
        cleanup.merge(self.cleanup.foreground_monitor.run(|| {
            let mut cleanup = super::shutdown::Cleanup::default();
            cleanup.record("foreground_monitor", self.foreground_monitor.shutdown());
            if self.foreground_monitor.cleanup_pending() {
                cleanup.retain_resources();
            }
            cleanup
        }));
        cleanup
    }

    fn transaction(
        &mut self,
        node: &HidNode,
        class: &str,
        prefix: u8,
        modulus: u8,
    ) -> anyhow::Result<u8> {
        ensure!(modulus > 0, "源事务范围无效");
        let key = format!(
            "{:x}:{}:{}:{}:{}",
            md5::compute(&node.path),
            node.usage_page,
            node.usage,
            class,
            prefix
        );
        let next = self.transactions.entry(key).or_default();
        let result = prefix | (*next % modulus);
        *next = (*next + 1) % modulus;
        Ok(result)
    }

    fn revalidate(node: &HidNode) -> anyhow::Result<()> {
        let current = with_backend(|backend| backend.enumerate())?;
        ensure!(
            current.iter().filter(|value| *value == node).count() == 1,
            "查询期间 HID collection 身份变化或不唯一"
        );
        Ok(())
    }

    fn mouse_receiver_read(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer_product_id: u32,
        kind: device_reads::DeviceReadKind,
    ) -> anyhow::Result<Value> {
        self.mouse_receiver_request(
            node,
            product_id,
            peer_product_id,
            kind,
            |route, device, next, validate| {
                Ok(serde_json::to_value(route.read(device, next, validate)?)?)
            },
        )
    }

    fn mouse_receiver_dpi_stages(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer_product_id: u32,
        draft: Option<&razer_device::mouse_dpi_stages::DpiStagesDraft>,
    ) -> anyhow::Result<Value> {
        if product_id != razer_device::mouse_receiver_polling::PRODUCT_ID {
            return self.receiver_setting(
                node,
                product_id,
                peer_product_id,
                super::receiver_settings::Setting::DpiStages(draft),
            );
        }
        self.mouse_receiver_request(
            node,
            product_id,
            peer_product_id,
            device_reads::DeviceReadKind::Dpi,
            |route, device, next, validate| {
                let mut result = if let Some(draft) = draft {
                    let mut result = serde_json::to_value(
                        route.apply_dpi_stages(device, draft, next, validate)?,
                    )?;
                    result["result"] = result["reading"].take();
                    result
                        .as_object_mut()
                        .expect("serialized receiver observation")
                        .remove("reading");
                    result
                } else {
                    serde_json::to_value(route.read_dpi_stages(device, next, validate)?)?
                };
                result["profile_scope"] = json!("current_active_table");
                result["obm_profiles_written"] = json!(false);
                result["method"] = json!(if draft.is_some() {
                    "setDPIStages"
                } else {
                    "getDPIStages"
                });
                Ok(result)
            },
        )
    }

    fn receiver_status(
        &mut self,
        node: &HidNode,
        source_product_id: Option<u16>,
    ) -> anyhow::Result<Value> {
        let cap = receiver_capabilities::for_source(node.product_id, source_product_id)?;
        ensure!(
            node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface),
            "接收器节点不符合源 VID/接口选择；未知接口号不能猜测"
        );
        let started = Instant::now();
        let validate = || {
            deadline(started)?;
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Receiver Feature collection length differs from the current source"
        );
        validate()?;
        let transaction = self.transaction(
            node,
            &cap.source_class,
            cap.transaction_prefix,
            cap.transaction_modulus,
        )?;
        let devices = device_query::read_receiver(device.as_ref(), &cap, transaction, validate)?;
        validate()?;
        let rows = devices
            .into_iter()
            .map(|(product_id, status)| json!({"product_id":product_id,"status":status}))
            .collect::<Vec<_>>();
        Ok(
            json!({"node":node,"query":"receiver_wireless_status_v2","devices":rows,"device_count":rows.len(),
            "transaction_id":transaction,"source_product_id":cap.source_product_id,"source_class":cap.source_class,
            "transport_metadata":device.metadata(),
            "elapsed_ms":started.elapsed().as_millis() as u64,"identity_scope":"hid_collection"}),
        )
    }

    fn receiver_setting(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer_product_id: u32,
        setting: super::receiver_settings::Setting<'_>,
    ) -> anyhow::Result<Value> {
        let session = setting.session(product_id, u32::from(node.product_id), peer_product_id)?;
        let cap = session.transport();
        ensure!(
            node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Receiver collection differs from the current source factory"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Receiver setting deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Receiver Feature collection length differs from source"
        );
        let mut response = setting.execute(
            &session,
            device.as_ref(),
            &mut || {
                self.transaction(
                    node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )
            },
            &validate,
        )?;
        response["node"] = serde_json::to_value(node)?;
        response["product_id"] = json!(product_id);
        response["peer_product_id"] = json!(peer_product_id);
        response["transport_metadata"] = device.metadata();
        response["identity_scope"] = json!("hid_collection_and_live_receiver_peer");
        response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        Ok(response)
    }

    fn mouse_receiver_request(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer_product_id: u32,
        kind: device_reads::DeviceReadKind,
        operation: impl FnOnce(
            &razer_device::mouse_receiver_polling::ReceiverRead,
            &dyn razer_device::backend::FeatureTransport,
            &mut dyn FnMut() -> anyhow::Result<u8>,
            &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<Value>,
    ) -> anyhow::Result<Value> {
        use razer_device::mouse_receiver_polling::ReceiverRead;
        let route = ReceiverRead::new(
            product_id,
            u32::from(node.product_id),
            Some(peer_product_id),
            kind,
        )?;
        let cap = route.transport();
        ensure!(
            node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Mouse receiver collection differs from the source factory"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Mouse receiver observation deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        let lengths = device.report_lengths()?;
        ensure!(
            lengths.feature.values().max().copied() == Some(cap.report_bytes),
            "Mouse receiver Feature descriptor differs from source"
        );
        let mut response = operation(
            &route,
            device.as_ref(),
            &mut || {
                self.transaction(
                    node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )
            },
            &validate,
        )?;
        response["node"] = serde_json::to_value(node)?;
        response["product_id"] = json!(product_id);
        response["peer_product_id"] = json!(peer_product_id);
        if response.get("method").is_none() {
            response["method"] = json!(route.method());
        }
        response["source_class"] = json!(cap.source_class);
        response["transport_metadata"] = device.metadata();
        response["identity_scope"] = json!("hid_collection_and_live_receiver_peer");
        response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        Ok(response)
    }

    fn hanbo_cooler(
        &self,
        node: &HidNode,
        product_id: u32,
        exchange: HanboExchange<'_>,
    ) -> anyhow::Result<Value> {
        use razer_device::hanbo_cooler;

        hanbo_cooler::validate_node(node, product_id)?;
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Hanbo cooler command deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        let lengths = device.report_lengths()?;
        ensure!(
            !lengths.output.is_empty() && !lengths.input.is_empty(),
            "Hanbo collection does not expose the source Output/Interrupt channels"
        );
        let command = match exchange {
            HanboExchange::Command(command) => command,
            HanboExchange::Telemetry => {
                let telemetry = hanbo_cooler::read_telemetry(device.as_ref(), validate)?;
                validate()?;
                let observation = razer_ipc::HanboCoolerObservation {
                    node: node.clone(),
                    product_id,
                    telemetry,
                };
                observation.validate_for(node)?;
                return serde_json::to_value(observation).map_err(Into::into);
            }
            HanboExchange::Startup(query) => {
                let reading = hanbo_cooler::read_startup(device.as_ref(), query, validate)?;
                validate()?;
                let observation = razer_ipc::HanboStartupObservation {
                    node: node.clone(),
                    product_id,
                    reading,
                };
                observation.validate_for(node, query)?;
                return serde_json::to_value(observation).map_err(Into::into);
            }
        };
        hanbo_cooler::exchange(device.as_ref(), command, validate)?;
        validate()?;
        Ok(json!({
            "node": node,
            "product_id": product_id,
            "command": command,
            "acknowledged": true,
            "transport_metadata": device.metadata(),
            "report_lengths": lengths,
            "identity_scope": "hid_collection",
            "elapsed_ms": started.elapsed().as_millis() as u64,
        }))
    }

    pub(super) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        match request {
            ServiceRequest::MappingRotationStart { .. }
            | ServiceRequest::MappingRotationRequest { .. }
            | ServiceRequest::MappingRotationPoll { .. }
            | ServiceRequest::MappingRotationStop { .. } => {
                bail!("Native mouse rotation requires the Windows mapping filter driver")
            }
            ServiceRequest::HyperpollingReceiver { target, command, transaction } => self.hyperpolling_receiver(&target, command, transaction),
            ServiceRequest::KeyboardIndicator { target, change } => {
                self.keyboard_indicator(&target, &change)
            }
            ServiceRequest::KeyboardLatestRestore { target, protocol_operation } => {
                self.keyboard_latest_restore(&target, &protocol_operation)
            }
            ServiceRequest::KeyboardLegacyCalibration { target, calibration_operation } => {
                self.keyboard_legacy_calibration(&target, &calibration_operation)
            }
            ServiceRequest::MouseReceiverLegacy { target, change } => {
                self.mouse_receiver_legacy(&target, &change)
            }
            ServiceRequest::SeirenAudioCaptureStart { session_id, product_id, options } =>
                serde_json::to_value(self.seiren_audio_capture.start(session_id, product_id, options)?).map_err(Into::into),
            ServiceRequest::SeirenAudioCapturePoll { session_id } =>
                serde_json::to_value(self.seiren_audio_capture.poll(session_id)?).map_err(Into::into),
            ServiceRequest::SeirenAudioCaptureStop { session_id } =>
                serde_json::to_value(self.seiren_audio_capture.stop(session_id)?).map_err(Into::into),
            ServiceRequest::SeirenAudioPlaybackStart { session_id, product_id, clip } =>
                serde_json::to_value(self.seiren_audio_capture.playback_start(session_id, product_id, clip)?).map_err(Into::into),
            ServiceRequest::SeirenAudioPlaybackPoll { session_id } =>
                serde_json::to_value(self.seiren_audio_capture.playback_poll(session_id)?).map_err(Into::into),
            ServiceRequest::SeirenAudioPlaybackStop { session_id } =>
                serde_json::to_value(self.seiren_audio_capture.playback_stop(session_id)?).map_err(Into::into),
            ServiceRequest::MouseRuntimeArtworkStart { operation_id, target } => {
                self.mouse_runtime_artwork.start(operation_id.clone(), target.clone())?;
                Ok(json!({"operation_id":operation_id,"target":target,"started":true}))
            }
            ServiceRequest::MouseRuntimeArtworkPoll { operation_id } => self.mouse_runtime_artwork.poll(&operation_id),
            ServiceRequest::MouseRuntimeArtworkStop { operation_id } => self.mouse_runtime_artwork.stop(&operation_id),
            ServiceRequest::MouseIndicatorLedStart { operation_id, target } => {
                self.mouse_indicator_led.start(operation_id.clone(), target.clone())?;
                Ok(json!({"operation_id":operation_id,"target":target,"started":true}))
            }
            ServiceRequest::MouseIndicatorLedPoll { operation_id } => self.mouse_indicator_led.poll(&operation_id),
            ServiceRequest::MouseTrackingStart { operation_id, target, setting } => {
                Ok(serde_json::to_value(self.mouse_tracking.start(operation_id, target, setting)?)?)
            }
            ServiceRequest::MouseTrackingPoll { operation_id } => {
                Ok(serde_json::to_value(self.mouse_tracking.poll(&operation_id)?)?)
            }
            ServiceRequest::MouseTrackingCancel { operation_id } => {
                Ok(serde_json::to_value(self.mouse_tracking.cancel(&operation_id)?)?)
            }
            ServiceRequest::MousePhilipsStart { operation_id, target, setting } => {
                Ok(serde_json::to_value(self.mouse_philips.start(operation_id, target, setting)?)?)
            }
            ServiceRequest::MousePhilipsPoll { operation_id } => {
                Ok(serde_json::to_value(self.mouse_philips.poll(&operation_id)?)?)
            }
            ServiceRequest::MousePhilipsCancel { operation_id } => {
                Ok(serde_json::to_value(self.mouse_philips.cancel(&operation_id)?)?)
            }
            ServiceRequest::MouseIndicatorLedStop { operation_id } => self.mouse_indicator_led.stop(&operation_id),
            ServiceRequest::KeyboardWristrestStart { operation_id, target } => {
                self.keyboard_wristrest.start(operation_id.clone(), target.clone())?;
                Ok(json!({"operation_id":operation_id,"target":target,"started":true}))
            }
            ServiceRequest::KeyboardWristrestPoll { operation_id } => self.keyboard_wristrest.poll(&operation_id),
            ServiceRequest::KeyboardWristrestStop { operation_id } => self.keyboard_wristrest.stop(&operation_id),
            ServiceRequest::KeyboardAnalogStart { operation_id, target, profile_guid, intent } => {
                self.keyboard_submission.start(operation_id.clone(), target, profile_guid, intent)?;
                Ok(json!({"operation_id":operation_id,"started":true}))
            }
            ServiceRequest::KeyboardAnalogPoll { operation_id } => self.keyboard_submission.poll(&operation_id),
            ServiceRequest::KeyboardAnalogCancel { operation_id } => {
                self.keyboard_submission.cancel(&operation_id)?;
                Ok(json!({"operation_id":operation_id,"cancel_requested":true}))
            }
            ServiceRequest::ThxStart { operation_id, scope, plan } => {
                self.thx.start(operation_id.clone(), scope, plan)?; Ok(json!({"operation_id":operation_id,"started":true}))
            }
            ServiceRequest::ThxOwnerStart { owner_id, operation_id, scope, plan } => {
                self.thx.start_owner(owner_id.clone(), operation_id.clone(), scope, plan)?;
                Ok(json!({"owner_id":owner_id,"operation_id":operation_id,"started":true}))
            }
            ServiceRequest::ThxOwnerSubmit { owner_id, operation_id, scope, plan } => {
                self.thx.submit_owner(&owner_id, operation_id.clone(), scope, plan)?;
                Ok(json!({"owner_id":owner_id,"operation_id":operation_id,"started":true}))
            }
            ServiceRequest::ThxOwnerRelease { owner_id } => self.thx.release_owner(&owner_id),
            ServiceRequest::ThxPoll { operation_id } => serde_json::to_value(self.thx.poll(&operation_id)?).map_err(Into::into),
            ServiceRequest::ThxCancel { operation_id } => { self.thx.cancel(&operation_id)?; Ok(json!({"operation_id":operation_id,"cancel_requested":true})) }
            ServiceRequest::CameraDiscover { scope } => serde_json::to_value(crate::camera::discover(&scope)?).map_err(Into::into),
            ServiceRequest::CameraStart { operation_id, scope, plan } => {
                self.camera.start(operation_id,scope,plan)?;Ok(json!({"started":true}))
            }
            ServiceRequest::CameraTaskStart { operation_id, scope, task_name, plan } => {
                self.camera.start_named(operation_id,scope,task_name,plan)?;Ok(json!({"started":true}))
            }
            ServiceRequest::CameraPoll { operation_id } => serde_json::to_value(self.camera.poll(&operation_id)?).map_err(Into::into),
            ServiceRequest::CameraCancel { operation_id } => {self.camera.cancel(&operation_id)?;Ok(json!({"cancel_requested":true}))}
            ServiceRequest::ControllerTriggers { node, product_id, trigger_operation } =>
                super::controller_triggers::execute(&node, product_id, &trigger_operation),
            ServiceRequest::ControllerLighting { node, product_id, lighting_operation } =>
                self.controller_lighting.execute(&node, product_id, &lighting_operation),
            ServiceRequest::MarciLighting { device_container_id, owner_id, lighting_operation } =>
                self.marci_lighting.execute(&device_container_id, &owner_id, lighting_operation),
            ServiceRequest::ControllerSleep { node, product_id, sleep_operation } =>
                super::controller_sleep::execute(&node, product_id, &sleep_operation),
            ServiceRequest::ControllerSocd { node, product_id, socd_operation } =>
                super::controller_socd::execute(&node, product_id, socd_operation),
            ServiceRequest::ControllerInitialization { node, product_id, plan } =>
                super::controller_initialization::execute(&node, product_id, &plan),
            ServiceRequest::ControllerTriggerCalibrationStart { operation_id, node, product_id, part_id } =>
                self.controller_calibration.start(operation_id, node, product_id, part_id),
            ServiceRequest::ControllerTriggerCalibrationPoll { operation_id } =>
                self.controller_calibration.poll(&operation_id),
            ServiceRequest::ControllerTriggerCalibrationCancel { operation_id } =>
                self.controller_calibration.cancel(&operation_id),
            ServiceRequest::AudioRoutingEnable { enable } => self.audio_router.enable(enable),
            ServiceRequest::AudioRouteDevice {
                primary_device,
                routed_device,
                primary_device_id,
            } => self
                .audio_router
                .route(primary_device, routed_device, primary_device_id),
            ServiceRequest::AudioRouterEvents => self.audio_router.drain(),
            ServiceRequest::ForegroundMonitorStart { view_url } => {
                Ok(json!(self.foreground_monitor.start(&view_url)?))
            }
            ServiceRequest::ForegroundMonitorStop { view_url } => {
                Ok(json!(self.foreground_monitor.stop(&view_url)?))
            }
            ServiceRequest::ForegroundMonitorEvents { view_url } => {
                let events = self.foreground_monitor.drain(&view_url)?;
                Ok(json!(
                    events
                        .into_iter()
                        .map(|event| json!({
                            "event": "foregroundWindow",
                            "data": {"name": event.name, "path": event.path}
                        }))
                        .collect::<Vec<_>>()
                ))
            }
            ServiceRequest::AudioNotificationsEnable { enable } => {
                self.audio_notifications.enable(enable)
            }
            ServiceRequest::AudioNotificationsDrain => {
                Ok(serde_json::to_value(self.audio_notifications.drain()?)?)
            }
            ServiceRequest::AudioVolumeRead { device_id } => Ok(serde_json::to_value(
                crate::simple_audio_volume::read(&device_id)?,
            )?),
            ServiceRequest::AudioVolumeWrite {
                device_id,
                mute,
                volume,
            } => Ok(serde_json::to_value(crate::simple_audio_volume::write(
                &device_id, mute, volume,
            )?)?),
            ServiceRequest::AudioDevices => {
                Ok(serde_json::to_value(crate::simple_audio::enumerate()?)?)
            }
            ServiceRequest::AudioEndpoints { flow } => {
                let observed = crate::audio_util::enumerate(flow)?;
                Ok(json!({
                    "source_response": observed.source_response(),
                    "observation": observed,
                    "vendor_dll_loaded": false,
                    "transport": "windows_core_audio",
                }))
            }
            ServiceRequest::HidNodeMixerRead {
                node,
                product_id,
                target,
            } => mixer_request(node, product_id, target, None),
            ServiceRequest::HidNodeMixerLocalWrite { node, product_id, command } => {
                mixer_local_request(node, product_id, command)
            }
            ServiceRequest::HidNodeMixerWrite {
                node,
                product_id,
                target,
                value,
            } => mixer_request(node, product_id, target, Some(value)),
            ServiceRequest::HidNodeMixerEqWrite {
                node,
                product_id,
                bands,
            } => mixer_eq_request(node, product_id, bands),
            ServiceRequest::HidNodeMixerRouteRead {
                node,
                product_id,
                route,
            } => mixer_route_request(node, product_id, route, None),
            ServiceRequest::HidNodeMixerRouteWrite {
                node,
                product_id,
                route,
                enabled,
            } => mixer_route_request(node, product_id, route, Some(enabled)),
            ServiceRequest::HidNodeMixerRestartStreams { node, product_id } => {
                #[cfg(windows)]
                {
                    super::native::mixer_restart_streams(node, product_id)
                }
                #[cfg(not(windows))]
                {
                    let _ = (node, product_id);
                    bail!("当前 Mixer 流重置使用 Windows 专属驱动，此平台未实现该能力")
                }
            }
            ServiceRequest::HidNodes => {
                let nodes = with_backend(|backend| backend.enumerate())?
                    .into_iter()
                    .filter(|node| matches!(node.vendor_id, 0x1532 | 0x068e))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"nodes":nodes, "transport":"hidapi", "platform":std::env::consts::OS,
                    "identity_scope":"hid_collection", "enumeration_completeness":"not_reported_by_backend"}),
                )
            }
            ServiceRequest::HidNodeReports { node } => {
                Self::revalidate(&node)?;
                let validate = || Self::revalidate(&node);
                let device = with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate)))?;
                let reports = device.report_lengths()?;
                Self::revalidate(&node)?;
                Ok(json!({"node":node,"reports":reports,
                    "transport_metadata":device.metadata(),"identity_scope":"hid_collection"}))
            }
            ServiceRequest::HidNodeMouseReceiverRead {
                node,
                product_id,
                peer_product_id,
                kind,
            } => self.mouse_receiver_read(&node, product_id, peer_product_id, kind),
            ServiceRequest::HidNodeMouseReceiverDpiStages {
                node, product_id, peer_product_id, draft,
            } => self.mouse_receiver_dpi_stages(&node, product_id, peer_product_id, draft.as_ref()),
            ServiceRequest::HidNodeMouseReceiverPolling { node, product_id, peer_product_id, hz } => {
                self.mouse_receiver_request(&node,product_id,peer_product_id,device_reads::DeviceReadKind::Polling,
                    |route,device,next,validate| {
                        let observed=route.apply_polling(device,hz,next,validate)?;
                        Ok(json!({"result":observed.reading,"receiver_before":observed.receiver_before,
                            "receiver_after":observed.receiver_after,"profile_scope":"current_active_profile","obm_profiles_written":false}))
                    })
            }
            ServiceRequest::HidNodeMouseReceiverIdle { node, product_id, peer_product_id, raw_time } => {
                self.mouse_receiver_request(&node, product_id, peer_product_id, device_reads::DeviceReadKind::Idle,
                    |route, device, next, validate| {
                        let observed = route.apply_idle(device, raw_time, next, validate)?;
                        Ok(json!({"result":observed.reading,"receiver_before":observed.receiver_before,
                            "receiver_after":observed.receiver_after,"method":"setTimeToSleep"}))
                    })
            }
            ServiceRequest::HidNodeRead {
                node,
                product_id,
                kind,
            } => {
                let cap =
                    device_reads::capability(product_id).context("产品没有源核实的基础查询能力")?;
                let bound_cap = cap.for_physical_product(u32::from(node.product_id))?;
                let cap = &bound_cap;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && cap.direct_pids.contains(&u32::from(node.product_id))
                        && node.interface_number
                            == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
                    "HID 节点不符合源产品/接口选择；未知接口号不能猜测"
                );
                let IdentityLookup::Unique(identity) =
                    device_identity::lookup(u32::from(node.product_id))
                else {
                    bail!("直接设备身份映射不唯一");
                };
                ensure!(
                    identity.product_id == product_id && !identity.is_dongle,
                    "此入口只查询已核实的直接设备；接收器节点不能冒充鼠标"
                );
                let command = cap
                    .queries
                    .iter()
                    .find(|command| command.name == kind)
                    .context("产品不支持该源查询")?;
                if kind == razer_device::device_reads::DeviceReadKind::Polling {
                    ensure!(
                        cap.polling_physical_product_id == Some(u32::from(node.product_id)),
                        "轮询率物理连接分支尚未核实"
                    );
                }
                let started = Instant::now();
                let deadline = || deadline(started);
                let validate_open = || { deadline()?; Self::revalidate(&node) };
                let device = with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate_open)))?;
                Self::revalidate(&node)?;
                let transaction = self.transaction(
                    &node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )?;
                let reading = device_query::read_device(
                    device.as_ref(),
                    cap,
                    command,
                    transaction,
                    deadline,
                )?;
                Self::revalidate(&node)?;
                deadline()?;
                Ok(
                    json!({"node":node,"product_id":product_id,"reading":reading,
                    "method":command.method,"source_class":cap.source_class,"transaction_id":transaction,
                    "transport_metadata":device.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64,
                    "identity_scope":"hid_collection"}),
                )
            }
            ServiceRequest::HidNodeKeyboardBrightnessRead { node, product_id } => {
                self.keyboard_brightness(&node, product_id, None)
            }
            ServiceRequest::HidNodeKeyboardReceiverBrightness { node, product_id, peer_product_id, percent } => {
                self.receiver_setting(&node, product_id, peer_product_id, super::receiver_settings::Setting::KeyboardBrightness(percent))
            }
            ServiceRequest::HidNodeKeyboardMetadataRead { node, product_id } => {
                self.keyboard_metadata(&node, product_id, KeyboardRead::Metadata, None)
            }
            ServiceRequest::HidNodeKeyboardSerialRead { node, product_id } => {
                self.keyboard_metadata(&node, product_id, KeyboardRead::Serial, None)
            }
            ServiceRequest::HidNodeKeyboardRelayMetadataRead { node, product_id, peer_product_id } => {
                self.keyboard_metadata(&node, product_id, KeyboardRead::Metadata, Some(peer_product_id))
            }
            ServiceRequest::HidNodeKeyboardRelaySerialRead { node, product_id, peer_product_id } => {
                self.keyboard_metadata(&node, product_id, KeyboardRead::Serial, Some(peer_product_id))
            }
            ServiceRequest::KeyboardProfileInventoryRead { target } => {
                use razer_ipc::keyboard_profile_inventory::Target;
                target.validate()?;
                match target {
                    Target::Collection { node, product_id, peer_product_id } => self.keyboard_metadata(
                        &node, product_id, KeyboardRead::ProfileInventory, peer_product_id,
                    ),
                    Target::WindowsContainer { target } => {
                        #[cfg(windows)]
                        { super::native::keyboard_profile_inventory(&target) }
                        #[cfg(not(windows))]
                        {
                            let _ = target;
                            bail!("Windows keyboard inventory requires its platform HID adapter")
                        }
                    }
                }
            }
            ServiceRequest::KeyboardOnboardProfilesRead { target, options } => {
                use razer_ipc::keyboard_onboard_profiles::{Target, validate_target};
                validate_target(&target)?;
                options.validate()?;
                match target {
                    Target::Collection { node, product_id, peer_product_id } => self.keyboard_metadata(
                        &node, product_id, KeyboardRead::OnboardProfiles(&options), peer_product_id,
                    ),
                    Target::WindowsContainer { target } => {
                        #[cfg(windows)]
                        { super::native::keyboard_onboard_profiles(&target, &options) }
                        #[cfg(not(windows))]
                        {
                            let _ = target;
                            bail!("Windows keyboard OBM requires its platform HID adapter")
                        }
                    }
                }
            }
            ServiceRequest::HidNodeMouseCurrentDpi { node, product_id, peer_product_id, change } => {
                if let Some(peer) = peer_product_id {
                    self.receiver_setting(&node, product_id, peer,
                        super::receiver_settings::Setting::CurrentDpi(change.as_ref()))
                } else {
                    self.mouse_current_dpi(&node, product_id, change.as_ref())
                }
            }
            ServiceRequest::HidNodeMouseDynamic { node, product_id, peer_product_id, change } => {
                if let Some(peer) = peer_product_id {
                    self.receiver_setting(&node, product_id, peer,
                        super::receiver_settings::Setting::Dynamic(change.as_ref()))
                } else {
                    self.mouse_dynamic(&node, product_id, change.as_ref())
                }
            }
            ServiceRequest::HidNodeMouseBrightness {node, product_id, peer_product_id, submission} => self.mouse_brightness(&node, product_id, peer_product_id, &submission),
            ServiceRequest::HidNodeMouseRotation { node, product_id, peer_product_id, change } => {
                if let Some(peer) = peer_product_id {
                    self.receiver_setting(&node, product_id, peer,
                        super::receiver_settings::Setting::Rotation(change.as_ref()))
                } else {
                    self.mouse_rotation(&node, product_id, change.as_ref())
                }
            }
            ServiceRequest::HidNodeMouseCurrentPolling { node, product_id, change } => {
                self.mouse_current_polling(&node, product_id, change.as_ref())
            }
            ServiceRequest::HidNodeMouseCurrentPower { node, product_id, peer_product_id, change } => {
                self.mouse_current_power(&node, product_id, peer_product_id, &change)
            }
            ServiceRequest::HidNodeMouseScroll { node, product_id, peer_product_id, submission } => {
                if let Some(peer) = peer_product_id {
                    self.receiver_setting(&node, product_id, peer, super::receiver_settings::Setting::Scroll(&submission))
                } else {
                    self.mouse_scroll(&node, product_id, &submission)
                }
            }
            ServiceRequest::HidNodeLeviathan { node, command } => self.leviathan(&node, command),
            ServiceRequest::LeviathanEventsStart{session_id,target}=>self.leviathan_events.start(session_id,target),
            ServiceRequest::LeviathanEventsPoll{session_id}=>self.leviathan_events.poll(&session_id),
            ServiceRequest::LeviathanEventsStop{session_id}=>self.leviathan_events.stop(&session_id),
            ServiceRequest::HidNodeMouseIndicatorLed { node, product_id, change } => {
                self.mouse_indicator_led(&node, product_id, &change)
            }
            ServiceRequest::HidNodeSeirenEq { node, product_id, change } => {
                self.seiren_eq(&node, product_id, &change)
            }
            ServiceRequest::HidNodeKeyboardBrightnessWrite {
                node,
                product_id,
                percent,
            } => self.keyboard_brightness(&node, product_id, Some(percent)),
            ServiceRequest::HidNodeDpiStagesRead { node, product_id } => {
                self.dpi_stages(&node, product_id, None)
            }
            ServiceRequest::HidNodeDpiStagesWrite {
                node,
                product_id,
                draft,
            } => self.dpi_stages(&node, product_id, Some(&draft)),
            ServiceRequest::HidNodeWrite {
                node,
                product_id,
                setting,
            } => {
                let cap =
                    device_reads::capability(product_id).context("产品没有源核实的基础查询能力")?;
                let bound_cap = cap.for_physical_product(u32::from(node.product_id))?;
                let cap = &bound_cap;
                let write_cap = razer_device::device_writes::capability(product_id)
                    .context("产品没有源核实的直接写入能力")?;
                razer_device::device_writes::prepare(write_cap, &setting)?;
                if setting.read_kind() == razer_device::device_reads::DeviceReadKind::Polling {
                    ensure!(
                        cap.polling_physical_product_id == Some(u32::from(node.product_id)),
                        "轮询率写入物理连接分支尚未核实"
                    );
                }
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && cap.direct_pids.contains(&u32::from(node.product_id))
                        && node.interface_number
                            == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
                    "HID 写入节点不符合源产品/接口选择"
                );
                let IdentityLookup::Unique(identity) =
                    device_identity::lookup(u32::from(node.product_id))
                else {
                    bail!("写入目标没有唯一的直接设备身份");
                };
                ensure!(
                    identity.product_id == product_id && !identity.is_dongle && !identity.is_ble,
                    "写入入口只接受已核实的直接设备，不接受接收器或 BLE 路由"
                );
                let started = Instant::now();
                let validate_open = || { deadline(started)?; Self::revalidate(&node) };
                let device = with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate_open)))?;
                let result = razer_device::device_writes::apply(
                    device.as_ref(),
                    cap,
                    &setting,
                    || {
                        self.transaction(
                            &node,
                            &cap.source_class,
                            cap.transaction_prefix,
                            cap.transaction_modulus,
                        )
                    },
                    || {
                        ensure!(
                            started.elapsed() < Duration::from_secs(20),
                            "设备写入已超过确认期限"
                        );
                        Self::revalidate(&node)
                    },
                )?;
                Ok(json!({"node":node,"product_id":product_id,"result":result,
                    "source_class":cap.source_class,"transport_metadata":device.metadata(),
                    "identity_scope":"hid_collection","elapsed_ms":started.elapsed().as_millis() as u64}))
            }
            ServiceRequest::HidNodeHanboCooler {
                node,
                product_id,
                command,
            } => self.hanbo_cooler(&node, product_id, HanboExchange::Command(&command)),
            ServiceRequest::HidNodeHanboCoolerRead { node, product_id } =>
                self.hanbo_cooler(&node, product_id, HanboExchange::Telemetry),
            ServiceRequest::HidNodeHanboStartupRead { node, product_id, query } =>
                self.hanbo_cooler(&node, product_id, HanboExchange::Startup(query)),
            ServiceRequest::HidNodeReceiverStatus { node } => {
                self.receiver_status(&node, None)
            }
            ServiceRequest::HidNodeReceiverStatusForSource { node, source_product_id } => {
                self.receiver_status(&node, Some(source_product_id))
            }
            ServiceRequest::HidNodeReceiverDockMode { node } => {
                let cap = razer_device::receiver_dock_mode::capability(node.product_id)?;
                ensure!(node.vendor_id == cap.vendor_id && node.interface_number == i32::from(cap.claim_interface),
                    "Dock mode collection differs from the current factory interface");
                Self::revalidate(&node)?;
                let started = Instant::now();
                let validate_open = || { deadline(started)?; Self::revalidate(&node) };
                let device = with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate_open)))?;
                ensure!(device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
                    "Dock mode Feature descriptor differs");
                let transaction = self.transaction(&node, &cap.source_class, cap.transaction_prefix, cap.transaction_modulus)?;
                let data = razer_device::receiver_dock_mode::read(device.as_ref(), node.product_id, transaction, || {
                    deadline(started)?; Self::revalidate(&node)
                })?;
                Self::revalidate(&node)?;
                Ok(json!({"node":node,"product_id":node.product_id,"is_dongle":true,
                    "method":"getDockExtendedFirmwareVersion","data":data,"transaction_id":transaction,
                    "transport_metadata":device.metadata()}))
            }
            _ => bail!("此操作需要尚未移植的平台服务或设备身份适配"),
        }
    }
}

#[cfg(test)]
#[path = "portable_cleanup_tests.rs"]
pub(super) mod cleanup_tests;

impl PortableRuntime {
    fn dpi_stages(
        &mut self,
        node: &HidNode,
        product_id: u32,
        draft: Option<&razer_device::mouse_dpi_stages::DpiStagesDraft>,
    ) -> anyhow::Result<Value> {
        use razer_device::mouse_dpi_stages;
        let settings = mouse_dpi_stages::capability(product_id)
            .context("Product has no current source-proven DPI stage capability")?
            .for_physical_product(u32::from(node.product_id))?;
        let settings = &settings;
        let cap = &settings.transport;
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&u32::from(node.product_id))
                && node.interface_number
                    == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
            "DPI stage collection does not match source product/interface"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            bail!("DPI stage target has no unique current device identity");
        };
        ensure!(
            identity.product_id == product_id && !identity.is_dongle && !identity.is_ble,
            "DPI stage route requires a source-proven direct device"
        );
        if let Some(draft) = draft {
            mouse_dpi_stages::pack(settings, draft)?;
        }
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "DPI stage operation exceeded its confirmation deadline"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "DPI stage observed Feature Report size does not match current source"
        );
        let mut response = json!({"node":node,"product_id":product_id,"source_class":settings.source_class,
            "transport_source_class":cap.source_class,
            "identity_scope":"hid_collection","transport_metadata":device.metadata(),
            "profile_scope":"current_active_table","obm_profiles_written":false});
        let mut next = || {
            self.transaction(
                node,
                &cap.source_class,
                cap.transaction_prefix,
                cap.transaction_modulus,
            )
        };
        if let Some(draft) = draft {
            response["result"] = serde_json::to_value(mouse_dpi_stages::apply_current(
                device.as_ref(),
                settings,
                draft,
                &mut next,
                &validate,
            )?)?;
        } else {
            response["reading"] = serde_json::to_value(mouse_dpi_stages::read_current(
                device.as_ref(),
                settings,
                &mut next,
                &validate,
            )?)?;
        }
        validate()?;
        response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        Ok(response)
    }

    fn keyboard_peer_online(
        &mut self,
        device: &dyn razer_device::backend::FeatureTransport,
        node: &HidNode,
        product_id: u32,
        peer_product_id: u32,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let receiver = razer_device::receiver_capabilities::capability(node.product_id)
            .context("Keyboard relay has no current receiver query capability")?;
        let transaction = self.transaction(
            node,
            &receiver.source_class,
            receiver.transaction_prefix,
            receiver.transaction_modulus,
        )?;
        let peers =
            razer_device::device_query::read_receiver(device, receiver, transaction, validate)?;
        razer_device::keyboard_metadata::validate_peer(product_id, peer_product_id, &peers)
    }

    fn keyboard_metadata(
        &mut self,
        node: &HidNode,
        product_id: u32,
        query: KeyboardRead<'_>,
        peer_product_id: Option<u32>,
    ) -> anyhow::Result<Value> {
        use razer_device::keyboard_metadata;
        if matches!(query, KeyboardRead::ProfileInventory) {
            ensure!(
                razer_device::keyboard_profile_inventory::supports(product_id),
                "Keyboard does not mount the current HID OBM inventory"
            );
        }
        if let KeyboardRead::OnboardProfiles(options) = query {
            razer_ipc::keyboard_onboard_profiles::validate_target(
                &razer_ipc::keyboard_profile_inventory::Target::Collection {
                    node: node.clone(),
                    product_id,
                    peer_product_id,
                },
            )?;
            options.validate()?;
        }
        let selected =
            match keyboard_metadata::hid_capability(product_id, u32::from(node.product_id)) {
                Some(cap) => std::borrow::Cow::Borrowed(cap),
                None if peer_product_id.is_some() => std::borrow::Cow::Owned(
                    keyboard_metadata::relay_capability(product_id, u32::from(node.product_id))?,
                ),
                None => bail!("Keyboard metadata transport has no current source evidence"),
            };
        let cap = selected.as_ref();
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&u32::from(node.product_id))
                && node.interface_number
                    == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
            "Keyboard metadata node differs from source product/interface"
        );
        if peer_product_id.is_none() {
            let IdentityLookup::Unique(identity) =
                device_identity::lookup(u32::from(node.product_id))
            else {
                bail!("Keyboard metadata target has no unique identity");
            };
            ensure!(
                identity.product_id == product_id && !identity.is_ble,
                "Keyboard metadata requires its source-verified physical device"
            );
        }
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < query.deadline(),
                "Keyboard metadata read deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Keyboard metadata Feature descriptor differs from source"
        );
        if let Some(peer) = peer_product_id {
            self.keyboard_peer_online(device.as_ref(), node, product_id, peer, &validate)?;
        }
        let next_transaction = || {
            self.transaction(
                node,
                &cap.source_class,
                cap.transaction_prefix,
                cap.transaction_modulus,
            )
        };
        if let KeyboardRead::OnboardProfiles(options) = query {
            let reading = razer_device::keyboard_onboard_profiles::read_complete(
                device.as_ref(),
                cap,
                options,
                next_transaction,
                &validate,
            )?;
            validate()?;
            return Ok(serde_json::to_value(
                razer_ipc::keyboard_onboard_profiles::Observation {
                    target: razer_ipc::keyboard_profile_inventory::Target::Collection {
                        node: node.clone(),
                        product_id,
                        peer_product_id,
                    },
                    options: options.clone(),
                    reading,
                },
            )?);
        }
        if matches!(query, KeyboardRead::ProfileInventory) {
            let inventory = razer_device::keyboard_profile_inventory::read_hid(
                device.as_ref(),
                cap,
                next_transaction,
                &validate,
            )?;
            if let Some(peer) = peer_product_id {
                self.keyboard_peer_online(device.as_ref(), node, product_id, peer, &validate)?;
            }
            validate()?;
            return Ok(serde_json::to_value(
                razer_ipc::keyboard_profile_inventory::Observation {
                    target: razer_ipc::keyboard_profile_inventory::Target::Collection {
                        node: node.clone(),
                        product_id,
                        peer_product_id,
                    },
                    inventory,
                },
            )?);
        }
        let mut response = json!({"node": node, "product_id": product_id, "peer_product_id": peer_product_id,
            "source_class": cap.source_class, "identity_scope": "hid_collection", "transport_metadata": device.metadata()});
        if matches!(query, KeyboardRead::Serial) {
            response["method"] = json!("getSerialNumber");
            response["serial_number"] = json!(keyboard_metadata::read_serial(
                device.as_ref(),
                cap,
                next_transaction,
                &validate
            )?);
        } else {
            response["method"] = json!("getEdition");
            response["reading"] = serde_json::to_value(keyboard_metadata::read(
                device.as_ref(),
                cap,
                next_transaction,
                &validate,
            )?)?;
        }
        validate()?;
        response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        Ok(response)
    }

    fn keyboard_brightness(
        &mut self,
        node: &HidNode,
        product_id: u32,
        percent: Option<u8>,
    ) -> anyhow::Result<Value> {
        use razer_device::keyboard_settings;
        let settings =
            keyboard_settings::capability(product_id).context("产品没有源核实的键盘亮度能力")?;
        let cap = &settings.transport;
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&u32::from(node.product_id))
                && node.interface_number
                    == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
            "键盘亮度节点不符合源产品/接口"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            bail!("键盘亮度目标没有唯一设备身份");
        };
        ensure!(
            identity.product_id == product_id && !identity.is_dongle && !identity.is_ble,
            "键盘亮度入口只接受已核实的直接设备"
        );
        if let Some(percent) = percent {
            keyboard_settings::encode_percent(percent)?;
        }
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "键盘亮度操作超过确认期限"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "键盘亮度接口实际 Feature Report 长度不匹配"
        );
        let mut response = json!({"node":node,"product_id":product_id,"source_class":settings.source_class,
            "identity_scope":"hid_collection","transport_metadata":device.metadata()});
        let mut next = || {
            self.transaction(
                node,
                &cap.source_class,
                cap.transaction_prefix,
                cap.transaction_modulus,
            )
        };
        if let Some(percent) = percent {
            response["result"] = serde_json::to_value(keyboard_settings::apply(
                device.as_ref(),
                settings,
                percent,
                &mut next,
                &validate,
            )?)?;
        } else {
            response["reading"] = serde_json::to_value(keyboard_settings::read(
                device.as_ref(),
                settings,
                &mut next,
                &validate,
            )?)?;
        }
        validate()?;
        response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        Ok(response)
    }
}

fn mixer_local_request(
    node: HidNode,
    product_id: u32,
    command: razer_device::audio_mixer_local::Command,
) -> anyhow::Result<Value> {
    ensure!(
        razer_device::audio_mixer::accepts(product_id, node.vendor_id, node.product_id),
        "Local Mixer collection does not match current product source"
    );
    use razer_device::audio_mixer_local::{Command, Receipt};
    let delegated = match &command {
        Command::LightingSample { sources } => {
            #[cfg(windows)]
            {
                Some(super::native::mixer_lighting_sample(&node, *sources)?)
            }
            #[cfg(not(windows))]
            {
                let _ = sources;
                anyhow::bail!("Mixer lighting peak sampling requires Windows Core Audio");
            }
        }
        Command::Route {
            input,
            output,
            enabled,
        } => Some(mixer_route_request(
            node.clone(),
            product_id,
            MixerRoute {
                input: *input,
                output: *output,
            },
            Some(*enabled),
        )?),
        Command::Endpoint { jack, flow, value } => {
            #[cfg(windows)]
            {
                Some(super::native::mixer_endpoint_write(
                    &node, *jack, *flow, value,
                )?)
            }
            #[cfg(not(windows))]
            {
                let _ = (jack, flow, value);
                anyhow::bail!("Mixer software endpoints require Windows Core Audio adapter");
            }
        }
        _ => None,
    };
    if let Some(response) = delegated {
        let receipt = Receipt {
            command,
            transport_completed: true,
            response,
            profile_updates: Vec::new(),
        };
        return Ok(json!({"node":node,"product_id":product_id,"receipt":receipt}));
    }
    PortableRuntime::revalidate(&node)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Local Mixer source command exceeded deadline"
        );
        PortableRuntime::revalidate(&node)
    };
    let device =
        with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate)))?;
    let control = |report: &[u8]| {
        #[cfg(windows)]
        {
            super::native::mixer_control_output(&node, report)
        }
        #[cfg(not(windows))]
        {
            let _ = report;
            anyhow::bail!(
                "Exact native Mixer control Output requires a platform control endpoint adapter"
            )
        }
    };
    let receipt =
        razer_device::audio_mixer_local::execute(device.as_ref(), &command, &validate, &control)?;
    Ok(
        json!({"node":node,"product_id":product_id,"receipt":receipt,"transport_metadata":device.metadata()}),
    )
}

fn mixer_route_request(
    node: HidNode,
    product_id: u32,
    route: MixerRoute,
    enabled: Option<bool>,
) -> anyhow::Result<Value> {
    match razer_device::audio_mixer::matrix_route(&route)? {
        MatrixRoute::Hid(target) => mixer_request(
            node,
            product_id,
            target,
            enabled.map(|enabled| MixerValue::Boolean { enabled }),
        ),
        MatrixRoute::Driver { .. } => {
            #[cfg(windows)]
            {
                super::native::mixer_driver_route(node, product_id, route, enabled)
            }
            #[cfg(not(windows))]
            {
                let _ = (node, product_id, route, enabled);
                bail!("此 Mixer 路由使用 Windows 专属驱动矩阵，此平台未实现该能力")
            }
        }
    }
}

fn deadline(started: Instant) -> anyhow::Result<()> {
    ensure!(
        started.elapsed() < Duration::from_secs(10),
        "设备查询已超过观察期限"
    );
    Ok(())
}

fn mixer_eq_request(node: HidNode, product_id: u32, bands: [i32; 10]) -> anyhow::Result<Value> {
    use razer_device::audio_mixer::{
        MixerChannel, MixerControl, mic_eq_values, validate_report_lengths,
    };
    ensure!(
        razer_device::audio_mixer::accepts(product_id, node.vendor_id, node.product_id),
        "Mic EQ HID collection does not match the current product source"
    );
    mic_eq_values(&bands)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Mic EQ submission exceeded its confirmation deadline"
        );
        PortableRuntime::revalidate(&node)
    };
    validate()?;
    let device =
        with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate)))?;
    let target = MixerTarget {
        control: MixerControl::PageEqEnabled,
        band: None,
        channel: MixerChannel::Both,
    };
    let lengths = device.report_lengths()?;
    validate_report_lengths(&target, &lengths)?;
    let source_property = razer_device::audio_mixer::source_property(&target)?;
    let session = MixerSession::new(device.as_ref(), &validate)?;
    let receipt = razer_device::audio_mixer::submit_mic_eq(&session, bands, &validate)?;
    Ok(
        json!({"node":node,"product_id":product_id,"target":target,"bands":bands,"results":receipt.results,
        "source_property":source_property,
        "completed_bands":receipt.completed_bands,"identity_scope":"hid_collection"}),
    )
}

fn mixer_request(
    node: HidNode,
    product_id: u32,
    target: MixerTarget,
    value: Option<MixerValue>,
) -> anyhow::Result<Value> {
    ensure!(
        razer_device::audio_mixer::accepts(product_id, node.vendor_id, node.product_id),
        "HID 节点不是当前源核实的 Audio Mixer DSP 设备"
    );
    PortableRuntime::revalidate(&node)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "DSP 操作超过确认期限"
        );
        PortableRuntime::revalidate(&node)
    };
    let device =
        with_backend(|backend| backend.open_validated(&node, std::rc::Rc::new(&validate)))?;
    let source_property = razer_device::audio_mixer::source_property(&target)?;
    let session = MixerSession::new(device.as_ref(), &validate)?;
    let result = if let Some(value) = value {
        let result = session.apply(&target, &value)?;
        validate().map_err(|error| {
            error.context(razer_device::audio_mixer::MixerSubmissionFailure {
                target: target.clone(),
                result: result.clone(),
            })
        })?;
        serde_json::to_value(result)?
    } else {
        let value = session.read(&target)?;
        validate().map_err(|error| {
            error.context(razer_device::audio_mixer::MixerObservationFailure {
                target: target.clone(),
                value: value.clone(),
            })
        })?;
        serde_json::to_value(value)?
    };
    Ok(
        json!({"node":node,"product_id":product_id,"target":target,"result":result,
        "source_property":source_property,
        "source_limits":razer_device::audio_mixer::mic_monitor_limits(),
        "endpoint_limits":razer_device::audio_mixer::endpoint_limits(),
        "identity_scope":"hid_collection","transport_metadata":device.metadata(),
        "elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}
