# 当前 SysUtilsNative 键盘布局查询

当前 `SysUtilsNative.dll` 的 `keyboardLayout` RVA `0x70b10` 调用 `0x70aa0`，后者清零 ANSI 缓冲区和 UINT 结果，调用 `GetKeyboardLayoutNameA`，忽略 BOOL 返回，以 `%x` 解析 KLID。当前宿主 FFI 声明返回 `int`，因此 Rust 保留 UINT 的 i32 位模式。原机器码、函数边界、当前 middleware getter/host 路由和 Rust 收据见 [证据](sysutils-keyboard-layout-current-evidence.json)。

共享入口在 `razer-platform::keyboard_layout::get`，Windows 实现在独立平台目录，typed worker 请求为 `KeyboardLayoutRead`，返回实际 `{layout:i32}`。这是调用线程的 Windows 布局，不能替换为前台窗口线程布局或保存的 profile 值；失败时原清零/解析语义不变。无需加载厂商 DLL、初始化服务或释放字符串。其他平台明确不支持这个 Windows KLID 能力，不能将其冒充原 `keyboardLayoutMac`。

页面及 macro 消费者、`StartMonitorKeyboardLayout/StopMonitorKeyboardLayout` 的两秒 timer、变更事件与 URL 订阅尚需独立完成；单项 getter 不代表完整 SysUtilsNative 替代。`cargo check --locked -p razer-platform --all-targets` 已通过，未执行此系统查询、应用、DLL 或测试。
