# 2636 低死区警告弹窗（当前源，partial）

来源为 `.ref/devices/2636/static/js/main.82d8a835.js`，SHA-256 为 `a5b49ece13b13202b65e97d565b77ca61d22c8c6e14f9ec560dcee68d1155f9a`。收据由 `node tools/prepare-gamepad-2636-dialog.cjs --check` 静态检查；不会执行厂商代码。完整 AST、CSS 与图标来源见 [证据](gamepad-2636-dialog-current-evidence.json)。

## 当前实现

- `gamepad_deadzone_dialog.rs` 使用 Base Dialog 管理弹层与焦点，Base Button 管理操作；2636 原来的内联提示被替换，未向其他未审手柄推广。
- 死区为 0 使用 ZERO_DEADZONES_WARNING 分支，其余小于 7 使用 LOW_DEADZONES_WARNING 分支；第二段说明共用 LOW_DEADZONES_WARNING_DESC_2。
- Continue 保留当前本地值，关闭按钮恢复进入页面/最近一次 >=7 编辑的回退值。7→3→Continue→5→关闭应恢复 7。
- Recalibrate 仅发导航事件，父 SourceProductWorkspace 更新页面与历史；不启动校准，不伪造设备结果。
- 每次关闭销毁展开状态。说明容器采用源码 200px 最大高度、250ms Ease 高度、200ms Ease 透明度；箭头 -90°/90°、200ms Ease。
- 原模块没有背景点击或 Escape/Enter 默认提交处理，因此关闭这些隐式决策，按钮保留 Base 的键盘操作与焦点约束。
- 面板宽 402px、padding/gap 20px、边框 #fd8611、背景 #111、阴影 0/6/10 #0003；按钮与图标来自源码。颜色归于产品主题角色。
- 以实际原生 THUMBSTICKS 容器测量为定位依据，应用源码背景 margin -40/-20、面板 margin-top 110；不写死宿主顶部坐标。

## 验证与边界

允许的格式化与 `cargo check --locked --all-targets` 已通过；保持原有 3 条未使用方法警告。资源准备脚本包含 22 项 AST、1 份相关 CSS、4 项图标收据（2 个新嵌入资源，2 个复用资源）。原生资源校验覆盖 SVG/XML、hash、注册键及重复键。

仍为 partial：THUMBSTICKS 整页未逐像素完成，原生容器与网页父布局还不能认定一致；没有运行应用或进行交互/截图测试。弹窗替换不代表产品完成。真实 DLL 查询状态接入及硬件校准流程另列；DLL 修改、写回和保存均在当前实现范围，实际提交、校准响应及持久化链仍未完成。UI 修改通过既有 capture/snapshot 链保存本地草稿。

独立静态复核未发现三条操作链的阻断缺陷，已确认文案、源码回退规则、父子导航和展开过渡。仍有作用范围差异：Base Dialog 的输入拦截和焦点约束覆盖窗口，源码则是产品容器内的 absolute 遮罩；视觉测量定位不等于已经消除该输入范围差异。
