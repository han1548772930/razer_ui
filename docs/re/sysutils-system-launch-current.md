# 当前系统属性及显示设置链

当前 `SysUtilsNative.dll` SHA-256 为 `01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318`。[IDA 原函数正文](evidence/sysutils-system-launch-ida.json)保留五个函数的 RVA、边界、字节哈希、Hex-Rays 伪代码和交叉引用；[当前源收据](sysutils-system-launch-current-evidence.json)独立记录产品页面、Electron action、host FFI 声明与 Rust 接入。

| 原函数 | RVA | 原命令 |
| --- | --- | --- |
| `OpenAudioProperties` | `0x70b40` | `control mmsys.cpl sounds` |
| `OpenKeyboardProperties` | `0x70b60` | `control keyboard` |
| `OpenMouseProperties` | `0x70b80` | `control main.cpl` |
| `OpenSoundVolume` | `0x70ba0` | `sndvol.exe` |
| `msSettings` | `0x70a30` | `explorer ms-settings:%s` |

原函数全部使用 `WinExec`，显示参数为 `5 / SW_SHOW`。`sounds` 参数决定声音属性初始页，键盘使用 `control keyboard`；不能以另一条 Control Panel 参数或 `cmd /c start` 作为原函数实现依据。显示设置页面传入固定 `display`：3880 当前页面 `jSA` 的下划线链接调用 `RiA.A.msSettings("display")`。

鼠标页面 182 的 `openMouseProperties`、键盘页面 555 的 `openKeyboardProperties`、音频页面 1398 的 `w/K` 经共享 wrapper 的 `electronAction` 进入当前 host SysUtils；host 声明四个属性函数 `void()`，`msSettings` 为 `void(string)`。当前 Rust 对应 [公共系统操作](../../crates/razer-platform/src/system.rs) 和 [Windows adapter](../../crates/razer-platform/src/platform/windows/system.rs) 已保留同一原命令和显示参数，页面现有点击直接使用该 adapter，无原 DLL 加载。Rust 将 `WinExec <= 31` 作为真实启动失败返回页面；原 JS FFI 的 `void` 声明会丢弃 native 返回值，两者分别记录。

这五个入口的反编译及现有页面调用已补齐，通用参数化 `msSettings` IPC、其他系统启动函数、完整 Initialize/Terminate 生命周期仍是独立缺口。`cargo check --locked -p razer-platform --all-targets` 已通过；没有执行系统命令、应用、DLL 或测试，运行验收尚未完成。
