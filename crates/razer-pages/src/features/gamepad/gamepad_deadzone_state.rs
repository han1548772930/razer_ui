//! The warning remembers the mounted control's previous value, independently
//! of device observations. Its scope is borrowed from the trigger owner.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

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
    warning: Option<Selection>,
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
        if value < 7 {
            self.warning = Some(Selection {
                side,
                value: previous[side.index()],
            });
            true
        } else {
            previous[side.index()] = value;
            false
        }
    }

    pub(super) fn warning_side(&self) -> Option<Side> {
        self.warning.map(|warning| warning.side)
    }

    pub(super) fn dismiss(
        &mut self,
        restore_previous: bool,
        current: &Arc<AtomicBool>,
    ) -> Option<Selection> {
        let previous = self.warning.take()?;
        // Source onClose always dispatches the previous value, including an
        // equal value. Continue, Recalibrate and unmount never restore it.
        (restore_previous && self.matches_owner(current)).then_some(previous)
    }
}
