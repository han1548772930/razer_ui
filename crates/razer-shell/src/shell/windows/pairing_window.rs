//! `displayMode=multiDevicePairing` 的窗口根。
//!
//! 原版这个根本身只是一个 iframe 宿主（180 `BG`/`FG`）：它把
//! `/synapse/multipairing/` 连同 `displayMode=multiDevicePairing`、`containerId`、
//! `productId`/`pid`、`category`、`canPairTwoDevices`、`isProductivity`、
//! `deviceName`、`serialNumber`、`lang`、`allMasters` 作为参数打开，并在加载完成后
//! postMessage `{eventName:"multiDevicePairingInit", payload:{deviceInfo, deviceName,
//! allMasters}}`。本仓库的 4130 页面就是被嵌入的那一页，所以这里直接承载它，并把
//! 同一份 `allMasters` 交给它。
//!
//! 两个已记录的限制：`deviceInfo` 与 `deviceName` 目前在本地页面里没有对应字段，
//! 不做映射以免自造状态；`allMasters` 在原版来自宿主写入的 `connectedDeviceInfo`
//! 投影（Dashboard `jt`/`Et`），该投影尚未审计，所以调用方没有真实数据时传 `None`，
//! 页面保持原有空态。

use super::{
    display_window::DisplayMode,
    pairing_page::{PairingPage, PairingPageEvent},
};
use razer_widgets::scroll::SourceScrollable as _;
use gpui_kit::component::*;
use gpui_kit::*;
use serde_json::Value;

/// 原版 `multiDevicePairingInit` 负载里本仓库能够如实消费的部分。
#[derive(Clone, Debug, Default)]
pub(super) struct PairingWindowPayload {
    /// 原版 `allMasters`：已经配对的主设备记录。只有真实的 DUALLINK 记录才传入。
    pub(super) all_masters: Option<Value>,
}

/// 具名配对窗口的根视图：承载 4130 页面，并在返回时关闭自己的窗口。
pub(super) struct PairingWindow {
    page: Entity<PairingPage>,
}

impl PairingWindow {
    pub(super) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        payload: PairingWindowPayload,
    ) -> Self {
        let page = cx.new(|cx| PairingPage::new(window, cx));
        if let Some(masters) = payload.all_masters {
            page.update(cx, |page, cx| page.apply_all_masters(masters, window, cx));
        }
        // 原版由根自己维护标题与 favicon；标题取该根的文档标题文案，图标见
        // `DisplayMode::icon`（gpui 无图标字段，记录在窗口契约里）。
        let _ = DisplayMode::MultiDevicePairing.icon();
        cx.subscribe_in(
            &page,
            window,
            |_, _, event: &PairingPageEvent, window, cx| match event {
                // 原版该窗口的返回走 `history.back()`，窗口由宿主关闭；这里直接关闭
                // 自己，不把返回动作冒泡到主窗口。
                PairingPageEvent::Back => {
                    cx.defer_in(window, |_, window, _| window.remove_window());
                }
                // 窗口内的设备卡再次触发打开时，同名窗口就是本窗口：原版会复用并
                // 聚焦它，所以这里不重复开窗。
                PairingPageEvent::OpenProductWindow(_) => {}
            },
        )
        .detach();
        Self { page }
    }
}

impl Render for PairingWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id(SharedString::from(format!(
                "display-mode-{}",
                DisplayMode::MultiDevicePairing.key()
            )))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("pairing-window-scroll")
                    .size_full()
                    .scrollable_both()
                    .child(self.page.clone()),
            )
    }
}
