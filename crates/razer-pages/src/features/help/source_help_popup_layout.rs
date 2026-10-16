//! Current Eo lifecycle: retained hidden measurement, 100 ms reserve and
//! active-change body scroll. Geometry is measured in the requesting window.
use super::*;
use std::{cell::Cell, rc::Rc};

pub(crate) struct HelpScrollRequested {
    pub distance: Pixels,
}
impl EventEmitter<HelpScrollRequested> for SourceHelp {}

#[derive(Default)]
pub(super) struct State {
    pub help_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub popup_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub unit: Rc<Cell<Pixels>>,
    pub viewport_height: Rc<Cell<Pixels>>,
    pub reserved_height: Option<Pixels>,
    mount_generation: u64,
    reserve_timer: Option<Task<()>>,
    overflow_timer: Option<Task<()>>,
}
impl State {
    pub(super) fn unmount(&mut self) {
        self.mount_generation = self.mount_generation.wrapping_add(1);
        self.reserve_timer = None;
        self.overflow_timer = None;
        self.reserved_height = None;
        self.help_bounds.set(Bounds::default());
        self.popup_bounds.set(Bounds::default());
    }
}
impl SourceHelp {
    pub(super) fn mount_inline_popup_layout(&mut self, cx: &mut Context<Self>) {
        self.popup_layout.unmount();
        if !self.metadata().is_some_and(|(_, page)| {
            page.inline_confirmation
                && page.popup_kind.as_deref() != Some("legacy-delete")
                && !(matches!(page.popup_kind.as_deref(), Some("oled" | "audio-oled"))
                    && !self.observed_ble.unwrap_or(self.device.use_ble))
        }) {
            return;
        }
        let generation = self.popup_layout.mount_generation;
        // The source's initial-active 50 ms branch is separate from later
        // opening. Do not run it for a hidden Eo mounted with active=false.
        if self.inline_reset {
            self.popup_layout.overflow_timer = Some(cx.spawn(async move |view, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let _ = view.update(cx, |this, cx| {
                    if this.popup_layout.mount_generation != generation {
                        return;
                    }
                    let popup = this.popup_layout.popup_bounds.get();
                    let overflow = popup.bottom() - this.popup_layout.viewport_height.get();
                    if overflow > px(0.) {
                        let height =
                            this.popup_layout.help_bounds.get().size.height + popup.size.height;
                        this.popup_layout.reserved_height = Some(height);
                        cx.emit(HelpScrollRequested {
                            distance: overflow + this.popup_layout.unit.get() * 20.,
                        });
                        cx.notify();
                    }
                });
            }));
        }
        if self.metadata().is_some_and(|(_, page)| {
            matches!(
                page.popup_kind.as_deref(),
                Some("plain-delete" | "normal-remove" | "mixer-remove" | "firmware-reset")
            )
        }) {
            return;
        }
        self.popup_layout.reserve_timer = Some(cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.popup_layout.mount_generation != generation {
                    return;
                }
                let popup = this.popup_layout.popup_bounds.get();
                let help = this.popup_layout.help_bounds.get();
                if popup.size.height <= px(0.) || help.size.height <= px(0.) {
                    return;
                }
                // Eo's literal 30 + 10 belongs to the original logical-pixel
                // scale; account for native rem zoom without guessing height.
                let base = this
                    .popup_layout
                    .reserved_height
                    .unwrap_or(help.size.height);
                this.popup_layout.reserved_height =
                    Some(base + popup.size.height - this.popup_layout.unit.get() * 40.);
                cx.notify();
            });
        }));
    }
    pub(super) fn activate_inline_popup_layout(&self, cx: &mut Context<Self>) {
        if self.metadata().is_some_and(|(_, page)| {
            matches!(
                page.popup_kind.as_deref(),
                Some("legacy-delete" | "plain-delete" | "normal-remove" | "mixer-remove")
            ) || matches!(page.popup_kind.as_deref(), Some("oled" | "audio-oled"))
                && !self.observed_ble.unwrap_or(self.device.use_ble)
        }) {
            return;
        }
        let distance = self.popup_layout.help_bounds.get().size.height;
        let generation = self.confirmation_generation;
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.inline_reset && this.confirmation_generation == generation {
                    cx.emit(HelpScrollRequested { distance });
                }
            });
        });
    }
}
