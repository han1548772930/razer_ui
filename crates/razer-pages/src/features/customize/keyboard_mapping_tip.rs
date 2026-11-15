//! The 653 image owns the source hover target and cancels deferred work on navigation.
use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub(super) struct State {
    generation: u64,
    target: Option<String>,
    pending: Option<String>,
    regions: BTreeMap<String, Rc<Cell<Bounds<Pixels>>>>,
    media_shown: bool,
    media_bounds: Rc<Cell<Bounds<Pixels>>>,
}
impl State {
    pub(super) fn new(layout: u32) -> Self {
        Self {
            regions: razer_assets::keyboard_keys_for_layout(layout)
                .iter()
                .map(|key| (key.id.clone(), Rc::default()))
                .chain(["ScrollUp", "ScrollDown"].map(|id| (id.into(), Rc::default())))
                .collect(),
            ..Self::default()
        }
    }
    pub(super) fn replace_layout(&mut self, layout: u32) {
        self.close();
        self.regions = Self::new(layout).regions;
    }
    pub(super) fn close(&mut self) {
        self.reset();
        self.media_shown = false;
    }
    pub(super) fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.target = None;
        self.pending = None;
    }
}
impl DeviceWorkspace {
    pub(in crate::features) fn keyboard_key_tip_region(
        &self,
        input: &str,
    ) -> Option<Rc<Cell<Bounds<Pixels>>>> {
        self.keyboard_key_tip.regions.get(input).cloned()
    }
    pub(in crate::features) fn hover_keyboard_key_tip(
        &mut self,
        input: &str,
        entered: bool,
        cx: &mut Context<Self>,
    ) {
        if self.pid() != 653 || self.page != Tab::Customize {
            return;
        }
        if input == "DIAL_CLICK" {
            if entered && !self.keyboard_key_tip.media_shown {
                self.keyboard_key_tip.reset();
                self.keyboard_key_tip.media_shown = true;
                cx.notify();
            }
            return;
        }
        if !entered {
            if self.keyboard_key_tip.target.as_deref() == Some(input)
                || self.keyboard_key_tip.pending.as_deref() == Some(input)
            {
                self.keyboard_key_tip.reset();
                cx.notify();
            }
            return;
        }
        if self.keyboard_key_tip.target.as_deref() == Some(input)
            || self.keyboard_key_tip.pending.as_deref() == Some(input)
        {
            return;
        }
        self.keyboard_key_tip.reset();
        // _M.hoverBtn uses setTimeout(0); navigation invalidates that callback.
        self.keyboard_key_tip.pending = Some(input.into());
        let generation = self.keyboard_key_tip.generation;
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.keyboard_key_tip.generation == generation {
                    this.keyboard_key_tip.target = this.keyboard_key_tip.pending.take();
                    cx.notify();
                }
            });
        });
    }
    pub(in crate::features) fn keyboard_mapping_key_tip(&self) -> Option<AnyElement> {
        if self.pid() != 653 || self.page != Tab::Customize {
            return None;
        }
        let input = self.keyboard_key_tip.target.as_deref()?;
        let anchor = self.keyboard_key_tip.regions.get(input)?.clone();
        let tip = self.profile_mapping_key_tip(input)?;
        let keyboard = &self.settings().keyboard;
        Some(tip.render(
            self.hypershift,
            keyboard.gaming,
            keyboard.in_game,
            anchor,
            self.customize_drawer.body_scroll(),
        ))
    }
}

#[path = "keyboard_media_tip.rs"]
mod media;

#[cfg(test)]
#[path = "keyboard_mapping_tip_tests.rs"]
mod tests;
