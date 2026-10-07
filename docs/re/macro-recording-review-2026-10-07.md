# Macro 录制独立复查（2026-10-07）

本批重新解析当前 `.ref/applications/synapse/macro/` manifest 声明的 JavaScript，并静态读取当前 host 4.0.827 的 Mapping Engine 包装器。检查对象包括实际 GPUI 挂载、录制 actor、事件转换、取消/失败/停止以及可访问的控件语义。没有运行应用、测试、下载 JavaScript 或 DLL；实际录制及视觉验收仍未完成。

## 当前来源收据

`tools/audit-macro-recording-current.cjs` 只使用 Acorn/CSS 静态解析；完整原文、SHA-256、AST 范围见 [当前证据](macro-recording-current-evidence.json)。本批补入 `Ka` 阶段录制入口，避免仅凭已有通用 recorder 推定阶段按钮已接通。

| 来源 | SHA-256 | 重新读取的 AST 范围（半开区间） |
| --- | --- | --- |
| Macro `static/js/main.3f4b9604.js` | `fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d` | `_r` 670679–672060；`wr` 673231–673610；`Nr` 681266–685280；`Rr` 685284–686620；13139 `G` 80776–88096、`H` 88232–88702 |
| Macro `static/js/8190.61506f5b.chunk.js` | `003b116007bb415462e5175c09fc84928d2a20c8d1a7c73113a432643d2cbe9f` | 58190 `le` 1217772–1226158；`xa` 1355870–1357340；`Ka` 1357344–1361178；`Gr` 1390956–1392670；`Fr` 1398926–1402147 |
| Host `electron/modules/mapping_engine/win/index.js` | `32b0dab3a1a83a970b2501608d53c6544cab0c9a2b47054d7bd9a660fe4133c0` | enable/disable 5925–5995；start/stop/register/unregister/callback 7781–8049 |

另重新运行 `tools/audit-macro-record-options.cjs --check`：15 项当前模块作用域收据、122 个有 browser keyCode 的有序按键、89 条 CSS 保持一致。录制设置与快捷键仍以当前 58190 的 `Za/Gr/$r/Ur/Lr/vr/Rr` 等为依据。

## 已确认并修复

1. **阶段按钮仍为占位。** `phased.rs` 的实际 `macro-phase-record` 点击原先仅选择阶段并报告“服务尚未连接”。当前 `Ka` 明确执行 `l(e), E()`，即选择阶段后调用同一个 `xa.changeRecording`。现连接 `toggle_recording`，复用倒计时、开始/停止、失败提示及阶段归属；忙碌时保留保护。此前 [阶段审计](macro-phased-current-audit.md) 对该按钮的服务占位描述已由本批替代。
2. **取消与完成交错可能提交。** actor 原来每圈只取一个命令，阻塞查询返回 `stopped` 时可绕过已排队的 Cancel。现在逐次清空命令队列，并在 stopped 前再次处理。UI 另保存取消意图；即使取消发生在 stopped 之后、恢复映射/Shutdown 期间，迟到的成功结果也不会追加草稿。清理失败仍报告失败，不冒充成功取消。
3. **异常关闭可能永久锁住页面。** UI 原来只在 `Update::Finished` 解除 Starting/Stopping；异步接收通道异常关闭后没有收口。现在清除临时预览、回到 Idle、保留原草稿，并明确显示“原生会话清理状态无法确认”。代际检查防止旧线程修改新会话。
4. **开始前失败保留过期状态。** 选项校验或线程创建失败时现在清除临时预览，显示“无法开始录制，未更改草稿”；倒计时取消也清除临时预览。
5. **录制选项的原始输入可能越过异步归一化。** 直接读取输入绕过 100ms debounce 的设计保留，但开始前验证固定延迟非负、随机上下界为有限的 0–5 秒且 min ≤ max。无效本地输入在启动原生 recorder 之前报错，避免生成负数/倒序随机延迟。本项是本地持久化完整性约束，不声称源编辑器对所有临时输入采用同样拒绝行为。
6. **自定义控件测试可见性不足。** `macro-record-settings`、`macro-record-shortcut`、清空快捷键控件和延迟输入框架增加 `TestSupportExt` 与值/状态标签；倒计时下拉取得明确 ID `macro-record-start-delay`；radio 通过 Kit 的 selected 与语义标签暴露当前选择。实际控件绑定保持在原渲染入口，未增加替代测试界面。

## 复查后仍成立的边界

- `body.rs` 实际挂载 action bar、录制状态、普通/阶段列表；`recording.rs` 的 Starting 直到真实 started 才转 Recording，Stopping 直到 stopped、清理和 Finished 才解除；编辑、Undo/Redo/Save、文档切换均有 recording busy 保护。
- actor 独占阻塞 ServiceClient 和线程析构。GPUI 只发送命令；本批没有修改 backend/runtime 文件，也没有加载厂商 DLL 验证。
- 最终转换按 `_r/Nr/Rr/G/H` 读取微秒 delay、tick 去重、按键/鼠标状态和先前鼠标前缀；输入来源保存在 surviving row 的 `recorded_input`。重复按下合并与局部 pair ID 使用现有实现；最终转换与 `le` 的临时预览保持独立。
- 预览仅保留最后 50 行且不写 Undo/已保存基线。有效 stopped 且清理成功后一次追加本地草稿；Save 仍为本机 MacroLibrary 保存。
- Standard/Sequence/Phased 中源转换差异不能按“都是宏录制”合并。特别是 Sequence XBUTTON 的 `Rr` 与 `le` 按钮顺序不同，当前实现保留各自行为。

## 未完成与后续复查

鼠标轨迹仍缺少开始/停止两次 monitor/virtual screen 查询与坐标变换；全局开始/停止快捷键仍未绑定。当前快捷键编辑只在本机前台窗口捕获，UI 明确说明不能借此控制全局录制。随机相邻 delay 没有复现源 `parseFloat(object)` 的 NaN 持久化结果；该差异已有完整性说明。

本批覆盖录制子页面和阶段录制入口，不代表 Macro 所有页面或项目全部已实现页面已重新验收。宏文件树、其它行编辑器、Key Binds、Help 及公共应用页在后续逐页矩阵中分别复查。

检查：修改的 Rust 文件执行 rustfmt；录制 AST/CSS/host 静态证据已重建，录制选项静态 `--check` 通过。`cargo check --locked --all-targets` 由父任务统一运行，本子任务未重复执行；没有应用/测试/DLL 运行通过记录。
