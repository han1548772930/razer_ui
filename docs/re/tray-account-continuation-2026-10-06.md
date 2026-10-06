# 托盘账户分支续接（2026-10-06）

本次直接读取当前 systrayv2 的 manifest、554 JS/CSS，扩充
`tray-account-current-evidence.json` 至 12 个 AST 区间，新增 `de` 提示生命周期
和 `ye` 文本按钮依据。没有运行厂商 JavaScript、应用或测试。

已补回设置齿轮的 bottom-left 提示：悬停立即挂载、100ms 后开始 100ms linear
淡入；离开或窗口失焦开始淡出，100ms 后卸载。重新进入取消旧计时器。
提示宽度为 300px、右缘对齐、位于按钮下方，边框 #5d5d5d、内边距 7px/8px、
字号 14px、行高 1.22、层级 1060，均取自源 CSS。颜色定义位于 TrayColors。
锁定 GPUI 的 Base Tooltip 只提供 tooltip 语义，没有监听器的外层和内容不会
建立交互 hitbox，因此不会截走指针。

空通知/空 Widgets 的设置按钮恢复独立的 100ms ease-in-out 颜色与 100ms linear
按压透明度过渡。访客账户按钮的 `account-guest-logout` 命令也不再被宿主静默丢弃；
本地没有账户服务，仍只报告服务未连接，不把访客登出当作已成功执行。
设置命令的独立窗口接线见 [Settings 实现](settings-window-implementation.md)。

只读验证：`node tools/prepare-tray-account.cjs --check`、
`node tools/audit-tray-account.cjs`、`python tools/prepare-tray-account-assets.py --check`。
最后一项覆盖三张原始/静态提取 SVG。统一资源校验也覆盖本组资源。

未完成：真实账户通道、填充通知/Widgets、多应用与动态窗口高度。默认仍为未登录。
当前 GPUI `on_hover` 在按住鼠标移动/拖拽时的行为与 DOM hover 存在边缘差异；
未作运行时或像素验收，不据此宣称整个托盘已完成。
