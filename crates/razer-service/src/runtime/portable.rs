//! Portable worker routes: native nodes and source-proved Razer queries.
//! Platform-specific audio/driver calls delegate to isolated OS adapters.
//! No guessed peer routing, downloaded DLL or direct OS API in this layer.
use super::ServiceRequest;
use anyhow::{Context as _, bail, ensure};
use razer_device::audio_mixer::{MatrixRoute, MixerRoute, MixerSession, MixerTarget, MixerValue};
use razer_device::backend::HidBackend;
use razer_device::backend::HidNode;
use razer_device::device_identity;
use razer_device::device_identity::IdentityLookup;
use razer_device::device_query;
use razer_device::device_reads;
use razer_device::receiver_capabilities;
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct PortableRuntime {
    transactions: HashMap<String, u8>,
    audio_notifications: crate::audio_notification::AudioNotifications,
    audio_router: crate::audio_router::AudioRouter,
    foreground_monitor: razer_platform::foreground_monitor::ForegroundMonitor,
    global_shortcuts: razer_platform::global_shortcuts::GlobalShortcuts,
}

impl PortableRuntime {
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

    pub(super) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        match request {
            ServiceRequest::RegisterShortcut { vkey_code, modifiers, argument } => {
                self.global_shortcuts.register(razer_platform::global_shortcuts::Shortcut {
                    virtual_key:vkey_code, modifiers, argument,
                })?;
                Ok(json!({"registered":true,"vkey_code":vkey_code,"modifiers":modifiers}))
            }
            ServiceRequest::UnregisterShortcut { vkey_code, modifiers } => {
                self.global_shortcuts.unregister(vkey_code,modifiers)?;
                Ok(json!({"registered":false,"vkey_code":vkey_code,"modifiers":modifiers}))
            }
            ServiceRequest::EnableGlobalShortcuts { enable } => {
                self.global_shortcuts.enable(enable)?;
                Ok(json!({"enabled":enable}))
            }
            ServiceRequest::GlobalShortcuts => Ok(Value::Array(self.global_shortcuts.registered()?.into_iter().map(|s|
                json!({"virtualKey":s.virtual_key,"modifiers":s.modifiers,"argument":s.argument})).collect())),
            ServiceRequest::ShortcutEvents => Ok(Value::Array(self.global_shortcuts.drain()?.into_iter().map(|s|
                json!({"event":json!({"virtualKey":s.virtual_key,"modifiers":s.modifiers,"argument":s.argument}).to_string()})).collect())),
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
                    "evidence": "docs/re/audio-util-enumerator-current-evidence.json"
                }))
            }
            ServiceRequest::HidNodeMixerRead {
                node,
                product_id,
                target,
            } => mixer_request(node, product_id, target, None),
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
                let device = with_backend(|backend| backend.open(&node))?;
                let reports = device.report_lengths()?;
                Self::revalidate(&node)?;
                Ok(json!({"node":node,"reports":reports,
                    "transport_metadata":device.metadata(),"identity_scope":"hid_collection"}))
            }
            ServiceRequest::HidNodeRead {
                node,
                product_id,
                kind,
            } => {
                let cap =
                    device_reads::capability(product_id).context("产品没有源核实的基础查询能力")?;
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
                let device = with_backend(|backend| backend.open(&node))?;
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
                    "identity_scope":"hid_collection", "evidence":"docs/re/mouse-read-capabilities-current-evidence.json"}),
                )
            }
            ServiceRequest::HidNodeKeyboardBrightnessRead { node, product_id } => {
                self.keyboard_brightness(&node, product_id, None)
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
                let device = with_backend(|backend| backend.open(&node))?;
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
                    "identity_scope":"hid_collection","elapsed_ms":started.elapsed().as_millis() as u64,
                    "evidence":"docs/re/device-write-capabilities-current-evidence.json"}))
            }
            ServiceRequest::HidNodeReceiverStatus { node } => {
                let cap = receiver_capabilities::capability(node.product_id)
                    .context("产品没有源核实的接收器查询能力")?;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && node.interface_number == i32::from(cap.claim_interface),
                    "接收器节点不符合源 VID/接口选择；未知接口号不能猜测"
                );
                let started = Instant::now();
                let device = with_backend(|backend| backend.open(&node))?;
                Self::revalidate(&node)?;
                let transaction = self.transaction(
                    &node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )?;
                let devices =
                    device_query::read_receiver(device.as_ref(), cap, transaction, || {
                        deadline(started)
                    })?;
                Self::revalidate(&node)?;
                deadline(started)?;
                let rows = devices
                    .into_iter()
                    .map(|(product_id, status)| json!({"product_id":product_id,"status":status}))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"node":node,"query":"receiver_wireless_status_v2","devices":rows,"device_count":rows.len(),
                    "transaction_id":transaction,"source_class":cap.source_class,"capability_evidence":cap.evidence_path,
                    "transport_metadata":device.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64,
                    "identity_scope":"hid_collection"}),
                )
            }
            _ => bail!("此操作需要尚未移植的平台服务或设备身份适配"),
        }
    }
}

impl PortableRuntime {
    fn dpi_stages(
        &mut self,
        node: &HidNode,
        product_id: u32,
        draft: Option<&razer_device::mouse_dpi_stages::DpiStagesDraft>,
    ) -> anyhow::Result<Value> {
        use razer_device::mouse_dpi_stages;
        let settings = mouse_dpi_stages::capability(product_id)
            .context("Product has no current source-proven DPI stage capability")?;
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
        // Retain the same open collection and process lock across set and get.
        let device = with_backend(|backend| backend.open(node))?;
        ensure!(
            device
                .report_lengths()?
                .feature
                .get(&cap.report_id)
                .copied()
                == Some(cap.report_bytes),
            "DPI stage observed Feature Report size does not match current source"
        );
        let mut response = json!({"node":node,"product_id":product_id,"source_class":settings.source_class,
            "identity_scope":"hid_collection","transport_metadata":device.metadata(),
            "evidence":"docs/re/mouse-dpi-stages-current-evidence.json","profile_scope":"current_active_table","obm_profiles_written":false});
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
        let device = with_backend(|backend| backend.open(node))?;
        let mut response = json!({"node":node,"product_id":product_id,"source_class":settings.source_class,
            "identity_scope":"hid_collection","transport_metadata":device.metadata(),
            "evidence":"docs/re/keyboard-settings-current-evidence.json"});
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
    let values = mic_eq_values(&bands)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Mic EQ submission exceeded its confirmation deadline"
        );
        PortableRuntime::revalidate(&node)
    };
    validate()?;
    let device = with_backend(|backend| backend.open(&node))?;
    let target = MixerTarget {
        control: MixerControl::PageEqEnabled,
        band: None,
        channel: MixerChannel::Both,
    };
    let lengths = device.report_lengths()?;
    validate_report_lengths(&target, &lengths)?;
    let session = MixerSession::new(device.as_ref(), &validate)?;
    let mut results = Vec::new();
    results.push(
        session
            .apply(&target, &MixerValue::Boolean { enabled: true })
            .context("Mic EQ enable transport did not complete; no EQ band submitted")?,
    );
    let mut completed = Vec::<usize>::new();
    for (index, value) in values.iter().enumerate() {
        let target = MixerTarget {
            control: MixerControl::EqBand,
            band: Some(index as u8),
            channel: MixerChannel::Both,
        };
        let result = session.apply(&target, value).with_context(|| format!(
            "Mic EQ band {index} transport did not complete; enable completed, submitted bands {completed:?}; no rollback or later band submitted"
        ))?;
        results.push(result);
        completed.push(index);
    }
    validate().context("Mic EQ reports submitted; final collection identity was not confirmed")?;
    Ok(
        json!({"node":node,"product_id":product_id,"target":target,"bands":bands,"results":results,
        "source_property":razer_device::audio_mixer::source_property(&target)?,
        "completed_bands":completed,"identity_scope":"hid_collection",
        "evidence":"docs/re/audio-mixer-page-bindings-source-current.json"}),
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
    let device = with_backend(|backend| backend.open(&node))?;
    // A retained path/collection lock covers all selector and query
    // reports. No second device, endpoint ID, receiver or DLL is guessed.
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "DSP 操作超过确认期限"
        );
        PortableRuntime::revalidate(&node)
    };
    let session = MixerSession::new(device.as_ref(), &validate)?;
    let result = if let Some(value) = value {
        serde_json::to_value(session.apply(&target, &value)?)?
    } else {
        serde_json::to_value(session.read(&target)?)?
    };
    validate()?;
    Ok(
        json!({"node":node,"product_id":product_id,"target":target,"result":result,
        "source_property":razer_device::audio_mixer::source_property(&target)?,
        "source_limits":razer_device::audio_mixer::mic_monitor_limits(),
        "endpoint_limits":razer_device::audio_mixer::endpoint_limits(),
        "identity_scope":"hid_collection","transport_metadata":device.metadata(),
        "elapsed_ms":started.elapsed().as_millis() as u64,
        "evidence":"docs/re/audio-mixer-controls-current-evidence.json"}),
    )
}
