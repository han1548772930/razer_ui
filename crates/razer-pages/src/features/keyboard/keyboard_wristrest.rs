//! Current PID653 Customize km wrist node. Device observations are transient,
//! independent of profile drafts and local persistence.
use gpui_kit::*;
use razer_device::keyboard_wristrest::Observation;
use razer_widgets::surface;

#[derive(Default)]
pub(in crate::features) struct State {
    observed: Option<Observation>,
    mounted: bool,
    animation_revision: u64,
}

impl State {
    pub(in crate::features) fn observe(&mut self, observation: Option<Observation>) -> bool {
        if self.observed == observation {
            return false;
        }
        let previous = self.observed.map(Observation::connected);
        let next = observation.map(Observation::connected);
        if next == Some(true) || previous.zip(next).is_some_and(|(a, b)| a != b) {
            self.mounted = true;
        }
        if previous != next {
            self.animation_revision = self.animation_revision.wrapping_add(1);
        }
        self.observed = observation;
        true
    }

    pub(in crate::features) fn is_mounted(&self) -> bool {
        self.mounted
    }

    pub(in crate::features) fn render(&self, edition_id: u32, owner: &str, cx: &App) -> AnyElement {
        let white = edition_id == 130;
        let (asset, width, margin, top) = if white {
            (
                "synapse/shared/keyboard-653-wristrest-white.png",
                710.,
                -8.,
                276.,
            )
        } else {
            ("synapse/shared/keyboard-653-wristrest.png", 746., 14., 278.)
        };
        // Current main.5425442a: config-block margin 6px; wrist has centered
        // static-position anchor, margin-left, exact dimensions and translateY.
        let node = div()
            .id("keyboard-653-wristrest")
            .absolute()
            .left(relative(0.5))
            .ml(surface::css(margin - width / 2.))
            .top(surface::css(top + 6.))
            .w(surface::css(width))
            .h(surface::css(110.))
            .child(img(asset).size_full().object_fit(ObjectFit::Fill));
        let Some(connected) = self.observed.map(Observation::connected) else {
            return node.opacity(0.).into_any_element();
        };
        if cx.reduce_motion() {
            return node
                .opacity(if connected { 1. } else { 0. })
                .into_any_element();
        }
        node.with_animation(
            SharedString::from(format!(
                "keyboard-653-wristrest-{owner}-{}",
                self.animation_revision
            )),
            Animation::new(std::time::Duration::from_millis(200))
                .with_easing(|phase| gpui_kit::base::motion::Easing::EaseOut.sample(phase)),
            move |node, phase| {
                let opacity = if connected { phase } else { 1. - phase };
                node.opacity(opacity)
                    .top(surface::css(top + 6. + 5. * (1. - opacity)))
            },
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn source_node_mounts_on_first_boolean_change_and_stays_mounted() {
        let mut state = State::default();
        state.observe(Some(Observation::initial(2)));
        assert!(!state.mounted);
        assert_eq!(state.observed.map(Observation::connected), Some(false));
        state.observe(Some(Observation::notification(2)));
        assert!(state.mounted);
        assert_eq!(state.observed.map(Observation::connected), Some(true));
        state.observe(Some(Observation::notification(0)));
        assert!(state.mounted);
        assert_eq!(state.observed.map(Observation::connected), Some(false));
        state.observe(None);
        assert!(state.mounted);
        assert_eq!(state.observed.map(Observation::connected), None);
    }
}
