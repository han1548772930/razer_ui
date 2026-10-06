//! 设备页右上角电量（`.right .battery` 里的 `.batt` 图标与百分比）。
//!
//! # 依据（全部来自当前源码，未做推测）
//!
//! 1. 渲染条件与结构 —— `.ref/devices/100/static/js/main.1734869e.js`：
//!    `this.props.hasBattery && undefined !== this.props.batteryState` 才渲染
//!    `div.battery`（`role="img" id="battery-level-tips"`），子节点顺序是
//!    先 `<span>{level>=0 ? "{level} %" : "-"}</span>`、再
//!    `<div class={this.getBatteryState()}/>`，最后是悬停才挂载的 tooltip；
//!    `level >= 0 && level <= 10` 时 span 额外带 `low-batt` 类。
//! 2. 状态机 —— `.ref/devices/104/static/js/main.9b94769e.js` 的
//!    `chargingStatus → battName/battTips` 映射（下面的 [`badge`]）：
//!    `off` → `batt batt-off`／`BATTERY_OFF`；`Charging` 且 `level>=99` →
//!    `batt charging100`／`BATTERY_CHARGED_FULL`；`Charging` → `batt charging`／
//!    `BATTERY_CHARGING`；`NoCharge_BatteryFull`、`ReachChargingLimit`
//!    （`pV.PAUSED_CHARGING`）与 default 走同一个分档函数／`BATTERY_PERCENT`；
//!    `batt-warning` → `batt batt-warning`／`BATTERY_ERROR_TIPS`。
//!    分档函数：`level <= 0` → `batt batt-disconnected`；`level === 10` → 10；
//!    `10 < level < 20` → 20；其余 `10 * floor(level / 10)`；暂停充电再加 `paused`。
//!    负载里没有 `chargingStatus` 时 `batteryState` 为 `undefined`，即整块不渲染。
//! 3. 样式 —— `.ref/devices/100/static/css/main.dd229426.css`：
//!    `.right .battery{align-items:center;color:#ccc;display:flex;font-size:14px;height:46px;justify-content:center}`、
//!    `.battery .low-batt{color:#c8323c}`、
//!    `.nav-tabs .batt{background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;margin:0 5px;width:26px}`、
//!    `.batt.charging{background-image:url(icon_battery_charging…)}`、
//!    `.batt.charging100{…icon_battery_charging_100…}`、`.batt.batt-0…100{…icon_battery_N…}`、
//!    `.batt.batt-disconnected{…icon_battery_disconnected…}`、`.batt.batt-off{…icon_device_off…}`、
//!    `.batt.batt-N.paused{…icon_battery_paused.svg#N…}`、
//!    `.batt.batt-warning{…icon_battery_error…}`。
//!    暂停档位在原文件里是 20×220 图标条上的 `view` 片段，资源准备阶段已按原
//!    `viewBox` 裁成 `battery-paused-N.svg`（见 `tools/prepare-resources.py`）。
//! 4. 文案 —— `BATTERY_OFF`／`BATTERY_CHARGING`／`BATTERY_CHARGED_FULL`／
//!    `BATTERY_PERCENT`（含 `{{level}}` 占位）／`BATTERY_ERROR_TIPS`，
//!    取自雷云语言包（`locales/*.json`），与源码里的 key 同名。
use crate::i18n;
use crate::model::Device;
use crate::ui::source_tooltip::{SourceTooltip, SourceTooltipKind};
use crate::ui::surface;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;

/// 一格电量的类名、图标与提示文案。
pub(crate) struct BatteryBadge {
    /// 原版 `battName`，例如 `batt batt-40`／`batt charging100`。
    pub(crate) class: &'static str,
    /// 已打包的图标路径（与原版 CSS 的 background-image 对应）。
    pub(crate) icon: &'static str,
    pub(crate) tip: String,
    pub(crate) level: i32,
    /// Battery error state; shares the mounted tooltip-razer portal.
    #[cfg(test)]
    pub(crate) warning: bool,
}

/// 原版分档：`10 === level ? 10 : (level > 10 && level < 20) ? 20 : 10 * floor(level / 10)`。
fn bucket(level: i32) -> i32 {
    if level == 10 {
        10
    } else if level > 10 && level < 20 {
        20
    } else {
        10 * (level / 10)
    }
}

fn percent_tip(level: i32) -> String {
    i18n::t("BATTERY_PERCENT").replace("{{level}}", &level.to_string())
}

/// 电量档位 → `battName` + 图标 + 提示文案（原版 `qe(payload)` 的等价实现）。
pub(crate) fn badge(level: i32, charging_status: &str) -> BatteryBadge {
    // The dashboard normally sends the title-cased enum values above, while
    // lower-level payloads have also been observed with all-caps values
    // (`OFF`, `CHARGING`, ...). Keep the source spelling for unknown states,
    // but make the known branches case-insensitive so those payloads do not
    // silently fall back to the 100% icon.
    let (class, icon, tip, _warning) = if charging_status.eq_ignore_ascii_case("off") {
        (
            "batt batt-off",
            "synapse/battery-off.svg",
            i18n::t("BATTERY_OFF"),
            false,
        )
    } else if charging_status.eq_ignore_ascii_case("charging") && level >= 99 {
        (
            "batt charging100",
            "synapse/battery-charging-100.svg",
            i18n::t("BATTERY_CHARGED_FULL"),
            false,
        )
    } else if charging_status.eq_ignore_ascii_case("charging") {
        (
            "batt charging",
            "synapse/battery-charging.svg",
            i18n::t("BATTERY_CHARGING"),
            false,
        )
    } else if charging_status.eq_ignore_ascii_case("NoCharge_BatteryFull") {
        bucket_badge(level, false)
        // `pV.PAUSED_CHARGING` 的字面值是 `ReachChargingLimit`（充电到上限后暂停）。
    } else if charging_status.eq_ignore_ascii_case("ReachChargingLimit") {
        bucket_badge(level, true)
    } else if charging_status.eq_ignore_ascii_case("batt-warning") {
        (
            "batt batt-warning",
            "synapse/battery-error.svg",
            i18n::t("BATTERY_ERROR_TIPS"),
            true,
        )
        // default 分支：`Sa(100)`，文案同样取 `BATTERY_PERCENT`。
    } else {
        (
            "batt batt-100",
            "synapse/battery-100.svg",
            percent_tip(level),
            false,
        )
    };
    BatteryBadge {
        class,
        icon,
        tip,
        level,
        #[cfg(test)]
        warning: _warning,
    }
}

/// `level <= 0` 是断连档；其余按 [`bucket`] 取图标，暂停充电用裁好的片段文件。
fn bucket_badge(level: i32, paused: bool) -> (&'static str, &'static str, String, bool) {
    if level <= 0 {
        return (
            "batt batt-disconnected",
            "synapse/battery-disconnected.svg",
            percent_tip(level),
            false,
        );
    }
    let bucket = bucket(level);
    let (class, icon) = match (bucket, paused) {
        (0, false) => ("batt batt-0", "synapse/battery-0.svg"),
        (10, false) => ("batt batt-10", "synapse/battery-10.svg"),
        (20, false) => ("batt batt-20", "synapse/battery-20.svg"),
        (30, false) => ("batt batt-30", "synapse/battery-30.svg"),
        (40, false) => ("batt batt-40", "synapse/battery-40.svg"),
        (50, false) => ("batt batt-50", "synapse/battery-50.svg"),
        (60, false) => ("batt batt-60", "synapse/battery-60.svg"),
        (70, false) => ("batt batt-70", "synapse/battery-70.svg"),
        (80, false) => ("batt batt-80", "synapse/battery-80.svg"),
        (90, false) => ("batt batt-90", "synapse/battery-90.svg"),
        (_, false) => ("batt batt-100", "synapse/battery-100.svg"),
        (0, true) => ("batt batt-0 paused", "synapse/battery-paused-0.svg"),
        (10, true) => ("batt batt-10 paused", "synapse/battery-paused-10.svg"),
        (20, true) => ("batt batt-20 paused", "synapse/battery-paused-20.svg"),
        (30, true) => ("batt batt-30 paused", "synapse/battery-paused-30.svg"),
        (40, true) => ("batt batt-40 paused", "synapse/battery-paused-40.svg"),
        (50, true) => ("batt batt-50 paused", "synapse/battery-paused-50.svg"),
        (60, true) => ("batt batt-60 paused", "synapse/battery-paused-60.svg"),
        (70, true) => ("batt batt-70 paused", "synapse/battery-paused-70.svg"),
        (80, true) => ("batt batt-80 paused", "synapse/battery-paused-80.svg"),
        (90, true) => ("batt batt-90 paused", "synapse/battery-paused-90.svg"),
        (_, true) => ("batt batt-100 paused", "synapse/battery-paused-100.svg"),
    };
    (class, icon, percent_tip(level), false)
}

/// 当前源里把 `hideBattValue` 传成 `!0` 的产品（例如
/// `.ref/devices/1330/static/js/main.f0797abf.js` 的产品工作区
/// `hideBattValue:!0`）。原版这是**产品代码里写死的 prop**，不是设备数据，
/// 所以这里按产品 id 列表；`tools/audit-battery-indicator.cjs` 会从当前包重算这张表。
/// 这些产品的顶栏只显示电量图标：不渲染百分比 `<span>`、不挂载悬停提示，
/// 并按 `.hideBattValue{margin-right:17px}` 加右边距。
const HIDE_BATTERY_VALUE: &[u32] = &[
    115, 131, 1330, 1342, 1370, 1372, 1374, 1443, 1453, 2636, 2647, 2676, 4115, 4133, 4144,
];

/// 设备页顶栏右侧的电量块。设备没有 `powerStatus` 时返回 `None`
/// （原版此时 `batteryState === undefined`，整块不渲染）。
pub(crate) fn element(device: &Device, cx: &App) -> Option<AnyElement> {
    if !device.has_battery {
        return None;
    }
    let power = device.power_status.as_ref()?;
    let level = power.level;
    let badge = badge(level, power.charging_status.as_str());
    let level = badge.level;
    let low = (0..=10).contains(&level);
    let color = if low {
        // `.battery .low-batt{color:#c8323c}`
        cx.theme().danger
    } else {
        // `.right .battery{color:#ccc}`
        cx.theme().foreground
    };
    let text = if level >= 0 {
        format!("{level} %")
    } else {
        "-".to_string()
    };
    let tip = badge.tip.clone();
    // `badge.icon` 是 `&'static str`；闭包要交给 `SourceTooltip::trigger`（要求 `'static`），
    // 所以按值捕获而不是借用 `badge`。
    let icon_name = badge.icon;
    let icon = move || {
        // `.nav-tabs .batt{background-position:50%;background-repeat:no-repeat;
        //  background-size:20px;height:26px;margin:0 5px;width:26px}`
        div()
            .w(surface::css(26.))
            .h(surface::css(26.))
            .mx(surface::css(5.))
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(SharedString::from(icon_name))
                    .w(surface::css(20.))
                    .h(surface::css(20.)),
            )
    };
    // 原版 `className={"battery " + (hideBattValue ? "hideBattValue" : "")}`、
    // `{!hideBattValue && !o && <span>{level} %</span>}`、
    // `{!hideBattValue && <Tooltip …/>}` 与 `.hideBattValue{margin-right:17px}`。
    let hide_value = HIDE_BATTERY_VALUE.contains(&device.product_id);
    if hide_value {
        return Some(
            h_flex()
                .id(SharedString::from(format!(
                    "device-battery:{} hideBattValue",
                    badge.class
                )))
                .role(Role::Image)
                .h(surface::css(46.))
                .items_center()
                .justify_center()
                .mr(surface::css(17.))
                .child(icon())
                .into_any_element(),
        );
    }
    Some(
        SourceTooltip::new("battery-level-tips", tip, 300.)
            .kind(SourceTooltipKind::Battery)
            .trigger(move |_, _, _| {
                h_flex()
                    // 元素 id 直接用原版类名（`batt batt-40` 等），便于对照源码排查状态。
                    .id(SharedString::from(format!(
                        "device-battery:{}",
                        badge.class
                    )))
                    // Dashboard mounts this block as `role="img"` with the
                    // battery status text and icon as one accessible unit.
                    .role(Role::Image)
                    .h(surface::css(46.))
                    .items_center()
                    .justify_center()
                    .text_size(surface::css(14.))
                    .text_color(color)
                    .child(text)
                    .child(icon())
                    .into_any_element()
            })
            .into_any_element(),
    )
}

#[cfg(test)]
#[path = "battery_tests.rs"]
mod tests;
