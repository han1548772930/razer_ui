# 当前 keyboard layout 监控

当前 `SysUtilsNative.dll` SHA-256 `01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318`。新 TimerFunc 和 formatter 已由安装的 IDA Pro 8.3 / Hex-Rays 分析私有副本取得；[五函数正文](evidence/sysutils-keyboard-monitor-ida.json)保留边界、哈希、伪代码和 xref，[静态验证收据](sysutils-keyboard-monitor-current-evidence.json)可重新检查原 PE 字节和当前 host/wrapper 文本。没有调用 DLL 或系统 timer。

`StartMonitorKeyboardLayout` RVA `0x77040` 仅在全局 timer ID 为零时执行：先 `0x70aa0` 读取调用线程 `GetKeyboardLayoutNameA` 的 KLID、零初始化并 `%x` 解析，再 `SetTimer(NULL,0,2000,TimerFunc)`。没有即时 change 事件；重复 start 不重设基线或 timer。`TimerFunc 0x77010` 每次调用同一个 getter，完整 32 位值不同时先更新缓存，再 `0x6bff0` 发送 `{event:"keyboardlayoutchange",data:{langId:整数}}`。formatter 的 `movsxd` 证明 `langId` 按 signed int32 扩展；不能只留下 LANGID 的低 16 位。

`StopMonitorKeyboardLayout 0x770b0` 调 `KillTimer(NULL,id)`，无论返回值如何均清 timer ID。当前 host FFI 声明 Start/Stop 为 void；Rust 清理失败返回错误，并移除 callback 状态，防止延后的 timer 消息访问已释放状态。

当前 host `sysutil/win/index.js` 具有真实订阅错误：Stop 将 `keyboardLayoutChangeEventList` 替换成 `foregroundEventList.filter(senderURL)`，随后依据原 foreground list 非空决定不调用 Stop。Rust 的 `source_stop_on_message_thread` 显式保留此分支，不把它改写成常见的键盘订阅计数逻辑。按 URL分发只消费该 view 的原通知。

[共享生命周期](../../crates/razer-platform/src/keyboard_layout_monitor.rs) 与 [Windows timer](../../crates/razer-platform/src/platform/windows/keyboard_layout_monitor.rs) 已实现原基线、2 秒 timer、变化缓存、signed 事件值、重复 start 和 stop 分支。Timer 为 `!Send` 并校验 owner thread；其回调直接查询创建线程的 keyboard layout。它必须由具有消息循环的 UI owner 调用。

当前 IPC worker 主线程阻塞于 stdin，未分发 HWND-less timer 消息。另建后台线程、轮询 foreground thread 或使用 GetKeyboardLayout 的其他 thread 参数都会改变原来 GetKeyboardLayoutNameA 的线程语义。因此没有添加假等效的 IPC start/poll，也没有将这部分标为完整接入：实际产品/feature caller、UI owner 路由仍是缺口。182 公共 wrapper 的方法存在只证明 API声明，不能证明产品启用监控。

已通过 `cargo check --locked -p razer-platform --all-targets`。未执行 timer、应用或测试，运行验收仍未完成；整库和整个本地程序的其他链路继续逐项处理。
