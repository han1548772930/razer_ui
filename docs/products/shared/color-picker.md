# 颜色选择与屏幕取色

普通产品颜色弹层和 Studio 颜色编辑器都让用户从色板、数值或屏幕采样选择颜色；它们对“取到颜色”和“提交颜色”的处理不同。取色工具本身没有直接完成设备灯光写入。

## 普通产品弹层

PID70 的完整颜色组件把选择保存在本地 `selectedColor`、输入字段和标记中。屏幕取色的 `Color` 事件调用 `onChangeHex` 更新这些预览；`Done` 回调只记录结束，不调用父级保存。只有 `saveColor` 才调用父组件传入的保存回调，`cancelColor` 调用父级取消回调。

因此，“屏幕取色完成”与“确认此灯效颜色”是两次动作。最终改变哪个配置档、灯效或设备字段，继续由挂载该弹层的父级保存函数决定。

普通包装器在调用原生 Show 以前设置 `isRegisterColorPicker`，回调先检查该标志。Done 先向上发布，再清除这个门禁。显式 Close 则先清除门禁再调用原生关闭。完整普通 picker 组件本身没有调用 Close；组件卸载会移除鼠标和 picker 事件监听。

## Studio 编辑器

Studio 的 `Color` 事件调用 `onChange`，同时更新本地 RGB／HSV、数值框和预览。`Done` 使用当前 ref 值调用 `onAfterChange`，携带 RGB 数据和数值颜色。

Studio 包装器的 Done 经过延后处理，先等待 `closeColorPicker`，之后才向 UI 发出结束事件。Show 以前设置 `isOnColorPicker`，Close 先清除；终止函数还处理现有区域窗口关闭、发出 `FreeFFI` 请求并清空 JS 初始化状态。当前宿主的 `FreeFFI` case 直接返回 true，没有删除库／回调映射或卸载，不能把这次请求记为原生库已释放；详细宿主与区域窗口边界见 [Studio](../host/chroma-studio.md)。普通弹层与 Studio 不能共用一个猜测的 Done 提交语义。

## 原生窗口到底怎样取色

两条当前原生实现都会创建 layered、tool、topmost 弹出窗口，并把虚拟桌面截取到兼容内存 DC；后续 `GetPixel` 采的是这张固定快照。鼠标移动并不重新抓取实时桌面。

最初的 `WM_START_COLOR_PICKER` 使用 `GetCursorPos` 的全局坐标直接取样，没有减去虚拟桌面的 left/top。后续鼠标移动和按下使用有符号 16 位 client 坐标。两种坐标来源是原版的实际差别，不能把它们写成已经统一的坐标算法。

原生 `COLORREF` 转 RGB24 时交换红蓝字节。自定义光标来自资源组 102，热点、DIB 像素和 AND mask 都是取色交互的一部分。最初的按下处理发布一次颜色并设置为非 topmost；鼠标移动和失焦处理又把窗口设为 topmost。

SysUtils 的 Done 先投递内部消息，再发布事件；Studio 的 Done 发布后依靠 JS Close、消息与线程 join 链结束。替换取色器会先销毁并等待前一个 owner。

## 原件与调用者

完整原件路径与已核对的 16 个完整所有者在 [取色捕获](../../../.analysis/results/host-current/local-audit.json) 的 `screen_color`。

| 功能 | 原件与字节区间 |
| --- | --- |
| 普通产品完整颜色弹层 | `mouse-70-current/.../main.8f24b6a1.js` `[5404008,5413927)` |
| 普通 Show／Close 和回调门禁 | 同原件 `[4797134,4797299)`、`[4797300,4797610)`、`[4816696,4816914)` |
| Studio 完整颜色编辑器 | `synapse-chroma-studio-current/.../606.b47ade77.chunk.js` `[861,9355)` |
| Studio 原生包装器 | `synapse-chroma-studio-current/.../main.6b22e9cc.js` `[233246,239514)` |
| 宿主声明与消息转发 | `.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/sysutil/win/index.js` `[243,7358)` |

各产品灯效弹层的父级保存消费者、不同版本与附件分支继续在对应灯光功能文档中核对；本页描述的是这两条已直接核实的 picker 行为。

## 当前 Rust 调用者与资源所有权

| 实际行为 | 当前 Rust 所有者与结果消费者 | 对照结论 |
| --- | --- | --- |
| 普通产品取色预览、Save、Cancel | [lighting_color.rs](../../../crates/razer-pages/src/features/lighting/lighting_color.rs) 的 `receive_screen_color` 只把 Color 写入 draft、HSV 和输入框；Done 不调用保存。`save_editor` 才发布 `Saved` 或更新自定义颜色；`cancel_editor` 发布取消并恢复所属控件焦点 | 已保留普通弹层“采样与保存分开”的语义。子编辑器 Saved、自定义色持久化及最终设备写回仍是不同边界，不能将采样完成当作灯光已写入 |
| Studio 单色及渐变中的取色 | [chroma_studio_color.rs](../../../crates/razer-pages/src/features/chroma/chroma_studio_color.rs) 的 Color 更新本地字段并标记 dirty，Done 调用 `commit` 发布 `StudioColorEvent::Changed`；[chroma_studio_gradient.rs](../../../crates/razer-pages/src/features/chroma/chroma_studio_gradient.rs) 的 observe 只改当前 stop 草稿，Changed 订阅才发布 `GradientChanged` | 与所读 Studio 完整 owner 的预览／结束分离相符。Done 即使没有新 Color 也按当前值走提交，不额外调用 BeforeChange；该事件仍须沿所属图层、保存和软件帧消费者核对，不能作为设备 ACK |
| 页面与窗口生命周期 | [screen_color.rs](../../../crates/razer-pages/src/features/lighting/screen_color.rs) 保留 capture entity、窗口关闭订阅和轮询 Task；局部 generation 与全局 request sequence 共同拒绝旧请求。关闭编辑器、切换颜色或禁用会取消旧 capture | UI 每 16 ms 读取内存队列；资源查询只查询内存并排队，资源 I/O 和屏幕操作有后台所有者。丢弃旧事件与确认原生资源已释放须分别核验 |
| Windows 实际采样 | [平台取色入口](../../../crates/razer-platform/src/input/screen_color.rs) → [Windows 实现](../../../crates/razer-platform/src/platform/windows/screen_color.rs)，由独立 Rust worker 调用系统窗口、GDI、光标和 DPI API | 已有真实系统调用，不依赖官方业务 DLL。保留虚拟桌面固定快照、初始全局坐标与后续 client 坐标差别、COLORREF 字节转换及资源组 102 的光标像素／热点。非 Windows 分支明确尚无适配，不能由共享事件类型推断跨平台取色完成 |
| 结束与替换 | Windows `run` 在返回前依次收尾 window、snapshot、cursor 和 DPI，并合并清理错误；平台 worker 随后发布 Done／Canceled／Failed。替换请求经 `NATIVE_OWNER` 锁等待前一原生操作完成其清理 | 正常终止事件包含本次 `run` 清理完成的含义；相邻未消费 Color 合并，终止事件后不再追加颜色。替换等待不能扩展成任意取消／应用退出均已等待线程退出 |

目前仍有明确的取消完成缺项：平台 `spawn` 没有保留 `JoinHandle`，`cancel`／`Drop` 只设置原子取消标志；页面 `ScreenColorCapture::cancel` 同时移除轮询 Task 并丢弃 picker，没有保留并等待本次 terminal 事件的消费者。下一 picker 的串行锁可防止原生 owner 重叠，但当前页面关闭或应用退出不能由此确认该 worker 已结束及全部清理完成。这是静态生命周期差异，未通过运行时复现泄漏；后续应由真实生命周期所有者异步确认取消完成，不能在 UI 线程阻塞 join。

## 验证范围

本次重新核对上述收据的 16 个完整 JS owner，原件字节长度、SHA-256、切片内容和 body SHA 均匹配；两个 native slice 所引用的原件、完整函数捕获及身份对象也按实际字节核对，共 35 个不同文件身份未发现不一致。这不是对整个宿主／Studio 依赖闭包或全部原生函数的完成声明。

现有 [平台纯测试](../../../crates/razer-platform/src/input/screen_color_tests.rs) 检查 COLORREF、光标格式、模拟替换等待、取消不提交与失败事件；页面内另有普通 Save／Cancel、Studio 渐变预览与 Done、旧代次拒绝测试。这些测试本次未重新运行。替换测试主动等待 terminal，Drop 测试另行等待测试 channel，不能据此证明生产取消已有完成确认。

后续自动化验证须分别覆盖普通／Studio 保存语义、没有新 Color 的 Done、资源 pending／失败／重试、同窗及跨窗替换、关闭／禁用／切换、清理失败与旧事件拒绝；原生对照须按当前官方完整调用链比较事件顺序、采样坐标、颜色及终止清理。多显示器、负坐标、DPI、实际光标、窗口层级、键盘／焦点与全部产品父级布局、文案和资源仍须逐分支验收。未运行真实窗口、官方 DLL 或设备，本页的静态一致项不等于像素、焦点或设备写入已通过，遵守[验证规则](../../../.agents/rules/verification.md)。
