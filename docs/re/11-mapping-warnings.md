# 映射警告的真实触发条件

2026-10-01 只读对照 `.ref` 中 182、653 的页面、动作和 CSS。当前本地设备配置保存不应用键盘映射到硬件或 Windows，因此不把下面两种原生提示接到本地 JSON 保存。既有“保存 / 丢弃 / 继续编辑”是未保存草稿保护，仍独立保留。

## Windows 登录提示

源入口为 182 [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) 的 `QP`，653 [main.7b71cce5.js](../../.ref/devices/653/static/js/main.7b71cce5.js) 的 `$m`。

| 项目 | 可核对的原行为 |
|---|---|
| 初始状态 | `showWindowsLoginWarning: false`、`windowsLoginWarningHide: false`。 |
| 持久偏好 | `getWindowsLoginWarningStatus` 读取 `windowsLoginWarningHide`，值非 null 时按 `Boolean(value)` 转为隐藏状态；这是原存储服务的偏好，不是 profile 属性。 |
| 可见条件 | `renderWindowsLoginWarningAlert` 首先排除隐藏偏好；只对设备类别 `SYSTEM`、`KEYBOARD` 渲染，且 backdrop 仅在 show 标志为 true 时有 `show` 类。鼠标类别不会显示。 |
| 设置 show | 原 `saveChanges`、`saveTwoTapChanges` 更新 mapping / 列表、清除脏状态和编辑器之后调用 `showWindowsLoginWarning`。调用本身不等待硬件写入确认，不能描述为“收到硬件成功回执后”。 |
| 例外 | Global Shortcut 保存路径、Custom Command Dial 的提前返回保存分支不经过上述调用。 |
| 关闭 | 唯一 OK 按钮调用 `confirmWindowsLoginWarning(true)`；同时隐藏当前提示，并将隐藏偏好写入存储。该提示没有 Save / Cancel 分支，不是保存前确认框。 |

保存调用点位于 182 [5107.48fb3f50.chunk.js](../../.ref/devices/182/static/js/5107.48fb3f50.chunk.js)、653 [3241.69d3018d.chunk.js](../../.ref/devices/653/static/js/3241.69d3018d.chunk.js)。中文 `WINDOWS_LOGIN_WARNING_NOTE` 说明映射可能影响 Windows 登录，并建议使用辅助功能屏幕键盘；[英文原文](../../locales/en.json) 也明确指向登录风险。

当前设备映射只写入本地配置，未接通原生设备映射提交，不能给它附加当前会影响登录的提示。未来接入时应遵守类别、持久隐藏偏好、原保存分支及保存后提示语义；不能仅因出现 `Keyboard` 输出就弹出此窗口。

## Quick Remapping 冲突

同一主页面的 `render` 只在 prop `isConflictedQuickRemapping` 为真时显示 182 `jP` / 653 `qm`。选择器通过可选访问读取 `quickRemappingReducer.isConflictedQuickRemapping`；关闭回调 `onClickConflictedPrompt` dispatch `setIsConflictedQuickRemapping(false)`，动作名称为 `ON_SET_IS_CONFLICTED_QUICK_REMAPPING`。弹层引用外部点击关闭 hook，没有保存或覆盖映射按钮。

[中文文案](../../locales/zh-CN.json) `CONFLICT_DETECTED_DESC` 为“如要使用‘游戏手柄重映射’小组件，请确保受影响的按键当前不存在绑定。”`QUICK_REMAPPING_TOOLTIPS` 说明 W/A/S/D 可以变成游戏手柄方向摇杆。[英文文案](../../locales/en.json) 与此一致。因此这里的冲突不是两枚普通按键使用同一输出，也不是全局快捷键注册冲突。

182、653 主 bundle 各只有两处 `quickRemappingReducer`，均为选择器读取，没有 reducer 注册或定义；777 主 bundle 没有该名称。对 `.ref` 的精确字段、动作名搜索只得到主 bundle、常量和提取笔记，未找到这些目标设备可用的冲突检测算法或状态初值。不能把缺失的状态默认值描述为 false，也不能自行用普通 bindings 重建它。

本项目尚无可激活的 Gamepad Remapping 小组件、受影响输入集合、摇杆运行状态或原生冲突事件。需要这些状态的可靠来源后才能实现对应提示；当前不增设假的 reducer 或无条件警告。

## 样式依据

两套 CSS 的通用 `.remove-alert,.warning-alert` 一致：背景 `#111`、橙色 `#fd8611` 1 px 边框、5 px 圆角、宽 400 px、内边距 20 px 30 px；位置为窗口 50% / 50%，再 `translateX(-50%) translateY(-100%)`。标题 16 px、下边距 20 px；图标 25 px、右边距 10 px；正文 14 px / 16.8 px、居中。Windows 提示的 OK 为 90 × 27 px。

来源为 [182 CSS](../../.ref/devices/182/static/css/main.48c20423.css)、[653 CSS](../../.ref/devices/653/static/css/main.5425442a.css)。不要误用 `.factory-default .warning-alert` 或 `.mode-switcher .warning-alert` 的局部覆盖作为通用窗口几何。

本次只完成来源和适用性审计，没有运行应用、硬件服务或测试。
