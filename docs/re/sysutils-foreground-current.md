# 当前 SysUtilsNative 前台监控

原文件是当前宿主 `.ref/host-4.0.827/native-evidence/CommonDLL/SysUtilsNative.dll`，SHA-256 `01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318`。安装的 IDA Pro 8.3 / Hex-Rays 仅分析 `.work/ida-static/sysutils-foreground/` 私有副本；没有调用导出、调试执行、注册系统 hook 或运行应用。23 个函数的边界、字节哈希、伪代码和交叉引用保存在 [IDA 正文](evidence/sysutils-foreground-ida.json)，静态 gates 和当前 JS 调用收据在 [依据](sysutils-foreground-current-evidence.json)。

`StartMonitorForegroundWindow` RVA `0x73870`：terminate 状态拒绝；未 initialize 则初始化；已有 monitor 时直接 true。首次分配 monitor、保存 callback 后，`0x40ff0` 创建线程，入口 `0x40c80` 建立 `RzMonitorForegroundWindowClass` / `RzMonitorForegroundWindow` layered window，随后安装 `SetWinEventHook(3,0x17,NULL,callback,0,0,0)` 并进入 GetMessageW 消息循环。开始时没有额外发送当前前台窗口快照。

`0x41640` 只处理 event `3/9`，要求 `idObject=idChild=0` 和非空 HWND。`0x40870` 获取 HWND 所属 PID，用 access `0x1000` 打开进程，`QueryFullProcessImageNameW(flags=0,capacity=260)` 读取路径，随后 CloseHandle；不能用设备 DLL 或名称猜测此系统路径。

路径与上次路径的完整比较、当前 foreground HWND 比较及 Explorer 分支决定是否改变状态。Explorer 路径额外等待 100 ms 后重新查询，保留原稳定性判断。包含 `applicationframehost` 或 `wwahost` 的原 host 路径走 EnumChildWindows；`0x40630` 忽略 applicationframehost 子项，首个可查询的其他子项终止枚举。没有找到子项时，原循环最多等待 30×100 ms 后退回 host 路径。当前默认 CRT `0x9ce78` 只把 ASCII A..Z 转小写；Rust 采用同一分支，非默认 native locale 的大小写分支仍是显式缺口。

更新路径后，`SetTimer(window,1,300,NULL)` 延迟发送；后续事件重设同一 timer，构成原 300 ms 合并行为。WM_TIMER(id=1) KillTimer，复制当前路径并调用 callback。`0x7e360` 原字节 `mov rdx,[rdx] → jmp 0x73b00` 证明 callback 的字符串指针传到 `0x3c5a0`；伪代码遗漏此寄存器参数不能被解释为额外 GetForegroundWindow 查询。

原 formatter 对路径取 Windows filename 和 parent_path，保留大小写，通过 CP_UTF8 转换后构造：

```json
{"event":"foregroundWindow","data":{"name":"Browser.EXE","path":"C:\\Apps"}}
```

`StopMonitorForegroundWindow → 0x73b80 → 0x41250/0x41300` 先设置 stop 并移除 callback，再关闭窗口、WM_QUIT、join，线程退出后 UnhookWinEvent。原码包含 stop 在 monitor thread 自身执行时 detach、以及等待 window 创建最多 1 秒的特殊分支。Rust callback 只入队，worker IPC 在自身线程负责 stop，因而不从 callback 执行 stop/join；不复现会产生悬挂引用的销毁方式。

当前 host `electron/modules/sysutil/win/index.js` 保存唯一 sender URL。重复 start 不重复 URL；某 URL stop 后其他 URL仍订阅则不停止原监控；事件只发送给当时有效、未销毁、未崩溃、已订阅的 view。Rust 保留 URL 订阅及按事件发生时订阅集合分发的队列，单 view drain 不消耗其他 view 的事件。

[共享生命周期](../../crates/razer-platform/src/foreground_monitor.rs) 和 [Windows 适配器](../../crates/razer-platform/src/platform/windows/foreground_monitor.rs) 实现上述线程、窗口、hook、路径、timer 和取消链。worker IPC 提供 `ForegroundMonitorStart { view_url }`、`ForegroundMonitorStop { view_url }`，返回 bool；`ForegroundMonitorEvents { view_url }` 返回原事件对象数组。未成功注册 class/window/hook 的错误明确返回；timer/message-loop 错误由 drain 返回，不能当作成功前台观察。其他平台明确 unsupported。

产品 182 的当前公共 middleware chunk 含 `startMonitorForegroundWindow → foregroundWindow → activeProfile.scrollWheelStages.browsingModeEnabled → name.toLowerCase() → BIS({appName})` 代码。但公共代码存在不证明该产品启用功能：182 是 DeathAdder V3 Pro，产品 bootstrap/feature 没有证明 haptic/scrollStage 的实际注入。此收据仅说明共享 helper；需要继续核实具备该 feature 的实际产品和其 profile/haptic 链，不能自动给 182 运行此任务。

已完成静态源码与 native 语义、Rust OS 隔离实现和 IPC；`cargo check` 通过不等于运行验收。Shell 实际 feature 产品消费者、非默认 CRT locale、完整 SysUtils 初始化/终止及其他监控能力仍未全部完成。本轮未执行原 DLL、Rust 应用、OS hook 或测试。
