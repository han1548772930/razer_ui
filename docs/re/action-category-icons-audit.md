# 自定义页 · 动作类别侧栏（`.action-wrapper`）依据审计

2026-10-03。机器可读结果 [action-category-icons-audit.json](action-category-icons-audit.json)，脚本 [tools/audit-action-category-icons.cjs](../../tools/audit-action-category-icons.cjs)（`--check` 失败即报错）。
源码：`.ref/devices/182/static/css/main.48c20423.css`。

## 侧栏行（`.action-wrapper`）

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.action-wrapper` | `background-color:#0000;transition:background-color .2s;will-change:background-color` | 透明底；`.hover{background-color:#393939}` |
| `.action-wrapper:hover` | `background-color:#393939` | **本轮修正**：原先借用主题 `list_hover=#383838`（那个值出自滑块拇指的 `:active`），现在用源码的 `#393939` |
| `.action-wrapper.open,:active` | `background-color:#111` | `cx.theme().popover = #111111` |
| `.action-wrapper.open .head` | `color:#44d62c` | `cx.theme().primary = #44D62C` |
| `.action-wrapper .head` | `color:#ccc;font-size:12px;height:40px;line-height:14px;padding:13px 0 13px 50px;position:relative` | `h(40px)`、`text_size(12px)`、`px(10px)` + `gap(20px)`，即图标左起 10px、文字左起 50px ✓ |
| `.action-wrapper .head:before` | `background-position:50%;background-repeat:no-repeat;background-size:20px;content:"";height:20px;left:10px;position:absolute;width:20px` | `img(...).size(20px)` |

## 类别 → 图标（15 个本地类别，全部与源码类名对应）

`Category::capability()` 直接用的就是源码类名，图标按 `<id>.svg` / `<id>-active.svg` 打包（灯光类别是 CSS 内联 PNG，所以是 `.png`）：

| 本地类别 | 源码类名 | 普通图标 | 展开态图标 |
| --- | --- | --- | --- |
| Default | `DEFAULT` | `icon_config_default` | `icon_config_default_a` |
| Keyboard | `KEYBOARD_FUNCTION` | `icon_config_keyboard` | `icon_config_keyboard_a` |
| Mouse | `MOUSE_FUNCTION` | `icon_config_mouse` | `icon_config_mouse_a` |
| Sensitivity | `SENSITIVITY` | `icon_config_mouse_sensitivity` | `icon_config_mouse_sensitivity_a` |
| Macro | `MACRO` | `icon_config_macro` | `icon_config_macro_a` |
| Interdevice | `INTERDEVICE` | `icon_config_interdevice` | `icon_config_interdevice_a` |
| Profile | `SWITCH_PROFILE` | `icon_config_switch_device_profile` | `…_a` |
| Lighting | `SWITCH_LIGHTING` | CSS 内联 base64 PNG | 同上（另一张） |
| Brightness | `DEVICE_BRIGHTNESS` | `icon_config_brightness` | `icon_config_brightness_a` |
| Hypershift | `RAZER_HYPERSHIFT` | `icon_config_hypershift` | `icon_config_hypershift_a` |
| Launch | `LAUNCH_PROGRAM` | `icon_config_launch` | `icon_config_launch_a` |
| Multimedia | `MULTIMEDIA` | `icon_config_multimedia` | `icon_config_multimedia_a` |
| Windows | `WINDOWS_SHORTCUT` | `icon_config_windows_shortcut` | `…_a` |
| Text | `TEXT_FUNCTION` | `icon_config_text` | `icon_config_text_a` |
| Disable | `DISABLE` | `icon_config_disable` | `icon_config_disable_a` |

审计脚本会重新解析 CSS 里全部 29 个 `.action-wrapper.<类名>` 图标规则（普通 + `.open`），逐个核对打包产物确实来自对应源文件（灯光那张核对来源是 CSS 本身），因此换版本后如果图标改名会立刻报错。

## 尚未接入的源码类别（当前产品数据里不会出现，仅记录）

源码侧栏还有一批本类别未覆盖的类名与图标：`AUDIO_FUNCTION`、`CONTROL_KNOB`/`DIAL_MULTI_FUNCTION`、`GLOBAL_BRIGHTNESS`（全局亮度）、`SCROLLING`、`AI_LAUNCHER`、`APP_SPECIFIC`、`KEYBOARD_COMBINE_MOUSE_FUNCTION`、`TEXT_FUNCTION`（已覆盖）等。其中 `GLOBAL_BRIGHTNESS` 的本地文案已存在（`全局亮度`，与 `设备亮度` 成对），但它只出现在 `accessory_controls_data.json` / `hue_data.json` 这类走源码工作区的配件产品里，传统映射编辑器（182/653/777）的函数表中没有它，因此本类别未添加该变体——等这些产品的映射界面接入时再按其函数表点亮。
