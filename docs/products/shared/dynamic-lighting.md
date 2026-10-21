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

本页逐字节复核 [完整捕获](../../../.analysis/results/host-current/local-audit.json) `dynamic_lighting.complete_source_owners` 的 54 个所有者与对应原件。

| 所有者 | 原件与 UTF-8 字节区间 |
| --- | --- |
| 设置控件 | `synapse-settings-current/.../720.1e5d1c8f.chunk.js` `[84051,86061)` |
| 存储事件串行化 | `synapse-lighting-engine-current/.../main.a383ea9e.js` `[908613,909593)` |
| 设备控制权切换 | 同原件 `[909593,911799)` |
| LampArray 原生初始化、查询与清理 | 同原件 `[278509,287000)` |
| 原生灯光驱动包装 | 同原件 `[405624,425118)` |

每类设备的最后协议翻译器与其关闭分支仍需沿该捕获继续追踪；下面已继续解析 JSON 动作、设备打开与元数据、COM 调用、事件和结果消费者。已闭合的选定调用不代表整套原生模块已闭合。

## 已追到的原生动作与结果契约

下表对应收据的 `dynamic_lighting.native_json_contract`。原生函数的 Image Base 为 `0x140000000`，表内均为 RVA；完整汇编、伪代码、函数边界和实际 PE 字节核验保留在同一查询的 `native` 中。部分 C++ 模板函数被反编译器错误推断为多参数函数，以下 ABI 以实际寄存器、虚表槽和完整调用方为准。

| 动作 | 已核实的输入与调用 | 返回和失败条件 |
| --- | --- | --- |
| `device_get_brightness` | `0x837a0 → 0x1530c0 → 0x152bc0 → 0x14f920`；`device_id` 缺省空串；最后调用 `ILampArray +0x80` 的 `HRESULT(this,double*)` | 乘 `100.0` 后截断为整数并存入 u8；仅成功才带 `nLevel`。设备不存在、未 open 或 manager 捕获异常均为 false，初始 out=0 不能解释为实测零亮度 |
| `device_set_brightness` | `0x84390 → 0x157530 → 0x156f60 → 0x14f8c0`；字段是 `nLevel`，缺省 100，转换为 u8；最后调用 `ILampArray +0x88` 的 `HRESULT(this,double)` | u8 除以 `100.0`，超出 `[0,1]` 时选 `1.0`。完整汇编 `0x157344` 确实替换 XMM6，`0x15738a` 再放入 XMM1 交给 COM，不能因伪代码漏参数而误判为只修改日志。成功只表示该 COM 写入返回成功，没有额外读回 |
| `device_indices_for_key` | `0x83b00 → 0x1537a0 → 0x153230 → 0x153180`；`key` 缺省 0；`ILampArray +0xa8` 为 `HRESULT(this,u32 key,u32* count,i32** indices)` | 拷贝数组后 `CoTaskMemFree` 原分配。不存在／未 open 的设备返回空数组，但 handler 不把 `succeeded` 改成 false，因此可能得到 `{succeeded:true,indices:[]}`。该 manager 没有局部 catch，异常由外层 `0x85cf0` 转成 false 与 reason |
| `effect_set_all_lamps_same_color` | `0x84630 → 0x156f30 → 0x156a90 → 0x157560`；`color` 为 u32，缺省 0；`ILampArray +0xb8` 为 `HRESULT(this,Color)`，四字节值放 EDX | 设备不存在、未 open 或捕获异常返回 false；成功没有颜色读回。本动作保留传入 alpha，不额外补不透明 alpha |
| `system_get_vk_and_vsc_map` | `0x88600`；`map_type`／`id_thread` 都缺省 0；先 `GetKeyboardLayout`，再按 VK 1–254 调 `MapVirtualKeyExA`，只保留非零结果，`GetKeyNameTextW(scanCode<<16,buf,128)` 取显示名 | 实际字段为 `vk_and_vsc:[{key,scan_code,key_name}]`，不是 `vk/vsc`。按完整输入 JSON dump 缓存，命中后不重查布局；本 owner 没有布局变化失效链。无效 layout 返回空数组，仍保留 dispatcher 的默认 true，不插入缓存 |

亮度的 JSON 数值 helper `0x74480` 区分缺字段、布尔、整数、浮点与错误类型：缺字段才使用 100；错误类型抛 type_error 302，输入不是对象抛 306。超范围 JSON 到 u8 的转换尚未经实际 DLL 对照，不能擅自把任意大整数、负数或 NaN 当成已确定的兼容规则。manager 的范围处理发生在转换之后，二者不能混为一个输入校验步骤。

颜色转换也已核到字节：RGB 数组 `[0x12,0x34,0x56]` 在 `0x84860` 打包为 `0xff563412`，`0x1585d0` 将这个 u32 **循环左移 8 位**成为 `0x563412ff`，小端内存为 `ff 12 34 56`，即 `Color{A,R,G,B}`。这不是四字节逆序。单色动作的 `0xaa563412` 同样得到内存 `aa 12 34 56`；默认 `color=0` 的 alpha 也为 0。上述向量从当前指令独立推导，属于静态预期，尚不是 DLL 实际输出样本。

帧 handler 的 owner `+224` 也已追清：它是受 `+288` mutex 保护的设备 ID 字符串集合。首次遇到该设备时打印帧日志，再经 `0x63ea0` 插入 ID；后续帧跳过这条日志，仍继续转换与 COM 写入。open 和 availability 事件经 `0x88280` 删除这个标记，使下一帧重新记录。它不是键位索引缓存，也不代表相同帧会被丢弃，独立实现不能据此省略真实帧传输。

### 页面和引擎怎样消费这些结果

当前灯光引擎 `main.a383ea9e.js` 的完整 `rzDeviceLampArray` 类 `[1118089,1128397)` 已加入查询。其 `connectDevice` 先等待 `device_open`，仅成功时调用 `getVKAndVSCMap`，再解析设备信息、标记 opened；map 调用的返回值没有成为另一道 open 成功门禁。map 成功时构造 `Map(key,scan_code)`，不消费 `key_name`。WASD、方向键、功能键、数字区和 logo 的灯珠分组来自枚举结果 `lamp.keys/purposes`，这段完整 owner 没有调用 `device_indices_for_key`。

`getBrightness` 仅在 `succeeded===true` 时将 `nLevel` 转为 `jsonData.brightness`。`setBrightness` 成功后返回的是请求中的 brightness，不能称为读回值；失败保留默认结果。连续帧经 `JsonApiNoReturn` 提交 `effect_set_colors_for_indices`，当前宿主 `LampArrayAction.setChromaFrame` 也走此动作。选定完整引擎 main 与宿主 LampArrayAction 内没有单色动作的字面调用方，不能为它虚构页面入口；本次已闭合的是该 native 动作本身，动态构造动作及其余部署仍要独立核对。

这些结果给出了可实现的参数与比较边界：空 indices、空键位表、setter 回显请求值、无返回帧和真实 getter 必须分别建模。实际对照须覆盖不存在／未打开、HRESULT 失败、亮度换算、颜色 alpha／通道、COM 数组释放、map 缓存与布局切换；不能把两边共同返回空数组或 setter 接受请求算成设备状态一致。

### 注册表的实际格式与警告消费者

`GetLightingSettings` 的 DLL 实现 `0x2a6a0` 先检查全局 RPC client；存在时以字面请求 `{"action":"device_get_all"}` 调 `JsonApi`，再经 `0x98f60` 解析设备 ID。`0x116eb0` 枚举 `HKCU\Software\Microsoft\Lighting`、其 `Providers`，以及 `Devices\<device>` 与每个设备的 providers。解析出的 ID 集合非空时，只有完整设备 ID 包含注册表子键名的设备才保留；ID 集合为空不能证明找到了与当前物理设备一致的注册表项。

序列化 owner `0x115100` 输出下列三组字段，均保留枚举顺序：

```text
globalSettings: [{name: string, data: string}]
globalProviders: [string]
deviceSettings: [{deviceId: string,
                  values: [{name: string, data: string}],
                  providers: [string]}]
```

值读取器 `0x117c30` 将 `REG_SZ` 当 ANSI 字符串，`REG_DWORD` 转为十进制字符串；其他类型输出字面值 `(Unsupported type)`。name／data 使用固定 256 字节缓冲区，`RegEnumValueA` 任意非零结果都会结束当前循环，包括错误。该外部 JSON 没有逐项读取状态，因此缺项、读取错误和真实不存在不能只凭空列表区分。

当前完整 `RzWDL` 类 `[278509,287000)` 与 `LampArrayEventHandler` 类 `[1138049,1147614)` 的消费者都要求 `AmbientLightingEnabled.data` **严格等于字符串 `"1"`**；Brightness 以十进制 `parseInt` 解析，`<=10` 时生成低亮度警告。它们用设备 ID 的 substring 筛选设备，以 `JSON.stringify` 严格比较全局／设备 provider 数组的内容和顺序，并只检查第一项是否包含 `RazerDynamicLighting`。不能将 provider 集合排序后再称比较等价。所需值的 `.find(...).data` 缺失会抛到 owner 的 catch，不会得到一个已确认默认值。

两处事件消费者也有区别：低层 availability 更新会修改保留设备、发布 update，并触发保留的 200ms debounce；高层 handler 为该事件临时创建一个 300ms debounce 后调用，分析结果经 `L.m_` 发布。高层没有把这个临时 debounce 保存成统一可取消任务；不能将低层 `unregisterEvent` 对其自有 debounce 的取消扩大到高层所有待执行分析。

### 控制器文件的路径、编码与真实返回语义

`GetControllerFilePath 0x2a220 → 0x119360 → 0x118ff0/0x119300` 的实际路径为：

```text
%localappdata%\Razer\RazerAppEngine\User Data\Apps\Common\LampArray\RzLampArray.json
```

路径 owner 会展开环境变量，并按目录情况尝试 `CreateDirectoryW`；返回路径不能当作目录已成功创建或安装完成的确认。这里的文件属于厂商外部交互边界，不是本项目内部持久化格式的依据。

`SetDeviceInfoToFile 0x2ac60 → 0x119610` 不解析 JSON：空字符串或**精确的 `[]`** 进入 `_wremove`；其他非空内容按原字节以 `wb` 打开并覆盖。`0x118d80 → 0x118650` 先写四字节本机小端有符号长度，再将 payload 与循环的 11 字节 ASCII key `RzLampArray` 逐字节 XOR 后写入；没有 NUL、认证或校验字段。key 的字符串布局及实际字节保留于查询的 PE 数据区 `0x1a42c0`。例如原文 `{}` 的静态独立预期为 `02 00 00 00 29 07`；这只是按已核实指令推导的文件向量，尚不是实际 DLL 生成的样本。

内部 helper 在打开／删除失败时返回 false，但导出 owner **忽略该布尔值**，非空指针经 helper 正常返回后仍返回 true。void 编码器遇到短写或捕获异常只记 debug 日志，`fclose` 返回值也未检查；导出 true 因而不能证明完整落盘，更不能证明设备状态。空指针或传播到导出 catch 的异常才使导出返回 false。已有 reader `0x118ad0` 用同一 key 解码，长度前缀缺失／非正或 payload 短读返回空串；仅找到该 helper 不能证明产品已调用它读取这个文件。

真正的前端 writer 在 `LampArrayEventHandler` 中：从 cache 按当前顺序投影 `ledConfig`、`productName`、`productId`、`vendorId`、`enabled`、`type`、`category`、`deviceContainerId`、`keyIndices`，每次创建新的 300ms debounce wrapper，再把数组交给 `w.TO(t)` 的 JSON 序列化路径。cache 为空时只记日志，**不会调用 native 的 `[]` 删除分支**，旧文件可能继续保留。另一个同名 facade `setLampArrayDeviceInfoToFile(e)` 实际调用 `w.TO()` 丢弃了 `e`，不能误认为它与 cache writer 参数链相同。

对应对照须分别覆盖字符串值／DWORD／不支持类型、缺失注册表项、枚举失败、provider 顺序、设备 ID 过滤、准确文件字节、`[]`／空串／带空格的数组、打开失败、短写、空 cache 和连续 debounce。记录 DLL 原始返回与实际文件结果两份观测；不得因导出返回 true 或独立 XOR 往返成功而判定持久化对照通过。

### 设备打开、完整元数据与可用性事件

`device_open` 不是按任意字符串创建设备：`0x83e90 → 0x155750 → 0x154d30` 在 Impl `+240` mutex 内，以完整 ID 精确查找当前发现表。未发现的 ID 返回空 optional；找到后才经 `0x152b30` 调 `ILampArrayStatics +0x38` 的 `FromIdAsync`。等待链 `0x15f8c0 → 0x148af0 → 0x148b20` 把 `10,000,000` 个 100ns 单位换算成 **1,000ms**，等待后要求异步状态为 Completed（1），再经 `0x153880` 的 `+0x40` 取结果。超时／空结果不返回设备，完整 owner 没有在超时调用 `IAsyncInfo.Cancel`；释放异步引用不能称为已取消后台请求。

非空 LampArray 的元数据读取严格按下列顺序进入 `0x152340`，不能用本地默认配置补齐：

| 顺序 | 实际读取及虚表槽 | 转换与输出边界 |
| --- | --- | --- |
| 1–6 | BoundingBox `+0x68`、ProductId `+0x40`、VendorId `+0x38`、IsConnected `+0x90`、IsEnabled `+0x70`、Brightness `+0x80` | 亮度乘 100 后截断存入 **u8**，不是 u16 |
| 7 | 可选 IsAvailable | 是否能查询取决于下述事件能力检查；不能从 connected／enabled 推导 |
| 8–11 | HardwareVersion `+0x48`、MinUpdateInterval `+0x60`、Kind `+0x50`、LampCount `+0x58` | 间隔除以 10,000；Kind 1–11 保留，其余归 0。读取到硬件版本／间隔仅供该链日志，现有 JSON serializer 没有因此输出同名字段 |
| 12 | 每灯 GetLampInfo `+0xa0`，随后读取 position `+0x40`、purposes `+0x38` | `0x85fe0` 仅在 optional presence byte `+160` 非零时序列化扩展字段；几何值按 `round(value*1e6)/1e6` 输出 |

随后 `0x159680` 按 VK 1–254 递增查询 `ILampArray +0xa8` 的灯索引，为每次 COM 数组调用 `CoTaskMemFree`。灯记录为 48 字节、预先清零；真实指令 `0x1599f0–0x159a07` 用比较和条件移动限制为 **min(key_count,8)**，每灯最多保存八个 key，serializer 丢弃零项。不能依据反编译器遗漏 `memmove` 参数误报无界复制，也不能把截断后的八槽当作设备只支持八个按键。

Impl 构造函数 `0x14ae70` 通过 `ApiInformation.IsEventPresent` 检查 `Windows.Devices.Lights.LampArray.AvailabilityChanged`。不支持该事件时，元数据直接赋 `available=true`，这是**源码指定默认值**，不是设备实测。支持时经 QI `0x148780` 取得实际接口，再由 `0x154580` 的 `+0x30` 读 IsAvailable；若为 false，最多执行五次 Sleep(20ms) 后重查，最后经 `0x14f4b0` 的 `+0x38` 注册回调。初始 false 不阻止 open 成功；订阅异常在局部 catch 只记日志，已收集的 optional 仍可返回。event token 仅局部保存，没有成为后续显式撤销的所有者。

availability 回调 `0x150990` 解析 ID、锁设备表，只更新该记录的 optional available 字段；同时查询的 connected／enabled／lampCount 等用于日志，不能称为完整元数据刷新。解锁后标记 changed 并调用保留 consumer，API `0x87900` 发 `DeviceAvailabilityChanged`，另删首帧日志标记。`device_get_all 0x14f060` 只另行刷新 enabled 与 optional connected，available 继续使用事件保留值；这三种状态必须分别比较。

close 的边界也不同：`0x14f990` 释放非空 LampArray COM 指针并置空，**保留 discovered record**。缺失 ID 返回 false；记录存在但已经关闭仍可正常返回 true。因此 close 成功、发现列表中存在、已打开、可用和已连接都不是同一件事。

引擎的完整 `rzDeviceLampArray` `[1118089,1128397)` 在 open 成功后先 await 键位表，再 `parseDeviceInfo`。该 parser 读取 native 的 vendor／product／enabled／connected／boundingBox／type／lamps，按数组顺序生成灯索引；以 `0.012` 为边缘补偿和网格单位，bounding box 小于灯坐标最大值时扩到最大值加 `0.012`，网格先向上取整，灯坐标四舍五入，再保证容纳最大索引。键组依据真实 `keys/purposes`，不是固定产品图推算。

同一 adapter 的 `getSerialNumber` 则是 **device ID 的 JS hash**，不是硬件序列号；`getEdition` 固定 `[0,0]`，`getDeviceMode/setDeviceMode` 固定 `[3,0]`，`setChromaEffectCommon` 不提交 native。实现和测试必须把这些兼容 adapter 结果与真实硬件读取／写入区分，不能将固定返回登记为设备确认。

实际对照应覆盖精确 ID、不存在／已发现未打开／已关闭、1s 超时及迟到完成、空结果、元数据逐项失败、Kind 边界、九个以上 key 的截断顺序、COM 数组释放、事件不支持、持续不可用、五次重查和订阅异常；页面核对另覆盖网格取整与上述 adapter 固定返回。这里的协议与分支来自完整函数及当前前端，尚未执行 DLL 或设备对照。

### Watcher 的启动、停止与资源边界

`StartWatcher` 的服务 opnum 4 经 `0x3db60` 到 `0x159be0`。首次使用才创建并保留 DeviceWatcher：`0x14fd90` 走 factory 虚表 `+0x70`，`0x14ed70`／`0x156760` 经 watcher `+0x30`／`+0x50` 注册 Added／Removed。返回的 event token 只在局部保存；这个完整 start owner 没有注册 Stopped 或 EnumerationCompleted。

随后先将 Impl `+1` 标志设为 true，再经 `0x159dd0`／虚表 `+0x80` 取状态，最后经 `0x159b80`／`+0x88` 调 Start。所取状态没有参与就绪判断；HRESULT 失败抛异常时，这个 owner 没有清回已置的标志。因此“已请求启动”“正在枚举”“初次枚举完成”不能合成一个布尔状态。

`StopWatcher` 的 opnum 5 经 `0x3dba0 → 0x159f00 → 0x159e40`：仅当 watcher 非空且标志为真时调用虚表 `+0x90` 的 Stop，返回后清标志。Stop 失败会在清标志前抛异常。这个完整函数保留 watcher 指针，不撤销两项事件注册，也没有等待 Stopped、在途回调或线程 join；不能从正常返回扩大为全部资源已经释放。重启复用同一 watcher，也不重新注册 Added／Removed。

客户端 `0x7e3e0`／`0x7e720` 在未连接时尝试初始化 RPC，仍断开才返回 false；正常完成对应 RPC 后返回 true。JS `unInitRzLampArray` 虽顺序等待 StopWatcher 和 Shutdown，但不检查两个布尔返回便清自身初始化标志。独立 Rust 实现需要保留这些调用和实际设备事件顺序，同时由自己的生命周期 owner 明确停止、回调排空和释放结果；源码标志、RPC 返回和真实清理不能互相冒充。

### RPC worker、Shutdown 与析构的实际分工

下列 DLL 函数的 Image Base 为 `0x180000000`，WDL 服务仍为 `0x140000000`，二者的 RVA 不可混用；完整函数与 PE 字节核验都归入同一 `dynamic_lighting.native` 收据。

DLL 的 `InitRpc 0x7c0f0` 将 client 对象及入口 thunk `0x5ed9` 交给线程包装器 `0x36120`，后者调用实际 worker `0x7b220`，线程结束时广播线程退出条件并释放自己的入口参数。worker 每轮在 mutex `+48` 下计算新的 **10ms** 截止时间：完整汇编 `0x7b284` 写入 `0x0a`，经 `0x377f0` 转成纳秒截止值。它使用 condition variable `+136` 等待，每次唤醒重查截止时间和 stop byte `+128`；到期且未停止才调用 `0x7b470`，连接标志为真时经 stub `0xa4410` 执行 RPC opnum 6。

这个 opnum 6 是反向事件队列排空：服务 `0x3db20 → 0x37470` 锁住队列，逐项调用反向 callback，正常返回后才删除对应条目。它不是再次枚举设备。DLL `OnLampArrayCallback 0x7cf10` 只有在 callback 指针非空时才调用它，参数为借用的 JSON 字符串；不能因队列处理完成就推定 JS 一定消费到了事件。

实际 `Shutdown` 的分工更窄：DLL `0x7dd70` 调服务 opnum 1，服务 `0x883f0` 只转入 `StopWatcher 0x159f00`。正常 RPC 返回后 DLL 返回 true；这个完整 owner **没有清除 connected byte、没有置空 callback、没有 join worker，也没有释放 RPC binding**。所以先 StopWatcher 再 Shutdown 并不是两层资源释放的证明，正常时还可能只是两次进入同一个停止判断。

真正的 client 析构在 DLL `0x733a0`：锁住 mutex 后置 stop byte，解锁、广播 condition variable，再通过 `0x96160` 等待 `+16` 的线程结束；正常路径随后释放 RPC string binding 与实际 binding，再销毁自有容器。此函数存在 catch-all：异常会直接进入成员销毁，若线程仍为 joinable 则调用 `terminate`。因此完整函数证明了正常析构顺序，不能据此声称异常析构也保证清理成功。worker 的计时和析构唤醒须分别测试，不能用等待固定时间代替终态确认。

DLL 卸载路径也已追到真实入口：`0x12f3 → 0x147c34 → 0x147a98 → 0x2a180`。reason 为 0 时，用户 detach owner 先将全局 client 指针置空，再析构并释放旧对象，最后关闭日志 owner。这条路径与导出的 Shutdown 分开；当前 JS `FreeFFI` 分支只是返回 true，不能把该请求当作 detach 已执行，也不能把内部析构当作外部回调已经排空的依据。

服务端另由 `0x22c00` 注册 exit owner `0x23cb20`，后者经 `0x7dbd0` 析构 API：先调用 Shutdown，再销毁集合与回调函数对象，最后经 `0x14cbb0 → 0x14c920` 销毁 manager。Impl 析构再次按标志停止 watcher，`0x13dc20` 逐项销毁设备表和键，再销毁保留的函数对象，通过 `0x13a6c0` 释放 factory／watcher COM 引用。所读完整析构没有显式撤销 Added／Removed token，也没有等待 Stopped 或回调 join；这说明原版在哪里停止、在哪里释放，不证明在途回调安全已经获得运行验证。

对应测试需分别观察：10ms 队列轮询、停止唤醒与 join、Shutdown 后 worker／callback 的保留、反向回调失败时队列是否仍保留、正常析构的释放顺序、异常析构的实际结果，以及服务退出时的设备 COM 释放。以上均为静态逆向所得契约；本次没有执行 DLL、服务或窗口，不能将这些静态边界登记成实际对照通过。

### 稀疏包、EXE 安装与卸载的实际契约

当前 `RazerWDLMsix` 是允许外部内容的稀疏包，只有 `AppxManifest.xml`、`AppxBlockMap.xml`、`[Content_Types].xml` 和 `AppxSignature.p7x` 四个 member，**不内含 EXE**。manifest 的 identity 是 `RazerDynamicLighting`、x64、版本 **1.0.3.0**；资源声明的 **1.0.46.0** 属于另一层版本，不能混用。Application 指向外部 `razerwdl.exe`，声明 `mediumIL`／`win32App`、隐藏应用列表入口，以及 `com.microsoft.windows.lighting` AppExtension。`AllowExternalContent=true`、`runFullTrust`、`unvirtualizedResources` 与 Desktop MinVersion `10.0.19000.0` 是包声明，不取代前述设置页／native 的 OS 门槛。

静态查询逐 member 保存实际长度／SHA，按原 block map 的 SHA-256 与 64KiB 块边界复算内容，拒绝重复 member；这些检查通过不等于验证了 `AppxSignature.p7x` 签名或执行了安装。完整 manifest 和 block 结果归入同一查询的 `native_package_manifest`，来源版本及原件 SHA 仍由获取收据保存。

EXE 的 `WinMain_0 0x1318c0 → 0xf5960` 从 Unicode 命令行构造 UTF-8 argv。CLI 要求恰好一个子命令：`start [-p|--parent-pid UINT] [-s|--session STRING]`、`install path`、`uninstall`，另有全局 `-v|--verbose`。只有 `path` 明确使用必需标志；parent PID 默认 0、session 默认空串，parent 仍要通过后续实际检查。

| 分支 | 实际调用与判断 | 调用者能观测到的结果 |
| --- | --- | --- |
| install | `0xf3d60 → 0x138850`；当前进程已有非空 package full name 直接 true；否则取 EXE 路径、校验包签名，将 EXE 父目录设为 ExternalLocationUri，再执行 AddPackageByUriAsync | 内部等待完成、GetResults，再读 DeploymentResult2.IsRegistered；HRESULT／取消／其他捕获异常为 false。**WinMain 忽略 bool，清参数后固定返回 1** |
| uninstall | `0xf6b70 → 0x138c40 → 0x138d20`；当前进程无 package identity 直接 true；有 identity 时先要求 GetModuleFileNameW 成功，再按当前 full name 调 RemovePackageAsync | 正常调用后立即释放 async operation，**没有等待完成或 GetResults**；true 仅代表没有同步抛异常。**WinMain 忽略 bool，公共清理后返回 0** |
| start | 核对 parent PID、要求当前进程有 package full name，再创建 session mutex、隐藏窗口和 RPC worker | PID 无效、无 package identity、隐藏窗口失败返回 1；正常消息循环结束并清理返回 0。RPC worker 的 RunForever 返回值没有传回 WinMain，退出码不证明服务就绪 |

package helper `0x1382f0` 两次调用 GetCurrentPackageFullName，先取长度，在 ERROR_INSUFFICIENT_BUFFER 后读取；仅 APPMODEL_ERROR_NO_PACKAGE（15700）返回空字符串，其他错误抛异常。检查的是**当前进程 identity**，不是机器上同名包是否存在，也没有核对 full name 为特定产品／版本；install 提前成功时不再核对传入包或版本。

部署方法以真实 PE IID、实参和 SDK 接口虚表交叉核实：`0x1381a0` 在 IAddPackageOptions `+0x68` 写 ExternalLocationUri；`0x135150` 用原件 IID `0x24b8c8` 取得 IPackageManager9，`0x137f30` 的 `+0x38` 是 **AddPackageByUriAsync**。`0x1352f0` 在 Started 状态经 `0x1351e0` 等待 INFINITE，再由 `0x138220` GetResults，Canceled 抛 hresult_canceled；`0x135030` 用 IID `0x24b898` 取得 IDeploymentResult2，`0x138680` 的 `+0x30` 读 IsRegistered。remove 则是 IPackageManager `+0x40` 的 RemovePackageAsync。反编译附带的 SDK 行号不能代替 IID／槽位依据。

包签名谓词 `0x161d30` 先经 `0x161b20` 调 WinVerifyTrust，再经 `0x161c10 → 0x160e80` 检查 signer subject：仅接纳 ValueType=4 的 CERT_RDN_PRINTABLE_STRING，OID `2.5.4.3`（CN）和 `2.5.4.10`（O）必须都逐字节等于 `Razer USA Ltd.`。签名／证书读取失败产生空 subject 并失败。WinVerifyTrust 使用 CACHE_ONLY_URL_RETRIEVAL（`0x1000`）与 STATEACTION_VERIFY；该完整 owner 没有随后执行 STATEACTION_CLOSE，不能宣称已验证 trust state 的完整清理。原件 SHA 与版本属于分析收据，不应成为产品运行时兼容门禁。

### session、父进程与消息循环的退出顺序

`0xe3290` 用 `Global\rzwdl_` 加原 session 字符串调用 CreateMutexA，bInitialOwner=false，以 ERROR_ALREADY_EXISTS（183）标记已有实例，没有获取所有权的等待。创建失败仍保留“新实例”标志，不能称单实例保障成功。已有实例时 `0xf3190` 仅按固定 class／title `RazerWDL` 查窗口并发 `(message=0x40a,wParam=0x1337)`，查找不携带 session；main 固定等 300ms 后继续，没有确认旧实例退出。

同一 session 由 `0xf71d0 → 0x341f0` 保留，经 `0x37570` 组成 RPC endpoint `\pipe\rzwdl_<session>`，以 `ncacn_np`、最大调用数 1234 调 RpcServerUseProtseqEpA，再 RpcServerRegisterIf，最后 RpcServerListen(1,1234,false)。这里没有将该字符串解释为 Windows session ID。Listen 前先置 started，Listen 返回错误才清；worker `0xad0f0` 丢弃其返回值，线程创建不能代表管道注册或监听已完成。

parent 检查 `0xf4730` 以 PROCESS_QUERY_LIMITED_INFORMATION（`0x1000`）打开指定 PID；打开失败为 false，GetExitCodeProcess 成功才比较 STILL_ACTIVE（259），**该 API 失败反而保留初始 true**，随后关闭句柄。没有核实父子关系、进程身份或 session。`0xf2c20` 每 300ms 重查，失活时 PostMessageW(hwnd,`0x12`,0,0)，监控入口 `0xad140` 返回前再置全局退出标志；WndProc `0xf31e0` 另在 WM_CLOSE 或 `0x40a/0x1337` 时 PostQuitMessage(0)并置同一标志。

`0xf7140` 按 GetMessageW → TranslateMessage → DispatchMessageW 运行，退出后 DestroyWindow。`0xf71d0` 随后先清全局 RPC server 指针并析构／释放对象，再 join RPC worker，最后 join 存在的 parent 监控；main 才 CloseHandle(session mutex)、清日志和参数。server 析构 `0x35c70 → 0x37a60` 先 RpcMgmtStopServerListening，**仅成功才继续** RpcServerUnregisterIf(nullptr,nullptr,false)；清理 bool 被析构调用者丢弃。请求退出、固定等待、停止请求、RPC 注销、析构与线程 join 因此要分别记录。

安装链的静态结果不能用 install exit=1 当失败、uninstall exit=0 当已卸载，也不能用启动线程当服务就绪。对应测试需分别核对内部 bool、实际包状态、IsRegistered、服务状态、退出码和清理；覆盖已有 identity 短路、坏签名／取消／部署错误、uninstall 异步失败、parent 查询失败、mutex／旧窗口／RPC 注册／Listen／停止失败。上述 EXE、安装／卸载和真实服务验收本次均未执行，仍受[执行限制](../../../AGENTS.md)约束。

## 实际页面与当前 Rust 消费者

设置页的 `ia` 经 `qs=memo(Js)` 挂载双向模式控件；完整 `Js` 在同一 chunk `[81362,82814)`。两个标签都调用同一 toggle，点击已选标签也反转模式；内层回调先 stopPropagation，避免冒泡重复切换。`wdl-devices.isSwitching` 给外层添加 disabled 并挂 spinner，它是后台切换状态，不能用当前选项或本地保存状态推导。

当前 CSS `720.dbc9cca5.chunk.css` 的字符区间 `[224300,224649)` 保留边框 `#5d5d5d`、背景 `#111`、外层高 36px／圆角 18px／padding 5px、标签高 26px／圆角 13px／padding `5px 10px`；按下背景为 `#0000001a`。`[371398,371515)` 的两向容器 gap 为 14px，spinner 使用 auto background-size。`[368793,368933)` 的 modes-area 默认隐藏、active 才显示，顶部 padding 20px；`[259842,259935)` 的说明行顶部 margin 10px、内容顶部对齐。这里的字符区间不能当作 JS 的 UTF-8 字节区间，也不能只由数值接近认定 Rust 的颜色、字体、焦点及缩放一致。

| 边界 | 当前生产所有者 | 核对结果 |
| --- | --- | --- |
| 显示门槛 | [Windows system.rs](../../../crates/razer-platform/src/platform/windows/system.rs) 的 `supports_dynamic_lighting`，由两个设置入口调用 | 读取 CurrentBuildNumber／UBR，采用 build ≥22631 或 22621 且 revision ≥2506，数值条件对应所读设置控件；注册表读取失败返回 false。它只决定页面显示，不证明原生 LampArray 能力已初始化 |
| 普通设置布局与编辑 | [settings_lighting.rs](../../../crates/razer-settings/src/preferences/settings_lighting.rs) → [settings_page.rs](../../../crates/razer-settings/src/preferences/settings_page.rs) | 已有两个模式、说明／警告图、系统设置链接和 switching spinner；两个按钮都反转模式，保留已选标签也切换的行为。renderer 接受 Option，但实际 caller 固定传 Some；页面从 `saved.dynamic_lighting_draft.unwrap_or(false)` 起步，没有真实控制权初值和切换观察消费者，不能把这个本地选择显示为已观测模式 |
| 普通设置事件 | `SettingsEvent::DynamicLightingRequested` → [Shell 分发](../../../crates/razer-shell/src/shell.rs) | 只更新本地选择并显示尚未实现的状态说明，没有设备控制权转移、LampArray 枚举／开关、`wdl-devices` switching 发布及确认；`observe_dynamic_lighting` 存在但当前没有生产调用者 |
| Chroma 设置入口 | [chroma_settings.rs](../../../crates/razer-app-pages/src/chroma/chroma_settings.rs) → [状态 owner](../../../crates/razer-state/src/chroma_settings.rs) → [本地存储](../../../crates/razer-storage/src/chroma_settings.rs) | Change::DynamicLighting 进入串行本地文件保存；有双模式、文案／图像和系统链接，但尚无本页后台控制权消费者。本地文件保存与本机控制权已转移须分别记录 |
| 产品灯效页的 WDL 观察 | [product_chroma.rs](../../../crates/razer-shell/src/shell/devices/product_chroma.rs)、[controller_chroma.rs](../../../crates/razer-shell/src/shell/gamepad/controller_chroma.rs) → [chroma_profiles.rs](../../../crates/razer-storage/src/chroma_profiles.rs) | 已有真实本机存储读取与缺失／错误区分，产品页可消费已读的 `isDynamicLighting`；手柄 watcher 带身份／guard 并后台重查。这是存储观察，不能当作 LampArray 原生 watcher 或两个设置入口的写入确认 |

上述两入口的同类界面都需要沿各自当前官方 root 验收；不能因为共用本机系统设置链接就跳过一方的状态、资源、关闭和失败语义。当前 OS 链接是真实 `ms-settings:personalization-lighting`，但打开系统设置不等于已完成本程序的控制权切换。

## 验证范围

本次重新运行 `python tools/workspace.py audit-host-local --query dynamic-lighting`，54 个完整 source owner、所用 CSS、选定原生函数与数据区的实际字节核验通过，查询 failures 为空；`python -X utf8 -m unittest discover -s tools -p test_host.py` 的 27 项维护工具测试通过。这些结果证明所列静态输入和维护行为，不将原生 RPC 间接目标或所有产品协议升级为已闭合。

后续纯／模拟验证须覆盖 OS 边界、已选项点击、切换禁用、缺失／错误存储、两个窗口并发、设备加入／移除顺序、readiness、失败后 finally 清除 switching、旧连接拒绝及完整注销释放；独立实际对照继续比较官方 DLL 的本机调用结果和 Rust 的控制权／设备集合／事件顺序。像素、图标、文案、键盘／焦点和真实窗口须另行验收。本次未运行相关 Rust 行为测试、应用、官方 DLL 或设备，遵守[验证规则](../../../.agents/rules/verification.md)。
