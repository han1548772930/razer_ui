//! Extra source pages retain their own editors while lighting stays in its
//! existing owner. Hardware layout drafts belong to the device; automation
//! preferences follow their own source scope.
use super::*;

pub(super) fn device_layout(pid: u32) -> bool {
    matches!(pid, 778 | 784 | 3871 | 3884 | 3886)
}

pub(super) enum AccessoryPage {
    Wireless(Entity<crate::features::wireless_argb::WirelessArgb>),
    Wired(Entity<crate::features::wired_argb::WiredArgbWorkspace>),
    Aether {
        strip: Entity<crate::features::aether_strip::AetherStrip>,
        lighting: Entity<crate::features::aether_strip::AetherLightingPage>,
    },
    Automation(Entity<crate::features::automation::Automation>),
}
impl AccessoryPage {
    pub(super) fn new(
        device: &Device,
        window: &mut Window,
        cx: &mut Context<SourceProductWorkspace>,
        subscriptions: &mut Vec<Subscription>,
    ) -> Option<Self> {
        if matches!(device.product_id, 3884 | 3886) {
            let view =
                cx.new(|cx| crate::features::wireless_argb::WirelessArgb::new(device, window, cx));
            subscriptions.push(cx.subscribe(
                &view,
                |this, view, _: &crate::features::wireless_argb::WirelessArgbChanged, cx| {
                    this.capture_accessory(view.read(cx).snapshot(), cx);
                },
            ));
            return Some(Self::Wireless(view));
        }
        if matches!(device.product_id, 778 | 3871) {
            let view = cx
                .new(|cx| crate::features::wired_argb::WiredArgbWorkspace::new(device, window, cx));
            subscriptions.push(cx.subscribe(
                &view,
                |this, view, _: &crate::features::wired_argb::WiredArgbChanged, cx| {
                    this.capture_accessory(view.read(cx).snapshot(), cx);
                },
            ));
            return Some(Self::Wired(view));
        }
        if device.product_id == 784 {
            let view =
                cx.new(|cx| crate::features::aether_strip::AetherStrip::new(device, window, cx));
            subscriptions.push(cx.subscribe(
                &view,
                |this, view, _: &crate::features::aether_strip::AetherStripChanged, cx| {
                    this.capture_accessory(view.read(cx).snapshot(), cx);
                },
            ));
            let lighting =
                cx.new(|cx| crate::features::aether_strip::AetherLightingPage::new(window, cx));
            return Some(Self::Aether {
                strip: view,
                lighting,
            });
        }
        if device.product_id == 3946 {
            let view =
                cx.new(|cx| crate::features::automation::Automation::new(device, window, cx));
            subscriptions.push(cx.subscribe(
                &view,
                |this, view, _: &crate::features::automation::AutomationChanged, cx| {
                    this.capture_accessory(view.read(cx).snapshot(), cx);
                },
            ));
            return Some(Self::Automation(view));
        }
        None
    }
    pub(super) fn supports_page(&self, pid: u32, key: &str) -> bool {
        match self {
            Self::Wireless(_) => crate::features::wireless_argb::supports_page(pid, key),
            Self::Wired(_) => crate::features::wired_argb::supports_page(pid, key),
            Self::Aether { strip: _, .. } => {
                crate::features::aether_strip::supports_page(pid, key)
                    || (pid == 784 && key == "TAB_LIGHTING")
            }
            Self::Automation(_) => crate::features::automation::supports_page(pid, key),
        }
    }
    pub(super) fn restore(
        &self,
        value: Option<&Value>,
        window: &mut Window,
        cx: &mut App,
    ) -> Value {
        match self {
            Self::Wireless(view) => {
                view.update(cx, |view, cx| view.restore(value, window, cx));
                view.read(cx).snapshot()
            }
            Self::Wired(view) => {
                view.update(cx, |view, cx| {
                    view.restore(value.unwrap_or(&Value::Null), window, cx)
                });
                view.read(cx).snapshot()
            }
            Self::Aether { strip: view, .. } => {
                view.update(cx, |view, cx| view.restore(value, window, cx));
                view.read(cx).snapshot()
            }
            Self::Automation(view) => {
                view.update(cx, |view, cx| view.restore(value, window, cx));
                view.read(cx).snapshot()
            }
        }
    }
    pub(super) fn dismiss(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Wireless(view) => view.update(cx, |view, cx| view.dismiss(window, cx)),
            Self::Wired(view) => view.update(cx, |view, cx| view.dismiss(window, cx)),
            Self::Aether { strip: view, .. } => {
                view.update(cx, |view, cx| view.dismiss(window, cx))
            }
            Self::Automation(view) => view.update(cx, |view, cx| view.dismiss(window, cx)),
        }
    }
    pub(super) fn element(&self, key: &str) -> AnyElement {
        match self {
            Self::Wireless(view) => view.clone().into_any_element(),
            Self::Wired(view) => view.clone().into_any_element(),
            Self::Aether { strip, lighting } => {
                if key == "TAB_LIGHTING" {
                    lighting.clone().into_any_element()
                } else {
                    strip.clone().into_any_element()
                }
            }
            Self::Automation(view) => view.clone().into_any_element(),
        }
    }
}
