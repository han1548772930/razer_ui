//! Read-only connection UI. The actor owns the child process, including shutdown;
//! no DLL, pipe operation, mutex wait or child destructor runs on the UI thread.
use crate::{
    backend::discovery::{self, DiscoverySnapshot},
    backend::runtime::{ServiceClient, ServiceRequest},
    ui::surface,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde_json::Value;
use std::{sync::mpsc, time::Duration};

#[derive(Clone)]
struct RuntimeBridge(mpsc::Sender<Command>);
enum Command {
    Refresh(bool, mpsc::Sender<Readings>),
    Disconnect(mpsc::Sender<Result<(), String>>),
}

#[derive(Default)]
struct Readings {
    connected: bool,
    usb: Option<Result<Value, String>>,
    hid: Option<Result<Value, String>>,
    version: Option<Result<Value, String>>,
    audio: Option<Result<Value, String>>,
    discovery: Option<Result<DiscoverySnapshot, String>>,
    services_requested: bool,
}

#[derive(Clone)]
pub(super) struct DiscoveryObserved(pub(super) Result<DiscoverySnapshot, String>);
impl EventEmitter<DiscoveryObserved> for RuntimePanel {}

impl Readings {
    fn has_partial_results(&self) -> bool {
        let required = if self.services_requested || self.version.is_some() || self.audio.is_some()
        {
            vec![&self.usb, &self.hid, &self.version, &self.audio]
        } else {
            vec![&self.usb, &self.hid]
        };
        required.iter().any(|result| !matches!(result, Some(Ok(_))))
            || self.discovery.as_ref().is_some_and(|result| match result {
                Ok(snapshot) => !snapshot.errors().is_empty(),
                Err(_) => true,
            })
            || [(&self.usb, "devices"), (&self.hid, "interfaces")]
                .iter()
                .any(|(result, key)| {
                    result
                        .as_ref()
                        .and_then(|result| result.as_ref().ok())
                        .is_some_and(|value| {
                            value.get(*key).and_then(Value::as_array).is_none()
                                || value.get("complete") != Some(&Value::Bool(true))
                                || value
                                    .get("failures")
                                    .and_then(Value::as_array)
                                    .is_some_and(|failures| !failures.is_empty())
                        })
                })
    }

    fn status(&self) -> &'static str {
        if !self.connected {
            "服务连接已结束；下方保留本次读取结果。"
        } else if self.has_partial_results() {
            "读取完成，部分信息不可用。"
        } else {
            "服务信息已读取。"
        }
    }
}

impl RuntimeBridge {
    fn start() -> Result<Self, String> {
        let (sender, commands) = mpsc::channel();
        std::thread::Builder::new()
            .name("razer-service-connection".into())
            .spawn(move || {
                let mut client: Option<ServiceClient> = None;
                while let Ok(command) = commands.recv() {
                    match command {
                        Command::Refresh(services, reply) => {
                            let result = match client.as_mut() {
                                Some(client) => read_services(client, services),
                                None => match ServiceClient::spawn() {
                                    Ok(mut connection) => {
                                        let result = read_services(&mut connection, services);
                                        client = Some(connection);
                                        result
                                    }
                                    Err(error) => {
                                        let error = format!("{error:#}");
                                        Readings {
                                            connected: false,
                                            usb: Some(Err(error.clone())),
                                            hid: Some(Err(error.clone())),
                                            version: services.then(|| Err(error.clone())),
                                            audio: services.then(|| Err(error.clone())),
                                            discovery: Some(Err(error)),
                                            services_requested: services,
                                        }
                                    }
                                },
                            };
                            if !result.connected {
                                client.take();
                            }
                            let _ = reply.send(result);
                        }
                        Command::Disconnect(reply) => {
                            let result = client.take().map_or(Ok(()), |mut client| {
                                client
                                    .request(ServiceRequest::Shutdown)
                                    .map(|_| ())
                                    .map_err(|error| format!("{error:#}"))
                            });
                            let _ = reply.send(result);
                            return;
                        }
                    }
                }
                // Closing the UI only drops senders. Native shutdown and child.wait
                // remain here, even when an in-flight UI task has been cancelled.
                if let Some(mut client) = client {
                    let _ = client.request(ServiceRequest::Shutdown);
                }
            })
            .map_err(|error| format!("无法创建服务连接线程：{error}"))?;
        Ok(Self(sender))
    }

    fn refresh(&self, services: bool) -> Result<Readings, String> {
        let (reply, response) = mpsc::channel();
        self.0
            .send(Command::Refresh(services, reply))
            .map_err(|_| "服务连接已结束，请重新连接。".to_string())?;
        response
            // Each worker request already has a deadline. A fixed aggregate
            // timeout discarded valid results when several receivers existed.
            .recv()
            .map_err(|error| format!("服务读取未完成：{error}"))
    }

    fn disconnect(&self) -> Result<(), String> {
        let (reply, response) = mpsc::channel();
        self.0
            .send(Command::Disconnect(reply))
            .map_err(|_| "服务连接已经结束。".to_string())?;
        response
            .recv_timeout(Duration::from_secs(30))
            .map_err(|error| format!("服务关闭未完成：{error}"))?
    }
}

fn read_services(client: &mut ServiceClient, services: bool) -> Readings {
    // Enumerate physical USB and HID separately before vendor services. Neither
    // enumeration's failure discards the other one's real observations.
    let usb = client
        .request(ServiceRequest::UsbDevices)
        .map_err(|error| format!("{error:#}"));
    let hid = client
        .request(ServiceRequest::HidDevices)
        .map_err(|error| format!("{error:#}"));
    let discovery = discovery::discover(client, &usb, &hid).map_err(|error| format!("{error:#}"));
    let version = services.then(|| {
        client
            .request(ServiceRequest::SimpleVersion)
            .map_err(|error| format!("{error:#}"))
    });
    let audio = services.then(|| {
        client
            .request(ServiceRequest::AudioDevices)
            .map_err(|error| format!("{error:#}"))
    });
    Readings {
        connected: !client.is_stopped(),
        usb: Some(usb),
        hid: Some(hid),
        version,
        audio,
        discovery: Some(discovery),
        services_requested: services,
    }
}

pub(super) struct RuntimePanel {
    bridge: Option<RuntimeBridge>,
    busy: bool,
    readings: Readings,
    status: String,
    error: Option<String>,
    details: bool,
}

impl RuntimePanel {
    pub(super) fn new() -> Self {
        Self {
            bridge: None,
            busy: false,
            readings: Readings::default(),
            status: "尚未连接".into(),
            error: None,
            details: false,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_queries(true, cx);
    }

    pub(super) fn discover_devices(&mut self, cx: &mut Context<Self>) {
        self.refresh_queries(false, cx);
    }

    fn refresh_queries(&mut self, services: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        self.status = "正在读取服务信息…".into();
        let bridge = self.bridge.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let bridge = bridge.map_or_else(RuntimeBridge::start, Ok)?;
                    let readings = bridge.refresh(services)?;
                    Ok::<_, String>((bridge, readings))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((bridge, readings)) => {
                        if let Some(observation) = &readings.discovery {
                            cx.emit(DiscoveryObserved(observation.clone()));
                        }
                        this.bridge = readings.connected.then_some(bridge);
                        this.status = readings.status().into();
                        this.readings = readings;
                    }
                    Err(error) => {
                        cx.emit(DiscoveryObserved(Err(error.clone())));
                        this.bridge = None;
                        this.readings.connected = false;
                        this.status = "连接失败；已有结果未更新。".into();
                        this.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn disconnect(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(bridge) = self.bridge.take() else {
            return;
        };
        self.busy = true;
        self.status = "正在断开连接…".into();
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { bridge.disconnect() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.finish_disconnect(result);
                cx.emit(DiscoveryObserved(Err(
                    "服务连接已断开，此前设备观察已过期".into()
                )));
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_disconnect(&mut self, result: Result<(), String>) {
        self.busy = false;
        self.readings.connected = false;
        self.status = if result.is_ok() {
            "已断开；下方保留最近读取的信息。"
        } else {
            "断开未正常完成；下方保留最近读取的信息。"
        }
        .into();
        self.error = result.err();
    }
}

impl Render for RuntimePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.bridge.is_some();
        surface::panel("服务连接", cx)
            .id("runtime-panel")
            .test_support()
            .w_full()
            .gap_3()
            .child(surface::note(
                "读取本机设备接口、音频服务和版本信息。设备配置仍保存在本机。",
                cx,
            ))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        Button::new("runtime-connect")
                            .label(if connected {
                                "刷新信息"
                            } else {
                                "连接并读取"
                            })
                            .primary()
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("runtime-disconnect")
                            .label("断开连接")
                            .outline()
                            .disabled(self.busy || !connected)
                            .on_click(cx.listener(|this, _, _, cx| this.disconnect(cx))),
                    ),
            )
            .child(
                div()
                    .id("runtime-status")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .child(self.status.clone()),
            )
            .when_some(self.error.clone(), |this, error| {
                this.child(
                    div()
                        .id("runtime-error")
                        .test_support()
                        .aria_label(error.clone())
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .when_some(self.readings.hid.as_ref(), |this, result| {
                this.child(hid_result(result, cx))
            })
            .when_some(self.readings.usb.as_ref(), |this, result| {
                this.child(usb_result(result, cx))
            })
            .when_some(self.readings.discovery.as_ref(), |view, result| {
                let label = match result {
                    Ok(snapshot) => format!(
                        "识别到 {} 项产品接口或接收器关联；配置尚未读取。{}",
                        snapshot.devices().len(),
                        snapshot.errors().join("；")
                    ),
                    Err(error) => format!("产品发现失败：{error}"),
                };
                view.child(
                    div()
                        .id("runtime-discovery-status")
                        .test_support()
                        .role(Role::Status)
                        .aria_label(label.clone())
                        .child(label),
                )
            })
            .when_some(self.readings.version.as_ref(), |this, result| {
                this.child(reading("服务版本", result, cx))
            })
            .when_some(self.readings.audio.as_ref(), |this, result| {
                this.child(reading("音频设备", result, cx))
            })
            .when(self.readings.hid.is_some(), |this| {
                this.child(
                    Button::new("runtime-details")
                        .ghost()
                        .label(if self.details {
                            "收起读取详情"
                        } else {
                            "查看读取详情"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.details = !this.details;
                            cx.notify();
                        })),
                )
                .when(self.details, |this| {
                    this.children(
                        [
                            ("USB 设备", &self.readings.usb),
                            ("HID 接口", &self.readings.hid),
                            ("版本", &self.readings.version),
                            ("音频", &self.readings.audio),
                        ]
                        .into_iter()
                        .filter_map(|(label, result)| {
                            result
                                .as_ref()
                                .and_then(|result| result.as_ref().ok())
                                .map(|value| {
                                    v_flex()
                                        .gap_2()
                                        .child(label)
                                        .child(
                                            div()
                                                .w_full()
                                                .whitespace_normal()
                                                .text_size(surface::css(12.))
                                                .child(display_value(value)),
                                        )
                                        .into_any_element()
                                })
                        }),
                    )
                })
            })
    }
}

fn reading(label: &str, result: &Result<Value, String>, cx: &App) -> AnyElement {
    let text = match result {
        Ok(Value::String(value)) if !value.trim().is_empty() => {
            format!("{label}：{}", limit_text(value, 500))
        }
        Ok(Value::Array(items)) => format!("{label}：已收到 {} 项信息，可展开查看。", items.len()),
        Ok(Value::Object(_)) => format!("{label}：已收到服务信息，可展开查看。"),
        Ok(_) => format!("{label}：服务未提供可显示的信息。"),
        Err(error) => format!("{label}读取失败：{error}"),
    };
    div()
        .whitespace_normal()
        .when(result.is_err(), |this| this.text_color(cx.theme().danger))
        .child(text)
        .into_any_element()
}

fn hid_result(result: &Result<Value, String>, cx: &App) -> AnyElement {
    let Ok(value) = result else {
        return reading("设备接口", result, cx);
    };
    let Some(interfaces) = value.get("interfaces").and_then(Value::as_array) else {
        return div()
            .text_color(cx.theme().danger)
            .child("设备接口响应格式无法识别。")
            .into_any_element();
    };
    let failures = value
        .get("failures")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let incomplete = value.get("complete") != Some(&Value::Bool(true));
    let summary = if incomplete {
        format!(
            "部分接口读取完成：已读取到 {} 个 Razer 设备接口，枚举未完成。",
            interfaces.len()
        )
    } else {
        format!(
            "读取到 {} 个 Razer 设备接口；同一设备可能包含多个接口。",
            interfaces.len()
        )
    };
    v_flex()
        .gap_2()
        .child(
            div()
                .id("runtime-hid-summary")
                .test_support()
                .role(Role::Status)
                .aria_label(summary.clone())
                .child(summary),
        )
        .children(interfaces.iter().map(|item| {
            let product = item
                .get("product")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("未提供产品名称");
            let serial = item
                .get("serial_number")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("未提供");
            let vid = item
                .get("vendor_id")
                .and_then(Value::as_u64)
                .map(|value| format!("{value:04X}"))
                .unwrap_or_else(|| "未知".into());
            let pid = item
                .get("product_id")
                .and_then(Value::as_u64)
                .map(|value| format!("{value:04X}"))
                .unwrap_or_else(|| "未知".into());
            v_flex()
                .id(SharedString::from(format!(
                    "runtime-hid-{}",
                    item["path"].as_str().unwrap_or("missing-path")
                )))
                .test_support()
                .role(Role::ListItem)
                .aria_label(format!("{product} · USB {vid}:{pid} · {serial}"))
                .gap_1()
                .child(product.to_string())
                .child(surface::note(
                    format!("序列号：{serial} · USB {vid}:{pid}"),
                    cx,
                ))
                .into_any_element()
        }))
        .when(failures > 0, |this| {
            this.child(
                div()
                    .text_color(cx.theme().danger)
                    .child(format!("另有 {failures} 项接口读取错误，详情可展开查看。")),
            )
        })
        .into_any_element()
}

fn usb_result(result: &Result<Value, String>, cx: &App) -> AnyElement {
    let summary = match result {
        Err(error) => format!("USB 设备枚举失败：{error}"),
        Ok(value) => match value["devices"].as_array() {
            None => "USB 设备响应格式无法识别。".into(),
            Some(devices) => {
                let count = devices
                    .iter()
                    .filter(|item| item["vendor_id"] == 5426)
                    .count();
                let failures = value["failures"].as_array().map_or(0, Vec::len);
                let state = if value["complete"] == true {
                    "枚举已结束"
                } else {
                    "枚举未完成"
                };
                format!(
                    "发现 {count} 项 Razer USB 设备；{state}，{failures} 项读取错误。配置尚未读取。"
                )
            }
        },
    };
    div()
        .id("runtime-usb-summary")
        .test_support()
        .role(Role::Status)
        .aria_label(summary.clone())
        .when(result.is_err(), |this| this.text_color(cx.theme().danger))
        .child(summary)
        .into_any_element()
}

fn display_value(value: &Value) -> String {
    limit_text(
        &serde_json::to_string_pretty(value).unwrap_or_default(),
        16_000,
    )
}
fn limit_text(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let mut text: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        text.push_str("\n…内容较长，后续部分已省略");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{Readings, RuntimePanel, limit_text};
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext, TestAppContext, px, size};
    use serde_json::json;
    #[test]
    fn diagnostics_keep_unicode_boundaries_and_signal_truncation() {
        assert_eq!(limit_text("鼠标🎮", 3), "鼠标🎮");
        assert_eq!(
            limit_text("鼠标🎮abc", 3),
            "鼠标🎮\n…内容较长，后续部分已省略"
        );
    }

    #[test]
    fn incomplete_hid_and_query_failures_never_report_full_success() {
        for hid in [
            json!({"interfaces":[],"failures":[],"complete":false}),
            json!({"interfaces":[],"failures":[{"reason":"denied"}],"complete":true}),
            json!({"unexpected":[]}),
        ] {
            let readings = Readings {
                connected: true,
                hid: Some(Ok(hid)),
                ..Readings::default()
            };
            assert!(readings.status().contains("部分信息不可用"));
        }
        let readings = Readings {
            connected: true,
            version: Some(Err("unavailable".into())),
            ..Readings::default()
        };
        assert!(readings.status().contains("部分信息不可用"));
    }

    #[test]
    fn failed_shutdown_keeps_error_and_does_not_claim_clean_disconnect() {
        let mut panel = RuntimePanel::new();
        panel.busy = true;
        panel.readings = Readings {
            connected: true,
            hid: Some(Ok(json!({"interfaces":[],"failures":[],"complete":true}))),
            ..Readings::default()
        };
        panel.finish_disconnect(Err("shutdown timeout".into()));
        assert!(!panel.busy);
        assert!(!panel.readings.connected);
        assert!(panel.status.contains("未正常完成"));
        assert_eq!(panel.error.as_deref(), Some("shutdown timeout"));
        assert!(panel.readings.hid.is_some());
    }

    #[gpui_kit::test]
    fn partial_enumeration_is_visible_without_starting_a_service(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
            let panel = cx.new(|_| {
                let mut panel = RuntimePanel::new();
                panel.readings = Readings {
                    connected: true,
                    usb: Some(Ok(json!({"devices":[{"vendor_id":5426,"product_id":3592}],"failures":[],"complete":true}))),
                    hid: Some(Ok(json!({"interfaces":[],"failures":[],"complete":false}))),
                    ..Readings::default()
                };
                panel.status = panel.readings.status().into();
                panel
            });
            Root::new(panel, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .find("runtime-status")
                    .label()
                    .unwrap()
                    .contains("部分信息不可用")
            );
            assert!(
                window
                    .find("runtime-hid-summary")
                    .label()
                    .unwrap()
                    .contains("枚举未完成")
            );
            assert!(
                window
                    .find("runtime-usb-summary")
                    .label()
                    .unwrap()
                    .contains("发现 1 项")
            );
            assert_eq!(window.find("runtime-disconnect").disabled(), Some(true));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn connection_page_does_not_start_services_on_render(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
            let panel = cx.new(|_| RuntimePanel::new());
            Root::new(panel, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("runtime-connect").disabled(), Some(false));
            assert_eq!(window.find("runtime-disconnect").disabled(), Some(true));
            assert_eq!(
                window.find("runtime-status").label().as_deref(),
                Some("尚未连接")
            );
        })
        .unwrap();
    }
}
