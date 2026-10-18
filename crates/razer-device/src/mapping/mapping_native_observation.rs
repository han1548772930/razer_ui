//! Analog callback selection (80B48) and interval scheduling (81030/815C0).
//! Logical ticks are source-clock microseconds; this owner performs no I/O.
use std::collections::BTreeMap;

#[derive(Debug)]
struct Key {
    skip: u32,
    counter: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Delivery<T> {
    pub events: Vec<T>,
    pub time_tick: u64,
}
#[derive(Debug)]
pub struct State<T> {
    all: bool,
    interval: Option<u32>,
    default_skip: u32,
    keys: BTreeMap<u8, Key>,
    queued: Vec<T>,
    scheduled: bool,
    last_schedule: u64,
    deadlines: Vec<u64>,
}
impl<T> Default for State<T> {
    fn default() -> Self {
        Self {
            all: false,
            interval: None,
            default_skip: 0,
            keys: BTreeMap::new(),
            queued: Vec::new(),
            scheduled: false,
            last_schedule: 0,
            deadlines: Vec::new(),
        }
    }
}
impl<T> State<T> {
    /// 72840/72C20 return success without replacing an existing all-key mode.
    pub fn register_all(&mut self, skip: u32) {
        if !self.all {
            self.all = true;
            self.interval = None;
            self.default_skip = skip;
        }
    }
    pub fn register_interval(&mut self, milliseconds: u32) {
        if !self.all {
            self.all = true;
            self.interval = Some(milliseconds.wrapping_mul(1000));
        }
    }
    /// 737F0 emplace preserves the first registration's skip and counter.
    pub fn register_key(&mut self, key: u8, skip: u32) {
        self.keys.entry(key).or_insert(Key { skip, counter: 0 });
    }
    pub fn unregister_key(&mut self, key: u8) -> bool {
        self.keys.remove(&key).is_some()
    }
    /// Native unregister leaves an already scheduled batch intact. Callback
    /// lifetime cancellation belongs to the enclosing connection owner.
    pub fn unregister_all(&mut self) {
        self.all = false;
        self.keys.clear();
    }
    pub fn next_deadline(&self) -> Option<u64> {
        self.deadlines.iter().copied().min()
    }
    pub fn observe(
        &mut self,
        key: u8,
        force: bool,
        tick: u64,
        event: impl FnOnce() -> T,
    ) -> Option<Delivery<T>> {
        if let Some(interval) = self.interval {
            if !self.all {
                return None;
            }
            self.queued.push(event());
            if self.queued.len() >= 10_001 {
                return self.flush(tick);
            }
            if !self.scheduled {
                self.scheduled = true;
                let elapsed = tick.wrapping_sub(self.last_schedule);
                self.last_schedule = tick;
                if elapsed >= u64::from(interval) {
                    return self.flush(tick);
                }
                self.deadlines
                    .push(tick.saturating_add(u64::from(interval) - elapsed));
            }
            return None;
        }
        if self.all {
            self.register_key(key, self.default_skip);
        }
        let key = self.keys.get_mut(&key)?;
        let next = key.counter.wrapping_add(1);
        let overflow = key.skip < next;
        key.counter = if overflow { 0 } else { next };
        (overflow || force).then(|| Delivery {
            events: vec![event()],
            time_tick: tick,
        })
    }
    pub fn poll(&mut self, tick: u64) -> Vec<Delivery<T>> {
        let due = self
            .deadlines
            .iter()
            .filter(|deadline| **deadline <= tick)
            .count();
        self.deadlines.retain(|deadline| *deadline > tick);
        let mut deliveries = Vec::new();
        for _ in 0..due {
            if let Some(delivery) = self.flush(tick) {
                deliveries.push(delivery);
            }
        }
        deliveries
    }
    fn flush(&mut self, tick: u64) -> Option<Delivery<T>> {
        self.scheduled = false;
        if self.queued.is_empty() {
            None
        } else {
            Some(Delivery {
                events: std::mem::take(&mut self.queued),
                time_tick: tick,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forced_event_advances_skip_without_resetting_it() {
        let mut s = State::default();
        s.register_all(2);
        assert!(s.observe(1, false, 1, || 1).is_none());
        assert_eq!(s.observe(1, true, 2, || 2).unwrap().events, vec![2]);
        assert_eq!(s.observe(1, false, 3, || 3).unwrap().events, vec![3]);
        assert!(s.observe(1, false, 4, || 4).is_none());
        assert!(s.observe(2, false, 5, || 5).is_none());
    }
    #[test]
    fn duplicate_registration_preserves_skip_and_all_keeps_existing_keys() {
        let mut s = State::default();
        s.register_key(1, 1);
        s.register_key(1, 0);
        s.register_all(0);
        assert!(s.observe(1, false, 1, || 1).is_none());
        assert_eq!(s.observe(2, false, 2, || 2).unwrap().events, vec![2]);
        assert_eq!(s.observe(1, false, 3, || 3).unwrap().events, vec![3]);
    }
    #[test]
    fn interval_retains_all_samples_and_schedule_origin_is_not_flush_time() {
        let mut s = State::default();
        s.register_interval(10);
        assert_eq!(s.observe(1, false, 100_000, || 1).unwrap().events, vec![1]);
        assert!(s.observe(1, true, 102_000, || 2).is_none());
        assert!(s.observe(2, false, 103_000, || 3).is_none());
        assert_eq!(s.next_deadline(), Some(110_000));
        assert!(s.poll(109_999).is_empty());
        assert_eq!(
            s.poll(110_000),
            vec![Delivery {
                events: vec![2, 3],
                time_tick: 110_000
            }]
        );
        assert!(s.observe(1, false, 111_000, || 4).is_none());
        assert_eq!(s.next_deadline(), Some(112_000));
    }
    #[test]
    fn unregister_all_does_not_cancel_native_pending_callback() {
        let mut s = State::default();
        s.register_interval(10);
        assert!(s.observe(1, false, 2_000, || 1).is_none());
        s.unregister_all();
        assert!(s.observe(1, true, 3_000, || 2).is_none());
        assert_eq!(s.poll(10_000)[0].events, vec![1]);
    }
    #[test]
    fn capacity_flush_leaves_previously_scheduled_task_alive() {
        let mut s = State::default();
        s.register_interval(10);
        for n in 0..10_000 {
            assert!(s.observe(1, false, 1_000, || n).is_none());
        }
        assert_eq!(
            s.observe(1, false, 1_000, || 10_000).unwrap().events.len(),
            10_001
        );
        assert!(s.observe(1, false, 2_000, || 10_001).is_none());
        assert_eq!(s.next_deadline(), Some(10_000));
        assert_eq!(s.poll(10_000)[0].events, vec![10_001]);
        assert!(s.poll(11_000).is_empty());
    }
    #[test]
    fn maximum_skip_keeps_wrapping_counter_and_force_still_works() {
        let mut s = State::default();
        s.register_key(1, u32::MAX);
        assert!(s.observe(1, false, 1, || 1).is_none());
        assert_eq!(s.observe(1, true, 2, || 2).unwrap().events, vec![2]);
        assert!(s.observe(2, true, 3, || 3).is_none());
    }
}
