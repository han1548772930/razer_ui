# 226 DPI 阶段行、分段滑条与编辑锁

2026-10-06。当前普通 Performance 的 DPI 主体已接入本地阶段编辑；仍是 partial，不能据此验收整页、产品或真实设备读取。

## 当前源码与独立核验

挂载链仍为 8355 的 Ri/wi → ys/Ms → cs/ls → ts/es；Qt=true 的 es 使用 Ft 分段滑条。仅使用当前 `.ref/devices/226/` 资源。维护工具 `tools/prepare-mouse-226-dpi.cjs` 记录 **26 项 AST、154 条 CSS、9 张原图**，包括当前 CONFIG、Pe/ve reducer、动作、文字、父链与 Ft。完整 SHA/UTF-16 范围见 [源证据](mouse-226-dpi-current-evidence.json)。分段表通过只允许 literal 与有限四则运算的静态 AST 求值得到，未执行厂商代码。

三个独立子任务分别核验了[阶段行及本地实现](prior-ui-verification-2026-10-06.md)、[滑条算法/样式](mouse-226-dpi-slider-source-review.md)和[轮询/连接/编辑锁生产者](mouse-226-polling-source-review.md)。滑条子任务另有 16 项 AST / 99 条 CSS 的独立收据；9 张父行资源从当前 manifest 指定 URL 补取，逐字节匹配现有嵌入资源。

## 阶段本地编辑

`mouse_dpi_rows.rs` 为独立核实过的 70/226 复用状态结构，字段通过 spec 明确区分。226 使用小写 x/y/independent/visible，70 继续使用源 profile 的 X/Y/Independent/Active。控件身份对应源 stage 槽位，拖排后同步新槽位的数值实体并清理临时编辑。

| 条件/操作 | 当前行为 |
| --- | --- |
| 阶段开启 | 每行显示 X，独立 XY 时另显示 Y；可见序号重新计数，隐藏行序号留空 |
| 阶段关闭 | 只保留当前 active 槽位，显示子列表的序号 1；隐藏可见性和拖动柄，数字及 XY 仍可编辑 |
| 隐藏行 | 滑条不可拖动，数字仍可编辑；数字提交不选择隐藏行。可见行提交才将 active 设为对应槽位 |
| 可见性 | 最少两条 visible；隐藏当前行时先找后方可见行，否则找全数组首个可见行 |
| XY 关闭 | x≠y 才将 y=x 并选择该行，即使其隐藏；x=y 时只改 independent，不改变 active |
| 拖排 | 移动完整对象，包含隐藏行与未知字段；active 随移动区间重映射。若新 active 隐藏，按源只找其后方可见行，找不到回槽位 1，不能替换成通用首可见回退 |
| 过期拖动 | 同时核 owner、原数组、原 active 和 restore 代际；切换到内容相同的新配置也不能提交旧拖动 |

源行背景、上下插入边框、拖动中的绿色覆盖、原 XY/拖动图标与影子徽标已接入。源 ls 的拖动影子编号读大写 Active，当前 226 数据只有 visible 时编号为空；本地保留这一实际源行为，空编号仍画圆徽标。没有凭共享函数新增阶段数量下拉或 two-way tab。

## 分段滑条与状态归属

`mouse_226_dpi.rs` 的 GridState 单独保留百分比位置、焦点和指针状态；原有 SliderState 数值模型仍保存实际 DPI，供数字编辑器和父工作区使用。

| DPI | 100 | 500 | 1500 | 10000 | 15000 | 50000 |
| --- | --- | --- | --- | --- | --- | --- |
| 轨道位置 | 0% | 15% | 30% | 45% | 70% | 100% |

位置到 DPI 使用各段 Math.round 语义，DPI 到位置使用源反算；动态步长按当前段更新。Base 的步长舍入可能产生略大于 100 的端点，进入分段查找前先钳制到 HTML range 的 0–100 范围，避免空查找。

Change 只更新实际 DPI 显示模型及数字框，不写 draft；Release 才通过 write_number 选择可见行、同步 linked Y 并发 MouseProductChanged。本地 profile/snapshot 从不包含未释放的滑条预览。单独的 row_value 对比父模型回传，避免当前预览值相同就漏掉源 props 更新所要求的百分比反算。

原位按下/松开 thumb 未移动时，Base 不发 Release；本地也监听真实 mouseup/up_out，以 pending.take 去重，补齐源 window mouseup 的同值提交。恢复配置、离页和锁定清理会取消 pending；阶段拖动使用独立状态，避免把滑条自身的 GPUI drag 错当阶段拖排并禁用自己。

每条滑条保留 FocusHandle，支持 Tab 与指针聚焦，四方向键按源抑制。Base 辅助功能增减只 set_value/notify 而不发 SliderEvent，因此 position observer 使用 expected_position 区分程序同步和独立语义调整；程序同步及 Change 均先记录期望位置，独立调整才提交。独立子任务已检查全部 setter、FIFO 事件顺序和反馈链，未发现程序回填被误当作编辑；实际辅助功能客户端未运行验收。

当前容器宽 300、高 20，覆盖普通 stage 的 250px 规则；命中轨道高 6，thumb 为 12×12。绘制顺序为非均匀刻度 → track/fill → 输入 → thumbTag/tip。填充仍保留源 `(W−16)*p+8`，不因 thumb=12 自行改公式；额外细刻度也只使用源数组。隐藏行只应用外层 opacity .3，不重复叠加 inactive slider 的 .3。

只有独立 XY 时显示 X/Y thumbTag 和浮动提示，普通行不新增数值 tip。提示文字是 X/Y；enter/move/leave/down 分别控制显隐，按源零时长切换；位置和尺寸来自当前 CSS/JS。字体与皮肤沿既有当前资源和语义颜色接口。

## OTFS 编辑锁与读取边界

`observe_dpi_editing_enabled` 已贯通 ProductWorkspace → SourceProductWorkspace → MouseProductWorkspace，只作用于 226。观察保存在 Option 字段，不进入 profile/snapshot；未观察时采用当前 Pe.enableStages=true 的 UI 初值，不能称为已成功查询 inactive。

false 时禁用 DPI 阶段主体及主开关，标题/说明保持；数字 Blur/step、grid、XY、序号选择、可见性、拖排和阶段开关均再次检查。只清理 DPI 的数字/滑条预览，不取消 Customize 滚轮的无关预览。普通切页/Help 仍清理全部相关编辑器。

独立子任务补齐了当前 `.ref/middleware/226/` 缺失源：manifest 与 2026-10-02 副本一致，index 版本/构建/git 匹配，manifest 声明的 **89 个 JS** 均完成 HTTP/SHA/Acorn 静态核验。实际发布链已证 `SET_ENABLE_STAGES = !runtimeData.isOTFSEnabled`，READY 和 OTFS 状态变化均有生产者。当前应用尚未接真实发布者，本批不能计为读取成功。

轮询 getter 实际走产品 HID command；同名功能不能直接当作现有 DLL Query。当前 mapping wrapper 的 isOtfsActive 是独立 bool/string/bool 回调候选，尚未证明与产品 runtime flag 同一 owner，不能复用 bool/string/string ABI 或执行验证。详见只读来源报告。DLL 修改、自动降档、OTFS task 和写回继续统一后置。

## 剩余范围与验证

浏览器动态 step 的 step-base/跨段量化、所有缩放/窗口输入/像素、提示实际遮挡、拖动影子指针偏移、真实辅助功能客户端及设备观察仍未运行验收。单独 70 的所有原始滑条样式和编辑锁未随复用自动完成。Polling Rate 的 BLE 隐藏、连接身份、目标字段和限速提示 O 仍待实际接入；Sensitivity Matcher、Mouse Properties 与整页其他条件继续待办。

允许的验证为 cargo check、格式化、JSON、AST/CSS 和资源字节校验。没有运行应用、构建、测试、安装器、厂商脚本或 DLL。整个目标保持 active，331 产品 / 1419 主页面为 partial，完整验收产品仍为 0。
