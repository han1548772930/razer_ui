//! Original SetTimer(NULL,0,2000,TimerFunc), on the retained calling thread.
use super::keyboard_layout;
use anyhow::{Result, ensure};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::c_void,
    marker::PhantomData,
    ptr::null_mut,
    rc::Rc,
    thread::{self, ThreadId},
};

#[link(name = "user32")]
unsafe extern "system" {
    fn SetTimer(
        window: *mut c_void,
        id: usize,
        interval: u32,
        callback: Option<unsafe extern "system" fn(*mut c_void, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(window: *mut c_void, id: usize) -> i32;
}

struct State {
    previous: i32,
    subscribers: BTreeSet<String>,
    events: VecDeque<(BTreeSet<String>, i32)>,
}

thread_local! { static TIMERS: RefCell<BTreeMap<usize, State>> = const { RefCell::new(BTreeMap::new()) }; }

unsafe extern "system" fn tick(_: *mut c_void, _: u32, timer_id: usize, _: u32) {
    // The original callback queries this same message-pumped thread. Never
    // substitute GetKeyboardLayout(GetForegroundWindow's thread).
    let observed = keyboard_layout::get();
    TIMERS.with(|timers| {
        if let Some(timer) = timers.borrow_mut().get_mut(&timer_id) {
            if observed != timer.previous {
                timer.previous = observed;
                timer
                    .events
                    .push_back((timer.subscribers.clone(), observed));
            }
        }
    });
}

pub(crate) struct Timer {
    id: usize,
    owner: ThreadId,
    pending: BTreeMap<String, Vec<i32>>,
    // Moving to an IPC reader/background thread would change the query and
    // prevent this HWND-less timer from receiving its callback.
    _ui_thread: PhantomData<Rc<()>>,
}

impl Timer {
    pub(crate) fn start() -> Result<Self> {
        let baseline = keyboard_layout::get();
        let id = unsafe { SetTimer(null_mut(), 0, 2000, Some(tick)) };
        ensure!(id != 0, "SysUtilsNative keyboard-layout SetTimer failed");
        TIMERS.with(|timers| {
            timers.borrow_mut().insert(
                id,
                State {
                    previous: baseline,
                    subscribers: BTreeSet::new(),
                    events: VecDeque::new(),
                },
            );
        });
        Ok(Self {
            id,
            owner: thread::current().id(),
            pending: BTreeMap::new(),
            _ui_thread: PhantomData,
        })
    }

    pub(crate) fn check_owner(&self) -> Result<()> {
        ensure!(
            self.owner == thread::current().id(),
            "keyboard-layout monitor must stay on its message-pumped owner thread"
        );
        Ok(())
    }

    pub(crate) fn set_subscribers(&self, subscribers: &BTreeSet<String>) -> Result<()> {
        self.check_owner()?;
        TIMERS.with(|timers| {
            if let Some(timer) = timers.borrow_mut().get_mut(&self.id) {
                timer.subscribers = subscribers.clone();
            }
        });
        Ok(())
    }

    pub(crate) fn drain(
        &mut self,
        view_url: &str,
        subscribers: &BTreeSet<String>,
    ) -> Result<Vec<i32>> {
        self.check_owner()?;
        let events: Vec<_> = TIMERS.with(|timers| {
            timers
                .borrow_mut()
                .get_mut(&self.id)
                .map(|timer| timer.events.drain(..).collect())
                .unwrap_or_default()
        });
        self.pending.retain(|url, _| subscribers.contains(url));
        for (audience, layout) in events {
            for url in audience.intersection(subscribers) {
                self.pending.entry(url.clone()).or_default().push(layout);
            }
        }
        Ok(self.pending.remove(view_url).unwrap_or_default())
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        self.check_owner()?;
        if self.id != 0 {
            let id = std::mem::take(&mut self.id);
            // Source always clears its ID even when KillTimer fails. Retired
            // timer callbacks cannot use freed state after the entry removal.
            let result = unsafe { KillTimer(null_mut(), id) };
            TIMERS.with(|timers| {
                timers.borrow_mut().remove(&id);
            });
            ensure!(
                result != 0,
                "SysUtilsNative keyboard-layout KillTimer failed"
            );
        }
        Ok(())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
