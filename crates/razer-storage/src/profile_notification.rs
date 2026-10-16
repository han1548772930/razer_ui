//! Current StreamMixer 44857.w/P and notification-manager 5568.J/H/W.
//! These are process-local WindowStorage records, never persisted device saves.
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const STORAGE_KEY: &str = "RzWinStorage_notification_manager";
pub const PROFILE_TYPE: &str = "switch-profile";
pub const TIMEOUT_MS: u64 = 3_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Notification(Value);

impl Notification {
    pub fn stream_mixer(
        container_id: String,
        product_name: String,
        preset_name: String,
        timer_tick: Value,
        available_left: f64,
    ) -> Self {
        Self(json!({
            "id":container_id,"type":PROFILE_TYPE,"appName":"synapse",
            "header":"Switch Profile","footer":product_name,"persistent":true,
            "timeout":TIMEOUT_MS,"timerTick":timer_tick,
            "payload":{"title":preset_name,"avaiLeft":available_left}
        }))
    }
    pub fn id(&self) -> &Value {
        &self.0["id"]
    }
    pub fn kind(&self) -> &str {
        self.0["type"].as_str().unwrap_or("")
    }
    pub fn key(&self) -> String {
        format!("{}-{}", js_string(self.id()), self.kind())
    }
    pub fn title(&self) -> &str {
        self.0["payload"]["title"].as_str().unwrap_or("")
    }
    pub fn footer(&self) -> &str {
        self.0["footer"].as_str().unwrap_or("")
    }
    pub fn timer_tick(&self) -> &Value {
        &self.0["timerTick"]
    }
    pub fn timeout_ms(&self) -> u64 {
        self.0["timeout"].as_u64().unwrap_or(0)
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
    /// 2864.u deliberately reads record.screen, not payload.avaiLeft.
    pub fn screen_point(&self) -> (f64, f64) {
        (
            self.0["screen"]["availLeft"].as_f64().unwrap_or(0.),
            self.0["screen"]["availTop"].as_f64().unwrap_or(0.),
        )
    }
    fn identity(&self, other: &Self) -> bool {
        strict_equal(self.id(), other.id()) && self.kind() == other.kind()
    }
    fn same_revision(&self, other: &Self) -> bool {
        self.identity(other) && strict_equal(self.timer_tick(), other.timer_tick())
    }
}

#[derive(Clone)]
struct Timer {
    serial: u64,
    deadline: u64,
    notification: Notification,
}

#[derive(Default)]
pub struct Producer {
    notifications: Vec<Notification>,
    timers: Vec<Timer>,
    own_handles: BTreeMap<String, u64>,
    serial: u64,
}
impl Producer {
    /// The helper's timeout is replaced; the caller's second timeout survives.
    pub fn publish(&mut self, notification: Notification, now: u64) -> String {
        let key = notification.key();
        if let Some(serial) = self.own_handles.remove(&key) {
            self.timers.retain(|timer| timer.serial != serial);
        }
        if let Some(current) = self
            .notifications
            .iter_mut()
            .find(|item| item.identity(&notification))
        {
            *current = notification.clone();
        } else {
            self.notifications.push(notification.clone());
        }
        self.serial += 1;
        self.own_handles.insert(key, self.serial);
        self.timers.push(Timer {
            serial: self.serial,
            deadline: now.saturating_add(TIMEOUT_MS),
            notification: notification.clone(),
        });
        self.serial += 1;
        self.timers.push(Timer {
            serial: self.serial,
            deadline: now.saturating_add(TIMEOUT_MS),
            notification,
        });
        self.encoded()
    }
    fn encoded(&self) -> String {
        Value::Array(
            self.notifications
                .iter()
                .map(|item| item.0.clone())
                .collect(),
        )
        .to_string()
    }
    /// Each P call writes while the source owner's array is nonempty, including
    /// a no-op filter. It does not cancel the helper's later timer.
    pub fn advance(&mut self, now: u64) -> Vec<String> {
        let mut expired = Vec::new();
        self.timers
            .sort_by_key(|timer| (timer.deadline, timer.serial));
        while self
            .timers
            .first()
            .is_some_and(|timer| timer.deadline <= now)
        {
            let timer = self.timers.remove(0);
            if self.own_handles.get(&timer.notification.key()) == Some(&timer.serial) {
                self.own_handles.remove(&timer.notification.key());
            }
            if !self.notifications.is_empty() {
                self.notifications
                    .retain(|item| !item.identity(&timer.notification));
                expired.push(self.encoded());
            }
        }
        expired
    }
    pub fn next_deadline(&self) -> Option<u64> {
        self.timers.iter().map(|timer| timer.deadline).min()
    }
}

#[derive(Default)]
pub struct Consumer {
    notifications: Vec<Notification>,
    timers: Vec<Timer>,
    handles: BTreeMap<String, u64>,
    serial: u64,
    always_on_top: bool,
}
impl Consumer {
    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }
    pub fn always_on_top(&self) -> bool {
        self.always_on_top
    }
    pub fn next_deadline(&self) -> Option<u64> {
        self.timers.iter().map(|timer| timer.deadline).min()
    }
    fn clear_timer(&mut self, item: &Notification) {
        if let Some(serial) = self.handles.remove(&item.key()) {
            self.timers.retain(|timer| timer.serial != serial);
        }
    }
    fn schedule(&mut self, item: &Notification, now: u64) {
        if item.timeout_ms() == 0 {
            return;
        }
        self.serial += 1;
        self.handles.insert(item.key(), self.serial);
        self.timers.push(Timer {
            serial: self.serial,
            deadline: now.saturating_add(item.timeout_ms()),
            notification: item.clone(),
        });
    }
    fn accept(
        &mut self,
        item: &Notification,
        now: u64,
        clear_all: bool,
        fullscreen: &mut impl FnMut() -> Result<bool, String>,
    ) -> Result<(), String> {
        let current = self.notifications.iter().position(|old| old.identity(item));
        if clear_all {
            self.clear_timer(item);
        }
        let mut failure = None;
        match current {
            Some(index) if js_less(self.notifications[index].timer_tick(), item.timer_tick()) => {
                if !clear_all {
                    self.clear_timer(item);
                }
                self.notifications[index] = item.clone();
            }
            Some(_) => {}
            None => match fullscreen() {
                Ok(false) => {
                    self.notifications.push(item.clone());
                    self.always_on_top = true;
                }
                Ok(true) => {}
                Err(error) => failure = Some(error),
            },
        }
        // H is independent of the asynchronous SHOW thunk, even when skipped
        // in fullscreen or when its platform query rejects.
        self.schedule(item, now);
        failure.map_or(Ok(()), Err)
    }
    pub fn close(&mut self, item: &Notification) {
        // O's truthiness guard also retains the original empty-container quirk.
        if !js_truthy(item.id()) || item.kind().is_empty() {
            return;
        }
        self.notifications.retain(|old| !old.identity(item));
        self.always_on_top = false;
    }
    pub fn advance(&mut self, now: u64) {
        self.timers
            .sort_by_key(|timer| (timer.deadline, timer.serial));
        while self
            .timers
            .first()
            .is_some_and(|timer| timer.deadline <= now)
        {
            let timer = self.timers.remove(0);
            self.close(&timer.notification);
            // An older orphaned H callback deletes the newest map handle too.
            self.handles.remove(&timer.notification.key());
        }
    }
    /// Consume the actual WindowStorage event and its aggregate getter result.
    /// Only the mounted switch-profile branch is decoded by this owner.
    pub fn consume(
        &mut self,
        event: &Value,
        aggregate: &Value,
        now: u64,
        mut fullscreen: impl FnMut() -> Result<bool, String>,
    ) -> Result<(), String> {
        if event["key"] != STORAGE_KEY {
            return Ok(());
        }
        let new = parse_notifications(&event["newValue"]);
        let old = parse_notifications(&event["oldValue"]);
        let state = event["keyState"].as_str().unwrap_or("");
        if matches!(state, "created" | "changed") {
            if new.len() < old.len() {
                for item in old.iter().filter(|item| {
                    item.kind() == PROFILE_TYPE && !new.iter().any(|new| new.same_revision(item))
                }) {
                    // Current J's shortened-array branch JSON.parse's each
                    // aggregate ROW rather than row.value; the host returns
                    // objects, so that exception means "absent".
                    if absent_tick(aggregate, item.timer_tick(), false)
                        && !self.handles.contains_key(&item.key())
                    {
                        self.close(item);
                    }
                }
            } else {
                let mut failure = None;
                for item in new.iter().filter(|item| {
                    item.kind() == PROFILE_TYPE && !old.iter().any(|old| old.same_revision(item))
                }) {
                    if let Err(error) =
                        self.accept(item, now, new.len() > old.len(), &mut fullscreen)
                    {
                        failure = Some(error);
                    }
                }
                if let Some(error) = failure {
                    return Err(error);
                }
            }
        } else if matches!(state, "removed" | "cleared") {
            for item in old.iter().filter(|item| {
                item.kind() == PROFILE_TYPE && !new.iter().any(|new| new.same_revision(item))
            }) {
                self.clear_timer(item);
                if (js_truthy(&item.0["persistent"]) || state == "removed")
                    && absent_tick(aggregate, item.timer_tick(), true)
                {
                    self.close(item);
                }
            }
        }
        Ok(())
    }
    pub fn restore(
        &mut self,
        aggregate: &Value,
        now: u64,
        mut fullscreen: impl FnMut() -> Result<bool, String>,
    ) -> Result<(), String> {
        let rows = parse_json(aggregate)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let mut failure = None;
        for row in rows {
            for item in parse_notifications(&row["value"]) {
                // StreamMixer w always supplies persistent:true.
                if item.kind() == PROFILE_TYPE && js_truthy(&item.0["persistent"]) {
                    if let Err(error) = self.accept(&item, now, true, &mut fullscreen) {
                        failure = Some(error);
                    }
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

fn parse_json(value: &Value) -> Option<Value> {
    value
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
}
fn parse_notifications(value: &Value) -> Vec<Notification> {
    parse_json(value)
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .map(Notification)
        .collect()
}
fn absent_tick(aggregate: &Value, tick: &Value, row_value: bool) -> bool {
    let Some(rows) = parse_json(aggregate).and_then(|value| value.as_array().cloned()) else {
        return true;
    };
    for row in rows {
        let encoded = if row_value { &row["value"] } else { &row };
        let Some(items) = parse_json(encoded).and_then(|value| value.as_array().cloned()) else {
            return true;
        };
        if items
            .iter()
            .any(|item| strict_equal(&item["timerTick"], tick))
        {
            return false;
        }
    }
    true
}
fn strict_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        _ => left == right,
    }
}
fn js_less(left: &Value, right: &Value) -> bool {
    if let (Some(left), Some(right)) = (left.as_str(), right.as_str()) {
        return left < right;
    }
    js_number(left)
        .zip(js_number(right))
        .is_some_and(|(left, right)| left < right)
}
fn js_number(value: &Value) -> Option<f64> {
    match value {
        Value::Null => Some(0.),
        Value::Bool(value) => Some(u8::from(*value) as f64),
        Value::Number(value) => value.as_f64(),
        Value::String(value) => {
            if value.trim().is_empty() {
                Some(0.)
            } else {
                value.parse().ok()
            }
        }
        _ => None,
    }
}
fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}
fn js_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, tick: u64) -> Notification {
        Notification::stream_mixer(
            id.into(),
            "Razer Seiren V3 Chroma".into(),
            format!("Preset {tick}"),
            json!(tick),
            1920.,
        )
    }
    fn event(state: &str, new: &[Notification], old: &[Notification]) -> Value {
        let encode = |items: &[Notification]| {
            Value::Array(items.iter().map(|item| item.0.clone()).collect()).to_string()
        };
        json!({"key":STORAGE_KEY,"keyState":state,"newValue":encode(new),"oldValue":encode(old)})
    }
    fn aggregate(items: &[Notification]) -> Value {
        Value::String(
            json!([{"windowName":"device","value":Value::Array(items.iter().map(|item|item.0.clone()).collect()).to_string()}])
                .to_string(),
        )
    }
    #[test]
    fn caller_timeout_can_remove_a_more_recent_source_record() {
        let mut producer = Producer::default();
        producer.publish(item("a", 1), 0);
        producer.publish(item("a", 2), 1000);
        assert_eq!(producer.advance(2999), Vec::<String>::new());
        assert_eq!(producer.advance(3000), ["[]"]);
        assert!(producer.advance(4000).is_empty());
    }
    #[test]
    fn newer_tick_updates_without_fullscreen_check_and_resets_consumer_timer() {
        let (a, b) = (item("a", 1), item("a", 2));
        let mut consumer = Consumer::default();
        consumer
            .consume(
                &event("created", &[a.clone()], &[]),
                &aggregate(&[a.clone()]),
                0,
                || Ok(false),
            )
            .unwrap();
        consumer
            .consume(
                &event("changed", &[b.clone()], &[a]),
                &aggregate(&[b.clone()]),
                1000,
                || panic!("UPDATE never queries fullscreen"),
            )
            .unwrap();
        consumer.advance(3000);
        assert_eq!(consumer.notifications(), [b]);
        consumer.advance(4000);
        assert!(consumer.notifications().is_empty());
    }
    #[test]
    fn fullscreen_skips_new_show_but_keeps_h_timeout() {
        let a = item("a", 1);
        let mut consumer = Consumer::default();
        consumer
            .consume(&event("created", &[a], &[]), &json!("[]"), 0, || Ok(true))
            .unwrap();
        assert!(consumer.notifications().is_empty());
        assert_eq!(consumer.next_deadline(), Some(3000));
        consumer.advance(3000);
        assert_eq!(consumer.next_deadline(), None);
    }
    #[test]
    fn shortened_source_array_does_not_close_while_h_is_pending() {
        let a = item("a", 1);
        let mut consumer = Consumer::default();
        consumer
            .consume(
                &event("created", &[a.clone()], &[]),
                &aggregate(&[a.clone()]),
                0,
                || Ok(false),
            )
            .unwrap();
        consumer
            .consume(
                &event("changed", &[], &[a.clone()]),
                &aggregate(&[a.clone()]),
                500,
                || Ok(false),
            )
            .unwrap();
        assert_eq!(consumer.notifications(), [a]);
        consumer.advance(3000);
        assert!(consumer.notifications().is_empty());
    }
    #[test]
    fn explicit_removal_checks_tick_across_all_owners() {
        let a = item("a", 1);
        let mut consumer = Consumer::default();
        consumer
            .consume(
                &event("created", &[a.clone()], &[]),
                &aggregate(&[a.clone()]),
                0,
                || Ok(false),
            )
            .unwrap();
        consumer
            .consume(
                &event("removed", &[], &[a.clone()]),
                &aggregate(&[a.clone()]),
                100,
                || Ok(false),
            )
            .unwrap();
        assert_eq!(consumer.notifications(), [a.clone()]);
        assert_eq!(consumer.next_deadline(), None);
        consumer
            .consume(&event("removed", &[], &[a]), &json!("[]"), 100, || {
                Ok(false)
            })
            .unwrap();
        assert!(consumer.notifications().is_empty());
    }
    #[test]
    fn out_of_order_update_keeps_orphan_timer_that_closes_newer_item() {
        let (a, b) = (item("a", 2), item("a", 1));
        let mut consumer = Consumer::default();
        consumer
            .consume(
                &event("created", &[a.clone()], &[]),
                &json!("[]"),
                0,
                || Ok(false),
            )
            .unwrap();
        consumer
            .consume(
                &event("changed", &[b], &[a.clone()]),
                &json!("[]"),
                1000,
                || Ok(false),
            )
            .unwrap();
        assert_eq!(consumer.notifications(), [a]);
        consumer.advance(3000);
        assert!(consumer.notifications().is_empty());
        assert_eq!(consumer.next_deadline(), Some(4000));
    }
    #[test]
    fn restoration_is_persistent_and_placement_ignores_payload_available_left() {
        let a = item("a", 1);
        assert_eq!(a.screen_point(), (0., 0.));
        let mut consumer = Consumer::default();
        consumer
            .restore(&aggregate(&[a.clone()]), 100, || Ok(false))
            .unwrap();
        assert_eq!(consumer.notifications(), [a]);
        assert_eq!(consumer.next_deadline(), Some(3100));
    }
    #[test]
    fn empty_container_retains_source_close_truthiness_quirk() {
        let a = item("", 1);
        let mut consumer = Consumer::default();
        consumer
            .consume(
                &event("created", &[a.clone()], &[]),
                &json!("[]"),
                0,
                || Ok(false),
            )
            .unwrap();
        consumer.advance(3000);
        assert_eq!(consumer.notifications(), [a]);
    }
}
