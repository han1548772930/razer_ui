//! 性能页：DPI 档位（可编辑）、轮询率、抬升距离、加速度、表面校准、传感器旋转。
//!
//! DPI 档位模型逐字对应雷云实测
//! `dpiStages { stages:[{x,y,independent,visible}], active, enable, count }`。
//! 其余项属于 [推断]（由设备模块下发），见 `crate::features`。
//!
//! 所有修改都会立即写入本地配置（见 `crate::store`）。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::features::{DebounceMode, LiftOffDistance, PollingRate};
use crate::ui::widgets::{
    PageLayout, SettingRow, btn, card, card_title, dpi_stage_chart, select_row, slider_row,
    toggle_button, widget_slot,
};

/// 雷云实测的每设备档位上限。
const MAX_STAGES: usize = 5;

/// 渲染性能页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return crate::ui::widgets::EmptyState::new("未检测到设备").into_any_element();
    };

    let is_mouse = device.is_mouse();
    let device_name = device.display_name();

    // 设备页统一使用原版的产品图区 + 600px 卡片网格。
    // 鼠标和键盘的性能能力不是同一套控件，不能先渲染全部控件再隐藏。
    let layout = PageLayout::new("性能", device_name)
        .widget(widget_slot(sensor_section(app, cx)))
        .when(is_mouse, |this| {
            this.widget(widget_slot(dpi_section(app, cx)))
                .widget(widget_slot(sensor_extras(app, cx)))
                .widget(widget_slot(clutch_section(app, cx)))
                .widget(widget_slot(matcher_section(app, cx)))
        })
        .when(!is_mouse, |this| {
            this.widget(widget_slot(keyboard_performance_section(app, cx)))
        })
        .widget(widget_slot(debounce_section(app, cx)));

    layout.into_any_element()
}

// ---------------------------------------------------------------------------
// 几何与颜色（全部带 CSS 出处）
// ---------------------------------------------------------------------------

/// `.stage-circle-{1..5}` —— 雷云给五个 DPI 档位各配一个颜色，
/// 用来在曲线图和档位列表里对应。
const STAGE_COLORS: [u32; 5] = [0x00FF_1A1A, 0x0024_FF00, 0x0000_6FFF, 0x0000_EDFF, 0x00FF_F700];

/// `.stage { height:68px; display:flex; align-items:center }`
const STAGE_H: f32 = 68.0;
/// `.stage-ordinal { width:30px; height:30px }`
const STAGE_ORDINAL: f32 = 30.0;
/// `.stage-input { height:26px; width:60px; background:#111;
///                 border:1px solid #5d5d5d; color:#ccc; font-size:14px; text-align:center }`
const STAGE_INPUT_H: f32 = 26.0;
const STAGE_INPUT_W: f32 = 60.0;
/// `.customize-polling-rate-button { height:27px; min-width:90px;
///   background:#222; border:1px solid #5d5d5d; border-radius:3px;
///   color:#ccc; font-size:14px; text-transform:uppercase }`
/// 选中态是 `.…:hover { border-color:#44d62c }` 的常驻版本。
const POLLING_BTN_H: f32 = 27.0;
const POLLING_BTN_MIN_W: f32 = 90.0;
/// `.lift-off { padding:20px; width:290px; background:#111; border-radius:5px }`
const LIFT_OFF_PAD: f32 = 20.0;
const LIFT_OFF_W: f32 = 290.0;

/// ① 传感器：轮询率（**按钮组**）+ 抬升距离。
///
/// 轮询率在雷云里**不是下拉**，而是 `.polling-btn-set` 里的一排按钮：
/// ```css
/// .polling-btn-set { display:flex; flex-wrap:wrap; gap:10px 10px }
/// .customize-polling-rate-button { height:27px; min-width:90px; background-color:#222;
///   border:1px solid #5d5d5d; border-radius:3px; color:#ccc; font-size:14px;
///   text-transform:uppercase }
/// ```
/// 之前这里做成了下拉，是不对的——下拉在雷云里用于**枚举型设置项**，
/// 而轮询率是「一排并列的档位按钮」。
fn sensor_section(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let polling = device.features.performance.polling_rate;
    let lift_off = device.features.performance.lift_off;

    let has_hyperpolling_dongle = app.devices.iter().any(|device| device.product_id == 179);
    let available_rates: Vec<PollingRate> = PollingRate::ALL
        .into_iter()
        .filter(|rate| !rate.needs_hyperpolling() || has_hyperpolling_dongle)
        .collect();

    card()
        .child(card_title("轮询率"))
        .child(div().text_xs().child(
            "雷云原文：轮询率越高，设备向电脑报告位置的频率越高，光标移动越顺滑。",
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .mt_2()
                .children(available_rates.into_iter().map(|rate| {
                    let selected = rate == polling;
                    div()
                        .id(("polling", rate.hz() as usize))
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(POLLING_BTN_H))
                        .min_w(px(POLLING_BTN_MIN_W))
                        .rounded(px(3.))
                        .bg(cx.theme().background)
                        .border_1()
                        // 选中与未选中的区别**只在边框色**：真实实现是
                        // `border-color:#44d62c`，底色不变。
                        .border_color(if selected {
                            cx.theme().primary
                        } else {
                            cx.theme().border
                        })
                        .text_sm()
                        .cursor_pointer()
                        // `text-transform:uppercase`
                        .child(rate.label().to_uppercase())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_polling_rate(rate, cx);
                        }))
                })),
        )
        .when(!has_hyperpolling_dongle, |this| {
            this.child(
                div()
                    .text_xs()
                    .mt_1()
                    .child("2000 Hz 及以上需要已连接的 HyperPolling 无线接收器。"),
            )
        })
        .when_some(lift_off, |this, value| {
            this.child(
                div()
                    .mt_3()
                    .w(px(LIFT_OFF_W))
                    .p(px(LIFT_OFF_PAD))
                    .rounded(px(5.))
                    .bg(cx.theme().group_box)
                    .child(card_title("抬升距离"))
                    .child(select_row(
                        "lift-off",
                        "档位",
                        value.zh().to_string(),
                        &LiftOffDistance::LABELS,
                        app.open_select.as_deref() == Some("lift-off"),
                        cx,
                        |this, index, cx| {
                            if let Some(picked) = LiftOffDistance::ALL.get(index).copied() {
                                this.set_lift_off(picked, cx);
                            }
                        },
                    )),
            )
        })
        .into_any_element()
}

/// 键盘在性能页只显示键盘能力，不显示鼠标 DPI、抬升距离或传感器控件。
fn keyboard_performance_section(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let Some(keyboard) = device.features.keyboard.as_ref() else {
        return card()
            .child(card_title("键盘性能"))
            .child(div().text_sm().child("当前设备未声明键盘性能能力。"))
            .into_any_element();
    };

    card()
        .child(card_title("键盘性能"))
        .child(div().text_xs().child(
            "仅显示当前设备声明的键盘性能能力；鼠标专属 DPI、抬升距离和传感器设置不会出现在此页。",
        ))
        .child(toggle_button(
            "perf-kb-gaming",
            "游戏模式（禁用 Windows 键）",
            keyboard.gaming_mode,
            cx,
            |this, cx| this.toggle_gaming_mode(cx),
        ))
        .child(
            h_flex()
                .gap_2()
                .child(toggle_button(
                    "perf-kb-alt-tab",
                    "锁定 Alt + Tab",
                    keyboard.lock_alt_tab,
                    cx,
                    |this, cx| this.toggle_lock_alt_tab(cx),
                ))
                .child(toggle_button(
                    "perf-kb-alt-f4",
                    "锁定 Alt + F4",
                    keyboard.lock_alt_f4,
                    cx,
                    |this, cx| this.toggle_lock_alt_f4(cx),
                )),
        )
        .child(toggle_button(
            "perf-kb-n-key",
            "全键无冲",
            keyboard.n_key_rollover,
            cx,
            |this, cx| this.toggle_n_key_rollover(cx),
        ))
        .child(SettingRow::new(
            "Snap Tap",
            if keyboard.snap_tap.is_some() {
                "已声明，可在键盘功能页配置".to_string()
            } else {
                "设备不支持".to_string()
            },
        ))
        .child(SettingRow::new(
            "Dynamic Keystroke",
            if keyboard.dynamic_key_stroke.is_some() {
                "已声明，可在键盘功能页配置".to_string()
            } else {
                "设备不支持".to_string()
            },
        ))
        .child(SettingRow::new(
            "Actuation",
            if keyboard.actuation.is_some() {
                "已声明，可在键盘功能页配置".to_string()
            } else {
                "设备不支持".to_string()
            },
        ))
        .into_any_element()
}

/// ② DPI 档位区。
///
/// ```css
/// .stages { display:flex; flex-direction:column }
/// .stage { height:68px; display:flex; align-items:center; flex:0 0 auto }
/// .stage-ordinal { width:30px; height:30px }
/// .stage-input { height:26px; width:60px; background-color:#111;
///                border:1px solid #5d5d5d; color:#ccc; font-size:14px; text-align:center }
/// .stage-control { height:27px }
/// .description-stages { color:#999; line-height:17px }
/// ```
fn dpi_section(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let stages: Vec<(u32, bool)> = device
        .active_profile_obj()
        .and_then(|p| p.dpi_stages.as_ref())
        .map(|s| s.stages.iter().map(|st| (st.x, true)).collect())
        .unwrap_or_default();
    let (min, max, step) = device.dpi_bounds();
    let count = stages.len();

    card()
        .child(card_title(format!("灵敏度（DPI）· {count} 档")))
        .child(div().text_xs().child(format!(
            "雷云原文：范围 {min}–{max}，步进 {step}。每个档位可单独设定 X / Y。"
        )))
        // 曲线图：展示各档位的高低关系。
        .child(dpi_stage_chart(&stages, cx))
        .child(
            v_flex()
                .w_full()
                .mt_2()
                .children(stages.into_iter().enumerate().map(|(index, (dpi, visible))| {
                    let color = STAGE_COLORS[index % STAGE_COLORS.len()];
                    h_flex()
                        .h(px(STAGE_H))
                        .items_center()
                        .gap_3()
                        // `.stage-ordinal { width:30px; height:30px }` ——
                        // 档位序号所在的圆点，颜色即该档在曲线图里的颜色。
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(STAGE_ORDINAL))
                                .h(px(STAGE_ORDINAL))
                                .rounded_full()
                                .bg(rgb(color))
                                // `.stage-ordinal-number { color:#111 }`
                                .text_sm()
                                .text_color(cx.theme().primary_foreground)
                                .child(format!("{}", index + 1)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(STAGE_INPUT_W))
                                .h(px(STAGE_INPUT_H))
                                .rounded(px(3.))
                                .bg(cx.theme().group_box)
                                .border_1()
                                .border_color(cx.theme().border)
                                .text_sm()
                                .child(format!("{dpi}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .when(!visible, |this| {
                                    this.text_color(cx.theme().muted_foreground)
                                })
                                .child(if visible { "已启用" } else { "已隐藏" }),
                        )
                })),
        )
        .child(div().text_xs().mt_1().child(format!(
            "雷云实测每设备上限 {MAX_STAGES} 档；当前 {count} 档。"
        )))
        .into_any_element()
}

/// ③ 鼠标专有：加速度、表面校准、传感器旋转。
fn sensor_extras(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let performance = &device.features.performance;
    let acceleration = performance.acceleration.unwrap_or(false);
    let rotation = performance.sensor_rotation.unwrap_or(0);
    let surface = performance.surface_calibration.unwrap_or(false);

    card()
        .child(card_title("传感器"))
        // `.configure-sensitivity { margin:28px 0 }` 对应这一组的间距。
        .child(toggle_button(
            "perf-accel",
            "加速度",
            acceleration,
            cx,
            |this, cx| this.toggle_acceleration(cx),
        ))
        .child(div().text_xs().child(
            "雷云原文：启用后，光标移动速度会随鼠标移动速度加快，适合某些游戏。",
        ))
        .child(
            div()
                .mt_4()
                .child(SettingRow::new(
                    "表面校准",
                    if surface { "已校准" } else { "未校准" }.to_string(),
                )),
        )
        .child(div().text_xs().child(
            "雷云原文：校准鼠标以适配当前使用的表面，可提升追踪精度。",
        ))
        .child(
            // `sensor_rotation` 是 `Option<u16>`，范围 0–180 度
            // （见 `features.rs` 的字段注释），所以这里做一次 u16 ↔ f32 转换。
            div().mt_4().child(slider_row(
                "sensor-rotation",
                "传感器旋转",
                rotation as f32,
                0.,
                180.,
                1.,
                format!("{rotation}°"),
                true,
                cx,
                |this, value, cx| {
                    this.edit_features(cx, |f| {
                        f.performance.sensor_rotation = Some(value.round() as u16);
                    });
                },
            )),
        )
        .into_any_element()
}

/// ④ 灵敏度离合器（`SENSITIVITY_CLUTCH`）。
///
/// 结构：左键 / 右键各一个档位，可「联动」——联动时改一个另一个跟随。
fn clutch_section(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let Some(clutch) = device.features.performance.sensitivity_clutch.as_ref() else {
        return card()
            .child(card_title("灵敏度离合器"))
            .child(div().text_sm().child("该设备没有灵敏度离合器"))
            .into_any_element();
    };
    let (left, right, linked) = (clutch.left, clutch.right, clutch.linked);

    card()
        .child(card_title("灵敏度离合器"))
        .child(div().text_xs().child(
            "雷云原文：按住指定按键时临时降低灵敏度，松开后恢复。",
        ))
        .child(SettingRow::new("左键", format!("{left} DPI")))
        .child(SettingRow::new("右键", format!("{right} DPI")))
        .child(toggle_button(
            "clutch-linked",
            "左右联动",
            linked,
            cx,
            |this, cx| {
                this.edit_features(cx, |f| {
                    if let Some(c) = f.performance.sensitivity_clutch.as_mut() {
                        c.linked = !c.linked;
                    }
                });
            },
        ))
        .into_any_element()
}

/// ⑤ 灵敏度匹配器（`SENSITIVITY_MATCHER`）。
fn matcher_section(app: &AppShell, _cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let profiles = device
        .features
        .performance
        .sensitivity_matcher
        .as_ref()
        .map(|m| m.profiles.clone())
        .unwrap_or_default();

    card()
        .child(card_title("灵敏度匹配器"))
        .child(div().text_xs().child(
            "雷云原文：把本设备的灵敏度换算成另一款鼠标的等效值。",
        ))
        .when(profiles.is_empty(), |this| {
            this.child(div().text_sm().child("尚未添加匹配配置"))
        })
        .children(profiles.into_iter().map(|profile| {
            SettingRow::new(
                profile.target,
                if profile.matched {
                    "已匹配".to_string()
                } else {
                    "未匹配".to_string()
                },
            )
        }))
        .into_any_element()
}

/// ⑥ 回弹模式（`DEBOUNCE_MODE`）。
///
/// 二选一的**枚举型**设置项 → 真实界面是 `.s3-dropdown`。
fn debounce_section(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return card().into_any_element();
    };
    let mode = device.features.performance.debounce_mode.unwrap_or(DebounceMode::Fast);

    card()
        .child(card_title("回弹模式"))
        .child(select_row(
            "debounce-mode",
            "模式",
            mode.zh().to_string(),
            &DebounceMode::LABELS,
            app.open_select.as_deref() == Some("debounce-mode"),
            cx,
            |this, index, cx| {
                if let Some(picked) = DebounceMode::ALL.get(index).copied() {
                    this.set_debounce_mode(picked, cx);
                }
            },
        ))
        .child(div().text_xs().child(crate::i18n::t_or(mode.desc_key(), "")))
        .into_any_element()
}
