//! Current je -> ze -> fe -> S. Only observed subDevices create IoT inventory.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq)]
pub struct Item {
    product_id: u32,
    serial_number: String,
    container: String,
    name: BTreeMap<String, String>,
    title: String,
    src: Option<String>,
    online: Option<bool>,
    power: Option<bool>,
    synapse_override: Option<bool>,
    group: usize,
    setup_status: String,
    locally_available: bool,
}
impl Item {
    fn from_source(device: &Device, child: Option<&Value>) -> Option<Self> {
        let object = child.and_then(Value::as_object);
        let get = |key| object.and_then(|value| value.get(key));
        let product_id = get("productId")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
            .unwrap_or(device.product_id);
        // Current je filters parent products; ze also excludes 780 children.
        if product_id == 780 {
            return None;
        }
        let group = match get("isSynapseOverride") {
            Some(Value::Bool(false)) => 1,
            Some(Value::Bool(true)) | None => 0,
            _ => return None,
        };
        let container = get("deviceContainerId")
            .and_then(Value::as_str)
            .unwrap_or(&device.device_container_id)
            .to_owned();
        if container.is_empty() {
            return None;
        }
        let name = if let Some(value) = get("name") {
            value
                .as_object()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            device.name.values.clone()
        };
        let title = match get("title") {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Number(value)) => value.to_string(),
            _ => String::new(),
        };
        let setup_status = get("setupStatus")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                match device.setup_status {
                    razer_model::model::SetupStatus::Ready => "ready",
                    razer_model::model::SetupStatus::Updating => "updating",
                    _ => "unknown",
                }
                .to_owned()
            });
        Some(Self {
            product_id,
            container,
            name,
            title,
            group,
            setup_status,
            serial_number: get("serialNumber")
                .and_then(Value::as_str)
                .unwrap_or(&device.serial_number)
                .to_owned(),
            src: get("src")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            online: get("isOnline").and_then(Value::as_bool),
            power: get("isPowerOn").and_then(Value::as_bool),
            synapse_override: get("isSynapseOverride").and_then(Value::as_bool),
            // User-authorized local entry policy; this does not create native
            // installedDevices metadata or a hardware READY acknowledgement.
            locally_available: razer_catalog::registered(product_id)
                .and_then(|product| product.primary_navigation())
                .is_some(),
        })
    }
    fn complete(&self) -> bool {
        self.locally_available || self.setup_status == "ready"
    }
    fn display_name(&self) -> Option<String> {
        self.name
            .get(&i18n::locale().to_lowercase())
            .filter(|name| !name.is_empty())
            .map(|name| {
                if self.title.is_empty() {
                    name.clone()
                } else {
                    self.title.clone()
                }
            })
    }
    fn status(&self) -> String {
        let key = match self.setup_status.as_str() {
            "waiting" => "WAITING",
            "downloading" => "DOWNLOADING",
            "installing" => "INSTALLING",
            "syncing" => "SYNCING",
            "updating" => "UPDATING",
            "install_canceled" | "error" => "INSTALLATION_FAILED",
            _ => return String::new(),
        };
        format!(
            "{}{}",
            i18n::t(key),
            if self.setup_status == "install_canceled" {
                ""
            } else {
                "..."
            }
        )
    }
}

impl GamerRoomPage {
    fn request_power(
        &mut self,
        product_id: u32,
        container: String,
        power: bool,
        cx: &mut Context<Self>,
    ) {
        // fe's power icon is a sibling of the selected .box-item. Its
        // document mousedown therefore also takes the popup click-away path.
        self.device_popup = None;
        cx.notify();
        // fe creates a fresh lodash debounce(..., 500) for each mouse-down.
        // Its trailing callback submits a request; no timer confirms hardware.
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let _ = this.update(cx, |_, cx| {
                cx.emit(GamerRoomEvent::DeviceCommand {
                    product_id,
                    device_container_id: container,
                    action: "ON_SET_POWER_STATE_IOT",
                    payload: json!({"isPowerOn": power}),
                })
            });
        })
        .detach();
    }
    pub fn sync_devices(&mut self, devices: &[Device], cx: &mut Context<Self>) {
        let mut items = Vec::<Item>::new();
        for device in devices
            .iter()
            .filter(|device| ![780, 3880, 3858].contains(&device.product_id))
        {
            let Some(children) = &device.sub_devices else {
                continue;
            };
            if children.is_empty() {
                if let Some(item) = Item::from_source(device, None) {
                    items.push(item);
                }
            } else {
                for child in children {
                    if let Some(item) = Item::from_source(device, Some(child)) {
                        if !items
                            .iter()
                            .any(|previous| previous.container == item.container)
                        {
                            items.push(item);
                        }
                    }
                }
            }
        }
        items.sort_by(|left, right| {
            let numeric = left
                .title
                .parse::<f64>()
                .ok()
                .zip(right.title.parse::<f64>().ok())
                .and_then(|(left, right)| left.partial_cmp(&right));
            numeric
                .filter(|value| !value.is_eq())
                .unwrap_or_else(|| left.title.cmp(&right.title))
        });
        if self.devices == items {
            return;
        }
        self.devices = items;
        // Current removeGamerRoomBanner receives true only when both groups
        // are empty; the reducer assigns that boolean to isBannerVisible.
        self.banner_visible = self.devices.is_empty();
        if self
            .device_popup
            .as_ref()
            .is_some_and(|id| !self.devices.iter().any(|item| &item.container == id))
        {
            self.device_popup = None;
        }
        cx.notify();
    }
    pub fn group_items(
        &self,
        group: usize,
        description: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items: Vec<_> = self
            .devices
            .iter()
            .filter(|item| item.group == group)
            .collect();
        h_flex()
            .w_full()
            .flex_shrink_0()
            .flex_wrap()
            .gap(surface::css(20.))
            .when(items.is_empty(), |view| {
                view.child(self.group_placeholder(group, description, cx))
            })
            .children(items.into_iter().map(|item| self.device_card(item, cx)))
            .into_any_element()
    }
    fn device_card(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.device_popup.as_deref() == Some(&item.container);
        let dimmed = item.complete() && (item.online != Some(true) || item.power != Some(true));
        let id = item.container.clone();
        let complete = item.complete();
        let body = v_flex()
            .id(SharedString::from(format!("gr-device-{}", item.container)))
            .relative()
            .w(surface::css(186.))
            .h(surface::css(176.))
            .flex_shrink_0()
            .p(surface::css(15.))
            .bg(rgb(0x111111))
            .rounded(surface::css(5.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if complete {
                        this.device_popup = if this.device_popup.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        };
                        cx.notify();
                    }
                }),
            )
            .child(
                h_flex()
                    .relative()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .child(div().size(surface::css(100.)).when_some(
                        item.src.clone(),
                        |view, source| {
                            let fallback = format!(
                                "https://app-assets.razer.com/files/synapse/devices/{}/cover.png",
                                item.product_id
                            );
                            view.child(
                                img(source)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .when(dimmed, |view| view.opacity(0.3))
                                    .with_fallback(move || {
                                        img(fallback.clone())
                                            .size_full()
                                            .object_fit(ObjectFit::Cover)
                                            .into_any_element()
                                    }),
                            )
                        },
                    ))
                    .when(!complete, |view| {
                        view.child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(surface::css(5.))
                                .bg(rgba(0x1212124d))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(img("synapse/gr-device-spinner.svg")),
                        )
                    }),
            )
            .when_some(item.display_name(), |view, name| {
                view.child(
                    div()
                        .w_full()
                        .flex_shrink_0()
                        .mt(surface::css(10.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .text_color(rgb(0xcccccc))
                        .whitespace_normal()
                        .when(dimmed, |view| view.opacity(0.3))
                        .child(name),
                )
            })
            .when(item.display_name().is_none(), |view| {
                view.child(
                    div()
                        .w_full()
                        .h(surface::css(15.))
                        .mt(surface::css(10.))
                        .bg(rgb(0xcccccc))
                        .opacity(0.5),
                )
            })
            .when(!complete, |view| {
                view.child(
                    div()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(17.))
                        .text_color(rgb(0x44d62c))
                        .child(item.status()),
                )
            })
            .when(selected, |view| view.child(self.device_popup(item, cx)));
        let power_id = item.container.clone();
        let keyboard_power_id = power_id.clone();
        let pid = item.product_id;
        let power = item.power != Some(true);
        let outline_width = surface::css(2.).to_pixels(cx.theme().font_size);
        let button = if item.online == Some(false) {
            img("synapse/gr-device-offline.svg")
                .size(surface::css(24.))
                .into_any_element()
        } else {
            BaseButton::new(SharedString::from(format!("gr-power-{}", item.container)))
                .accessibility_label(i18n::t("TAB_POWER"))
                .size(surface::css(32.))
                .p(surface::css(4.))
                .rounded(surface::css(40.))
                .bg(rgb(0))
                .cursor_pointer()
                .hover(move |style| {
                    style.shadow(vec![BoxShadow {
                        color: rgb(0x44d62c).into(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: outline_width,
                        inset: false,
                    }])
                })
                .child(
                    img(if item.power == Some(true) {
                        "synapse/gr-device-power-active.svg"
                    } else {
                        "synapse/gr-device-power.svg"
                    })
                    .size(surface::css(24.))
                    .mt(surface::css(-0.5)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.request_power(pid, power_id.clone(), power, cx);
                    }),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.is_keyboard() {
                        this.request_power(pid, keyboard_power_id.clone(), power, cx);
                    }
                }))
                .into_any_element()
        };
        div()
            .id(SharedString::from(format!(
                "gr-device-wrapper-{}",
                item.container
            )))
            .relative()
            .w(surface::css(186.))
            .h(surface::css(176.))
            .flex_shrink_0()
            .when(selected, |view| {
                view.on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if !this.device_popup_bounds.get().contains(&event.position) {
                        this.device_popup = None;
                        cx.notify();
                    }
                }))
            })
            .child(body)
            .child(
                div()
                    .absolute()
                    .right(surface::css(15.))
                    .top(surface::css(15.))
                    .child(button),
            )
            .into_any_element()
    }
    fn device_popup(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let below = item.group == 0;
        let pid = item.product_id;
        let container = item.container.clone();
        let serial = item.serial_number.clone();
        let override_id = container.clone();
        let popup_bounds = self.device_popup_bounds.clone();
        let online = item.online == Some(true);
        let popup = v_flex()
            .id(SharedString::from(format!(
                "gr-device-popup-{}",
                item.container
            )))
            .absolute()
            .left_0()
            .w(surface::css(380.))
            .min_h(surface::css(119.))
            .when(below, |view| {
                view.top_full().mt(surface::css(10.)).border_b_1()
            })
            .when(!below, |view| {
                view.bottom_full().mb(surface::css(10.)).border_t_1()
            })
            .p(surface::css(30.))
            .gap(surface::css(10.))
            .items_start()
            .border_l_1()
            .border_r_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(5.))
            .bg(rgb(0x111111))
            .text_color(rgb(0xcccccc))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .shadow(vec![BoxShadow {
                color: rgba(0x0000004d).into(),
                offset: point(px(0.), surface::css(3.).to_pixels(cx.theme().font_size)),
                blur_radius: surface::css(6.).to_pixels(cx.theme().font_size),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_prepaint(move |bounds, _, _| popup_bounds.set(bounds))
            .child(
                img(if below {
                    "synapse/gr-popup-bottom.svg"
                } else {
                    "synapse/gr-popup-top.svg"
                })
                .absolute()
                .left(surface::css(-0.5))
                .w(surface::css(380.))
                .h(surface::css(20.))
                .when(below, |view| view.top(surface::css(-11.)))
                .when(!below, |view| view.bottom(surface::css(-11.))),
            )
            .when(online, |view| {
                view.child(
                    h_flex()
                        .w_full()
                        .gap(surface::css(10.))
                        .child(
                            div()
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .child(i18n::t("SYNAPSE_OVERRIDE_HEADER").to_uppercase()),
                        )
                        .child(
                            surface::SynapseSwitch::new(SharedString::from(format!(
                                "gr-override-{}",
                                item.container
                            )))
                            .accessibility_label(i18n::t("SYNAPSE_OVERRIDE_HEADER"))
                            .checked(item.synapse_override == Some(true))
                            .on_change(cx.listener(
                                move |_, value: &bool, _, cx| {
                                    cx.emit(GamerRoomEvent::DeviceCommand {
                                        product_id: pid,
                                        device_container_id: override_id.clone(),
                                        action: "ON_SET_SYNAPSE_OVERRIDE",
                                        payload: json!({"isSynapseOverride": value}),
                                    });
                                },
                            )),
                        ),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .gap(surface::css(10.))
                    .child(
                        img(if !online {
                            "synapse/gr-device-offline-description.svg"
                        } else if item.synapse_override == Some(true) {
                            "synapse/gr-device-synapse.svg"
                        } else {
                            "synapse/gr-device-app.svg"
                        })
                        .size(surface::css(32.))
                        .flex_shrink_0()
                        .when(online, |view| view.mt(surface::css(2.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .whitespace_normal()
                            .child(i18n::t(if !online {
                                "DEVICE_OFFLINE"
                            } else if item.synapse_override == Some(true) {
                                "SYNAPSE_OVERRIDE_DESC1"
                            } else {
                                "SMART_HOME_APP_CONTROLLING"
                            })),
                    ),
            )
            .when(!online, |view| {
                view.child(
                    div()
                        .whitespace_normal()
                        .child(i18n::t("CONFIGURE_OFFLINE_DEVICE")),
                )
            })
            .when(online, |view| {
                view.child(
                    h_flex()
                        .w_full()
                        .justify_center()
                        .pt(surface::css(10.))
                        .child(
                            BaseButton::new(SharedString::from(format!(
                                "gr-settings-{}",
                                item.container
                            )))
                            .accessibility_label(i18n::t("ALL_SETTINGS"))
                            .w_auto()
                            .min_w(surface::css(90.))
                            .px(surface::css(16.))
                            .pt(surface::css(7.))
                            .pb(surface::css(6.))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(17.))
                            .border_1()
                            .border_color(rgb(0xcccccc))
                            .rounded(surface::css(3.))
                            .hover(|view| view.bg(rgb(0x2d2d2d)))
                            .child(i18n::t("ALL_SETTINGS").to_uppercase())
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(GamerRoomEvent::OpenDevice {
                                        product_id: pid,
                                        serial_number: serial.clone(),
                                        device_container_id: container.clone(),
                                    })
                                },
                            )),
                        ),
                )
            });
        SourceLayer::new(popup, 0., 0.)
            .priority(21)
            .into_any_element()
    }
}
