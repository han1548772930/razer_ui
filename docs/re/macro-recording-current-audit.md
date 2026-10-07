# Macro 全局录制接口复核（2026-10-07）

范围为当前 Macro 应用及 host 4.0.827 的全局录制链路。原先 `macro-record` 恒为 disabled，没有录制会话；现已连接独立后台 actor 和原生 Mapping Engine recorder。开发过程中未运行应用、测试、厂商 JavaScript 或 DLL。运行验证仍未完成，不能将静态复核描述成实际设备录制成功。

## 当前来源

`tools/audit-macro-recording-current.cjs` 只用 Acorn 解析 manifest 声明的当前源，输出 [JSON 证据](macro-recording-current-evidence.json) 的文件 SHA-256、AST 区间和原文。涉及：

- `.ref/applications/synapse/macro/` 的业务 IIFE：`_r` 回调数组、`wr`/`br` 状态、`Or` 订阅、`Nr` 录后处理、`Rr` 类型分派、`dr` 停止。
- 当前 58190 的 `xa` 倒计时与 `Gr` 分段录制按钮；录制时只显示 Stop、收起选项并隐藏录制图标。倒计时显示 `RECORDING_IN`，再次点击取消。
- 当前 58190 的 `le` 是独立临时行生成器：`recordingAction` 创建行，`fake_ui_container` 只保留最新 50 行；与录后转换器的归并和 Sequence XBUTTON 分支存在差异。相关 `SplitBtn` CSS 同步收录。
- 当前 13139 的 `G` 事件转换和 `H`（`combineSameKeydownsToOne`）归并。`ar` 来自业务 IIFE 的解构 `l.xJ`，不是未解析的全局符号。
- `.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js` 的 register/unregister、start/stop、callback、disable/enable Mapping ABI。

## 实现与数据边界

`recording_actor.rs` 独占 ServiceClient、子进程、阻塞请求及析构。GPUI 只持有命令 Sender，接收异步状态；关页、取消和退出不会在 UI 线程加载 DLL 或等待子进程。

Start 返回 accepted 后保持 Starting；实际 started 后才显示 Recording 并暂停本会话的映射。若同一事件批次已经含 stopped，则不再暂停已结束的会话。Stop 返回 accepted 后保持 Stopping，直到实际 stopped、草稿转换、映射恢复和 actor Shutdown 完成。先点 Stop 后收到 started 时，actor 仍等待 started，随后暂停/停止。Cancel 使用相同清理链路但不提交草稿。

`recording_decode.rs` 将实际 callback 数组转换为本地行：

- `time` 单位为微秒，转秒后保留三位；`time_tick` 只作去重标识。Recorded delay 的第一行使用 started 与首个 item 的回调接收时间差。
- 使用当前 144 键表与 source flag/makecode 映射，包括 numpad、右侧修饰键和键按下/松开状态。重复按下归并；本地配对使用文档既有 ID 高水位，保留首个按下的身份，避免重复按下归并后的悬空配对。
- 支持 Standard 的 Recorded/Fixed/Random/None、Sequence 的释放/滚轮和 Phased 当前阶段。Fixed 0 按源逻辑不插入延迟。Sequence 的 XBUTTON 顺序及 State 0 保留 `Rr` 的实际分支差异。
- Random 保留上下界，不抽样。固定延迟按 `H` 合并；相邻随机对象保留合法边界，避免复现 JS `parseFloat(object)` 得到 NaN 的持久化无效值。
- 点击停止按 `Nr` 去掉最后一个左键行；Esc 松开沿原录制取消路径丢弃这次录制。不会虚构补齐硬件没有返回的释放事件。
- 每个保留的输入行 `recorded_input` 保存原始 callback，包括完整鼠标前缀 buffer、事件 tick 与接收时间。该字段是本地来源信息，不代表设备宏文件或已写入设备。

收到有效 stopped 且完成清理后，整次录制作为一次 Undo 步骤追加到当前本地草稿。Save 仍沿现有本机 MacroLibrary 持久化流程；没有设备宏保存或 DLL 配置写回。录制期间编辑、行增删、Undo/Redo/Save、文档切换、重命名和删除均有守卫。

录制中使用独立的 `le` 预览路径，保留最后 50 行，最多每 50ms 更新显示；临时行不带持久化来源字段，不进入原始 actions、Undo 或已保存基线。它保留 source 临时视图仅合并相邻 State 0 按下、累计到后续释放延迟、Sequence XBUTTON 使用 4/3 的差异。按 `le.createMacroItemUI` 每次追加后的实际路径，列表跟随新增内容滚动。完整录制数据由独立最终转换器持有，因此 50 行显示窗口不会截断停止后的宏。

数组格式错误、未知键、无效延迟、回调错误、overflow、缺失 started/stopped、超过 100000 个输入或 32 MiB 原始事件会明确报错并保留原草稿；不将截断内容显示为录制成功。

## GPUI Kit test-support

使用实际控件路径的稳定标识：`macro-window`、`macro-item-list`、`macro-phased-item-list`、`macro-record`、`macro-record-options`、`macro-record-countdown`、`macro-recording-status`、`macro-recording-error`、`macro-recording-row`、`macro-action-row`、`macro-save`、`macro-undo`、`macro-redo`。临时行 ID 使用累计预览序号，50 行窗口移动时不会复用旧行 ID。自定义根、列表、状态、错误和行接入 `TestSupportExt`；按钮沿 Kit 原生 instrumentation。行包含值和 down/up 语义，错误/状态可通过语义标签读取。

未添加以普通单元测试替代真实界面验证的测试，也未运行 GPUI 测试。运行限制放开后，仍须通过真实渲染的 MacroPage 点击上述控件，验证 started 前状态、停止等待、失败保留草稿、Undo/Save 与关闭清理；本次静态检查不构成这些操作的通过记录。

## 尚未完成的录制子项

鼠标轨迹录制需要原版录制前后的 monitor info 与 virtual screen rect，当前未连接，模式控件禁用并显示原因；原始 callback buffer 保留，不伪造监视器状态。录制快捷键仍可作为本地设置编辑，但尚未绑定系统全局 start/stop，界面明确提示使用录制/停止按钮。前台单键编辑器的 Windows 消息钩子未被挪用为全局录制器。

本次进展不意味着 Macro 全部录制能力完成，也不改变其他 UI 与 DLL 只读状态观察的后续清单。
