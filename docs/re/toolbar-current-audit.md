# 当前工具栏与保存入口复核

> 2026-10-03 更新：本文后续关于本地底部保存栏、统一工作区保存和退出保存流程的实现记录已被取代。当前已移除额外底栏及通用退出保存弹窗；保留有来源的条件工具栏菜单和映射编辑器提示。详见 [当前托盘与关闭行为复核](tray-ui-current.md) 和 `save-close-current-source.json`。

2026-10-02 静态复核当前 Dashboard `0.0.86` / `2609221012`。来源版本见 [20-current-source-version.md](20-current-source-version.md)。以下位置是 UTF-8 解码后的零基字符偏移，不能套用到旧 App 构建。

## 官方实际上有哪种保存入口

当前工具栏没有始终显示的普通“保存”按钮；**有条件显示的未保存配置入口，其菜单提供“全部保存”和“全部丢弃”**。此前把移除本地 `save-all` 表述为“官方完全没有全局保存入口”不准确。

本次独立追踪 [App.72827d47.chunk.js](../../.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js) 的模块 `96776`（156043 起），没有用旧文档里的压缩符号替代这次追踪：

| 位置 | 当前代码证据 |
| --- | --- |
| 191777 | `We.render()` 的 `toolbar flex`：左侧后退、前进、刷新，中间标题，右侧条件入口、应用探索、设置、账户。 |
| 192722 | `right.children` 实际包含 `R`。不能只检查直接写在这里的 `div` 或搜索大写 `SAVE` 就认定没有保存入口。 |
| 163428 | `R` 读取 `profile.hasUnsavedChanges`，通过 `y.A` 维护同一应用来源的未保存设备/模块项目，监听窗口关闭、列表变化、`saveAll` 和 `discardAll`。 |
| 165361 | `0===r.length` 返回 `null`，没有项目时入口不出现。 |
| 165441 | 有项目时挂载 `header-unsaved`，图标上显示 `r.length` 徽标；点击打开下拉。 |
| 166162 | 菜单按钮调用 `y.A.saveAll(v)`，翻译符号 `l.Ltc`。 |
| 166284 | 另一按钮调用 `y.A.discardAll(v)`，翻译符号 `l.Uu5`。 |
| 157675、159482 | 应用探索 `E` 挂载独立 `/rz-app-menu/` iframe；它与保存菜单不是同一个入口。 |

菜单先显示两段说明，然后列出设备/模块名称，最后显示两个操作。点击项目行只收起菜单，不导航、不保存。来源 `v.source` 区分 Synapse 与 Chroma；不能用所有设置页和所有文本输入的 dirty 总数充当徽标。

[55.4e8559cb.chunk.css](../../.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css) 的对应规则：

- 166050 起：工具栏高 38；历史项 40×38；右侧项 46×38。
- 84020 起：`header-unsaved`；32px 圆底，`save-white.783e6aa4.svg` 图案 20px。
- 84899 起：徽标高 18、最小宽 18、字体 10、横向 padding 6。
- 85154 起：说明最大宽 220；菜单 padding `7px 14px 12px`；分隔线 margin `10px 0`。
- 85608 起：按钮最小宽 105、自然文字宽度、字体 12、padding `7px 13px 6px`；保存位于右侧，二按钮相距 8。

## 本地对应实现及保存范围

[header_status.rs](../../crates/razer-shell/src/shell/header_status.rs) 原先只包含离线、更新、兼容模式的独立预览；它不曾实现上述 `R`。现由 [header_unsaved.rs](../../crates/razer-shell/src/shell/header_unsaved.rs) 补回条件菜单。没有恢复本地额外的常驻 `save-all`。

[DeviceWorkspace](../../crates/razer-pages/src/features/workspace.rs) 将已提交设备配置差异 `committed_pending()` 与尚未提交的映射编辑草稿区分开：

- 条件菜单仅列出已提交配置与本机保存版本不同的设备，按设备身份生成稳定 ID。设置表单和快捷键编辑草稿不进入列表。
- “全部保存”调用独立 `Profiles` 范围，只捕获设备配置模型；不调用映射编辑器的 `prepare_save`，映射草稿仍由编辑器自身验证并提交。它也不提交设置和快捷键编辑草稿。
- “全部丢弃”仅恢复列表中设备的已提交配置，并保留同一目标上的映射草稿、录制和焦点，更新其原值比较基线。若恢复会切换/移除草稿所属配置文件或旋钮模式，先使用已有 `SourceAlert` 明确确认；选择继续编辑不回退任何设备，只有明确选择丢弃才移除受影响草稿。其他设备的未提交映射、设置表单、快捷键草稿不受影响。
- 项目行只关闭菜单。弹出层使用 GPUI Base `PopoverState` 的焦点捕获与返回、Escape 和窗口失焦关闭；`Positioner` 负责锚定，外点关闭只在可见阶段命中菜单。已有写入在执行时，两项提交操作暂时禁用。
- 当前只存在本机配置模型；这里没有宣称已连接原版跨窗口 `saveAll` 通道或已向硬件写入。保存结果仍由外壳底部状态说明。

## 下拉的实际显示与隐藏时序

`App` 模块 `96776` 的 `w`（约 159890 起，随后赋给 `M`）不是立即显示/关闭：

- 打开先设置 `shouldRender=true`，100ms 后添加 `.show`；公共 CSS 使用 200ms 线性透明度、200ms ease-out 位移，从 `translateY(-7px)` 到 0。
- 隐藏立即清除 `.show`，100ms 后移除内容并调用 `onHidden`；不能因为 CSS 写了 200ms 就擅自延长卸载时间。`R` 的 active 标记直到 `onHidden` 才清除。
- 打开会取消旧的 `_timeout`/`timeoutOnShow`；`timeoutOnShow` 只延迟调用 `onShown`，而 `R` 没有传入该回调，使用空默认值，因此没有额外可观察动作。
- 本地保留相同 100/200/100ms 阶段，并用框架 `Presence`/`Transition` 采样中断位置。未显示和正在隐藏阶段使用无按钮命中区的静态内容，保持原 `pointer-events:none`；关闭或项目清空会取消旧任务。减弱动画时直接切换状态。

这些是源码及测试源码核对；仍未运行应用或动画测试，不称为实窗验收。

## 删除常驻保存按钮暴露的快捷键问题

[Shortcuts](../../crates/razer-pages/src/features/shortcuts.rs) 的“保存快捷键”、离开编辑器时选择保存、确认删除，原来只改变内存 `items`；[AppShell](../../crates/razer-shell/src/shell.rs) 的 `ShortcutsChanged` 订阅仅 `notify()`。删除常驻保存按钮后，只剩退出时全量保存或设备保存顺带提交，不能视为独立保存已完成。

现已让已提交快捷键列表复用辅助偏好的串行 writer：

1. `committed_pending()` 只比较 `items` 与 `saved`，文本草稿不触发写入。
2. 捕获 `snapshot()` 的已提交列表；设备取 `saved_snapshot()`，设置取 `tutorial_snapshot()`，不会自动提交其他编辑器。
3. 成功后只确认捕获版本。写入期间再次提交的快捷键仍有差异，随后补写；当前新开的编辑草稿保持不变。
4. 失败保留未保存差异并显示错误，不无限自动重试。原配置读取失败时仍禁止覆盖。

显式工作区、条件配置菜单、设置保存共用 `PreparedSave` FIFO：在原点击时验证相应范围并捕获快照，再等待正在执行的写入。开始排队写入时仅更新不属于该请求范围的已保存基线，防止旧基线覆盖上一笔成功写入；不会重新提交等待期间新增的编辑草稿。写入及完成状态不依赖窗口是否仍存在，窗口句柄仅用于成功通知。

“保存并关闭”的意图由外壳单独保留；后续显式队列与辅助写入成功完成且没有新草稿后才退出。任何写入失败或最后发现未捕获的新草稿都会取消原关闭意图，保留窗口与实际状态，避免后续一次无关保存意外退出。关闭确认中的取消也撤销该意图。

设备页自身“保存到本机”和设置页自身保存仍可达；关闭窗口的全量保存路径保留。显式全量保存仍按原有范围验证和提交设备映射及快捷键草稿。

## 验证范围

已静态复核上述当前官方组件及 CSS，格式化修改的 Rust。新增 [header_unsaved_tests.rs](../../crates/razer-shell/src/shell/header_unsaved_tests.rs) 的条件显示、项目关闭、Escape、两个操作、保存中禁用和显示/隐藏延迟回归源码；补充 [shortcuts_tests.rs](../../crates/razer-pages/src/features/shortcuts_tests.rs) 的旧写入完成、后续提交和草稿隔离，以及 [mapping_focus_tests.rs](../../crates/razer-pages/src/features/mapping_focus_tests.rs) 的配置徽标与映射草稿隔离、混合状态回退和目标失效保护。关闭队列状态回归见 [save_queue_tests.rs](../../crates/razer-shell/src/shell/save_queue_tests.rs)。

本子任务没有运行应用、build、测试、下载 JS 或 DLL。主任务统一执行的 `cargo check --locked --all-targets`、`cargo fmt --all -- --check`、资源静态检查均已通过；上述新增测试源码已纳入全目标编译检查，没有运行交互测试或实窗验收。
