//! Current 226 wi/Xs/zs polling panel. Observations never become profile writes.
use super::*;
use gpui_kit::{base::Button as BaseButton, prelude::FluentBuilder as _};
use std::collections::BTreeSet;

pub(super) const LOCAL_FIELDS: &str = "_pollingLocalFieldsV1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PollingConnection {
    Wired,
    Dongle,
    Ble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PollingField {
    Wired,
    Wireless,
}
impl PollingField {
    fn key(self) -> &'static str {
        match self {
            Self::Wired => "pollingRate",
            Self::Wireless => "pollingRateWireless",
        }
    }
    fn rates(self) -> &'static str {
        match self {
            Self::Wired => "POLLING_RATE",
            Self::Wireless => "POLLING_RATE_WIRELESS",
        }
    }
}

/// Capture before issuing a read. Reacquire after a connection observation or
/// profile restore; a late result from either previous owner is rejected.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct MousePollingScope {
    owner: EntityId,
    profile_epoch: u64,
    connection_epoch: u64,
}

#[derive(Clone)]
#[allow(dead_code)] // Current product HID/runtime publishers remain unconnected.
pub(crate) enum MousePollingObservation {
    Connection(Option<PollingConnection>),
    Rate(PollingField, u32),
    /// Ordered Object.values of browser localStorage `duallink-devices`.
    /// Preserve source order for find(); this is not host window storage.
    DualLinkSnapshot(Vec<Value>),
}

#[derive(Default)]
pub(super) struct State {
    connection: Option<PollingConnection>,
    profile_epoch: u64,
    connection_epoch: u64,
    rates: BTreeMap<PollingField, u32>,
    topology: Option<Vec<Value>>,
    local_fields: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum Limit {
    DualLink,
    MultiDeviceDock,
}
impl Limit {
    fn label(self) -> &'static str {
        match self {
            Self::DualLink => "POLLING_RATE_DUAL_LINK_LIMITED",
            Self::MultiDeviceDock => "POLLING_RATE_MULTI_DEVICE_DOCK_LIMITED_MOUSE",
        }
    }
}

fn ids(value: &Value) -> impl Iterator<Item = &Value> {
    [value.get("productId"), value.get("dongleId")]
        .into_iter()
        .flatten()
        .filter(|id| !id.is_null())
}
fn source_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.as_f64().unwrap_or(0.).to_string(),
        _ => value.to_string(),
    }
}
fn strict_id_equal(left: &Value, right: &Value) -> bool {
    if left.is_number() && right.is_number() {
        left.as_f64() == right.as_f64()
    } else {
        left == right
    }
}
fn current_device(value: &Value) -> bool {
    ids(value).any(|id| matches!(source_string(id).as_str(), "226" | "227"))
}
fn truthy_id(value: &&Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

impl State {
    fn field(&self) -> PollingField {
        match self.connection {
            Some(PollingConnection::Dongle | PollingConnection::Ble) => PollingField::Wireless,
            // Current deviceReducer starts false/false. This is only a local
            // editing fallback; None remains None and is identified in the UI.
            _ => PollingField::Wired,
        }
    }
    fn limit(&self) -> Option<Limit> {
        if self.connection != Some(PollingConnection::Dongle) {
            return None;
        }
        let topology = self.topology.as_ref()?;
        let pair = topology.iter().find(|pair| {
            let slave = pair.get("slave").filter(|v| !v.is_null()).unwrap_or(pair);
            current_device(slave) || current_device(&pair["master"])
        })?;
        let master = &pair["master"];
        let supports_8k = master["supports8KHzPollingRate"] == true;
        if supports_8k {
            // js/Ls uses JS strict includes (number 179 != string "179"),
            // unlike isCurrentDevice's explicit String conversion above.
            let count = topology
                .iter()
                .filter(|other| {
                    ids(master).filter(truthy_id).any(|id| {
                        ids(&other["master"])
                            .filter(truthy_id)
                            .any(|other_id| strict_id_equal(id, other_id))
                    })
                })
                .count();
            if count >= 2 {
                return Some(Limit::MultiDeviceDock);
            }
        }
        let hyper_master = matches!(master["productId"].as_f64(), Some(179. | 164.)) || supports_8k;
        (!hyper_master).then_some(Limit::DualLink)
    }
}

impl MouseProductWorkspace {
    pub(super) fn polling_226_visible(&self) -> bool {
        self.polling_state.connection != Some(PollingConnection::Ble)
    }
    pub(crate) fn mouse_polling_scope(&self, _cx: &App) -> Option<MousePollingScope> {
        (self.spec.product_id == 226).then(|| MousePollingScope {
            owner: self.polling_owner.expect("226 polling owner"),
            profile_epoch: self.polling_state.profile_epoch,
            connection_epoch: self.polling_state.connection_epoch,
        })
    }

    pub(crate) fn observe_mouse_polling(
        &mut self,
        scope: MousePollingScope,
        observation: MousePollingObservation,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_polling_scope(cx) != Some(scope) {
            return;
        }
        match observation {
            MousePollingObservation::Connection(connection) => {
                self.polling_state.connection = connection;
                self.polling_state.connection_epoch =
                    self.polling_state.connection_epoch.wrapping_add(1);
                self.polling_state.rates.clear();
                self.polling_state.topology = None;
            }
            MousePollingObservation::Rate(field, rate) => {
                if rate == 0 {
                    return;
                }
                self.polling_state.rates.insert(field, rate);
            }
            MousePollingObservation::DualLinkSnapshot(topology) => {
                self.polling_state.topology = Some(topology);
            }
        }
        // No Changed event, snapshot write, auto-downgrade or device command.
        cx.notify();
    }

    pub(super) fn restore_polling(&mut self, saved: Option<&Value>) {
        if self.spec.product_id != 226 {
            return;
        }
        self.polling_state.profile_epoch = self.polling_state.profile_epoch.wrapping_add(1);
        self.polling_state.rates.clear();
        self.polling_state.local_fields.clear();
        for field in [PollingField::Wired, PollingField::Wireless] {
            let Some(saved) = saved.filter(|saved| saved.get(field.key()).is_some()) else {
                continue;
            };
            let local = match saved.get(LOCAL_FIELDS) {
                Some(Value::Array(fields)) => fields.iter().any(|key| key == field.key()),
                None => true, // Legacy saved profile values remain local drafts.
                _ => false,
            };
            if local {
                self.polling_state.local_fields.insert(field.key().into());
            }
        }
        self.draft[LOCAL_FIELDS] = json!(self.polling_state.local_fields);
    }

    fn polling_value(&self, field: PollingField) -> u32 {
        if !self.polling_state.local_fields.contains(field.key()) {
            if let Some(value) = self.polling_state.rates.get(&field) {
                return *value;
            }
        }
        self.draft[field.key()].as_u64().unwrap_or(0) as u32
    }

    fn choose_polling(
        &mut self,
        scope: MousePollingScope,
        field: PollingField,
        rate: u32,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_polling_scope(cx) != Some(scope)
            || !self.active
            || self.page != "TAB_PERFORMANCE"
            || self.polling_state.connection == Some(PollingConnection::Ble)
            || self.polling_state.field() != field
            || self.polling_value(field) == rate
            || self.polling_state.limit().is_some() && rate > 1000
            || !self
                .spec
                .rates
                .get(field.rates())
                .is_some_and(|rates| rates.contains(&rate))
        {
            return;
        }
        self.draft[field.key()] = json!(rate);
        self.polling_state.local_fields.insert(field.key().into());
        self.draft[LOCAL_FIELDS] = json!(self.polling_state.local_fields);
        // Even if the old default draft equals rate, replacing a different
        // displayed observation is an explicit local edit and must persist.
        cx.emit(MouseProductChanged);
        cx.notify();
    }

    pub(super) fn polling_226(&self, cx: &Context<Self>) -> AnyElement {
        if !self.polling_226_visible() {
            return div().into_any_element();
        }
        let field = self.polling_state.field();
        let wireless = field == PollingField::Wireless;
        let value = self.polling_value(field);
        let limit = self.polling_state.limit();
        let scope = self.mouse_polling_scope(cx).expect("226 polling scope");
        let rates = self
            .spec
            .rates
            .get(field.rates())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut panel = surface::panel_with_control(
            t(if wireless {
                "POLLING_RATE_HEADER"
            } else {
                "WIRED_POLLING_RATE_HEADER"
            }),
            surface::help_control(
                "mouse-226-polling-help",
                t(if wireless {
                    "POLLING_RATE_V2_TOOLTIP"
                } else {
                    "WIRED_POLLING_RATE_V2_TOOLTIP"
                }),
            ),
            cx,
        )
        .child(
            div()
                .mb(surface::css(10.))
                .text_color(rgb(0xcccccc))
                .child(t("POLLING_RATE_DESC")),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(surface::css(10.))
                .children(rates.iter().map(|rate| {
                    let rate = *rate;
                    let disabled = limit.is_some() && rate > 1000;
                    BaseButton::new(SharedString::from(format!("mouse-226-polling-{rate}")))
                        .accessibility_label(format!("{rate} Hz"))
                        .selected(value == rate)
                        .disabled(disabled)
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(surface::css(72.))
                        .h(surface::css(27.))
                        .p_0()
                        .rounded(surface::css(3.))
                        .text_size(surface::css(14.))
                        .bg(rgb(0x222222))
                        .text_color(rgb(0xcccccc))
                        .border_1()
                        .border_color(if value == rate {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x5d5d5d)
                        })
                        .when(disabled, |button| button.opacity(0.4))
                        .hover(|style| style.border_color(rgb(0x44d62c)))
                        .child(rate.to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.choose_polling(scope, field, rate, cx)
                        }))
                })),
        );
        if let Some(limit) = limit {
            panel = panel.child(
                h_flex()
                    .items_start()
                    .mt(surface::css(15.))
                    .text_color(rgb(0x999999))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(
                        img("synapse/mouse-226-polling-info.svg")
                            .size(surface::css(20.))
                            .mr(surface::css(5.))
                            .flex_shrink_0(),
                    )
                    .child(t(limit.label())),
            );
        }
        if limit.is_none() && value > 1000 {
            panel = panel.child(div().mt(surface::css(10.)).opacity(0.7)
                .child(t(if wireless { "POLLING_RATE_WARN" } else { "POLLING_RATE_WARN_NOBATTERY" }))
                .child(h_flex().id("mouse-226-polling-learn-more").pl(surface::css(5.)).underline().cursor_pointer()
                    .child(t("LEARN_MORE"))
                    .child(img("synapse/mouse-226-polling-external.svg").size(surface::css(16.)).ml(surface::css(5.)))
                    .on_click(|_, _, cx| cx.open_url("https://www.razer.com/technology/razer-hyperpolling#best-practices-tips"))));
        }
        if self.polling_state.connection.is_none() {
            panel = panel.child(surface::note(
                "连接状态尚未读取；当前编辑本地有线配置。",
                cx,
            ));
        } else if self.polling_state.local_fields.contains(field.key()) {
            panel = panel.child(surface::note("当前显示本地草稿；尚未发送到设备。", cx));
        } else if !self.polling_state.rates.contains_key(&field) {
            panel = panel.child(surface::note("当前显示本地配置；设备回报率尚未读取。", cx));
        }
        panel.into_any_element()
    }
}
