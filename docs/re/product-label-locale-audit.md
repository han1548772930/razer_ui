# 产品页文案的语言键对账（鼠标 / 键盘）

2026-10-04。鼠标与键盘产品页原先直接写中文字面量（面板标题、开关、滑条行、效果名、
输入名），与「文案必须回溯原代码」的要求不符。本轮把这些字面量换成源码自己的语言键，
方法如下，全部可机检：

1. **按键名找键（反向查表）**：`locales/zh-CN.json` 是前端语言包的原文导出，所以某个
   中文字面量只要与某个键的中文值完全相同，这个键就是源码用的键。例如「抬升距离」→
   `LIFTOFFDISTANCE`、「非对称中止」→`ENABLEASYMMETRICCUTOFF`。
   工具：`.work/label-to-key.py`（本轮脚本，未纳入仓库）。
2. **在设备包里核对别名**：设备包的导出表把 `别名:()=>变量` 与 `变量="语言键"` 分开写，
   因此 `alias → 变量 → 键` 两跳解析后，再用「该键是否存在于本地语言包」过滤掉同名变量
   冲突。工具：`.work/resolve-alias-key.py`、`.work/resolve-titles.py`。
3. **机检闸门**：`python tools/audit-locale-keys.py --check` 现在覆盖 409 个字面量
   `t("KEY")` 调用，任何一个不存在的键都会失败（本轮新增 27 个键，0 缺失）。

## 鼠标页（`crates/razer-pages/src/features/mouse_products.rs`）

| 原字面量 | 语言键 | 依据 |
| --- | --- | --- |
| 灵敏度 | `SENSITIVITY_HEADER` | 设备 100 的 `AFG` 组件 `title:Rt.AFG`→`SENSITIVITY_HEADER` |
| 灵敏度阶段 | `SENSITIVITY_STAGES` | 键值 "Sensitivity Stages" |
| 启用 X-Y 灵敏度 | `ENABLE_XY` | 键值 "Enable X-Y"（中文「启用 X-Y 轴」） |
| 回报率 | `POLLING_RATE_HEADER` | 键值 "POLLING RATE" |
| 无线省电 | `POWER_SAVING_HEADER` + `POWER_SAVING_DESC` | 共享功耗组件 `BR` 的 `ra.bSO`/`ra.rGE` |
| 低功耗模式 | `LOW_POWER_MODE_HEADER` + `LOW_POWER_MODE_DESC` | 路径 `/lowPowerMode` 与键名一致 |
| 低电量警告 | `LOW_BATTERY_EFFECTS_HEADER` + `LOW_BATTERY_EFFECTS_DESC` | 路径 `/lowBatteryEffects` 与键名一致 |
| 智能追踪 | `SMARTTRACKING` | 键值 "Smart Tracking" |
| 非对称中止 | `ENABLEASYMMETRICCUTOFF` | 键值 "Enable Asymmetric Cut-off" |
| 抬升 / 着陆 / 追踪距离 | `LIFTOFFDISTANCE` / `LANDINGDISTANCE` / `TRACKINGDISTANCE` | 键中文值与原字面量相同 |
| 表面校准 | `MOUSE_MAT_CALIBRATION_HEADER` | 键值 "MOUSE MAT SURFACE CALIBRATION" |
| 动态灵敏度 | `DYNAMIC_SENSITIVITY` | 键中文值相同 |
| 经典 / 自然 / 跳跃 / 自定义 | `CLASSIC` / `NATURAL` / `JUMP` / `CUSTOM` | 键中文值逐项相同 |
| 旋转 / 旋转角度 | `ROTATION` | 键值 "ROTATION"（滑条沿用面板标题，与亮度滑条同法） |
| 滚轮阶段 | `SCROLL_WHEEL_STAGES` + `SCROLL_WHEEL_STAGES_DESCRIPTION` | 键值 "Scroll Wheel Stages"（中文「滚轮触觉等级」） |
| 标准 / 清晰 / 超精细 / 自适应 / 平滑滚动 / 自定义 | `SW_STANDARD` / `SW_DISTINCT` / `SW_ULTRA_FINE` / `SW_ADAPTIVE` / `SW_SMOOTH_SCROLL` / `SW_CUSTOM` | 阶段 id 本身就是语言键，直接 `t(stage)` |
| 滚动张力 / 滚动刻度 | `SCROLL_TENSION` / `SCROLL_STEPS` | 键值 "Scroll Tension" / "Scroll Steps" |
| 智能滚动 / 滚动加速 | `SMART_REEL` / `SCROLL_ACCELERATION` | 键值 "SMART-REEL" / "SCROLL ACCELERATION" |
| 按键自定义 | `TAB_CUSTOMIZE` | 导航页签键 |
| 配对 | `PAIR` | 键中文值相同 |
| 默认（重置按钮） | `DEFAULT` | 键值 "DEFAULT" |
| 左键单击 / 右键单击 / 滚轮单击 / 上下滚动 / 后退 / 前进 | `LEFT_CLICK` / `RIGHT_CLICK` / `SCROLL_CLICK` / `SCROLL_UP` / `SCROLL_DOWN` / `STEP_BACK` / `STEP_FORWARD` | 键中文值逐项相同；分配按钮（Click/Menu/ScrollButton/Previous/Next…）走同一张表 |
| 提高 / 降低 / 循环灵敏度阶段、循环配置文件 | `SENSITIVITY_STAGE_UP` / `SENSITIVITY_STAGE_DOWN` / `CYCLE_UP_SENSITIVITY` / `CYCLE_UP_PROFILE` | 键值 "Sensitivity Stage Up" 等 |

效果名来自源码的效果表（设备 100 的
`{we.DU0:Static_Effect, we.xkP:Breathing_Effect, we.k9y:Spectrum_Effect, we.wrM:Wave_Effect,
we.ckz:Reactive_Effect, we.NnH:Ripple_Effect, we.fYY:Starlight_Effect, we.ZKq:Fire_Effect,
we.eaG:Wheel_Effect, we.ncy:Audio_Meter_Effect, we.jag:Tidal_Effect, we.p$l:Battery_Level_Effect,
we.TK1:Lamborghini, we.Rb7:Ambient_Effect}`），两跳解析后得到
`STATIC`/`BREATHING`/`SPECTRUM_CYCLING`/`WAVE`/`REACTIVE`/`RIPPLE`/`STARLIGHT`/`FIRE`/`WHEEL`/
`AUDIO_METER`/`TIDAL`/`BATTERY_LEVEL`/`LAMBORGHINI`/`AMBIENT`，全部与语言包中文值吻合。

未知输入/效果不再编中文，而是原样显示内部名（原版对未知项也是原样显示），并有测试固定这条回退。

## 键盘页（`crates/razer-pages/src/features/keyboard_products.rs`）

| 原字面量 | 语言键 | 依据 |
| --- | --- | --- |
| 调暗灯光 | `DIM_KEYBOARD_LIGHTING_TITLE` + `DIM_KEYBOARD_LIGHTING_DESC` | 键值 "Dim Lighting"，说明句与源码 `.h1-body` 一致 |
| 无线省电 | `KEYBOARD_POWER_SAVING_TITLE` + `KEYBOARD_POWER_SAVING_DESC` | 键值 "Wireless Power Saving" |
| 闲置后调暗灯光 / 闲置后进入睡眠 | `IDLE_FOR_MIN` | 与「关闭灯光」组件同一键 |
| 闲置时间（分钟） | 删除该行；取值按钮改用 `MIN`（"`{{value}} min.`"）模板 | 源码组件把说明放在 `.h1-body`，滑条本身无文案 |
| 游戏模式（面板 / 开关） | `GAMING_MODE_HEADER` / `GAME_MODE` | 键值 "GAMING MODE" / "Game Mode" |
| 仅在游戏中启用 | `GAMING_MODE_IN_GAME` | 键值 "Enable in-game" |
| 禁用 Windows 键 / Alt+Tab / Alt+F4 | `DISABLE_WINDOWS_KEY` / `DISABLE_ALT_TAB` / `DISABLE_ALT_F4` | 键值逐项相同 |

## 仍未接入的语言键

`python .work/cjk-summary.py` 的统计（本轮结束时的状态）：产品页三族（鼠标、键盘、手柄）
已经没有「当标签用」的中文字面量，剩下的按文件分布如下（含测试与界面预览模块，它们不进应用 UI）：

| 文件 | 标签类中文字面量 |
| --- | --- |
| `device_pages.rs` | 62（旧版设备页渲染器；只被 `system_button` 复用，是否仍挂载待下一轮确认） |
| `mapping_editor.rs` | 45 |
| `profile_transfer.rs` | 45 |
| `shortcuts.rs` | 44 |
| `dock_pairing/preview.rs`、`hue/preview.rs`、`aether_strip/preview.rs`、`wired_argb/preview.rs`、`wireless_argb/preview.rs` | 21/19/15/13/13（界面预览工具，不进应用） |
| `onboard_memory.rs`、`audio_page.rs`、`profile.rs`、`keyboard_controls.rs`、`linked_games.rs`、`customize_page.rs`、`settings.rs`、`keyboard_calibration.rs` … | 20/17/17/16/15/14/12/10 |

状态说明类文案（`surface::note(...)`、Tooltip、`accessibility_label`）保持中文，这是本项目
自己的「服务未接入」提示，不是雷云文案，例如
`mouse_products.rs` 的「配对界面尚未接入。」与「此页面的原生控件仍在接入。」。
`mouse_products_tests.rs` 里出现的 14 处中文是**测试断言里的期望值**：它逐条检查旧字面量
不再出现在页面源码里，防止回退。

本轮验证：`python tools/audit-locale-keys.py --check`、`python tools/prepare-device-locales.py --check`、
19 条静态审计/校验全绿，`cargo check --locked --all-targets` 零警告，`cargo fmt --all -- --check`
与 `git diff --check` 干净。测试只做编译检查（禁止运行）。
