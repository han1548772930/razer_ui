//! The warning retains the source UI's previous-value state under the
//! trigger owner's lifetime.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn is_low(value: i64) -> bool {
    value < 7
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}

impl Side {
    pub(super) fn from_path(path: &str) -> Option<Self> {
        match path {
            "/controller/leftStick/deadzoneValue" => Some(Self::Left),
            "/controller/rightStick/deadzoneValue" => Some(Self::Right),
            _ => None,
        }
    }

    pub(super) fn path(self) -> &'static str {
        match self {
            Self::Left => "/controller/leftStick/deadzoneValue",
            Self::Right => "/controller/rightStick/deadzoneValue",
        }
    }

    pub(super) fn part(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    side: Side,
    value: i64,
}

impl Selection {
    pub(super) fn path(self) -> &'static str {
        self.side.path()
    }

    pub(super) fn value(self) -> i64 {
        self.value
    }
}

#[derive(Default)]
pub(super) struct DeadzoneState {
    previous: Option<[i64; 2]>,
    owner: Option<Arc<AtomicBool>>,
    warning: Option<Side>,
}

impl DeadzoneState {
    pub(super) fn enter(&mut self, values: [i64; 2], owner: Arc<AtomicBool>) {
        self.previous = Some(values);
        self.owner = Some(owner);
        self.warning = None;
    }

    fn matches_owner(&self, current: &Arc<AtomicBool>) -> bool {
        self.owner.as_ref().is_some_and(|owner| {
            Arc::ptr_eq(owner, current)
                && owner.load(Ordering::Acquire)
                && current.load(Ordering::Acquire)
        })
    }

    pub(super) fn synchronize(
        &mut self,
        displayed: [i64; 2],
        follows_displayed_values: bool,
        owner: Arc<AtomicBool>,
    ) {
        if !self.matches_owner(&owner) {
            self.enter(displayed, owner);
        } else if follows_displayed_values {
            // The 2676/2684 widget effects update each previous value when
            // its displayed value becomes nonlow. Legacy classes only update
            // their previous values from the selection callback.
            let previous = self.previous.as_mut().expect("mounted deadzone values");
            for (previous, value) in previous.iter_mut().zip(displayed) {
                if !is_low(value) {
                    *previous = value;
                }
            }
        }
    }

    pub(super) fn select(
        &mut self,
        side: Side,
        value: i64,
        displayed: [i64; 2],
        owner: Arc<AtomicBool>,
    ) -> bool {
        if !self.matches_owner(&owner) {
            self.enter(displayed, owner);
        }
        let previous = self.previous.as_mut().expect("mounted deadzone values");
        if is_low(value) {
            self.warning = Some(side);
            true
        } else {
            previous[side.index()] = value;
            false
        }
    }

    pub(super) fn warning_side(&self) -> Option<Side> {
        self.warning
    }

    pub(super) fn dismiss(
        &mut self,
        restore_previous: bool,
        current: &Arc<AtomicBool>,
    ) -> Option<Selection> {
        let side = self.warning.take()?;
        // Source onClose always dispatches the previous value, including an
        // equal value. Continue, Recalibrate and unmount never restore it.
        (restore_previous && self.matches_owner(current)).then(|| Selection {
            side,
            value: self.previous.expect("mounted deadzone values")[side.index()],
        })
    }
}
