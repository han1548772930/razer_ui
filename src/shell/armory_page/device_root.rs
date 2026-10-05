//! Product-side `displayMode=armory` root, mounted inside the Armory window.
//!
//! The current Armory bundle embeds the product page at
//! `<origin>/synapse/products/<referenceUuid>/ui/index.html?displayMode=armory`
//! (`main.3d0e8bd0.js`, `oe`/`re`) and sizes the frame from the device tag's
//! type: `KEYPAD` 460px, `HEADSET`/`AUDIO` 340px, everything else 420px. The two
//! sides hand data over with `armoryIframeReady` (product → host),
//! `armoryMappings-<productId>` (host → product), `armory-button-list` and
//! `armory-change-viewIndex` (product → host), plus `hypershiftMode` when the
//! host's HyperShift toggle changes. Both sides are in this process here, so the
//! view mounts the audited mapping page directly and keeps the frame height; the
//! message names stay recorded rather than exchanged.
//!
//! See [display mode audit](../../docs/re/display-mode-audit.md) and
//! [display mode roots receipt](../../docs/re/display-mode-roots-audit.json).
use crate::{features::ProductWorkspace, model::DeviceCategory, ui::surface};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// `.iframe` height by device type from the current Armory bundle.
pub(super) fn device_frame_height(category: DeviceCategory) -> f32 {
    match category {
        // `case"KEYPAD": return "460px"`
        DeviceCategory::Keypad => 460.,
        // `case"HEADSET": case"AUDIO": return "340px"`
        DeviceCategory::Headset | DeviceCategory::Audio => 340.,
        // `default: return "420px"`
        _ => 420.,
    }
}

/// Product root for one device, as the Armory application mounts it.
pub(super) struct ArmoryDeviceRoot {
    workspace: Entity<ProductWorkspace>,
    _subscription: Subscription,
}

impl ArmoryDeviceRoot {
    /// `None` when the device has no local armory root; the caller keeps the
    /// entry disabled instead of fabricating one.
    pub(super) fn new(workspace: Entity<ProductWorkspace>, cx: &mut Context<Self>) -> Option<Self> {
        if !workspace.read(cx).has_armory_device_page(cx) {
            return None;
        }
        let subscription = cx.observe(&workspace, |_, _, cx| cx.notify());
        Some(Self {
            workspace,
            _subscription: subscription,
        })
    }

    pub(super) fn device_name(&self, cx: &App) -> String {
        self.workspace.read(cx).device(cx).display_name()
    }
}

impl Render for ArmoryDeviceRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let category = self.workspace.read(cx).device(cx).category;
        let height = device_frame_height(category);
        let page = self
            .workspace
            .update(cx, |workspace, cx| workspace.armory_device_page(window, cx));
        div()
            .id("armory-device-frame")
            .test_support()
            .w_full()
            .h(surface::css(height))
            .min_h(surface::css(height))
            .bg(rgb(0x111111))
            .overflow_hidden()
            .when_some(page, |view, page| view.child(page))
    }
}

#[cfg(test)]
#[path = "device_root_tests.rs"]
mod tests;
