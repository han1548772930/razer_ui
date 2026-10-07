# 产品界面核对范围

本页列出当前分组证据和仍需完成的范围；[总队列](ui-review-queue-2026-10-07.json)中的路由/AST 覆盖不等于完整 UI 验收。

| 分组 | 已有实现与源码依据 | 剩余范围 |
| --- | --- | --- |
| 鼠标 | [75 页轮询矩阵](mouse-polling-pages-review-2026-10-07.json)；共享能力、滚轮、DPI、轮询与观察的当前行为见 [Mouse](mouse-ui-current.md) | 每款独立的帮助/说明/条件、Sensitivity Matcher、完整拓扑和运行读值；不可把某一产品实现推广至全组 |
| 键盘 Gaming Mode | [52 款来源与按键资格](keyboard-gaming-review-2026-10-07.json)；Windows 行读取 isWindowsKeyDisabled，Menu 仅在实际按键和源条件允许时显示，Copilot 有独立门控 | 各自完整开关、帮助、快捷键、混合选择值、Rapid Trigger 编辑及重置确认 |
| 键盘特殊页与手柄 | [逐根矩阵](keyboard-gamepad-groups-review-2026-10-07.json)、[2636 校准](gamepad-2636-calibration-native.md)、[输入指针生命周期](gamepad-trigger-pointer-lifecycle-native.md) | 2636 已有校准 UI；其余型号仍需分别完成向导/按侧弹窗、计时/仪表、错误/关闭/切配置状态。真实采样未知时不显示成功 |
| 音频 | [76 产品/309 页面子树证据](audio-review-current-evidence.json)，布局与本地操作见 [音频实现](audio-products-native.md) | 完整 EQ/预设/reset/重命名、混音通道增删、电平、真实播放列表及剩余空内容分支 |
| 系统与附件 | [范围矩阵](accessory-system-review-queue-2026-10-07.json)、[显示器](monitor-ui-current.md)、[Armory](armory-products-current.md)、[接收器](receiver-ui-current.md) | 每页的实际条件、弹层、动画和运行服务，不以描述符清单代替审核 |

音频亮度依赖已按各自源表达式接入：33 产品的两项关灯复选框依赖 brightness，idleMinutes 同时依赖 idle 开关和 brightness，共 99 条修复；1303/1304 原有两条门控继续保留。`AudioProductWorkspace::enabled` 同时用于渲染与编辑入口，迟到事件不能绕过禁用。对应维护工具为 `audit-audio-review-current.cjs --check` 和 `validate-audio-products.cjs`。

所有 UI 编辑、增删、Apply/Save 和本地草稿仍需完成。仅 DLL 设备/服务写回后置。分组 JSON 中的 source 状态用于查找证据，当前实现状态优先使用本文所链契约与[修复登记](ui-fix-registry.json)；静态检查不能证明视觉或硬件运行完成。
