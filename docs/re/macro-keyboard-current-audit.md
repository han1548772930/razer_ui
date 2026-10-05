# Macro Keyboard：当前源码续接

2026-10-05。来源仅为当前 `.ref/applications/synapse/macro/`。没有运行应用、测试、安装器、下载的 JavaScript 或 DLL。

## 来源与数据

维护中的 [audit-macro-keyboard.cjs](../../tools/audit-macro-keyboard.cjs) 静态解析 **58190.Nn/yn/pn/va**、**25572.O/C/R/M/P**、**13139/47990** 键盘模板、**46114.o** 键码目录、**90857.F/Y** 显示名称、**13139.d** 扩展键映射，以及当前 manifest 的 CSS。收据见 [macro-keyboard-current-evidence.json](macro-keyboard-current-evidence.json)。指纹用于追踪已复核文件，不代表运行或像素验收。

`keyboard_data.json` 保留 **144 个有序目录项、20 个布局名称表（含 default）和 3 个扩展映射**。目录里的 `parseInt(字面量, 字面量)` 仅做静态常量折叠，没有执行参考代码。名称表仅保留该键盘目录需要的 inputID；未把其他应用的目录替代为 Macro 来源。

## 已实现

- Keyboard 不再使用自由文本输入。捕获单个键，keydown/keyup 都可更新；Enter/Escape 是捕获内容。每次识别输入重置 150ms 提交期限，外点、失焦、切页或外层保存立即提交已有捕获结果。
- 按源浏览器规则：keydown 的 92 归一为 91，keyup 不归一；主 Enter 与数字键盘 Enter 区分；右 Shift 单独覆盖。Ctrl/Alt 仍按源浏览器目录首项匹配，不能把未接入的设备 inputredirect 当成已支持的左右区分路径。`outputFlag || flag` 保留 JS 的零值回退。
- Standard 新增按下/松开两行，共享本地 pair ID；Sequence 新增一条 null-state 行；Phased 的插入模板规则同样保留，但完整 Phased 编辑器仍未完成。更新一行通过 ID 更新另一行，保持按下/松开奇偶。整次更新进入一个 undo 快照。
- 可选 `KeyboardEvent` 随动作保存，包含 makecode/state/flag/key_type 和本地配对身份。旧文本行不自动推断为实体键，不从名称生成配对。分配 ID 时检查保存动作、活动动作、undo/redo 和保留草稿，避免恢复历史时错误配对；身份按文档解释，不是原生 GUID。载入时拒绝超过两行共用同一键盘 pair ID 的歧义数据。
- 保留 **25572.C** 的边界：Sequence 的 State 始终 null；孤立行找不到伙伴时只修改 Makecode/Type，不更新原 flag/State。退出捕获后按源字段重新解析名称，因此 Sequence 和孤立行可能失去依赖旗标的显示区别。这不是伪造真实硬件回读。
- 捕获框外层为 **160×27**；内层 1px 边框、5px padding、13.5/17px 字体，使用 `InputKey_kbf > div` 的 **margin:0 / width:inherit** 覆盖，未套用未挂载的 210px 宽度和 5px 顶部 margin。灰色未知项与绿色悬停来自源 CSS。
- Keyboard 行不再显示多余的“键盘”标签。使用源 **20×20 / mr10** 的按下/松开 SVG，null-state 不显示方向图。两张资源逐字节核对并加入常规资源准备流程。清除图复用与当前 Macro 字节一致的 `shortcuts-search-clear.svg`。

## 原始窗口输入适配

GPUI Windows 0.3.8 的 `events.rs::parse_immutable/parse_normal_key` 将两种 Enter 合并为 `enter`，修饰键经 `ModifiersChangedEvent` 合并；公开 `Keystroke` 不保留 virtual key、扫描码或 extended 位。字符还会经过当前键盘布局转换。直接从显示字符串反推这些信息无法满足源目录匹配规则。

因此 `keyboard_windows.rs` 在捕获期间安装 **当前 UI 线程、当前 HWND** 的 `WH_GETMESSAGE` hook，读取 WM_KEYDOWN/UP、WM_SYSKEYDOWN/UP 的键码和位置位。仅处理 PM_REMOVE；仅当前前台窗口可正常捕获，PrintScreen keyup 保留源码的特殊判断。没有低级全局 hook、其他进程窗口监听、按键注入、Razer 服务调用或自动执行动作。

捕获期间将该窗口已取出的键盘消息变为 WM_NULL，避免 Enter/Escape 和组合键落入页面快捷键；结束时 RAII 卸载 hook。只保留最近一次可识别按键，16ms UI 轮询消费它，以消息时刻计算 150ms 防抖期限；期限到达的实际提交有最多一轮正常调度延迟。退出路径同步消费未轮询消息。generation、文档、索引和动作基线共同拒绝过期提交。

当前线程 hook 无法承诺拦截操作系统保留的快捷键，且只会收到投递给该窗口的消息；没有冒充设备输入重定向。非 Windows 平台没有原始输入适配，入口不进入假捕获状态。原生 hook、焦点、缩放、系统保留键和窗口生命周期尚未运行验收。

## 源缺陷与剩余边界

`pn` 清除按钮调用 `onChange("")`，但 `yn` 在任何 state 更新之前就解引用未找到目录项的 `a.scancode/a.inputID`；目录中没有空 inputID。因此源按钮实际不清除键。本地保留“不改变内容”的结果，不复现异常。

`systemKeyboardLayout` 服务未连接：外层按 `yn` 显式使用 UnitedStates 回退，捕获标签按 `pn -> F(undefined, …)` 使用 default 表；没有把操作系统布局猜成 Razer 服务状态。`KEY_NON_US_BACKSLASH` 在 default 表缺失时，原生回显 inputID，避免源 F 的空值解引用。

仍缺设备 inputredirect 与映射/全局快捷键服务、原钢笔光标、行配对连线与动画、完整行复制/多选拖拽配对约束、Sequence/Phased 完整创作与 XML 导入导出。整份 Macro 和所有产品 UI 仍未完成。

## 验证

本批已通过 `cargo check --locked --all-targets`、rustfmt、Keyboard/Text/Launch/Macro/Shortcuts 专项静态收据和 scoped diff 检查。只有既有 `customize_page::layer_button` 未使用警告。JSON 校验为 35 份、32 份结构检查、3 份既有跳过、0 失败；语言校验为 442 个字面量键、0 缺失。Macro 资源准备完成时验证了 1,115 个主资源和 35 个服务 SVG；后续接收器/favicon 批次可继续增加资源。

没有运行测试或启动应用；这些检查不能证明窗口交互已经通过验收。
