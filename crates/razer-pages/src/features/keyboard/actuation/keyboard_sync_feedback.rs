//! Logical time for current ACTUATION Sync feedback (500/2000/2500ms).
use std::time::Duration;

#[derive(Default)]
pub(super) struct SyncFeedback {
    started: Option<Duration>,
    selected_all: bool,
    disable_at: Option<Duration>,
}
impl SyncFeedback {
    pub fn begin(&mut self, now: Duration, all: bool, has_mappings: bool) {
        self.started = Some(now);
        self.selected_all = all;
        if all && has_mappings {
            self.disable_at = Some(now + Duration::from_millis(2500));
        }
    }
    pub fn spinner(&self, now: Duration, all: bool) -> bool {
        self.selected_all == all
            && self.started.is_some_and(|start| {
                let elapsed = now.saturating_sub(start);
                elapsed >= Duration::from_millis(500) && elapsed < Duration::from_millis(2000)
            })
    }
    pub fn next(&self, now: Duration) -> Option<Duration> {
        self.started
            .into_iter()
            .flat_map(|start| {
                [
                    start + Duration::from_millis(500),
                    start + Duration::from_millis(2000),
                ]
            })
            .chain(self.disable_at)
            .filter(|deadline| *deadline > now)
            .min()
    }
    pub fn advance(&mut self, now: Duration) -> bool {
        if self
            .started
            .is_some_and(|start| now >= start + Duration::from_millis(2000))
        {
            self.started = None;
        }
        if self.disable_at.is_some_and(|deadline| now >= deadline) {
            self.disable_at = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spinner_and_disable_have_separate_deadlines() {
        let mut feedback = SyncFeedback::default();
        feedback.begin(Duration::ZERO, true, true);
        assert!(!feedback.spinner(Duration::from_millis(499), true));
        assert!(feedback.spinner(Duration::from_millis(500), true));
        assert!(!feedback.spinner(Duration::from_millis(500), false));
        assert!(!feedback.advance(Duration::from_millis(2000)));
        assert!(!feedback.spinner(Duration::from_millis(2000), true));
        assert_eq!(
            feedback.next(Duration::from_millis(2000)),
            Some(Duration::from_millis(2500))
        );
        assert!(feedback.advance(Duration::from_millis(2500)));
        assert_eq!(feedback.next(Duration::from_millis(2500)), None);
    }
    #[test]
    fn later_selected_sync_resets_flags_without_losing_all_disable() {
        let mut feedback = SyncFeedback::default();
        feedback.begin(Duration::ZERO, true, true);
        feedback.begin(Duration::from_millis(1000), false, false);
        assert!(!feedback.spinner(Duration::from_millis(1200), true));
        assert!(feedback.spinner(Duration::from_millis(1500), false));
        assert!(feedback.advance(Duration::from_millis(2500)));
        assert!(feedback.spinner(Duration::from_millis(2500), false));
        assert!(!feedback.advance(Duration::from_millis(3000)));
        assert!(!feedback.spinner(Duration::from_millis(3000), false));
    }
}
