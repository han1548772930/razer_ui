//! Named OS windows. Named policy=3 pages use the shell's host tabs instead.
//!
//! Dashboard 打开这些根时传的是「窗口名 + 标志 + URL 参数」：宿主先查同名窗口是否
//! 已存在，存在就复用它并聚焦，否则按策略添加页签或新建窗口。契约（13 个标志、三条窗口名规则、配对
//! 窗口的参数构造）与逐字段收据见 `docs/re/display-window-contract.md`。
//!
//! 这里只实现窗口层；根的内容由各自视图提供，服务未接通时不会伪造设备事实。

use gpui_kit::{AnyWindowHandle, App, Entity, Render, SharedString, Window, WindowOptions};
use std::cell::RefCell;

/// Independent Chroma host-window icon from host 4.0.827's app launcher. This
/// is distinct from the Dashboard `chromaIcon` flag (`ChromaApp/icon.ico`).
pub(super) const CHROMA_APP_WINDOW_ICON_PATH: &str = "ChromaApp\\chroma-app.ico";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct WindowIdentity {
    pub(super) container_id: Option<String>,
    pub(super) product_id: Option<u32>,
    pub(super) serial_number: Option<String>,
}

pub(super) fn multi_device_pairing_name(identity: &WindowIdentity) -> SharedString {
    if let Some(container) = identity
        .container_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return format!("multi-device-pairing-{container}").into();
    }
    match (
        identity.product_id,
        identity
            .serial_number
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (Some(product_id), Some(serial)) => {
            format!("multi-device-pairing-p{product_id}-{serial}").into()
        }
        _ => "multi-device-pairing".into(),
    }
}

/// 打开结果：命中了既有窗口并聚焦，还是新建了窗口。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WindowOutcome {
    Focused,
    Opened,
}

thread_local! {
    /// 具名窗口登记表：窗口名与句柄。原版按窗口名单例，
    /// 同名只保留一个窗口；窗口关闭后条目在下一次查询时按 `App::windows()` 清理。
    static OPEN_WINDOWS: RefCell<Vec<(SharedString, AnyWindowHandle)>> = const { RefCell::new(Vec::new()) };
}

/// 打开或聚焦具名窗口。同名的活动窗口存在时聚焦它并返回
/// [`WindowOutcome::Focused`]。独立窗口调用方已选择 policy=5；
/// policy=3 页面由宿主页签所有者挂载。
pub(super) fn open_or_focus<V: Render + 'static>(
    cx: &mut App,
    name: SharedString,
    options: WindowOptions,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<(WindowOutcome, AnyWindowHandle)> {
    let live = cx.windows();
    let existing = OPEN_WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|(_, handle)| live.contains(handle));
        windows
            .iter()
            .find(|(registered, ..)| registered == &name)
            .map(|(_, handle)| *handle)
    });
    if let Some(handle) = existing {
        handle.update(cx, |_, window, _| window.activate_window())?;
        return Ok((WindowOutcome::Focused, handle));
    }
    let (handle, _) = gpui_kit::open_window(options, cx, build)?;
    OPEN_WINDOWS.with(|windows| windows.borrow_mut().push((name, handle)));
    Ok((WindowOutcome::Opened, handle))
}

/// 该具名窗口当前是否存在（登记表按实际窗口列表清理后判断）。
pub(super) fn is_open(name: &SharedString, cx: &App) -> bool {
    let live = cx.windows();
    OPEN_WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|(_, handle)| live.contains(handle));
        windows.iter().any(|(registered, ..)| registered == name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(
        container: Option<&str>,
        product: Option<u32>,
        serial: Option<&str>,
    ) -> WindowIdentity {
        WindowIdentity {
            container_id: container.map(str::to_string),
            product_id: product,
            serial_number: serial.map(str::to_string),
        }
    }

    #[test]
    fn container_wins_over_product_and_serial() {
        assert_eq!(
            multi_device_pairing_name(&identity(Some("{A1}"), Some(104), Some("SN1"))),
            SharedString::from("multi-device-pairing-{A1}")
        );
        assert_eq!(
            multi_device_pairing_name(&identity(Some(" {A1} "), Some(104), Some("SN1"))),
            SharedString::from("multi-device-pairing- {A1} ")
        );
        assert_eq!(
            multi_device_pairing_name(&identity(Some("  "), Some(104), Some("SN1"))),
            SharedString::from("multi-device-pairing-p104-SN1")
        );
    }

    #[test]
    fn product_without_serial_falls_back_to_the_plain_name() {
        assert_eq!(
            multi_device_pairing_name(&identity(None, Some(104), None)),
            SharedString::from("multi-device-pairing")
        );
        assert_eq!(
            multi_device_pairing_name(&identity(None, None, Some("SN1"))),
            SharedString::from("multi-device-pairing")
        );
    }
}
