# Windows 动态照明设置与设备控制权切换

该设置决定支持的设备交由 Windows 动态照明还是本机 Chroma 引擎管理。开关不是一个单独的颜色命令；它会改变设备集合、LampArray 会话与灯光驱动的处理路径。

## 设置页的显示与操作

当前设置控件只在 Windows build 至少为 22631，或 build 为 22621 且 revision 至少为 2506 时显示。它读取并监听 `isDynamicLighting`，同时监听 `wdl-devices` 中的切换状态；卸载移除这两种订阅。

用户切换后，控件立即改变自身状态并写 `isDynamicLighting`。这个完整控件没有 Apply，也没有等设备 ACK 后才更新或失败后回滚的分支。旁边的系统设置入口调用本机 `msSettings("personalization-lighting")`。

原生 LampArray 初始化有另一条 OS 门槛：Windows major 大于 10，或 major 为 10 且 minor 大于 0，或 major/minor 为 10/0 且 build 大于 22621，或 build 为 22621 且 revision 至少 2506。它和设置控件的显示条件不完全一致。

## 开关如何影响后台

灯光引擎用已有 mutex 串行处理存储变更，解析开关后调用 `updateRazerWDLDevices`，再发布模式事件。控制权转移期间把 `wdl-devices.isSwitching` 设为 true，并在 finally 清除。

切向 Windows 时，先从 Chroma 集合移除匹配的设备，再经每设备 mutex、窗口共享存储和本机缓存加入枚举到的 Razer LampArray 设备。

切回 Chroma 时，先移除 Razer LampArray 的 Chroma 项、关闭并移除 LampArray 设备，再枚举本机 USB 并准备匹配 container。原版对设备准备有一秒间隔；存在 readiness 任务时，还会等待 readiness 和两秒稳定间隔。这些延迟是该完整函数的实际顺序。

## 驱动、服务与应答分别表示什么

引擎调用 Configure，载荷为 `{type:"mode.set",skip_wdl_devices:useWDL}`；这个调用者没有等待该 Promise。原生分发表把 `mode.set` 指向 RVA `0xb790`：载荷是对象时按存在的布尔字段修改 pause 和 skip 标志，即使两字段均未改变也会返回 true。

真正处理设备帧的 RVA `0xf4d0` 另读 skip 标志。仅当设备对应标志等于 1 且 skip 位为真时，直接返回 true 并跳过协议翻译器；其他设备仍走各自协议虚表。因此 mode.set 的返回不能解释成所有设备都收到了灯光帧。

LampArray `JsonApi` 通过本机命名管道 `\\pipe\\rzwdl_%s` 进入 RPC；服务调用表和反向 callback 表是两套契约。原生初始化顺序为配置 FFI、SetupRPC、Startup、SetNodeFFIEvent、StartWatcher；某阶段失败会按原函数返回 false，不能额外补写原函数没有的恢复步骤。

灯光驱动 Configure 返回 malloc 所有的 UTF-8 JSON，包装器解析后用配对 FreeString 释放。Startup／Shutdown 在当前 JS 声明中是 void；不能采用反编译器推断出的残留寄存器返回值当作 JS 的返回契约。

## 原件定位

本页逐字节复核 [完整捕获](../../../.analysis/results/host-current/local-audit.json) `dynamic_lighting.complete_source_owners` 的 53 个所有者与对应原件。

| 所有者 | 原件与 UTF-8 字节区间 |
| --- | --- |
| 设置控件 | `synapse-settings-current/.../720.1e5d1c8f.chunk.js` `[84051,86061)` |
| 存储事件串行化 | `synapse-lighting-engine-current/.../main.a383ea9e.js` `[908613,909593)` |
| 设备控制权切换 | 同原件 `[909593,911799)` |
| LampArray 原生初始化、查询与清理 | 同原件 `[278509,287000)` |
| 原生灯光驱动包装 | 同原件 `[405624,425118)` |

原生 RPC 的全部间接目标、每类设备的最后协议翻译器与其关闭分支仍需沿该捕获继续追踪；本页没有把选定调用的反编译结果扩展成整套原生模块已闭合。
