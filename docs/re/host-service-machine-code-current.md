# 当前 CommonDLL 服务读链函数正文逆向

2026-10-09，依据官方 host 4.0.827 静态提取的 `mapping_engine.dll` 和 `simple_service.dll`。本次新增 8 个导出、100 个代码范围的静态反汇编，包含导出、singleton、构造器、虚表目标、回调适配器、服务线程任务、最终版本/模式/timeTick序列化及 Windows 音频枚举实现。完整地址、指令、机器码 SHA-256、`.pdata` 边界、内部 call/jmp、字符串和未解析间接调用在 [机器证据](host-service-machine-code-current-evidence.json)。没有加载 DLL、调用导出或执行应用。

这是由机器码得到的控制流和数据流说明，不是恢复出的厂商原始 C++ 文件。`.pdata` 为区间边界证据；没有完整 unwind 项的叶函数/thunk 使用明确 96 字节上限，并在 ret/jmp 截止展示。此类窗口 hash 不代表证明了完整函数长度。编译器内联、任务对象类型、部分虚表动态目标与最终设备/系统 API 尚未全部还原。

## 二进制身份

| DLL | SHA-256 | 新检查导出 |
| --- | --- | --- |
| mapping_engine.dll | `6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a` | Initialize/Shutdown/getGlobalMode/getGlobalShortcuts |
| simple_service.dll | `f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b` | Initialize/Shutdown/GetVersionInfo/EnumerateAudioDevices |

两份 PE 均为 AMD64，ImageBase `0x180000000`；下文统一使用 RVA。导出中保留编译来源字符串 `mapping_engine_dll_impl.cc` / `simple_service_dll_impl.cc`；Initialize 日志引用版本字符串分别为 `1.3.16.5` / `1.2.2.10`。这是文件内部日志字符串，不能替代真实版本查询响应。

## 导出到真实内部方法

| 导出 | RVA | singleton/thunk | 构造器证明的首虚表 RVA | slot | 内部方法 RVA | 导出回调 closure |
| --- | --- | --- | --- | --- | --- | --- |
| mappingEngineInitialize | `0x1fc30` | `0x1e454 → 0x1000` | `0x2df010`，构造器 `0x1060` 写入 `[this]` | `+0x08` | `0x1350` | `0x1fb30` |
| mappingEngineShutdown | `0x1fed0` | 同上 | 同上 | `+0x10` | `0x2090` | `0x1fdd0` |
| getGlobalMode | `0x4b990` | 同上 | 同上 | `+0x338` | `0x13340` | `0x4b740` |
| getGlobalShortcuts | `0x4bd20` | 同上 | 同上 | `+0x348` | `0x138d0` | `0x4bad0` |
| simpleServiceInitialize | `0xedf0` | `0xd754 → 0x1000` | `0x1f2040`，singleton 首次初始化写入 | `+0x08` | `0x12f0` | `0xecf0` |
| simpleServiceShutdown | `0xf090` | 同上 | 同上 | `+0x10` | `0x19c0` | `0xef90` |
| simpleGetVersionInfo | `0xf420` | 同上 | 同上 | `+0x38` | `0x2780` | `0xf1d0` |
| simpleEnumerateAudioDevices | `0x16d40` | 同上 | 同上 | `+0xb0` | `0x4cb0` | `0x16af0` |

这不是根据方法名字猜虚表 slot。导出实际读 `[object]` 再读该 slot；singleton/构造器实际写入相应表；工具再从表的具体字节读取目标 VA 并验证指向可执行节。

八条导出的共通逻辑可描述为以下伪码；`internal_*` 名字是本次解释用名称：

```text
export(callback_ptr):
    object = internal_singleton()
    closure = allocate(0x30)
    initialize_closure_refcount_and_call_destroy_thunks(closure)
    closure[0x20] = export_specific_completion_function
    closure[0x28] = callback_ptr
    virtual_method = object.vtable[verified_slot]
    virtual_method(object, owning_closure_reference)
    // 日志与 stack-cookie 检查省略；没有直接返回设备数据。
```

closure 的 `+0x20/+0x28` 只属于这些已验证代码，不是可公开复用的 native 结构 ABI。`0x1cb370` / `0x94990` 的指令显示写入引用计数与调用/释放相关指针。导出把 owning reference 移交内部方法；不能仅用返回执行结束推断回调已完成。

## 初始化、关闭与外部回调门控

mapping `0x1350` 引用 `MappingEngine::Initialize`、`already initialized; ignoring repeat call`、`MappingEngineMainThread`；shutdown `0x2090` 引用 `not initialized; ignoring Shutdown`。simple 内部 Initialize `0x12f0` 引用 `SimpleServiceMainThread`，Shutdown `0x19c0` 保留线程/任务清理逻辑。完整线程 helper 实现尚未全部恢复，不能据此宣称所有后台服务均已 ready。

回调适配器正文证明：

```text
mapping initialize completion (0x1fb30):
    atomic_exchange(byte_at_0x394748, 1)
    if callback != null: callback()

mapping shutdown completion (0x1fdd0):
    if callback != null: callback()
    atomic_exchange(byte_at_0x394748, 0)

simple initialize completion (0xecf0):
    atomic_exchange(byte_at_0x25b088, 1)
    if callback != null: callback()

simple shutdown completion (0xef90):
    if callback != null: callback()
    // 该已检查 closure 内没有 mapping 的对应 flag-clear 指令。
```

两个 DLL 的 query completion 先检查 callback 非空，再检查各自 flag 的 bit 0；然后将 success 放入 ECX、reason 放入 RDX、结果文本放入 R8，调用外部回调。结果字符串有 inline/heap 分支，通过对象 `+0x17` 标志选择字符串地址；JS wrapper 的 `bool,string,string` 声明与此数据流一致。字符串在当前 callback 路径使用，不应保留其原始指针跨回调访问。

这里支持的是**这些具体 AMD64 函数的数据流**。没有证明所有声明、任意同名 DLL、其他版本或 callback 线程 ABI。也不能把 simple shutdown 没有在该 closure 清 flag 当成整库从不清 flag，其他线程清理未全追踪。

## query 经由内部线程，不在导出里直接读硬件

| 读接口 | 内部方法/第一任务 | 服务层/第二任务 | 可确认内容 | 尚未到达的边界 |
| --- | --- | --- | --- | --- |
| getGlobalMode | `0x13340 → 0x13530` | `0xc352a → 0xc3710 → 0xc0124` | 不同失败分支；读取 hypershift/otfs、模式记录并生成 JSON；timeTick qword转十进制字符串对象 | 模式记录的创建/删除、事件更新与 timeTick 来源/单位需继续 |
| getGlobalShortcuts | `0x138d0 → 0x13ac0` | 查询 hotkey service；任务闭包内输出 JSON | 源名字 `GetHotkeys`，键为 `virtualKey`、`modifiers`、`argument`；缺线程是 `invalid hotkey service thread` | hotkey 具体容器类型、排序/去重、映射持久化全过程需继续 |
| simpleGetVersionInfo | `0x2780 → 0x2970` | `0x21ad2 → 0x21cc0 → 0x1d4ba → 0x28200` | apps service 再提交到 IO thread；最终版本对象键/常量与动态 elevation 值已追到 | elevation getter 成功/失败规则、服务/IO thread 创建与销毁需继续 |
| simpleEnumerateAudioDevices | `0x4cb0 → 0x4ea0` | `0x32082 → 0x32270 → 0x3a170` | IDA 已核实 `0x31db0 → 0x38f60` 构造 AudioServiceWin，虚表 `0x1f54d0 +0x30` 返回 `type/id/containerId/name` 数组；缺实例另有明确错误分支 | Rust 已接入当次枚举列表；持续事件缓存、通知/音量/会话回调与完整失败语义仍需实现，详见 [音频专项](simple-audio-current.md) |

query 内部方法的第一部分都读取线程对象及其子对象；缺对象时构造 false + 错误字符串回调；正常路径构造任务，把原 callback ownership 移入任务，再提交给内部线程。线程提交与执行是两个步骤，因此同步进入导出成功不等于读数成功。

当前可确认的概念伪码为：

```text
internal_query(thread_state, callback):
    if required_thread_object is absent:
        callback(false, observed_invalid_thread_text, empty_result)
        return
    task = owning_task(captured_state, move(callback), observed_task_function)
    submit_to_thread(task)

query_task_on_thread(...):
    if required_service_thread_or_instance is absent:
        callback(false, observed_service_error_text, empty_result)
        return
    invoke_verified_service_method_or_build_verified_hotkey_json(...)
```

这段没有虚构服务成功、音频设备列表、版本值或全局模式 schema。完整函数正文保留在 JSON，真正逻辑缺口仍需逐节点继续逆向，不应用“get 开头所以可直接调用”填补。

## 最终模式和版本字段的进一步逆向

`getGlobalMode` 的最终实例方法 `0xc0124` 创建 JSON 对象，检查实例 `+0x58/+0x59` 状态字节，仅在为真时分别写入 `hypershift:true` / `otfs:true`。它调用 `0xbfe62` 遍历实例内模式记录列表：每个记录经 `0xbec9a` 生成对象，字段为 `guid`、`scope`、`input`、`output` 和 `timeTick`。前四项从记录内字符串对象读取；timeTick 经过独立转换 helper 后写入。列表非空才加入 `globalmode`，然后序列化，并回调 success=true、空 reason、序列化结果。

```text
DeviceModeService_GetGlobalMode(instance, callback):
    result = {}
    if observed_byte(instance + 0x58): result["hypershift"] = true
    if observed_byte(instance + 0x59): result["otfs"] = true
    records = []
    for source_record in observed_mode_record_list:
        records.append({guid, scope, input, output, timeTick_from_conversion_helper})
    if records is nonempty: result["globalmode"] = records
    callback(true, "", serialize(result))
```

这解释了为什么未出现某键不能擅自补成 false。记录结构是当前私有实现证据，不是新的公开 ABI。timeTick进一步追到`0xbedE5`从记录`+0x68`加载qword，`0xbedF1`调用`0x1bf9f0`，再由`0xbee17`传入`0x1b5020`的JSON插入路径。新增两段`.pdata`正文保存在同一机器证据，使本轮范围为100段。

`0x1bf9f0`以无符号乘法常量`0xCCCCCCCCCCCCCCCD`及高位右移3计算除10，余数与`0x30`组合成ASCII数字，使用无符号`ja`判断是否继续，最后复制数字并添加零终止。函数含长度22的small/heap分支；当前输入为uint64，十进制最多20位，因而该调用落在small分支。`0x1b5020`移走该字符串对象、清输入并以内部tag4交给JSON构造/插入方法。可证明它在这一层把64位输入转成十进制文本再交给JSON；不能补成日期格式，也不能因字段名将其猜成Unix毫秒。单位、epoch、原始值写入者与完整JSON序列化分派仍待继续。

`simpleGetVersionInfo` 通过 apps service 的 `0x1d4ba` 移交到 IO thread，最终 `0x28200` 引用 `GetVersionInfoOnIOThread` 和 `services/apps_service/win/apps_service_win.cc`。此函数向 JSON 对象写以下键/值，然后加入由动态 helper 取得的 elevation 版本：

| 已确认字段 | 该文件机器码中的值来源 |
| --- | --- |
| `Simple Service Version` | 常量字符串 `1.2.2.10` |
| `Apps Service Version` | 常量字符串 `1.2.2.4` |
| `Audio Service Version` | 常量字符串 `1.2.1.16` |
| `Elevation Service Version` | `0x280a0 → 0x2dc9a` 路径所得字符串，实际值未执行读取 |

它把 success=true、reason/result 字符串保存进新任务，提交回线程，再逐层完成回调；临时 JSON、字符串、引用计数在函数退出前清理。`0x25922` 实际是捕获实例/callback 的任务对象构造器，并非版本字段解析器。静态常量是该签名二进制中的逻辑证据，UI 仍必须使用实际响应或明确 unavailable，不能把文档常量冒充本机成功读取。

audio 的 `0x32270` 只在 service instance 存在时，通过它的首虚表 `+0x30` 传入两个可写字符串对象，保存 AL 返回成功位；不存在实例时构造错误字符串。`0x6cb52` 是小字符串/heap 分支的字符串写入，并非枚举函数。后续 IDA/Hex-Rays 证据已将该虚表目标消歧为 `0x3a170`，详见 [原件、32 个函数及 Rust 消费者](simple-audio-current.md)；本页原 dumpbin 证据保留原取证范围，不把后续结论倒填为当时已证明。

## 同一当前 DLL 中的 Windows 音频设备实现

独立追到 `AudioServiceWin::Initialize`（`0x39070`）调用 `0x397aa`。后者保留 `AudioServiceWin::EnumerateAudioDevices`、`CoreAudioUtil::GetDeviceName`、`EnumAudioEndpoints/GetCount/Item` 错误分支字符串；COM 枚举器由 `0x3e84c → 0x3e864` 创建。证据记录了导入槽 `CoCreateInstance`（`0x23d5b8`）、`CoInitializeEx`（`0x23d5c0`）以及 GUID 的原始 RVA/字节：

| 常量 | 原始 RVA | GUID |
| --- | --- | --- |
| MMDeviceEnumerator CLSID | `0x1f5b50` | `bcde0395-e52f-467c-8e3d-c4579291692e` |
| IMMDeviceEnumerator IID | `0x1f5b40` | `a95664d2-9614-4f35-a746-de8db63617e6` |

`0x3e864` 首先 `CoCreateInstance(CLSID, NULL, 1, IID, &ptr)`。若返回 `0x800401f0` 且允许重试标志为真，则调用 `CoInitializeEx(NULL, 4)`，并只递归重试一次。`0x397aa` 通过枚举器虚表 `+0x18` 以 `eAll=2`、active 状态掩码 `1` 请求端点；集合 `+0x18` 取得数量、`+0x20` 取得 Item、`+0x10` 释放，随后查询名称并交给音频设备集合代码。

这证明原版同一 DLL 中确实调用 Windows Core Audio，不能把原版 DLL 描述成不依赖 Windows API。后续 [IDA 证据](simple-audio-current-evidence.json) 独立核实构造器、实例写入和虚表归属，补齐导出到该具体对象及 JSON schema 的链路；Rust 已接入真实系统枚举。持续对象生命周期、通知及完整错误分支仍未等价，不据此宣称整个服务完成。

## SysUtilsNative 滚轮系统设置读写

[当前滚轮链证据](sysutils-wheel-scroll-current-evidence.json) 独立核对产品 182 的当前 JS、宿主分发、FFI 类型、IDA 正文和原 PE 代码 SHA-256。产品 `jo` 任务从活动配置读取浏览应用列表与高分辨率滚动开关，将应用路径去空格、转小写并取 basename；进入匹配应用且高分辨率开启时把系统滚动行数设为 1，离开且当前行数为 1 时设为 3。系统 setter 返回 false 会立即标为 error，后续设备 haptic browser-mode 查询/写入不再执行；任务入口取消与异常分支也保留在证据中。

宿主 `main.js → sysutil/win::callDLL → callDLLMain` 将无参 getter 或 `payload.actionArgs:[e]` 的 setter 交给 `SysUtilsNative.dll`。`getWheelScrollLines` RVA `0x77f10` 先将 UINT 初始化为 0，调用 `SystemParametersInfoW(0x68,0,&value,0)` 并忽略 BOOL，返回 FFI `int` 位模式；`setWheelScrollLines` RVA `0x77f60` 用参数原位模式调用 `SystemParametersInfoW(0x69,value,NULL,0)` 并返回 BOOL。失败 getter 返回初始化后的值，setter 返回 false；没有额外的注册表持久化、系统设置广播、服务初始化、回调、heap 分配或句柄清理。

[Rust 平台适配](../../crates/razer-platform/src/wheel_scroll.rs) 直接复现这两个 Windows 调用和有符号位模式，保留 `-1` 整页滚动 sentinel。其他平台明确返回 unsupported，不映射成另一套系统设置。已接 worker IPC：`WheelScrollLinesRead` 返回 `{lines}`，`WheelScrollLinesWrite {lines}` 返回 `{requested_lines,accepted}`，setter 的 false 不会冒充成功。前台事件调度、活动配置及 haptic 任务/页面消费者尚未闭合，因此这两个导出的恢复不计为完整滚轮页面或 SysUtilsNative 整库替代。未执行系统读取、写入或设备命令。

静态复核：`python -X utf8 tools/audit-sysutils-wheel-scroll-current.py --check`。

## RzPowerTool 本地服务状态查询与控制

[14 个 IDA 函数的服务证据](powertool-services-current-evidence.json) 包含 `WinMain`、服务名称门控、比较函数和启动/停止/依赖服务内部函数的原字节 hash、RVA、交叉引用与 Hex-Rays 正文。当前宿主 `serviceFunction.getServiceStatus` 将 `payload.actionArgs.toString()` 作为 `--get-service-status` 参数交给 `simpleLaunchRazerApp`；helper 的服务状态用进程 exitCode 返回。

当前前端 RSA 桥先给服务名加双引号作为 `actionArgs`，`getServiceStatus` 从 launcher 响应取 `exitCode`（无响应默认 0），`startService` / `stopService` 返回 `!!exitCode`（无响应默认 false）。证据还保存了真实共享消费者：THX 服务任务按 `thxCarol` 选择 `Carol Routing Service` 或 `VSSrv`，状态为 1 时启动后重新读状态；Audio Mixer 的 `restartAudioService` 先停 `Audiosrv`，失败立即返回，成功才启动并返回其布尔结果。这些当前文件中的共享实现不代表 product 1308 的实际 factory 已启用所有功能，产品门控和本应用对应页面消费者仍须分别闭合。

`WinMain` RVA `0x36430` 先经过 RVA `0x356e0` 的 12 个服务名门控，再以 `0xF003F` 打开本机 SCM、以 `0xF01FF` 打开服务，调用 `QueryServiceStatusEx` 读取 36 字节 `SERVICE_STATUS_PROCESS`。状态 1（stopped）与 3（stop-pending）原样返回，其余任何查询到的状态均折叠成 4，包括 start-pending、paused 等；不能把原值 4 解读成仅证明 running。名称不支持、打开失败或查询失败返回 0，service 与 SCM 的成功/失败路径都释放句柄，命令行数组最终 `LocalFree`。

[Rust 查询适配](../../crates/razer-platform/src/windows_service_status.rs) 保留原权限、默认 C locale 的 ASCII 名称比较、0/1/3/4 语义和句柄释放，在 Windows 直接查询本地 SCM；其他平台明确 unsupported。已接 worker IPC `WindowsServiceStatus {name}`，响应 `{name,status}` 中 status 保存原 helper 的 exitCode 语义；这个 envelope 是本应用 IPC 结构，不冒充原进程 launcher 响应。原 helper 的 locale 标志位位于 `.data` 的虚拟补零尾部，初始为 0；另一个 CRT locale 分支已反编译，但它的进程 locale 变更未复现。原 helper 的启动/日志与真实页面调用链分别保留缺口，不将独立适配或 IPC 路由计为所有服务 UI 完成。

同一 Rust 适配已实现 `start(name)` / `stop(name)` 的本地实际控制，返回原 helper 的退出码：1 成功、0 失败或服务不支持，非 Windows 明确 unsupported。已接 worker IPC `WindowsServiceStart {name}` / `WindowsServiceStop {name}`，响应 `{name,exit_code}` 保留该 0/1 值，并不冒充原 launcher 的 `exitCode` envelope；真实调用方需沿原布尔转换及刷新链分别接入。顶层经过同一 12 名称门控与 `SC_MANAGER_ALL_ACCESS=0xF003F`；启动用服务权限 `0xF01FF`，停止用 `0x2C`。SCM 返回的依赖服务名直接交给内部递归函数，不再次套顶层名单。

启动链为 `DoStartService 0x35de0 → DoStartServiceHandle 0x35cf0 → HandleStartService 0x35810 → StartDependentServices 0x35b90`。第一次查询状态若不是 stopped=1 或 stop-pending=3，原版直接返回 true，并不补发启动命令。stop-pending 时按 `clamp(waitHint/10,1000,10000)` 毫秒等待后查询，checkpoint 增长会重置时间基线；否则 elapsed 大于新 waitHint 时失败，此检查发生在下一次状态判断之前。调用 `StartServiceW(handle,0,NULL)` 后再查询；start-pending 的 checkpoint 增长同样重置基线，否则 elapsed 超过本次已选等待时长的 30 倍时跳出。这个阶段查询失败也跳出并判断同一个状态缓冲区，最终状态等于 running=4 才成功。没有额外固定期限，持续 checkpoint 增长可延长整个请求。

停止链为 `DoStopService 0x36360 → DoStopServiceHandle 0x36290 → StopDependentServices 0x36130 → HandleStopService 0x35eb0`。原版先递归停止 active dependent（筛选 1），再处理父服务；其时间基线在父服务第一次状态查询之前取得，此后不会因 checkpoint 或 `ControlService` 重置。已 stopped 立即成功，stop-pending 按同一 clamp 等待并查询，先判 stopped 成功，再判 elapsed 大于 30,000 毫秒失败。`ControlService(STOP=1)` 后则按完整 waitHint 睡眠，不作 clamp，再查询并先判 stopped，仍未停止才检查同一 30 秒阈值。因此阈值不是一个强制最大请求时长，睡眠可能超过它。

依赖枚举先探测所需字节，首次 API 成功即返回 true；失败只有 ERROR_MORE_DATA=234 才用 `HeapAlloc(HEAP_ZERO_MEMORY=8)` 分配并再次枚举。启动筛选全部 dependent（3），父启动成功之后才递归；停止筛选 active dependent（1），先递归再停父服务。返回顺序保持 SCM 提供的顺序；原版忽略孩子失败、依赖枚举返回值，且第二次枚举失败仍会释放缓冲并返回 true。Rust 保留这些语义，用 RAII 释放枚举 heap 缓冲、每个 child/parent service 和 SCM 句柄。原返回 1 仅代表父服务成功，不证明每个依赖都成功，也没有回滚含义。

原控制链没有取消或撤销接口，本应用也不添加自动回滚；调用方超时不能证明系统命令未发生。原 PE 的 `RT_MANIFEST` 明确要求 `requestedExecutionLevel=requireAdministrator`、`uiAccess=false`，收据保留 manifest 的 RVA、字节 SHA-256 和 XML。Rust 直接 worker 不会自动获得这个管理员令牌；保留原权限掩码不等于已复现原 helper 的提权环境，权限不足会如实返回 0。原 helper 提权/日志/进程 launcher 生命周期、非默认 CRT locale 行为和真实服务页面消费者仍未等价，不将这些内部状态机实现记为整个 helper 或全部界面完成。没有执行 helper、UAC、SCM 查询或系统写入。

静态复核：`python -X utf8 tools/audit-powertool-services-current.py --check`；`--acquire` 仅在私有副本调用已安装 IDA/Hex-Rays，不运行目标。

## 重现静态验证与下一层

`python -X utf8 tools/audit-host-service-code-current.py --check` 重新核对实际 DLL SHA、导出 RVA、`.pdata`、构造器/虚表、源码字符串、原始机器码 SHA 与反汇编。工具只执行微软 dumpbin 读取 PE，没有执行被分析的 DLL。

后续原生逆向按用户要求以 IDA/Hex-Rays 为主，保留原字节、函数范围、伪代码和交叉引用；本页既有 dumpbin 收据仍用于复核原取证范围。[IDA 全量输入索引](ida-native-corpus-current.json) 覆盖 81 个当前 PE 与 4 个宿主 helper，其中 84 个取得静态证据、1 个 ARM64 缺匹配后端；保存 320,630 个 native 函数索引和 10,630 个选定正文。三个 Intel 混合 CLR 库另用 native PE loader 取得机器码正文并单独保留托管索引，托管语义仍缺。该覆盖不表示全部函数或整个库已恢复，具体边界见 [DLL 当前分析](dll-device-communication-current.md)。继续追线程创建/调度/停止、事件注册/反注册、异常释放与最终 Windows/服务调用，并按 [全库清单](dll-function-inventory.md) 逐产品 DLL、其他 Node 插件与辅助程序展开。其余库不因有 FFI 表或导出表而标成已恢复内部实现。
