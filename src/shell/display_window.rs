//! Named OS windows. Named policy=3 pages use the shell's host tabs instead.
//!
//! Dashboard 打开这些根时传的是「窗口名 + 标志 + URL 参数」：宿主先查同名窗口是否
//! 已存在，存在就复用它并聚焦，否则按策略添加页签或新建窗口。契约（13 个标志、三条窗口名规则、配对
//! 窗口的参数构造）与逐字段收据见 `docs/re/display-window-contract.md`。
//!
//! 这里只实现窗口层；根的内容由各自视图提供，服务未接通时不会伪造设备事实。

use gpui_kit::{AnyWindowHandle, App, Entity, Render, SharedString, Window, WindowOptions};
use std::cell::RefCell;

/// 产品包按 `?displayMode=` 选择的根，取值即原版
/// `searchParams.get("displayMode")` 的比较字面量。
///
/// 四个取值都来自 [分支审计](../docs/re/display-mode-audit.md)；
/// The application modules live in host tabs. Product-side macro/armory roots
/// are separate embedded modes, not an OS-window policy. Pairing retains an
/// explicitly requested second window; `chromaApp` belongs to Chroma.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DisplayMode {
    Macro,
    ChromaApp,
    Armory,
    MultiDevicePairing,
}

/// Independent Chroma host-window icon from the current Chroma application
/// contract. This is distinct from the Dashboard `chromaIcon` flag below.
pub(super) const CHROMA_APP_WINDOW_ICON_PATH: &str = "ChromaApp\\window.ico";

impl DisplayMode {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Macro => "macro",
            Self::ChromaApp => "chromaApp",
            Self::Armory => "armory",
            Self::MultiDevicePairing => "multiDevicePairing",
        }
    }

    /// 原版 `app_icon_path`。gpui 的 [`WindowOptions`] 没有图标字段，这里只保留
    /// 来源值；不拿别的图标冒充，也不在窗口上画自造图标。
    pub(super) fn icon(self) -> &'static str {
        match self {
            Self::ChromaApp => "app_icon_path=ChromaApp\\window.ico",
            Self::Macro | Self::Armory | Self::MultiDevicePairing => {
                "app_icon_path=Synapse/icon.ico"
            }
        }
    }
}

/// 打开参数：Dashboard 从设备记录里读到的容器、产品与序列号。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct WindowIdentity {
    pub(super) container_id: Option<String>,
    pub(super) product_id: Option<u32>,
    pub(super) serial_number: Option<String>,
}

/// Dashboard policy flags. Same attaches a named tab to the current host
/// window; Different and DifferentSingleProcess create OS windows. Name
/// deduplication happens before policy dispatch in the original host.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WindowPolicy {
    Same,
    Different,
    DifferentSingleProcess,
}

impl WindowPolicy {
    #[allow(dead_code)]
    pub(super) fn flag(self) -> &'static str {
        match self {
            Self::Same => "policy=3",
            Self::Different => "policy=5",
            Self::DifferentSingleProcess => "policy=7",
        }
    }
}

/// 多设备配对窗口名，逐字对应 Dashboard 模块 84058 的 `xc()`：
/// 容器 → 产品 + 序列号 → 纯模式名。
pub(super) fn multi_device_pairing_name(identity: &WindowIdentity) -> SharedString {
    if let Some(container) = identity
        .container_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
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
    /// 具名窗口登记表：窗口名、打开该窗口时用的策略与句柄。原版按窗口名单例，
    /// 同名只保留一个窗口；窗口关闭后条目在下一次查询时按 `App::windows()` 清理。
    static OPEN_WINDOWS: RefCell<Vec<(SharedString, WindowPolicy, AnyWindowHandle)>> = const { RefCell::new(Vec::new()) };
}

/// 打开或聚焦具名窗口。同名的活动窗口存在时聚焦它并返回
/// [`WindowOutcome::Focused`]，不重复创建；`policy` 是 Dashboard 传下来的窗口标志，
/// 与句柄一起登记，便于按契约核对是哪个策略开的窗口。
pub(super) fn open_or_focus<V: Render + 'static>(
    cx: &mut App,
    name: SharedString,
    policy: WindowPolicy,
    options: WindowOptions,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<(WindowOutcome, AnyWindowHandle)> {
    anyhow::ensure!(
        policy != WindowPolicy::Same,
        "policy=3 must be opened through the host tab registry"
    );
    let live = cx.windows();
    let existing = OPEN_WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|(_, _, handle)| live.contains(handle));
        windows
            .iter()
            .find(|(registered, ..)| registered == &name)
            .map(|(_, _, handle)| *handle)
    });
    if let Some(handle) = existing {
        handle.update(cx, |_, window, _| window.activate_window())?;
        return Ok((WindowOutcome::Focused, handle));
    }
    let (handle, _) = gpui_kit::open_window(options, cx, build)?;
    OPEN_WINDOWS.with(|windows| windows.borrow_mut().push((name, policy, handle)));
    Ok((WindowOutcome::Opened, handle))
}

/// 该具名窗口当前是否存在（登记表按实际窗口列表清理后判断）。
#[allow(dead_code)] // 供入口判断是否已经打开过，尚未接线。
pub(super) fn is_open(name: &SharedString, cx: &App) -> bool {
    let live = cx.windows();
    OPEN_WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|(_, _, handle)| live.contains(handle));
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

    #[test]
    fn mode_keys_and_policies_match_the_extracted_contract() {
        assert_eq!(DisplayMode::MultiDevicePairing.key(), "multiDevicePairing");
        assert_eq!(
            DisplayMode::ChromaApp.icon(),
            "app_icon_path=ChromaApp\\window.ico"
        );
        assert_eq!(CHROMA_APP_WINDOW_ICON_PATH, "ChromaApp\\window.ico");
        assert_eq!(WindowPolicy::Same.flag(), "policy=3");
        assert_eq!(WindowPolicy::Different.flag(), "policy=5");
        assert_eq!(WindowPolicy::DifferentSingleProcess.flag(), "policy=7");
    }
}
