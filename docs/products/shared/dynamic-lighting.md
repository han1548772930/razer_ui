# Windows 动态照明设置与设备控制权切换

该设置决定支持的设备交由 Windows 动态照明还是本机 Chroma 引擎管理。开关不是一个单独的颜色命令；它会改变设备集合、LampArray 会话与灯光驱动的处理路径。

## 设置页的显示与操作

当前设置控件只在 Windows build 至少为 22631，或 build 为 22621 且 revision 至少为 2506 时显示。它读取并监听 `isDynamicLighting`，同时监听 `wdl-devices` 中的切换状态；卸载移除这两种订阅。

用户切换后，控件立即改变自身状态并写 `isDynamicLighting`。这个完整控件没有 Apply，也没有等设备 ACK 后才更新或失败后回滚的分支。旁边的系统设置入口调用本机 `msSettings("personalization-lighting")`。

两个设置入口的初读顺序不同：Synapse 在 await OS 版本之后才读取 `isDynamicLighting`；Chroma 的本地读取在 OS 查询的异步函数之外，二者并行发生。两个控件均不预读 `wdl-devices`，只消费后续进度事件。这里读取的是本地控制意图，没有调用 OS 的动态照明启用 getter；初始 false、已保存意图、switching 和真实设备控制权须分别解释。

原生 LampArray 初始化有另一条 OS 门槛：Windows major 大于 10，或 major 为 10 且 minor 大于 0，或 major/minor 为 10/0 且 build 大于 22621，或 build 为 22621 且 revision 至少 2506。它和设置控件的显示条件不完全一致。

## 开关如何影响后台

灯光引擎用已有 mutex 串行处理存储变更，解析开关后调用 `updateRazerWDLDevices`，再发布模式事件。控制权转移期间把 `wdl-devices.isSwitching` 设为 true，并在 finally 清除。

切向 Windows 时，先从 Chroma 集合移除匹配的设备，再经每设备 mutex、窗口共享存储和本机缓存加入枚举到的 Razer LampArray 设备。

切回 Chroma 时，先移除 Razer LampArray 的 Chroma 项、关闭并移除 LampArray 设备，再枚举本机 USB 并准备匹配 container。原版对设备准备有一秒间隔；存在 readiness 任务时，还会等待 readiness 和两秒稳定间隔。这些延迟是该完整函数的实际顺序。

### 两个设置入口到后台的真实通道

[双设置页静态切片](../../../.analysis/results/frontend/synapse-settings-current/local-startup.json)的 `dynamic_lighting_control` 分别连接两个实际挂载根、词法绑定、完整 AST 所有者、宿主来源与当前 Rust 消费者。它补充下述传输与释放边界；已有原生协议、RPC 和设备语义继续引用本页原有专项，不将前端接线扩称为整个 DLL 已逆向完成。

该切片保留 166 个角色对应的 160 个不同完整 JS AST 所有者、38 处词法绑定、16 项导出、49 条静态断言、36 条调用／清理边及 234 个证据文件。Rust 对照另核六份原件、九个完整 AST 所有者，并扫描 1,689 份 Rust 文件的完整字节及可解析 AST；三个文件存在解析器语法限制，仍用完整字节词法匹配复核引用集合。独立复核拒绝 31 个篡改反例；这些计数只描述所选证据与核对器，不代表功能行为测试通过。

| 数据／动作 | 当前实际调用链与消费结果 |
| --- | --- |
| 控制意图 | 两页的 module2478 写 localStorage `isDynamicLighting`；其他同源文档的 browser storage 事件进入 LE 的 `window.onstorage`，再由 mutex 保护的分发器调用 `onStorageChange_dynamicLighting`。浏览器原生 storage 事件不通知原发送文档 |
| driver skip | LE → module2578 → module2198 → module4407.Vj → preload `doLightingDriverAction` → `lightingDriver` IPC → 宿主 Configure；payload 为 `mode.set/skip_wdl_devices`。driver setter 内部 await，但不返回 Configure 结果；LE 不 await 这个 setter，不能从后续进度推断 Configure 完成 |
| LampArray 查询／关闭 | module8905 → module2578 → module2198 → module4407.oE → preload `doDLLMainActionAsync` → `ffiPreloadAsync` → 宿主 JsonApi。JSON 参数在原生边界序列化；枚举 wrapper 对失败／缺失／异常结果返回空数组，关闭 wrapper 丢弃返回结果；空列表与 await 完成均不能单独证明设备不存在或已关闭 |
| 切换进度 | LampArray handler → module2010 → `electronAction` → 宿主 WindowStorage，以原始对象保存 `wdl-devices` 的 isSwitching／devices，并发送 `windowStorageEvent`，由两页 wrapper 转为 `windowstorageevent`。set 返回旧值，事件里的 newValue 已是对象；它不是 JSON 文本或设备 ACK |

已找到的本机 WebSocket 是 `rzchromabroadcastmanager` 与 `rzapichromasdk` 的独立链。上述意图／进度及 Configure／JsonApi 的实际边均没有走它们，不能把“存在本机 socket”当作这些动作已接通。存储分发先 parse 意图、await update，再 emit mode；update 内部 catch 日志并在 finally 清 switching，因此 mode 事件与 switching=false 仍可能发生于失败之后，不能作为控制权已转移的确认。

宿主 WindowStorage 按 sender URL 登记并去重订阅，注销时移除该 URL，没有同 URL 引用计数；同 URL 的一个页面注销会影响其他同 URL 页面。广播另排除发送者、remote、disposed 和未订阅的接收者。clear 只删所属 URL 的值，不广播也不移除订阅列表；get 返回 JSON 文本并省略 falsy 值，不能与 set 的原始对象事件混为一个契约。

两个 WindowStorage wrapper 都丢弃 `apiElectron.response` 返回的实际 disposer；组件卸载移除自己的 storage／emitter 回调，wrapper 的 beforeunload 注销宿主 URL 与 DOM fallback。另一方面，宿主 preload 的 pagehide 确实会 removeAllListeners("windowStorageEvent")，不能据 disposer 丢失断言页面销毁后必然泄漏；同 URL 生命周期、迟到任务和在途事件仍需分别测试。

LampArray handler 同样没有保留 response disposer。所选引擎没有显式调用独立 `RzWDL.registerEvent/unregisterEvent`；后者引用的 preload removeListener 并未暴露，catch 后仍返回 true，因而不能将这个返回值当作回调已注销。Configure 的宿主包装先 JSON.parse 再 FreeString，没有 finally，解析异常进入 null 结果路径；该 owner 也不能证明异常时已释放原指针。

3942 Quick 灯光的 Profile 配置由另一条 `synapse_` storage 消费链进入 LE：`onStorageChange_device → applyRegionQuickEffect/applyCustomQuickEffect`。它与 connectedDevices 的 driver／power／ports 更新分别核对；页面 completed、runtime 和 cache 都不能作为帧提交确认。当前十项 Quick／八编辑器、颜色草稿、Wave 挂载及 manifest 的即时／退出同步语义见[3942 灯光效果](../audio/playback-and-eq.md#3942-quick-灯光颜色草稿与实际消费者)；该片未执行 native 引擎、帧传输或设备读回。

LE 初始化在工作前设置 isInit，catch 失败后没有复位；terminate 只清 isInit，不恢复 window.onstorage。beforeunload 先 await onExit，再 reset 和移除监听，没有包住后续的 finally；exit 拒绝可阻止后续清理。本切片不证明浏览器真的等待 unload 的异步工作、所有回调排空或重连拒绝旧代次。

| 后续行为测试 | 必须区分的观察与失败 |
| --- | --- |
| 初读与同键外部变化 | 两页各自的 OS／本地读取顺序、损坏／非布尔／删除值、组件新编辑和迟到结果；意图不冒充设备读回 |
| Configure 与控制权 | 未 await 的提交、null／坏 JSON、driver 失败、进度清除与 mode 事件继续；分别比较参数、实际结果和设备集合 |
| LampArray 枚举与关闭 | 真实空列表、错误映为空、关闭返回被丢弃、prepare／readiness 失败；按设备确认真实状态 |
| 订阅身份 | 同 URL 双页面与不同 URL、发送者排除、一个页面注销、clear、pagehide、beforeunload 及迟到异步事件 |
| 初始化、取消与退出 | isInit 已置后失败、onExit 拒绝、重复初始化、回调 disposer、Stop／Shutdown 及真实资源释放；不以 true、finally 或窗口关闭当成功 |

以上是当前原件的静态接线与失败语义。两页的 Rust 目前仍只保存本地选择；真实控制权提交、设备读回、纯／模拟行为测试、官方实际 DLL／设备对照及窗口验收分别保持未验证。

## 驱动、服务与应答分别表示什么

引擎调用 Configure，载荷为 `{type:"mode.set",skip_wdl_devices:useWDL}`；这个调用者没有等待该 Promise。原生分发表把 `mode.set` 指向 RVA `0xb790`：载荷是对象时按存在的布尔字段修改 pause 和 skip 标志，即使两字段均未改变也会返回 true。

真正处理设备帧的 RVA `0xf4d0` 另读 skip 标志。仅当设备对应标志等于 1 且 skip 位为真时，直接返回 true 并跳过协议翻译器；其他设备仍走各自协议虚表。因此 mode.set 的返回不能解释成所有设备都收到了灯光帧。

LampArray `JsonApi` 通过本机命名管道 `\\pipe\\rzwdl_%s` 进入 RPC；服务调用表和反向 callback 表是两套契约。原生初始化顺序为配置 FFI、SetupRPC、Startup、SetNodeFFIEvent、StartWatcher；某阶段失败会按原函数返回 false，不能额外补写原函数没有的恢复步骤。

灯光驱动 Configure 返回 malloc 所有的 UTF-8 JSON，包装器解析后用配对 FreeString 释放。Startup／Shutdown 在当前 JS 声明中是 void；不能采用反编译器推断出的残留寄存器返回值当作 JS 的返回契约。

## 原件定位

本页逐字节复核 [完整捕获](../../../.analysis/results/host-current/local-audit.json) `dynamic_lighting.complete_source_owners` 的完整所有者与对应原件；当前查询范围和数量见文末。

| 所有者 | 原件与 UTF-8 字节区间 |
| --- | --- |
| 设置控件 | `synapse-settings-current/.../720.1e5d1c8f.chunk.js` `[84051,86061)` |
| 存储事件串行化 | `synapse-lighting-engine-current/.../main.a383ea9e.js` `[908613,909593)` |
| 设备控制权切换 | 同原件 `[909593,911799)` |
| LampArray 原生初始化、查询与清理 | 同原件 `[278509,287000)` |
| 原生灯光驱动包装 | 同原件 `[405624,425118)` |

下面继续解析全部 11 个已注册灯光协议的实际 translator、writer 与释放，以及 JSON 动作、设备打开与元数据、COM 调用、事件和结果消费者。它们是当前入口的具体静态结果，不代表整个产品集合、原生模块或运行时验证已完成。

## 11 类灯光协议、帧字节与实际发送结果

本节 LightingDriver 的 Image Base 为 `0x180000000`，均使用 RVA；不能与下节 WDL EXE 的 `0x140000000` 混算。`0x2e4f0 → 0x2e500 → 0x9240` 注册全部 11 个 key，以下 translator 均由实际 PE 的虚表 `+8` 取出，完整函数与原字节纳入同一查询。

| 注册 key | 构造函数 | 虚表 RVA | translator RVA |
| --- | --- | --- | --- |
| rzDevice25 | `0x2edd0` | `0xb3968` | `0x7feb0` |
| rzDevice25Chroma65 | `0x2ee70` | `0xb3b58` | `0x80460` |
| rzDevice25LedMatrix | `0x2ef10(false)` | `0xb3c68` | `0x80a00` |
| rzDevice25LedMatrixSkipSetEffect | `0x2ef10(true)` | `0xb3c68` | `0x80a00` |
| rzDevice25LedMatrixBatchSender | `0x2efb0` | `0xb3d78` | `0x811b0` |
| rzDevice25Glitter | `0x2f050`，长度 321 | `0xb3e88` | `0x85340` |
| rzDevice25GlitterWireless | `0x2f050`，长度 381 | `0xb3e88` | `0x85340` |
| rzDevice25MarciT1 | `0x2f0f0`，长度 781 | `0xb3f98` | `0x85610` |
| rzDevice25AudioCamy | `0x2f190` | `0xb40a8` | `0x85930` |
| rzDeviceIoT | `0x2f230`，自有 buffer 726 字节 | `0xb41b8` | `0x85d20` |
| rzDeviceLampArray | `0x2f360` | `0xb42c8` | `0x85fa0` |

实际注册 `0xa770` 只把精确的 category `keyboard` 映射为 4，其余为 0，`is_dock=true` 再改为 7。`link_seq_id` 必须是 unsigned JSON 且至少为 2，才进入身份的高 32 位。IoT／LampArray 协议还要求 `is_electron_supported=true`。`protocol.use_base_class 0xb310` 只登记明确 key；当前 JS `initLightingDriver` `[406017,406388)` 登记 Linker／DualLinkMouse／DualLinkKeyboard／Oled／Noa 五个 base-class key。`0x9920` 仅在 set 命中且 base_class_name 为字符串时选择基类，不存在按名称猜测的通用回退。字符串形式虽已插入 set，`0xb310` 却返回 false；当前调用者传数组，不能将两分支合并。

metadata 的实际布局为：`+0 category:u8`、`+1 reportID:u8`、`+4 deviceHandle:u32`、`+8 writeFunction:string`、`+40 protocol:string`、`+72 rowDelay:u8`、`+73 maxRows:u8`、`+74 batch:bool`、`+75 WDL:bool`。`HandleCustomRgbEvent 0xe250` 要求行列非零、frame 为数组且长度精确为 `rows*cols*3`；仅在 maxRows 为 0 时将首帧行数转换为 u8 保存。remap 的 `0xc7b0 → 0xea90` 先建零目标 buffer，按 `3*(row*cols+col)` 拷贝；源或目标三字节越界时跳过该项，函数仍可返回 true。`0xefb0` 传递行列起止、region、frameByte5、RGB 指针和长度；`0xf4d0` 再精确按 protocol 查 translator，未命中为 false。

下表 offset 从**包含报告 ID 的完整报文首字节**起算。正常列范围设 `n=3*(colEnd-colStart+1)`；`r` 为当前行，`q` 为 translator 自有 u8 计数。普通事务字节为 `(q&31)|(category<<5)`。25／65／Matrix／MatrixBatch 初值为 1，Camy 为 0；协议对象由同 key 的设备共享，不能为每台设备各自重新归零。

| 协议 | 已核实的完整布局与限制 | 返回、等待与特殊分支 |
| --- | --- | --- |
| 25／NewChroma | 91 字节全零；`[0]=reportID,[2]=事务,[6]=5+n,[7..8]=0f 03,[9..13]=frameByte5,region,r,colStart,colEnd`，RGB 从 14 开始；正常 n≤75 且源数据足够。`0x7fc30` XOR `[3,len)`，写 checksum `[89]`，不含事务 `[2]` | 消费 writer bool，false 终止；只在两行之间 rowDelay。空行范围返回 true；尺寸超限／RGB 不足记录日志后也返回 true，不能认作完成传输 |
| Chroma65 | 65 字节全零，同上字段，n≤48；`0x7fd70` 写 checksum `[63]`，同样 XOR `[3,len)` | 消费 writer bool、行间 delay；超限／数据不足返回 true。它有独立长度和上限，不是任意截断 91 字节 |
| LedMatrix／SkipSetEffect | 91 字节全零；`[0]=reportID,[2]=事务,[6]=4+n,[7..8]=03 0b,[9]=ff,[10]=r,[11..12]=cols`，RGB 从 13 开始，n≤75 | 消费 writer bool，**每行之后**均 delay。`0x80f90` 在 skip=false 时另发 91 字节帧：`[2]=下一事务,[6..9]=02 03 0a 05`、checksum `[89]`；额外帧的 `[0]` 保持 0。rowStart>rowEnd 也会进入额外 effect 分支。skip=true 才省略它 |
| LedMatrixBatchSender | 375 字节全零；`[0]=reportID,[2]=事务,[5..8]=01 36 03 0b,[9]=0,[10]=0,[11..12]=cols`，RGB 从 13 开始，n≤360；`0x81070` XOR `[3,len)`，写 `[373]` | `[10]` 固定 0，不是当前 r；消费 writer bool，每行后 delay；超限／RGB 不足仍返回 true。它的名字不代表外层 batch 容器已完成 |
| Glitter／Wireless | 总长 321／381；逐行 `[1]=84`（r+1 等于 maxRows）否则 04，`[2]=r,[3]=r,[4..5]=cols`，RGB 从 6 开始 | 忽略 writer bool，只在行间 delay。原汇编 `0x8543b` 的边界是 **`n <= sendLen+6`**；没有清零整份发送 buffer，也没有给 `[0]` 写 reportID，日志复用栈区保留 `[0]=5b`。未被覆盖的尾部没有确定初始化；不能补零或改成减 6 后声称与原版一致 |
| MarciT1 | 总长 781；总 RGB≤774 时，一包头为 `17 00 84 00 01 00 57`，RGB 从 7 开始。否则每行 n≤775，采用 Glitter 的逐行头与 RGB6 | 忽略 writer bool。逐行 fallback 的 `[0]` 同样保留日志栈区 5b，尾部未整体清零。整帧与逐行分支须分别对照，不能把未初始化字节解释为固定 padding |
| AudioCamy | 65 字节；`[0]=reportID,[1]=0,[2]=独立未掩码u8计数,[6]=n+4,[7..8]=0f 03,[9..13]=frameByte5,region,r,cols`，RGB14，n<49 | checksum `[63]` 的自有 XOR **包含事务 `[2]`**，不能共用普通 65 的排除规则。循环前及每行有明确清零；忽略 writer bool，仅按尺寸返回，行间 delay |
| IoT | 校验行列顺序及总 RGB+6≤726；紧凑格式 `[region,84,rowStart,rowEnd,colStart,colEnd]+RGB`，实际长度为 6+RGB | 忽略 writer bool，随后 delay，返回 true；没有 HID 报告 ID／通用 checksum |
| LampArray | len 非零时将 RGB 指针和 len 原样交给 writer，不加 HID 头 | 忽略 writer bool，随后 delay；只按 len 非零返回 |

前四类会在尺寸检查前推进计数，错误输入之后的下一帧也须测试。静态独立向量可以反驳错误编码，但不代替 DLL 实际样本：首次 category=4、reportID=0、row=0、单列 0、region/frameByte5=0、RGB=`11 22 33` 时，普通 91／65 的事务为 `81`、`[6..8]=08 0f 03`、checksum=`04`；Matrix 为 `81`、`[6..9]=07 03 0b ff`、checksum=`f0`，随后 effect 事务 `82`、checksum=`0e`；MatrixBatch 的 `[5..8]=01 36 03 0b`、checksum=`3f`；Camy 首次事务 `00`、`[6..8]=07 0f 03`、checksum=`0b`。期望来自原件指令与布局，不能由待测 Rust 编码器生成自身期望。

### 从协议选择到真实 writer 的 JS 消费者

当前引擎 `createLightingDeviceEx` `[410545,411775)`、`addDevice` `[461405,463857)` 和 `addMultiLinkSeqDevice` `[463858,465176)` 从实际 rzDevice 取得协议名、category、设备 handle、writer 名、identifier、base class、batch、reportId 与 WDL 标志；normal 分支另附 remap，multi 分支另附 link_seq_id。WDL 标志为 `info.isWDLSupported && !info.lampArrayDeviceId`，不能仅按 PID 或“支持 Windows”推断 skip。

| 当前协议构造 | 实际 writer／接口选择 |
| --- | --- |
| 通用 rzDevice25，main `[511629,516074)` | `hid.sendFeatureReportInBatch`、batch=true、reportLength=91；reportID 来自 extraParam，缺省 0 |
| Chroma65，`334.68120187.chunk.js` `[644,1250)` | 独立名称与基类名，继承通用 batch writer |
| Matrix，`3061.825e8b6b.chunk.js` `[715,5064)`；Skip，`269.63b2c328.chunk.js` `[644,1268)` | 分别明确 skipSetCustomEffect=false／true，继承通用 writer |
| Glitter，`8648.80c7d0df.chunk.js` `[715,2649)`；Wireless，`1452.f8a38141.chunk.js` `[661,5365)` | 专用 interface 1，reportMaxDataSize 分别 320／380；writer 沿 rzDevice25。无线 chunk 内 `[6110,8044)` 还有普通 Glitter 构造，不能只依文件名选择 |
| MatrixBatchSender，`8086.08213062.chunk.js` `[297563,298327)` | 灯帧 identifier 为 interface 3，继承 Matrix／25；实际 loader 是该 chunk 的 module 7752 |
| Marci，`330.d43d2948.chunk.js` `[878,5093)` | `hid.writeAsync`、batch=false；灯帧 interface2／usagePage65300／usage1、长781。普通命令则 reportID16／长91／usagePage12／usage1，不能混同 |
| Camy，`2237.e8b29886.chunk.js` `[746,5727)` | reportID7／长65，继承 25 的 batch writer |
| IoT，`1794.13beaac0.chunk.js` `[690,3254)` | 通常 `IoT.SetChromaFrameV2`；isJadeT1 改为 SetChromaFrame 并登记 whiteLED12，identifier 含 host，支持状态来自异步查询 |
| LampArray，main `[1118109,1126709)` | `LampArray.effect_set_colors_for_indices`、batch=false；identifier 包含 deviceID／indices／container／interface／PID |

`0x7ec90` 的 FFI writer 将 `function_name`、逐字节 buffer 和 `device_handle` 序列化为 UTF-8 JSON，调用 **void callback** 后固定返回 true。宿主 `ffiLightingDriver.js` 的完整 initDll `[262,1108)` 声明 SetWriteFFICallback／callback 均为 void(pointer)，查 deviceMap 后调用 onWriteCallback，既不 await 也不回传异步值。`electron/main.js` 的 `to.onWriteCallback` `[54855,55533)` 再将 batch 转 ArrayBuffer，其他 HID 转 Uint8Array，IoT／LampArray 转 Array，调用对应 handleAction，同样不 return／await Promise。因此即使 translator 消费了 writer 的 true，也没有取得最终 I/O／设备结果。

外层 batch `0xf4d0` 把 translator 写入收集容器，translator 返回 true 才调 `0x7e710` flush；**flush 的返回值被丢弃**。FFI true、尺寸错误返回 true、聚合 flush、HID 长度、设备 ACK 和页面状态必须分栏比较，不能都命名为“写入成功”。

### HID 实际 I/O、输出报告与取消释放

宿主 `UsbRzDeviceAction.js` 的 batch case `[14442,17648)` 按完整 key／container／usbInstanceId 定位设备，不存在时返回 0；它持有每设备 mutex，queue≥2 时丢帧，发送前和取得锁后检查 `lightingManagerFrames`，保留最小 2ms 间隔。该层 await addon 数值，finally 中 queue--，仅 mutexUrl 仍匹配才释放锁；上游 void callback 没有消费这个结果。

当前 `HID.node` 的 batch 入口 `0xc8d0` 要求 ArrayBuffer 并复制到独立 buffer，经 `0x5280` 排 request type 3、Promise 与 worker：`0x50e0 → 0x4800 → 0x16370`。parser 的 callback 虚表 `0x51c60 +0x10` 指向 `0x13700`，后者使用 `0x165f0` 实际 `GetProcAddress(hid.dll,"HidD_SetFeature")` 保存的地址，执行真正的 **HidD_SetFeature**；成功返回请求长度，失败保存系统错误并返回 -1，没有设备 ACK 或读回。

`BatchWriter` 的 `0x7e2e0` 生成 magic `37 13 fe ca`，`0x7e4e0` 按长度<256／≥256 编码 `f0 + u8长度 + report` 或 `f2 + u32LE长度 + report`，`0x7e430` 编码 `f1 + u8延迟`。HID parser 从 offset 0 逐字节扫描，**不验证 magic**；未知 opcode 跳过一字节。截断的长度字段或零长度返回 -1，但没有在传给 writer 前完整核对 report body 剩余长度。正常累计每份报告的写入长度，遇 writer≤0 立即返回该值并丢弃此前累计，已经发送的报告不会撤销。`0x4800` 对 -1 reject，其余 resolve 数值。应对合法、边界和失败输入分别建模拟测试；不能因为容器可解析而称各份设备命令已确认。

Marci 经过宿主 `writeAsync` `[27428,28165)` 与 HID binding `0x9750`，复制 array／buffer、检查 open，排 request type 4；worker 进入 **`hid_write 0x175e0`**。短于 collection OutputReportByteLength 时补零到该长度，实际执行 overlapped WriteFile；ERROR_IO_PENDING（997）时等 1000ms，再 GetOverlappedResult。当前完整函数仅在该完成分支赋 transfer count，不能把未核实的同步成功另推为相同数值。它与 Feature 报告是不同传输，也没有设备 ACK。

IoT V2 的当前宿主直接执行 `IOT_RunCmdFw25(host,"SetChromaFrameV2",JSON.stringify(array),array.length,true)`。legacy 重排为 `[0,region,rowEnd,colStart,colEnd,...RGB,whiteBrightness]`，whiteBrightness 没缓存时使用 125；支持查询返回固定 true，`runCmdFw25` 不消费原生返回。其实际队列、编码和 UDP 结果见下节。LampArray 宿主验证 deviceID／indices／FFI 后调用 `JsonApiNoReturn({action:"effect_set_colors_for_indices",device_id,indices,colors})`，随后返回 true；其服务 COM 下游与本节 writer 返回分别核对，不能用支持查询或宿主 true 代替设备结果。

停止、关闭、析构各自执行不同步骤：

| 所有者 | 已核实的实际动作与边界 |
| --- | --- |
| stopLightingFrames，宿主 `[25943,26096)` | 只设 `lightingManagerFrames=false` 并返回 true；后续排队／取锁检查会跳过，已经进入 addon 的 batch 没有由此建立逐帧取消 |
| setDevicesToDefault 与退出 NoAsync | 先置 frames=false；producer、基类报文、发送／ACK／重试和退出去重见下一节，不能将 false 标志当作已恢复 |
| removeDevice，宿主 `[5486,6845)` | 先取设备集合锁，按 PID＋container＋可选 usbInstanceId 找 key；逐个 hidDevice.close、mutex.cancel，删除 deviceMap／hidDeviceOpenMutexMap。没有显式等待所有 batch Promise，也没有取消 mutexLockBatch |
| nodehid.js close／pause | close 依次置 `_closing`、移除监听、`_raw.close()`、置 `_closed`；pause 只改变 read loop 标志，不取消已经发出的 read |
| native close `0x94e0 → 0x6170` | CancelIo(device)，关闭两个 overlapped events 和设备句柄，LocalFree 错误字符串，释放 input buffer／hid struct，将 object+40 清空；此 owner 没先等待共享 worker 排空 |
| HID 对象析构 `0x53a0` | 先执行 close，再释放 worker 引用；排队 request 创建时保存的 HID 指针不能仅由 object+40=null 推定已失效 |
| worker 析构 `0x4710` | 设置 stop byte+200、notify、join 线程，再清 queue。worker `0x50e0` 在外层和等待后检查 stop，进入内部 queue drain 后逐项执行 `0x4800`，不逐项重新检查 stop |
| Promise／请求释放 | `0x15ad0` 建 TSFN，`0x4800` resolve/reject，`0x15fe0/0x160e0` release TSFN；finalizer `0x161e0 → 0x15d40` 在结果消费后释放 request buffer、request、deferred 与 context 函数对象 |
| env cleanup `0xc600` | 仅 FreeLibrary(hid.dll)，清初始化标志与 module。parser 内的 Concurrency::cancel_current_task 仅用于缺 callback 对象分支，不是用户取消接口 |

LightingDriver 导出 `Shutdown 0x2550` 仅清全局 callback 指针，再跳 `TimerResWatcher::Stop 0x30610`：加锁、置 stop byte+80、解锁、notify，线程 ID 非零时拒绝 self-join，正常 join 后清 handle／ID。它**没有调用 LightingDriver 析构**。独立析构 `0x2e0e0` 才释放 FFIWriter、函数对象、回调数组、writer map、base-class set、protocol／metadata map、vector 和 shared state；不能把这条独立链写成 Shutdown 已执行。宿主 shutdown `[1194,1261)` 同样只调用 Shutdown 并清 libFFI，没有清 callback／deviceMap，更没有等待宿主 HID Promise。timer worker 已 join 与整套发送系统已释放是两项结果。

本次逐字节核验 LightingDriver 60 个函数／60 chunks，以及 HID 24 个函数／24 chunks，并逐条核验 6,952／3,152 条指令；均与对应 PE 相符。选定函数完整并不证明所有间接目标或所有产品恢复已闭合。对照必须覆盖共享计数、rowDelay、flush false、缺 map／队满丢帧、传输部分完成、停止后排队工作、在途关闭、Promise finalizer 与 worker join；本次未实际执行 DLL 或设备，不能将这些静态契约登记为运行通过。

### 3942 Marci 的引擎帧、返回值与退出恢复

[帧与退出附片](../../../.analysis/results/host-current/local-audit.json#lighting_frame_lifecycle)将当前 3942 manifest、引擎回调、LightingDriver、宿主 HID 和退出消费者连接，保留 48 个完整源码 owner、76 个原生函数及 350 个证据文件。它逐项核对保留的 2026-10-15 获取原件，本片没有再次访问部署服务器；禁用 collector 后的独立复核通过，43 个证据篡改反例全部拒绝。该结果验证所选静态证据，不代表 Rust、官方 DLL 或设备行为测试已经执行。

引擎的 `0x4980 → 0x32cb0 → 0x405b0 → 0x41ad0 → 0x410b0 → 0x40060 → 0x407a0 → 0x3f4c0 → 0x323d0 → 0x32330` 产生 eventId=2 的逐行 RGB 帧。mode=2 下首次到期为 10ms、周期为 40ms；待推送 map 按 device_handle 替换为最新帧，timer 取出并清空该 map。这不是每一渲染帧都送达的 FIFO。driver hook 经 `0x14f0 → 0x90f0 → 0x7e220` 解析已加载模块的 SetNodeFFIEvent，保留原 Node callback；pause、driver enable 和前端过滤／锁分别控制转发。前端以版本字符串 truthy 选择 driver，忽略 HookLightingCallback 的结果；这些条件均不证明设备已接收。

| 边界 | 当前实际行为与对照要求 |
| --- | --- |
| Marci 灯帧与控制报告 | 实际矩阵为 1×176。灯帧走 interface 2、usagePage `0xff14`／usage 1 的 781 字节 OutputReport，头为 `17 00 84 00 01 00 57`；普通控制走 reportID 16、usagePage 12／usage 1 的 91 字节 FeatureReport。两条传输不能互换 |
| native 与 JS 字节 | native 整帧分支只确定头与 RGB，1×176 时确定前 535 字节，后 246 字节未整体初始化；JS `Uint8Array(781)` 则将尾部清零。fallback 另保留日志栈区首字节 `0x5b`。实际差分必须保留原始报告，不得按固定 padding 清零或忽略差异 |
| driver／宿主返回 | `0x7ec90` 组 JSON 后调用 void callback 并返回 true；宿主 onWriteCallback 不 return／await USB Promise。`writeAsync` 对缺 handle／输入及捕获异常可返回 undefined，而前端只要 await 未抛异常就设 STATUS_SUCCESS；该状态不等于传输成功 |
| OutputReport 完成 | `hid_write 0x175e0` 的 result 初始为 -1，只有 ERROR_IO_PENDING 后等待及 GetOverlappedResult 成功才赋完成字节数；即时 WriteFile 成功仍留下 -1。须将该原件分支与 pending 成功分别对照，不能擅自归一化。完成字节数也不是设备 ACK／读回 |
| 控制确认 | 91 字节发送结果、transaction／class／id、status 与 busy／timeout 重试另由控制链检查；上层 common-effect wrapper 捕获日志并丢弃结果。Marci CustomFrame 的 set-effect 分支还会在不发送控制命令时构造源码级成功，不能据此认作硬件确认 |

退出链先置 exiting、关闭 SDK 状态、停止宿主帧、terminate 引擎，随后 await 外部停止 helper、清引擎全局，再进行 direct／registered 恢复并清设备表、发布退出标志。Marci 不在十项 host-default 注册名单内，实际进入 direct restore，写 normal mode 0 及适用的退出效果；当前 manifest 的 syncExit=true、useUser=false、effect4／direction=`"up"` 是退出目标，不能解释为原值快照。停止宿主帧只是 gate，LightingDriver Shutdown 只停止其 timer；两者都没有建立跨引擎、回调、HID worker 与在途写入的完成屏障。

外部停止 helper 的 JS owner 已确定：[当前 lighting JS](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js)的 `6191.$g → 2449.simpleLaunchRazerAppNoWait` 固定拼接 `5596.kM` 提供的 Interhaptics／Synesthesia 安装目录与 `SynesthesiaStop.exe`，参数为空；没有使用相邻工具函数的 installedResources 路径解析，也没有检查或返回宿主结果。原件 SHA-256 为 `c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`，[获取收据](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/acquisition.json)实际 SHA-256 为 `d453c6631d8f1e14368d8f26b91832ca2a1bb9829e9acccd779253b1e53eb407`。宿主回调声明为 `(void,[bool,string,int])`，实际 JS 只消费前两个参数；失败对象不会阻止后续恢复，bridge Promise 拒绝则可中止恢复，回调始终不到达可使等待一直 pending。helper 原件及其内部退出仍缺证据，不能将函数名中的 NoWait 或一次回调当作子进程退出／恢复屏障。

另沿当前宿主 4.0.827 包核对实际 unversioned `simple_service.dll` 1.2.2.10；[官方获取收据](../../../.analysis/inputs/host-current/acquisition.json)实际 SHA-256 为 `149596e4bcc7c06b2f953fa7e177dcc7919e07e8783e9fa255fdab89ad53f7b1`，静态提取原件与 installed alias 均为 `f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b`。[完整捕获](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/capture.json)实际 SHA-256 为 `0acfaec64d8ace705ffe4f6b258cdce4772a13faf8c955374ef8c6ab9f3e71dd`，其中 53 个续追函数、53 chunks、22,266 原始字节及 5,115 条指令均重新与 PE 比较。该续追独立于冻结的帧附片，不改写其未验证范围。[NoWait 导出](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/14d30.json)把等待参数置 0，经实际 vtable `0x1f2040+0x80 → 0x3f90 → 0x4170 → 0x23a26 → 0x23c10 → 0x1d9f0` 排到 [IO 工作函数 `0x2af90`](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/2af90.json)。它先检查实际路径及签名，文件不存在返回 `Error : Razer App does not exist.`；[提升判定 `0x4dfa8`](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/4dfa8.json)静态读取 EXE manifest 的 requestedExecutionLevel，发现 requireAdministrator／highestAvailable 才尝试 COM NoWait，未要求提升或 COM 失败再走本地创建分支。此链不下载或部署 helper。[完成回调](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/14b10.json)仅向 FFI 传 `(bool,reason)`，exitCode 只用于日志，并受全局初始化 bit0 gate 控制。声明的第三参数不是已经观测到的进程退出码，gate 关闭时也不能推定回调必达。

工作目录须区分真实分支：[普通创建路径的目录 owner](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/7c440.json)来自 GetTempPathW，获取失败时创建参数为 NULL；[ShellExecuteExW 提升回退](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/79e60.json)的 lpDirectory 为 NULL；优先的 [COM 本地提升服务](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/functions/2e89a.json)调用表内偏移 80 字节（`0x50`）的槽，当前官方服务端的内部创建和工作目录仍未闭。关闭同样有可替换的 HandleVerifier；默认链 `0xe1d60 → 0xe1ab0 → CloseHandle` 沿正常返回给 1，没有把原 BOOL 交回调用方。默认关闭器、启动结果或 COM HRESULT 均不能代替 helper 已退出及灯光已恢复的确认。

该 COM 槽已由[客户端与类型库证据](../../../.analysis/results/host-current/installation-elevation-client.json)明确为 `IElevator::LaunchProcessNoWait(BSTR filePath,BSTR params,UI4 cbId)`，没有退出码输出参数。当前客户端的方法返回即尝试 Unadvise、释放 176 字节栈上临时 sink；其释放没有在途回调屏障，不能承诺未来的退出通知。完整 ABI、事件消费者及保留服务副本的目录／等待分支见[安装提升链](../host/firmware-and-installation.md#提升-com-的客户端类型库与事件释放)。该服务副本是本机保留的 1.1.0.8，尚无当前官方部署字节绑定；不能将其内部行为升级为当前 helper 的退出证明。

helper 的获取缺项也已与相关包分开：[当前 Chroma manifest](../../../.analysis/inputs/official-native-current/manifests/27f1518bf49220045840c8688b1ca83f47eb38bb8591868bed085a33782bdb68/manifest.json)声明 `Skip_RzInterHaptics` v1.0.0.49；[官方资源获取收据](../../../.analysis/inputs/official-native-current/acquisition.json)实际 SHA-256 为 `43c6d74023265e062051e966b479747f277ccb39c082e846faff92c7c3ae4811`，绑定该 manifest 的真实 URL、包长及包 SHA。其[完整静态提取收据](../../../.analysis/results/native-packages/33954697ea3c99d8a7e1ffd99255834c29a01e4d0712b9e8fbc83f5db346d83f/inspection.json)及[完整文件列表](../../../.analysis/results/native-packages/33954697ea3c99d8a7e1ffd99255834c29a01e4d0712b9e8fbc83f5db346d83f/listing.stdout.txt)没有 SynesthesiaStop.exe。原包、11 个提取文件与四份输出共 16 个身份重核一致；这只能证明该包未提供所找成员，不能证明所有官方载荷都没有 helper。所缺的是包含该文件的当前官方载荷、版本、获取映射和依赖，不能按 InterHaptics 名称或 HapticService 的停止代码替代它的内部证据。

安装检查也不能补足该缺项：[当前 Chroma chunk 4461](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/raw/apps.razer.com/chroma-app/dashboard/static/js/4461.7a5efde4.chunk.js)的 `isInstalled` 对带 runOnInstall 且 resourceName 匹配 `/(?<=Skip_).+/` 的条目直接返回 true，不检查安装目录、helper 或服务。显式 excludedInstallResources 列表只有 `Skip_ExperienceService`，由 `global.immersiveExperience.provisionFlags` 控制，没有据此排除 `Skip_RzInterHaptics`；`Skip_` 前缀既不证明 helper 已安装，也不代表该资源从安装清单移除。该 JS 原件 SHA-256 为 `ce7f0b7af360a64242945bfcecad3e96a33b397aa1f61862fa5cd67f73c79d2b`，[获取收据](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/acquisition.json)实际 SHA-256 为 `1be25c3fe55b0707a564333c9fb1ef0aa6771a689daa03b33e8c2561a30a5a16`；此处仅证明当前更新检查消费者的判断，helper 内部、实际部署及真实停止结果仍未验证。

当前 Rust 已由 [Marci 协议所有者](../../../crates/razer-device/src/lighting/marci_lighting.rs)、[独立服务所有者](../../../crates/razer-service/src/runtime/marci_lighting.rs)及 [后台渲染／传输／退出链](../../../crates/razer-service/src/runtime/marci_lighting_worker.rs)消费 3942 的 781 字节 OutputReport。它没有扩展 PID4115 的 controller owner。Windows 在当前完整 SetupAPI 枚举中确认同一 container、同一 parent 的两条 collection，再与 HID 枚举身份及实际 781／91 字节 descriptor 交叉检查；每次 I/O 前后重核保留身份，拒绝设备替换。连接先按当前 JS 顺序发空 RGB Output probe，随后读 firmware、GET mode 并按需 SET driver3；CustomFrame 的源码级成功不增加设备 ACK。

[当前 manifest 几何](../../../crates/razer-device/src/lighting/marci_geometry.rs)保留 31×32 软件矩阵、1×176 设备矩阵及 60 项实际 LED 映射，不补齐另外 116 列。软件帧按 handle 替换，首次 10ms、后续 40ms 聚合并清空；generation 变化清除旧帧，后台 gate 丢弃不允许转发的到期帧。实际 Output 完成字节数、连接 probe、Feature 应答与退出确认分别记录；短写保留实际 count 并停止渲染，不能显示硬件已确认。JS 分配的 246 字节零尾与 native opaque 尾仍分别建模，原生整帧对照必须显式提供捕获尾部，不能把零尾解释成 native padding。

[页面捕获](../../../crates/razer-pages/src/features/workspace/source_workspace.rs)仅在当前 3942 活动 Profile 的 Quick 对象实际变更时发布请求，[Shell 消费者](../../../crates/razer-shell/src/shell/audio/marci_lighting.rs)先完成独立配置快照的原子保存，再向当前唯一物理连接发布；保存成功不冒充帧完成。所有者保留最新请求、递增 generation、连接租约和后台任务，配置档／连接切换拒绝迟到结果；断开和退出先关闭请求入口，再等待 Stop、真实 worker join 及子进程释放。页面打开没有据本地缺文件或读取失败写默认的分支。全局 Windows 控制权、SDK hook、其它产品和各 gate 的界面来源仍须按各自链路验收，不能由该消费者推断完成。

退出目标同样跟随当前源码：[硬件 Quick 转换](../../../crates/razer-device/src/receiver/receiver_quick_effect.rs)将 UI ID1..8 按当前参数转换；其余软件效果按 manifest 的 defaultHWEffect=4 转 Wave，保留当前 settings 后再转换方向／速度。它记录计划目标，不把它当成设备观测；只有尚未同步前的 manifest 目标才是 effect4／字符串 `"up"` 转 flag0。正常、错误和取消路径均先清理 renderer，再以独立身份检查执行 GET／必要 SET normal0、实际退出效果应答，最后释放两条 HID handle。恢复失败不跳过剩余释放，取消不取消已经进入退出阶段的恢复。

[Wheel 渲染器](../../../crates/razer-device/src/lighting/renderer/wheel.rs)补齐当前 UI ID13 的共享计算：预设十个颜色停点、speed360、25 帧周期、mode1024／2048、manifest 中心的 40 单位变换、1024 色表、按 row／column 的 atan2 取色、有限 cycles 及仅清 cursor 的 reset。其依据是当前主 JS 的 6394／8905／2578 owner，以及已保留引擎的 `0x66b40/0x66d60/0x66f20/0x692e0`；混合 float／integer 参数由汇编核实，未采用丢失实参的伪代码。该效果复用独立 Rust renderer，不加载官方 DLL。

`marci_` 设备测试覆盖定义字节／opaque 尾、实际计数／短写／传输失败、双 handle 最新帧、gates／旧 generation、控制应答分支、20×30 重试上限、事务号 31 回绕、60 项几何和 15 组硬件退出参数。`marci_lighting` 服务测试覆盖完整渲染到报告、短写、在途取消、部分初始化、gate 失败、恢复失败后继续释放及首次 I/O 前取消。Wheel 有固定颜色／角度向量、方向截断、周期、有限 cycles 和 palette／中心转换测试；实际执行结果以统一 [Rust 验证收据](../../../.analysis/validation.json)为准。

显式维护入口 `python tools/workspace.py compare-local-lighting` 已实际执行当前官方 4.0.55.0 DLL 的公开 JSON 调用链，以固定 3×3 软件几何对照 Static 与 speed=0 Wheel，两个场景各九个 COLORREF 像素与独立 Rust 结果逐值一致。来源、完整函数与 vtable 原字节、ABI、系统依赖、双方原始输入／输出、退出及释放保留在唯一[灯光功能对照收据](../../../.analysis/results/lighting-runtime-comparison.json)的 `attempts` 中；每次比较固定并复核完整 Rust 输入闭包，旧输入上的通过不自动适用后续修改。场景通过原版 preview 路径禁用物理输出，首次完整非黑帧是独立就绪条件，不按 Rust 期望值挑选帧。Fade、动态时序、其余效果／参数、调用中的取消、真实设备写回／读回和窗口验收仍未验证。

Ambient 的源码与 native owner 为 `0x5ce60/0x5cf80/0x5d0b0`：screen rect 使用 65535 单位，full／四边 10% 裁区，当前 Quick preset 的 Blur=1；原生采用独立 screen-capture 对象、帧身份去重、最小 3×3 缓冲和 `0x5d9d0` 的逐轴 Deriche 平滑。`0x74010` 按系统能力选择 Desktop Duplication，初始化失败再尝试 Direct3D 9，旧系统走 GDI；`0x74180` 在最后一个引用释放时析构捕获对象。现有完整捕获中的相关 23 个函数、2,790 条指令／10,924 字节已与官方 PE 比较，无字节差异。

[Ambient 渲染器](../../../crates/razer-device/src/lighting/renderer/ambient.rs)独立实现裁区、最近邻采样、逐轴 byte 截断、原版 padded stride 和 1×1 中心取样；Deriche 的指数返回值及浮点运算顺序以完整汇编核实。[屏幕采集所有者](../../../crates/razer-service/src/runtime/windows/ambient_capture.rs)保留三个 Windows 后端、30ms 共享刷新门、每个效果的已交付帧身份、丢失访问后的重建与停止释放。Desktop Duplication 的 Frame／Map、Direct3D 9 的 Lock 和 GDI 的选中对象分别配对释放；GDI 保留 HALFTONE 缩放及逐消费者 postincrement 的帧号语义。它们由实际后台灯光 owner 持有，不在页面 render 中执行。

屏幕采集状态与已缓存灯光输出分别保留；首次无新帧时 `frame_id=None`，原生初始化黑帧不冒充屏幕实读，失败后的旧输出也不冒充恢复。Marci 服务状态贯通 IPC，Shell 独立显示屏幕采集错误。纯／模拟测试覆盖固定滤波向量、裁区与 pitch／旋转、同帧去重、首次超时、多个消费者、失败、重建和释放；当前执行结果仍以统一验证收据为准。实际桌面采集、Ambient 官方 DLL 对照、真实设备与窗口验收尚未执行，SynesthesiaStop 原件缺项独立保留。

旧附片的 `followup_plans`／`remaining` 等字段保留其历史收据语义。冻结基线的 61 项静态复核现为 0 未决、0 失败，当前结果见 [unresolved-current](../../../.analysis/results/reverse-pending/unresolved-current.json)；这不等于全部 Rust 功能、界面接线或设备验收完成，也不要求重复执行已完整捕获的静态提取。

### HID 中先前失败函数的真实职责

[编码语义结果](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/utf16-semantic-audit.json)已重新核实 `0x30888`：它是 **UTF-16 code-unit 到 UTF-8 的增量转换**，不是 HID 构帧或设备传输。PE 的 `.pdata 0x6170c` 与 unwind `0x539a0` 明确给出单块 `[0x30888,0x3092b)`、163字节／49指令，无异常拆块；跳到 `0x3ac8c` 是尾调用，并非丢失函数体。

基础捕获和 `566417cf283f8a4e` 的失败均保留。另有独立的 `c98d562484439a06` 成功补捕获，按汇编恢复两个 helper 原型后保留8函数／8chunks／821指令；本次只复核该既有结果，没有重做类型恢复或覆盖失败记录。补捕获全部2,702字节及16个辅助caller的17chunks均与当前官方同字节原PE相符。

Microsoft x64 参数为 RCX输出缓冲、DX码元、R8可空的8字节state、R9错误上下文。state为空使用静态地址`0x5dca8`，没有在此函数内分配独立线程状态。pending只检查state低DWORD；无pending时高surrogate只写低DWORD并返回0。合法高低对在输出非NULL时输出4字节并清整个state。孤立低surrogate或pending后遇到非低surrogate，清整个state、写context+0x2c=42／+0x30=1并返回-1，错误context没有NULL保护。NULL输出只有进入scalar helper后才清state并返回1，不绕过owner的surrogate校验：无pending的高surrogate仍返回0，上述非法码元仍返回-1。函数没有输出容量参数，NUL、普通BMP和非零pending各自的分支也不能合成通用“转换成功”位。

| 汇编推导输入（初态0、输出非NULL） | 返回计数／输出／终态 |
| --- | --- |
| `0041` | 1／`41`／0 |
| `d83d,de00` | 0再4／`f09f9880`／0 |
| `dbff,dfff` | 0再4／`f48fbfbf`／0 |
| `dc00` | -1／无输出／清state并置错误字段 |
| `d800,0041` 或 `d800,0000` | 0再-1／无输出／清state并置错误字段 |

实际caller为宽字符串formatter `0x24f88` 和整串转换 `0x3bdf8`，二者都由codepage65001条件进入。formatter在有界码元序列末尾可留下pending而不额外验证；整串转换遇到终止NUL则会检查并拒绝pending。数字／指针格式化与它们共用上层formatter，并不能证明这些具体调用会走宽字符串／65001分支。无heap、句柄或transport由此owner获取，清理是状态重置；上层另释放locale／formatter上下文。编码／容量／surrogate／错误传播仍需对应测试，但不再将这个函数误列为未找到的设备命令。表中数据是静态独立期望，**没有实际执行DLL取得输出**。

### IoT 的真实队列、协议选择、UDP 字节与释放

对应当前 [IoTNative 1.0.20.0](../../../.analysis/results/native/f2e86e50e994dc6f04385cdc60e79e38941199e81b0560f61b9039acd95dbeb8/all-functions.c) 与 [IoT SDK 2.0.10.071100](../../../.analysis/results/native/6469bf29d42a7dae225bdb74797015d34120bda6116ed30f00f160069afd721b/all-functions.c)，两者均 AMD64、Image Base=`0x180000000`，当前官方获取收据及同字节安装副本由统一查询复核。这里的 TCP／UDP 是本地 LAN 设备传输，不属于已排除的账号或在线云业务。

`IOT_RunCmdFw25 0x37590 → 0x36da0` 复制 host、command、JSON 到独立对象，按 host 取得 CommandQueue，再排队、signal，最后固定返回 1。isUdp=true 跳过入口的已连接检查。worker `0x2720 → 0x1050` 出队后经 vtable+16 调 `0x48fe0`，丢弃返回值。该函数对 UDP 比较同 host 上次放行时间：相差不足 **15,000,000 ns（15 ms）直接丢帧**，否则更新时间再发送；queue 构造 `0x2dc0` 已初始化时间，首帧也可能命中这一门槛。它不是等待 15 ms 后补发。

`0x35b40` 核 SDK handle／参数，把 JSON Array 转 u8 vector，但向下传递的 length 仍来自调用者，未核对实际 vector 长度。SDK 入口 `0xad17 → 0x26dd20 → 0x6e97 → 0x25cc60` 对这两种帧命令拒绝非 UDP 分支。注册的 class=`0f`，`SetChromaFrame` id=`03`，V2 id=`07`；legacy 与 V2 的 RGB 头不能互换。

协议选择值来自**接收报头识别**：SocketCallBack 构造与 open 将 `+540` 置0；`0x2517c0` 在缓冲至少5字节后，将 `02 00` 识别为1，`aa 02` 为2，未知仍0。`0x24acf0` 最多发三次识别 probe，等该 callback 条件变量／超时参数。它不是 firmware version，也不是“支持IoT”的通用能力值。

generic command `0x247ef0 → 0x247920 → V25::Package 0x227050` 生成内框：`[0]=00`、`[1]=transaction`、`[2..3]=总长度BE16`、`[4]=00`、`[5]=payload长度低8位`、`[6..7]=class/id`、payload 从8开始，尾部为 XOR 和 `00`。XOR 覆盖总长度、reserved、payload长度、class/id 与编码的 payload 槽，不含0／1。选择值1使用 payload+10 的可变内框；选择值2由 serializer 将 payload 截至80、补零为**固定90字节内框**，再经 `0x264870 → 0x1f4dc0` 加 `aa 00 总长BE16 00` 五字节外框。构造器有4096字节存储，extended 编码调用缓冲为2048；80限制发生在固定帧 serializer，不能提前把所有输入都截为80。中间 JSON／大写hex会被解回字节，实际网络载荷不是 JSON。

以下为按已核 PE 指令独立推导的完整调用链向量，**没有执行 DLL 取样**。RGB=`11 22 33`、默认 whiteBrightness=`7d`：

| 命令／选择值 | 具体结果 |
| --- | --- |
| V2／1 | payload=`00 84 00 00 00 00 11 22 33`；19字节 `00 00 00 13 00 09 0f 07 00 84 00 00 00 00 11 22 33 96 00` |
| V2／2 | 95字节：`aa 00 00 5f 00` + `00 00 00 5a 00 09 0f 07` + 上述9字节payload + 71个`00` + `df 00` |
| legacy／1 | payload=`00 00 00 00 00 11 22 33 7d`；19字节 `00 00 00 13 00 09 0f 03 00 00 00 00 00 11 22 33 7d 6b 00` |
| legacy／2 | 同样固定90内框及5外框，id=`03`、上述legacy payload、checksum=`22`，不是将19字节内框直接加5 |

`0x2637d0` 按 isUdp 选 host 的 transport vector 项，只有该 socket 的 ready byte+57 为 true 才调 primary vtable `0x3ebb28+8 → 0xa6c3 → 0x263670`。open `0x252420` 创建 TCP10003 和 UDP10005；UDP 实体 `0x2532b0` 使用 AF_INET／SOCK_DGRAM／IPPROTO_UDP。真正传输为 **sendto**（TCP为send），返回OS发送字节数／-1，并设 ready=`result>0`。**该逐帧UDP链不读 ACK／不读回效果**；发送失败后 ready=false，后续可在实际发送前返回0，直到其他连接链重建状态。UDP open 本身不启动 receive worker，不能借 TCP 状态事件冒充帧确认。SDK `0x25cc60` 仍把发送数值返回 IoTNative `0x35b40`，后者也返回并可记录日志；仅非UDP分支发命令结果事件。真正忽略执行返回的是队列 worker `0x2720`；`0x37590` 的即时返回1只表示提交，JS又忽略该值，最终传输数值没有反馈到页面。

Disconnect `0x6d75 → 0x26d720 → 0x19ce → 0x249b20` 在 host 不属于 transport Map 时直接返回-1，不设置stop、不通知也不发Status14。存在时置 callback stop+705，逐项通过 secondary vtable `0x3ebb50+24 → 0x82e2 → 0x247510` 调 closesocket／WSACleanup；随后严格按 **`0x244830` 删除transport Map → `0x250fd0` 通知多个condition → `0x244620` 删除callback Map → `0x262fb0` 发Host／Status14** 执行。返回值是最后一个非空transport的close结果，不能解释为两项聚合成功。原生 `0x38690` 还要求Node callback和payload存在、a1／a3均为0、原payload含精确`"Status":14`、提取host非空且对应Block存在，才调用`0x15b0`置stop，并在线程存在且不是当前worker时join，再删除host队列；在途任务完成后才join结束，剩余任务释放，不能把SDK断连解释成队列必然清空。open 创建的 reader／keepalive 线程随后 detach，Disconnect 此 owner 没有 join，线程与回调的完整归属见下节。

IoTNative Init `0x346e0 → 0x34170` 先核固定版本 SDK 文件，再 LoadLibrary 和11个导出，只有成功才注册原生 callback。**UnInit `0x34c60` 仅 FreeLibrary，忽略其BOOL并清 handle**，有handle返回true、无handle返回false；没有主动注销callback、停止发现、断开host、清导出指针或join队列。另一个 CRT 析构 `0x6e1b0 → 0x1cc0` 才走 manager joinAll／Map清理，不能写成UnInit主动完成。SDK版本 getter只是固定字符串，IoTNative包装器在未加载／异常时返回空串，版本读到值不等于传输可用。

本节核对 IoTNative 24个函数／24chunks／3710条指令及SDK80个函数／80chunks／18281条指令，另核两侧各2个版本函数；所有chunk和指令字节与官方PE一致。纯／模拟测试应覆盖同host与异host、首帧／15ms边界、队列取消与卸载竞争、JSON与length不一致、未知selector、80／2048容量、两种完整帧、socket未ready／短写／错误、断连重连、条件通知与在途callback。逐帧无ACK是当前协议证据；实际设备效果、Rust实现、真实DLL差分及整SDK语义仍分别未验证。

### IoT 发现原件、设备身份与停止扫描

当前发现源是 **MDNSEvent Status12 → JS发现缓存 → NEW／UPDATE**；没有当前SDK／Native的`GetMdnsDeviceList`导出。MW的`getAllDevices`返回已配对的`iot_devices`，不能代替发现结果。

Native `IOT_StartMdnsDeviceScan 0x34f40`在SDK已load且`0x903d8`未标扫描时，固定调用SDK Start(-1)，再创建detached timer `0x3c900`，默认8秒、传参不为-1时用传入值；到期才调SDK Stop并清标记。重复Start在标记true时直接返回1，不能解释成新扫描已开始。**`IOT_StopMdnsDeviceScan 0x35400`本体只日志并返回1，没有调用SDK Stop**，不能用该返回值证明扫描已取消。

两层duration单位不同：Native正数按**秒**设置steady deadline，0及除-1外负数以now为deadline，timer调用异步Stop后就清Native标记。SDK按**毫秒**、每轮query返回并发布记录后才比较wallclock elapsed；汇编将duration零扩展，-1实际是4294967295ms阈值，0也先做完首轮才自然结束。不能把同一个数字从一层直接搬到另一层，或把清Native标记当作worker已退出。

SDK `0x87e2 → 0x26df20 → 0x4b06 → 0x2672f0`使用固定service `_wled._tcp.local`、query timeout2000ms，注册MdnsCallBack后`0x8c5b → 0x267700`创建detached worker `0x9ddb → 0x24f1b0`。已有global holder`0x47e118`则不新开worker；56字节capture持有duration、service字符串与timeout至worker结束。每轮先清上轮record，再经`0x24fdf0 → 0x264b60`查询。

网卡枚举`0x2536b0`用GetAdaptersAddresses(AF_UNSPEC,flags6)，8000字节初始buffer，overflow最多重分配4次；只取OperStatusUp，排除Teredo、所列loopback／非preferred地址，最多32个client socket。IPv4／IPv6均建UDP、SO_REUSEADDR并加入`224.0.0.251`／`ff02::fb`，bind所选本机地址的**10003**端口、设nonblocking。`0x2503e0`发transaction0的PTR(type12)问题到组播5353；源端口非5353使QCLASS为`0x8001`。接收buffer2048字节，`0x250100 → 0x2508b0`处理answer／authority／additional。每次select重新使用2000ms，持续数据可延长整轮；它是安静等待窗口，不能当作整轮硬deadline。此查询内部不检查stop。

真实record callback是未单独成函数的跳板`0x13c0`（5字节`e94b722500`）到`0x258610`，不是默认打印器。它用单一全局168字节聚合record：SRV target／PTR owner／A IPv4文本／TXT mac／TXT exdata／SRV端口分别在+0／+32／+64／+96／+128／+160。TXT key转小写后按位置0匹配mac／exdata；record没有按host分别聚合。PTR／SRV／A／TXT各递增counter，达到4且处于additional section才提交，**不代表四类各已齐全**。callback未解析AAAA字段，不能从IPv6 socket支持推出IPv6设备身份支持。

查询结束后，仅PTR／SRV／A非空、IP不以字符0开头、端口>0、exdata非空、PTR owner小写后从位置0匹配service的record才进入`0x251260(Status12)`。端口只作筛选，连接层仍使用TCP10003／UDP10005。`0x239880`还要求exdata区分大小写同时有`ProductType_`、`&VID_`、`&PID_`、`&EID_`、`&NAME_`和`\MAC_`六个marker；字段是相邻区间的字符串，不在此转成数值。Host来自A；HostName取NAME至MAC前，序列化异常写InvalidHostName；Mac优先独立TXT mac，否则取exdata的MAC后缀，未另核MAC格式。marker存在但区间无效时字段仍可为空。

发布链为`0x251260 → 对象0x45da70 → vtable0x3eb8d8[0] → 0x8e0e → 0x239880 → 0x9e44 → 0x23d390(event2,cmdType0,JSON)`，再走共享adapter和四参raw callback。worker内部还构造空record的Status8／13／10，但六marker门槛会抑制这些通知，不能说Node已收到开始／结束／停止事件。SDK这层没有产品PID白名单，产品匹配由JS消费者处理。

SDK真实Stop `0x2bcb → 0x26df40 → 0x2b4e → 0x267910`创建detached `0x23fa00 → 0xbccb → 0x23d270(wait=true)`便返回；不是同步等待。该任务仅在holder存在且stop原为false时置stop，并在done=false时无限条件等待；stop已true则跳过等待。随后释放holder并清全局指针，属于done等待，不是join。worker先done／notify，再可能发Status10及析构capture，因此连内部等待完成也不保证全部回调和线程已退出。查询内不查stop、select会续满超时，均须纳入取消测试。

### IoT 连接队列、识别与连接状态

Native `IOT_ConnectNetDevice 0x35940 → 0x35710`复制host并取得同host队列，插入AddConnectTask、signal后**立即固定返回0**。真实任务经vtable`0x767f0+16 → 0x49220 → 0x35590`调用SDK；内部`0x35590`在SDK未load／host指针为NULL时返回-2，但worker不把该结果交回入口；`0x35710`先strlen，不能因此假定导出入口可安全接收NULL。SDK `0x2315 → 0x26d430 → 0x8666 → 0x2481c0`另返回真实open结果。已有连接则发Host／Status16并返回-1；`IsConnected 0x18e8 → 0x24bac0`仅检查保存的TCP和UDP都非空、ready都true，没有主动ping或设置读回。

`0x252420`按 **TCP10003 → reader → selector probe → 登记callback与TCP map → keepalive → UDP10005** 执行。host经inet_addr，本连接链为AF_INET，没有域名解析。TCP先WSAStartup1.1、SO_REUSEADDR、nonblocking，一次connect为负便进select(read＋write)，等待值>0按毫秒，否则10秒；select ready后还须SO_ERROR=0，失败则close／WSACleanup。成功后恢复blocking，没有重建socket的自动重试；上层MW的三次连接是另一条链。

selector probe `0x9c5f → 0x24acf0`以KeepAlive时间payload编码extended帧，再改byte0=AA、byte4=02并将checksum XOR02。每轮要求TCP ready且sendCount>0，**先send再清`+704`**并等待condition`+552`，每次10秒、最多3次send；第三次超时或任意send失败返回-1。等待谓词为`+704`，不是selector；至少5字节垃圾也会置位，三次均被唤醒时可在selector仍0时返回0。不能把open非负标成“已确认协议”，也不能补写源码没有的transaction ACK关联。

识别负值时close TCP，尚未登记host map；此时显式Disconnect只查map，不能据它声明取消了在途识别。识别非负才登记并启保活，然后尝试UDP；UDP失败**不改写整体返回值**。因此NETEvent Status0与随后IsConnected=false可以同时发生。`0x2481c0`按open非负／负分别发Host／Status0／1，并返回原结果；通用`0x262fb0`的Status来自调用点，不是在函数内固定14。这些连接事件也没有VID／PID／Mac，身份另来自发现链。

### IoT 的 TCP 接收、帧边界与保活

`open 0x252420` 只为 TCP10003 创建 reader `0xb302 → 0x259190` 与 keepalive `0x4ce1 → 0x24bd30`；UDP10005 不创建这两个 worker。reader 在 socket ready `+57` 为真且 callback 存在时，清零1024字节buffer及outcount，经 primary vtable slot0 `0xb66d → 0x259010` 接收。调用方虽然传10000，callee没有使用该timeout参数；connect将socket临时改为nonblocking后恢复blocking，所核链没有设置SO_RCVTIMEO，**不能把它当作10秒recv超时**。

`0x259010` 对TCP调用recv、UDP调用recvfrom，仅返回大于0才填outcount，并将ready设为`count>0`。reader的TCP分支对count≥0经`0x6181 → 0x2517c0`发status2和实际长度：count=0也发status2／len0，下一轮因ready=false退出，本分支不主动close。count<0记录WSA错误，发status3／len0，再经secondary slot+24调用`0x247510`关闭，没有自动重连。底层另经`0xa678 → 0x251490`发通知，但只有socket内callback指针`+424`非空才分发，不能与reader通知直接相加推断数量。

`0x2517c0` 对status4／5只清metadata。status2在锁`+784`内按selector`+540`分流；识别阶段先把数据追加到buffer`+712`，累计至少5字节后只看最前两字节`02 00`／`aa 02`，没有在中间搜索帧头。达到5字节即置`+704=true`并notify，即使selector仍0；**通知不等于识别成功**，前缀垃圾可能持续阻塞识别。只有selector非0才把整个识别buffer交parser并清空。

V25链为`0x2541d0 → 0x6230 → 0x22bcb0`，数据追加到persistent buffer`+760`。selector1走extended且要求XOR，selector2解析内层时不强制XOR。候选至少8字节、status≤13；transaction拆高`e0`／低`1f`，total为BE16。extended上限4096+10，payload=`total-10`；fixed为90字节，payload取size byte，payload必须`0≤n<4096`。8个header字节全相同时跳过8字节。XOR覆盖bytes2..7及payload，fixed始终覆盖80字节含填充；fixed的status1／2还可绕过XOR。尾字节被读取但未要求为0。

这里有两个影响独立实现的真实边界：parser仅在已接受候选的`skip+frameLength == 当前全部输入长度`时返回status，**不是找到第一完整帧便交付的通用粘包parser**；失败返回`-21082`时可能保留先前consumed。调用方`0x2541d0`先按consumed删除前缀、copy／swap缩容，再检查错误，所以失败不代表全部原数据仍保留。成功才进`0xaeac → 0x2600d0`，handler的`-20994`仅记录日志，解包成功也不等于设备设置确认。

下表是指令推导的独立静态边界向量，记`F=0200000a000012021a00`，结果为`(status,skip,consumed)`；没有实际执行DLL。

| 输入／解析条件 | 结果 |
| --- | --- |
| F，extended且核XOR | `(2,0,10)` |
| F的前9字节 | `(-21082,0,0)` |
| F连接F | `(-21082,0,10)`，首帧未dispatch；调用方删首10字节，第二帧暂留 |
| F的XOR由1a改1b | `(-21082,0,10)` |
| F末尾00改ff | `(2,0,10)` |
| ff前缀接F | `(2,1,11)`；此向量直接给parser，不绕过前述识别门槛 |
| `0200005a00001202`＋80个00＋`4b00`，selector2／不核XOR | `(2,0,90)`，实际应XOR为4a |
| 同一fixed向量，强制核XOR但status2 | `(2,0,90)`，命中fixed status1／2例外 |

Wled链`0x4fc5 → 0x255630`使用buffer`+736`，扫描AA标记，以5字节header及BE16 total定界。外层先允许total5..0x405，但payload=`total-5`必须严格小于0x400，故最大完整payload为**1023**字节、total1028；没有外层checksum。首个完整合法候选即可返回，不受V25的whole-input限制。cmd0将payload交V25 parser；其余经`0x956b → 0x261480`，剩余buffer有限循环处理，不能宣称任意垃圾都能恢复。

`runV25 0x2600d0`／`runWled 0x261480`两个handler均先置`+248`并notify，再置`+88`并notify，**所有到达handler的解析帧都会喂活，不只KeepAlive**。V25 class0x12／cmd5及Wled class0x4e4554／cmd5把30秒interval改回8秒；`SetWifiConfiguration 0x2626a0`按命令名前20字节匹配先置30秒。keepalive `0x24bd30`初始retry4／interval8秒；timeout先检查retry<1，否则递减，剩≥1时interval0、降0时interval4秒，再一次timeout才close。解析通知仅改变局部等待状态，**不会重置retry**；局部状态0时清`+248`再等8秒，有解析继续等、无通知才probe。stop`+705`避免重复Status14。

V25 class16另进入firmware等待槽`+496/+504/+408`并notify，这里只是静态定位，没有执行固件操作。其余按class／cmd表调用decoder slot+24：V25 class18来自`0x4581e8`，cmd2经`0x1a96c0 → 0x710d → 0x13bfc0`；Wled class0x4e4554来自`0x4581f0`，cmd2经`0x1eb380 → 0xb3c5 → 0x13c210`。KeepAlive decoder匹配时取package.status，未匹配取5，按flag可带payload hex。已找到handler的V25发event1／cmdType1或2，Wled发event1／cmdType1；缺handler发Status15／cmdType0并返回`-20994`。喂活、发送Status4和命令结果需分别观察。

### IoT 回调 ABI、队列删除与 detached 生命周期

Status14真实链为`0x262fb0 → qword47e120 → 对象0x45e2a8 → vtable0x3eb8f8[0] → 0x6ab9 → 0x23e390 → 0x6721 → 0x246ca0`。`AddSdkIotCallBack 0x19ec → 0x26d030`／Remove `0xbaaf → 0x26d830`在global mutex`0x45e140`内维护map`0x47e220`。发布时持锁为每个非空rawFn复制72字节capture，含JSON、注册id、event、cmdType和rawFn，并单独创建detached线程。`0x2374 → 0x23f860 → 0x2b8f → 0x25a100`的汇编明确以ECX／EDX／R8D／R9传**四个实参**`rawFn(id,event,cmdType,payloadCString)`，不能照伪代码误认三参。Remove只删map，已经capture的rawFn仍可执行，通知完成顺序也不保证与发送顺序一致。

注册id0的断开实际到`IoTNative 0x38690(0,1,0,payload)`。eventId／cmdType经`0x59370`转十进制**字符串**；event参数0／1／2投影为BLEEvent／NETEvent／MDNSEvent，其余为原拼写UnknowEvent。payload能解析则`0x3a350`深拷贝JSON，失败则`0x426e0`保留string。典型对象为`{"eventId":"0","event":"NETEvent","cmdType":"0","data":{"Host":"…","Status":14}}`；原payload没有VID／PID／Mac，不能推断这些身份。

Native要求Node callback已登记且payload指针非null；空C字符串仍进入解析／原字符串路径。重新dump的字符串同时含CommandName和KeepAlive时直接return，连队列清理也不执行。正常分支**先调用Node callback，再检查**原始id／cmdType、精确`"Status":14`、解析Host和Block条件并删除队列。`IOT_DisconnectNetDevice 0x359c0`只转SDK，故其返回不证明Native host队列已经join。队列stop也不能中断正在执行的同步任务；worker存在且不是当前线程时才join。SDK注销不等待已capture回调，Native UnInit又未注销／join，不能称安全卸载已经闭环。

两个接收／保活worker各持独立40字节capture，其中callback与socket各有strong shared_ptr。`0xa010 → 0x23f950`增加weak并作为worker实参，不代表capture没有strong引用；擦除host map不会单凭weak失效停止线程。worker返回才`0x5556 → 0x2964ec → 0x76b7 → 0x242db0`释放strong及40字节capture。线程均detach、没有join，退出依赖ready／stop、显式close中断blocking recv或保活超时。

SocketTransfer strong归零`0x8076 → 0x2442a0 → 0x835a → 0x243e50 → 0x8b89 → 0x243060`只清字符串／free，**不兜底closesocket**；真实关闭在`0x247510`。SocketCallBack析构`0x419c → 0x244290 → 0x58e4 → 0x243e00 → 0xa0b5 → 0x2430e0`清接收buffer、map和字符串；weak归零后另经`0x244270/0x244250 → 0x243ce0/0x243ca0`释放控制块。释放C++对象、关闭OS socket、停止worker和清空Node队列分别有不同的完成条件。

接收／回调扩展新增SDK83函数／83chunks／13,744条指令及Native3函数／3chunks／358条指令；发现／连接另新增SDK69函数／69chunks／5,733条指令及Native9函数／9chunks／711条指令，两组另核14处静态区域。所有chunk和逐条指令均与原PE相符，新增集合与前节不重复，累计选定SDK234／Native38函数，不代表整库完成。测试须覆盖mDNS聚合／marker／源端口／select续期、扫描重复与停止、单TCP／双transport状态、识别垃圾唤醒／send后清标志、拆包／连帧／XOR例外／末尾非零、1023／1024边界、EOF／负recv、喂活不重置retry、注销后回调、stop与在途同步任务、map删除后strong存活及无close析构；实际DLL／设备差分仍未验证。

### 七个 IoT 产品的断连恢复与页面消费者

[当前恢复链收据](../../../.analysis/inputs/frontend/synapse-iot-reconnect-current/acquisition.json)在2026-10-22T11:46:42Z至11:54:51Z逐个核对35个URL的前后字节，另7次入口复核，共77次HTTP200、无重定向、字节全部稳定；[完整所有者与产品投影](../../../.analysis/results/frontend/synapse-iot-reconnect-current/source-owners.json)保留329个完整JS AST所有者及18个选定原生快照。它是七个实际发布入口的有界核对，不是全产品／全前端依赖已获取。

FFIPreloadMain callback以void(string)取得JSON，向允许的channel及有效webContents发送完整event；preload只移除Electron event参数，没有另投影data。engine `rzIoT._ipcInit [390224,390747)`把外层eventId／cmdType parseInt；NET Status14先`_onDisconnect [380521,380796)`再per-host callback。原payload只有Host／Status，经`_ioTDeviceMock [392131,392524)`得到NaN／undefined身份，但Host完整保留；该层不删发现缓存、不改online。同host再次发现会复用已缓存对象发UPDATE，没有在该分支刷新其身份字段。

**真正恢复所有者是产品MW**。初始化先载DLL、读取配对记录及本地cache，逐对象创建mgr并将其runtimeData引用加入subDevices，等待Promise.all(connect)，再调用未await的事件订阅setup，随后checkNeedScan并置isInit。DISCONNECT消费者按Host找已连接mgr，锁设备并等待entity.release；release仅off(host)，不调Native Disconnect。随后清proxy／device，发布`setupStatus=WAITING,isOnline=false,isLocked=false,isLightingOff=true`并检查重扫。只要有已配对对象未连接，就立即`startMdnsDeviceScan(10)`，此后每12000ms再扫；全连上时仅清自身timer，没有调用停止原生扫描。

NEW／UPDATE按已配对productId＋Mac对应container匹配离线mgr；Host／name变化先更新对象及本地iot_devices，再connect。factory参数2表示最多三次尝试，间隔1000／2000ms；entity先查IsConnected，false时不await低层connect，再等待一次性的CONNECT0／FAIL1事件，Status14在entity.handleIotEvent被忽略。MW上层重试与扫描不能当作SDK自身自动重连，更不能用Native入口0证明连接成功。

七个入口都显式启用rzDeviceIoT、isDualLinkDevice=false；下表仅记录当前入口声明，`—`表示未声明。checkPowerSupply／hasLedLayout的getter使用`!!`，iotLedType则可保持undefined；它们是软件分支，不能当设备实测能力。

| PID | iotType | checkPowerSupply | hasLedLayout | iotLedType |
| --- | --- | --- | --- | --- |
| 780 | keyLights | — | — | — |
| 781 | gameRoom | — | — | — |
| 782 | gameRoom | — | — | — |
| 783 | gameRoom | — | — | — |
| 784 | gameRoom | — | true | RGB |
| 790 | gameRoom | true | — | WhiteAndRGB |
| 791 | gameRoom | true | — | — |

文件目录统一为`.analysis/inputs/frontend/accessories-mw-{PID}-current/raw/apps.razer.com/synapse/products/{PID}/mw/`。以下为逐文件核实的完整函数区间；manager列startScanDevice，shared列NEW／UPDATE及DISCONNECT，entity列connectDevice及release。

| PID | manager | shared | entity |
| --- | --- | --- | --- |
| 780 | `7254.5893937c4383b81bd58e.js` `[38142,38413)` | `6120.0ffd890be8f3f4f39c13.js` `[1463725,1464190)`／`[1464191,1464419)` | `rzDeviceIoT.ef9c7f861050887ce61d.js` `[8418,9126)`／`[7102,7212)` |
| 781 | `7254.7952178e74ab61499946.js` `[38148,38419)` | `6120.022be1af222ddec67d71.js` `[1465995,1466460)`／`[1466461,1466689)` | `rzDeviceIoT.09f867a539247ac3f643.js` `[8424,9132)`／`[7108,7218)` |
| 782 | `7254.35ef50f7d9c092784a80.js` `[38152,38423)` | `6120.a59590d1c56d528c6d87.js` `[1463680,1464145)`／`[1464146,1464374)` | `rzDeviceIoT.6441493748090fadf0f0.js` `[8424,9132)`／`[7108,7218)` |
| 783 | `7254.9f4f24c41ea8fb79b350.js` `[38144,38415)` | `6120.885db62922a07b0c28f0.js` `[1463672,1464137)`／`[1464138,1464366)` | `rzDeviceIoT.da772f02da5a177dced5.js` `[8416,9124)`／`[7100,7210)` |
| 784 | `7254.b8d42764a8e37147b82d.js` `[38142,38413)` | `6120.56f948af7b9ecebf8833.js` `[1463725,1464190)`／`[1464191,1464419)` | `rzDeviceIoT.f1601ff2e464579e99d9.js` `[8418,9126)`／`[7102,7212)` |
| 790 | `7350.9d42e14f014cb6adb137.js` `[37789,38060)` | `6259.47e56554b90c6439277c.js` `[1434180,1434645)`／`[1434646,1434874)` | `rzDeviceIoT.343c998d6cb1874a3599.js` `[8128,8836)`／`[6812,6922)` |
| 791 | `7254.69ab92122dc708d910f5.js` `[38142,38413)` | `6120.cc62b6ba444a5b1c77a0.js` `[1463725,1464190)`／`[1464191,1464419)` | `rzDeviceIoT.97b7b2bdf90bf616c8a7.js` `[8418,9126)`／`[7102,7212)` |

每个mgr的updateRuntimeData浅写原对象，比较整份root runtimeData hash后，把产品runtimeData写到WindowStorage connectedDevices。engine `RzIotMgr.filter [403362,403534)`只取IOT subDevices且`isOnline===true/isLocked===false`，再与本地配对身份匹配；`init [403534,404531)`按Host比较新旧，先REMOVE并按Mac删除，有删除时等待1000ms后再NEW。引擎入口B／V分别`[1616271,1616923)`／`[1616924,1617584)`，在PID＋container mutex内初始化／移除后端；mounted Ko的effect`[1766633,1768811)`按同一身份改页面列表。它们只订阅RzIotMgr NEW／REMOVE，不订阅底层DISCONNECT，页面恢复依赖上述MW状态发布。

清理的真实限制也须保留：entity等待超时只resolve FAIL，不移除once监听；release只移除Host监听。MW destroy停止自身扫描timer及事件setup，但设备mgr的模式／GrabControl恢复不await、清proxy前未release／disconnect，storage listener也未解除；engine mounted cleanup仅移除scroll，没有解除设备订阅，lower destroy仅日志。必须针对超时后迟到事件、相同Host／新Host、相同PID多container、七个feature分支、发布离线到页面移除的间隔、先连接再订阅的窗口及取消／destroy交错分别测试；没有DLL／设备或窗口运行验收，不能把静态链闭合登记为自动恢复已经通过。

### 默认恢复命令、应答分类与停止后的真实结果

当前引擎的 `configureDevicesDefaultData` main `[876285,878505)` 只处理有 DeviceInfo、非 Chroma Studio region、有 mode builder 且名称在十项白名单内的设备。白名单 `[905181,905456)` 为 `rzDevice25`、`rzDevice25LedMatrix`、`rzDevice25Oled`、`rzDevice25Linker`、`rzDevice25Glitter`、`rzDevice25GlitterWireless`、`rzDevice25DualLinkMouse`、`rzDevice25DualLinkKeyboard`、`rzDevice25LedMatrixBatchSender`、`rzDevice25LedMatrixSkipSetEffect`。65／Marci／Camy／IoT／LampArray 不因“都是灯光设备”自动落入此路径。这里生成的是退出时恢复设备模式和 effect 的已登记命令，**不是恢复出厂，也不是首次发现或读取失败后覆盖设备**。

每台设备先生成 normal-mode，再按条件追加 quick-effect：优先已读取的 user effect／退出同步 effect，否则使用 quickEffectOnExit；电量效果按 support 与当前 effect 另选。特殊 effect 193 只处理 commandArray 内的 `setChromaEffectCommon`。profile／region／effect 的缺省表达式分别为 `||1`／`||0`／`||3`，不等于保留显式 0。只有 mode frame 存在才把整台设备加入结果，delay 固定 3；产物没有 usbInstanceId／usagePage／usage，不能替它补成已核实的完整设备身份。

基类 mode builder `[514281,514466)` 使用 `[2,0,4]` 和 payload `00 00`；effect builder `[514467,514855)` 使用 `[80,15,2]` 和实际长度 `6+colorArray.length`，payload 为 profile、region、effect、flags、rate、numOfColor、RGB。`_createDataSend` `[516287,516581)` 构造未加 reportID 的 90 字节：transaction 在 `[1]`，size／class／id 在 `[5..7]`，payload 从 `[8]` 起，`[88]` 是 `[2,88)` 的 XOR；基类 transaction 从 0 开始使用 0…30，遇 31 重置。依次生成 mode 与 spectrum 时，前者 `[5..9]=02 00 04 00 00`、checksum=`06`，后者 `[5..13]=06 0f 02 01 00 03 00 00 00`、checksum=`09`；这是静态指令推导的独立向量，尚未实际执行原版。

十名称对应当前部署的 **14 份实体类**，已逐一核实际 class 与 base import；重复的 Linker／Glitter 不能因类名相同就只读一份：

| 子类／原件 | 已核实的恢复协议差异 |
| --- | --- |
| Linker，4026 `[690,7437)`、3961 `[2130,8877)`、2314 `[695,7442)`、8086 `[193746,200493)` | 四份均继承 module4001 基类；transaction 为 `e0 \| counter`，不是普通 0…30 |
| DualLinkKeyboard，986 `[698,8732)`；DualLinkMouse，8508 `[700,9258)` | 均继承4001；前者 transaction=`80 \| counter`，后者无该高位标记 |
| OLED，7826 `[3362,23590)`；Glitter，8648 `[689,6856)` 和1452 `[6084,12251)`；Wireless，1452 `[635,5935)` | OLED／两份Glitter继承4001；Wireless实际继承8648。没有覆盖上述 mode／effect builder／构帧方法，普通命令不等于各自灯帧格式 |
| Matrix，3061 `[689,8065)`；BatchSender，8086 `[297537,300692)`；Skip，269 `[618,1269)` | 后两者实际继承3061；共同使用 Matrix 覆盖的 effect builder `[1937,4921)`，不能使用基类 `[15,2]` 的统一 payload |

表中编号均为前节同一固定版本 JS chunk 的文件名前缀；完整实体、继承绑定与三份枚举／header 导出保留在动态灯光审计收据。Matrix 将通用 Off／Static／Breathing／Spectrum／Wave／Reactive／BasicRipple／Starlite／CustomFrame 映射成 `00/06/03/04/01/02/13/19/05`。默认 header=`[80,3,10]`，payload[0] 是该 Matrix effect；Off／Spectrum 长1，Wave／CustomFrame 长2，Static 长4，Afterglow 长5，Breathing 按 flags1／2／3 长5／8／2，Ripple／Starlight 长9。Fire 另改 `[80,15,2]`、长9并写 profile1／region0／effect9／flags／rate。由同一恢复列表生成的 Matrix spectrum 为 class03／id0a、size1、payload04、checksum=`0c`，因此 ACK 要读03／0a，不能沿用基类0f／02。下文已按当前141个不同PID／edition的gate分支补核DeviceInfo值源；证据范围之外的产品／版本及实际quick-effect应答仍须另核。

effect 参数生成器 `[723897,726536)` 的当前分支如下；生成后是 `{...原输入去掉effectParams,...计算值}`，未赋值字段不会自动补齐：

| effect | 实际参数分支 |
| --- | --- |
| 0／3／8 | flags=0、rate=0、空颜色；没有自动生成 numOfColor |
| 1 | 从 colorArray 或颜色值展开 RGB，numOfColor=length/3，flags／rate 缺省 0 |
| 2／7 | random／双颜色／单颜色决定 flags=3／2／1；直接给 colorArray 时不推断 flags。numOfColor=length/3；7 的 rate 缺省 duration，2 缺省 0 |
| 4／14 | flags 依次取 flags、direction、1；rate 缺省 40，numOfColor=0 |
| 5／6 | 5 的 rate 依次取 rate、duration、3，颜色数量由数组计算；6 的 rate 缺省 40、数量固定 1 |
| 9／10／11 | 9 固定 rate=90／红色，10 的 flags 依次取 flags、boost、0，11 的 rate 缺省 40；三者都未生成 numOfColor，不能由 RGB 长度自行修补 |
| 12／13／其他 | 12 将 flags 的 truthiness 或 counterclockwise 转为 2／1、rate 缺省 40；13 固定 flags=1、rate=0、空颜色；未识别 effect 只日志并返回原输入字段 |

基类 effect builder 仅在 `numOfColor>0` 时复制 RGB，故上表缺数量与“长度包含 RGB”可能同时存在。特殊 193 分支传入 flat flags／rate／colorArray，而生成器按 `effectParams` 取色；相应错误只在该条命令的 catch 记日志，之前已加入的恢复命令不撤销。这些原版边界必须保留为对照样例，不能把自行规整后的参数当作原版结果。

宿主 `hid.configureDevicesDefaultData` `[19290,20246)` 克隆数组、重建 PID 列表并设置 `useDefaultCmdNoAsync=true`；如果已处于 quit 且此前执行过退出恢复，会立即再执行一次。async `hid.setDevicesToDefault` `[17648,19290)` 先置 frames=false，但 NoAsync 标志一旦开启就直接返回；其余分支仅在 HID handle 和 mutex 存在时持锁发送 protocol 25，不读 ACK，异常只日志。它给 90 字节命令添加的前缀实际为 **`d.payload?.reportId ?? t.claimInterface`**，不是单设备 `t.reportId`，与同步分支不同。

NoAsync 外层 `[34823,35589)` 清 sentSetDeviceModeCommands 和 frames，克隆数据只计算最大命令数；等 10 ms 后逐命令下标等 3 ms，再把**原始存储数组**传给 sender `[35590,39409)`。sender 不获取设备 mutex；同一下标出现重复 key 时另等 2 ms。它允许显式 protocol 25，或缺 protocol 且长度为 90／91；没有 Sync API 时只登记异步 then／catch，不等待 I/O 完成。有 Sync API 时最多发送三轮，返回 91／65 才进入下列判定：

| 应答判定 | 实际行为 |
| --- | --- |
| 三类命令 | 仅 size=2／class00／id04 的 mode、class03／id0a 的 matrix-effect、class0f／id02 的 Chroma-effect 读 GET；其他命令在接受发送长度后直接结束 |
| GET 次数／等待 | mode 最多 5 次，其余 3 次；首轮 min(delay??5,30) ms，后续该值+3 ms；当前 producer 给 delay=3 |
| 应答读取 | `protocol25.js` 只接受 Array 长度90／91／65，status 在0／1，class／id 在6／7或7／8；**没有核 transaction、checksum、payload 或目标值** |
| 状态 | 0=new、1=busy、2=success、3=failed、4=timeout、5=notSupported；class／id 不匹配也归 notSupported，非数组／长度错误归 failed |
| 重试结束 | busy／timeout 继续 GET，其他非 success 触发重发；但 busy／timeout 耗尽或 GET 一直异常可让 resend 标志保持 false，直接退出，不能读作恢复成功 |
| 去重与结果 | mode 在接受发送长度后、首个成功 ACK 前就写 sentSetDeviceModeCommands；退出命令会据此跳过同目标 mode。返回 undefined、日志尾标记及这个集合都不构成设备实测状态 |

完整宿主电源／退出顺序见[窗口与退出生命周期](../host/window-lifecycle.md)。本节已查清 producer、十名称 gate、14 份实体继承及构帧覆盖、两个 sender、ACK parser 和结果消费者；DeviceInfo 默认、读取与消费者的值源继续细分于下两节。独立 Rust 行为及实际 DLL／设备结果须分别验证。测试必须区分 mode frame 缺失、193 部分失败、注册后再退出、重复 key、Sync 缺失、短写、未知应答、busy／timeout 耗尽、GET 异常和退出去重，不能只验证“恢复函数被调用”。

### 退出目标的 manifest 来源与硬件 snapshot 的实际条件

本节的主输入为 **229 份 manifest（211 个 PID／edition、86 个 PID）和3份 JS**，共232份输入，长度／SHA与67份 acquisition及总索引一致。SKU专项观察另取得736/0、737/0的两份同字节副本；协议选择专项再按当前目录和实际MW选择入口取得3871/0、515/0、521/0、545/0的四份引擎备用manifest。逐一核对原SHA、完整字节、JSON及字段后，当前manifest范围为 **235份物理输入、215个PID／edition、90个PID**；manifest来源目录共68个，加引擎收据共69份。两份SKU副本不增加产品分支，四份备用件则补回Glitter和Matrix的明确产品；原229份分析身份、两份副本及四份新分支分别保存在值源结果的`manifest_scope_supplement`与`manifest_protocol_selection_supplement`，原144个语义owner未改。

2026-10-22对Dashboard／灯光引擎入口、manifest、相关JS／CSS、设备表及Service Worker共15个官方URL的两轮静态GET均前后稳定；本节所用灯光引擎main和两个相关chunk与保留件相同。Dashboard Service Worker已经改变，保留旧件与新件分别记录；SKU专项的另14个URL观察见下节。这些有边界的在线核对不代表全部产品、所有manifest或整个应用依赖闭包均已重证。

DeviceInfo 模块8199 `[1281275,1358467)`、类 `[1307479,1358421)` 的 `configure [1307512,1319984)` 直接把 manifest 的 `C.quickEffectOnExit` 赋给实例，**没有合并通用默认 manifest**。真实 `prepareDevice [798037,800387)`／`createNewDeviceInfo [820622,821045)` 传入发现身份、edition、layout及LampArray参数；`onLightingEngineDeviceInfo [913087,913381)` 仅解析、日志，不是同一配置producer。

`getManifestData [1330761,1333740)` 的官方选择顺序为 MW `products/<primaryPid>/mw/lighting-manifests/DeviceManifest_<pid>_<edition>.json` → 引擎自身 manifest路径 → 与IndexedDB比较版本／内容，较新缓存可以胜出 → 前两者都无结果时再读Cache API `razer-device-manifest-aio`。请求非零edition且最终无可用结果时，`configure`重试**同PID的edition0**，第二次正常完成后明确把实例editionId设为0；返回undefined仍使configure失败，具体缓存和消费者条件见SKU小节。Partner走独立vendor路径。LampArray `shouldGetManifest=false` 会提前返回并设置 skipQuickEffectOnExit。以上描述原版来源，不能转为本产品的在线依赖；独立实现须从安装目录库取得已核实本机表并保留版本／身份语义。

默认 chunk `4238.286b7bbe.chunk.js` 的完整 `r [610,1748)` 确有 effect3／空params，但它只属于PID0对象；动态context7531 `[1186068,1186476)` 也没有把它合并到所有产品。AvailableDevices原件与完整有序映射现已取得，具体消费者和缓存边界见下节；不能按产品号或别的目录位置自行补齐。

原件还保留一个身份差异：[226 MW的DeviceManifest_227_128.json](../../../.analysis/inputs/frontend/mouse-mw-226-current/raw/apps.razer.com/synapse/products/226/mw/lighting-manifests/DeviceManifest_227_128.json) 文件名为227／128，内部productId却为226（version0.0.0.5）。`configure` 采用发现参数的productId，并没有赋值 `C.productId`；审计分别记录文件身份、字段值和实际消费者，不将它们改写成一致，也不凭内部字段覆盖已发现设备。

`quickEffectOnExit` 是**退出目标**：它可来自manifest，也可由软件灯效转换而来。`getInfoFromLocalStorage [1337557,1340073)` 读当前profile的软件quick effects／ports；`setUIEffect [1334912,1335556)` 在 keepSynapseQuickEffectOnExit分支将其转换为硬件格式。`syncHWEffect [825229,826331)` 在syncExit路径直接改成 `{profileId:1,regionId,effectId,params}`，随后重配host退出包。229份主输入及两份同字节补充均未发现keepSynapseQuickEffectOnExit=true或effect193，相关代码分支存在不等于已观察到对应产品。

真正来自硬件的 `userQuickEffectOnExit` 由 `onDeviceInfoReady [800388,820621)` 内 `[814211,814505)` 的条件分支取得：相关Chroma路径、非Studio region、effect maps非空、需要电源按钮的设备已开机、非hardware-only，再加useUser=true。顺序为 addChromaDevice → 需要时登记退出包／setDeviceMode(3,0)／OLED active-item → 准备effect maps及power-plan／BLE FPS → **读取snapshot** → 写profile0／region0／effect8并应用各region → 重新登记退出包。因此它不是任何应用干预之前的完整原始状态。

读取链 `getChromaEffectCommon [731819,732880)` → rzDevice25 getter `[548800,548901)` → encoder `[1091719,1092237)`／parser `[1091425,1091718)`；header为 `[80,0f,82]`，profile／region使用 `quickEffectOnExit.profileId||1`／`regionId||0`。只有STATUS_SUCCESS才构造snapshot，但helper对缺字段填effect3、region／flags／rate／颜色数0、空颜色；调用方还把effect11改成1并清rate。它是**经过缺值填充及规范化的读取结果**，不能冒充原始应答。读取失败返回undefined，不生成新snapshot；这段也没有先清旧snapshot，不能把旧结果贴到新连接。独立Rust须保留原始字段是否实际返回、连接代次及规范化来源。

| 已保留manifest分支 | 实际退出与读取条件 |
| --- | --- |
| useUser=true的21分支／13PID | `642/0、678/0、688/0、719/0、721/0、728/0、739/0、740/0、742/0、746/0`，`679/{0,128,130}`、`720/{0,128,129,130,131}`、`741/{0,128,129}`：类rzDevice25、退出effect3 |
| 另3分支／3PID，合计24／16 | 756/0=rzDevice25、757/0=DualLinkKeyboard、758/0=rzDevice30，退出effect4／direction1；**758的hardware-only=true，不执行上述snapshot GET**，也不在十名称gate内。三个原件均由756 MW的lighting-manifests声明 |
| syncExit=true，共5个唯一分支，useUser均false | 3940/0=GlitterWireless、736/0与737/0=MatrixBatchSender、1465/0=rzDevice40，退出effect3；3942/0=MarciT1，退出effect4／direction=`"up"`。加两份同字节副本后物理输入为7份，唯一分支仍为5个；这些是软件效果同步条件，不证明硬件snapshot |
| skipQuickEffectOnExit=true | 3929/0、3946/0，即使manifest含effect3仍跳过该退出动作 |
| OLED691 | edition0／128／130／131均useUser=false、退出effect3，不能由OLED能力推断读取snapshot |
| 不同edition的明确颜色／效果 | 659/133为effect1／`#63B1BC`，659/0为effect3；716/{0,128,129}为effect14／direction2；725的129／130／133为effect1／`#F2A900`、131／132为`#305CDE`；1396/131为effect12 |

当前补回的3871/0明确使用rzDevice25Glitter，515/0、521/0、545/0明确使用rzDevice25LedMatrix；这四项均有当前目录、实际MW选择／构造及引擎manifest依据，具体见下节。235份物理输入中仍没有直接选择rzDevice25LedMatrixSkipSetEffect的manifest；loader／十名gate保留该类不能替代具体产品证明。241的LinkerMultiDevices也不在该十名gate，继承关系不能改写显式名单。

两个退出消费者还存在实际差别：批量包在 `(useUser || syncExit) && userQuickEffectOnExit` 时用snapshot；JS direct restore只在 `useUser && userQuickEffectOnExit` 时使用。缺snapshot时原版选择manifest／当前退出目标，**不等于已恢复设备原值**。差分测试须区分产品／edition、缓存版本、hardware-only、读取前已有写入、缺字段规范化、读取失败保留旧snapshot、软件同步覆盖和两个消费者；本轮未执行相关设备／DLL，也未把原版缺值规则作为Rust伪造实测状态的授权。

### 14 份实体的值源、附件与版本边界

[完整值源结果](../../../.analysis/results/frontend/synapse-lighting-engine-current/deviceinfo-value-sources.json)保留144个完整AST所有者、14份实体、229份主manifest、两份同字节补充及四份引擎备用件，共235份物理manifest和69份获取收据，另按输入索引复核引擎153份JS。两份副本的完整字段投影与原件一致；四份新分支保留全部非几何字段、完整JSON的规范化身份和原始字节，几何数据仍在对应原件中。原144个语义所有者、原231个分支及扫描范围保持不变；旧分析身份、主范围和增量验证均独立记录。以下是实际初始化、值来源和消费者差异，没有读取本机运行中的缓存或执行JS。

| 对象／阶段 | 真实来源与分支 |
| --- | --- |
| DeviceInfo构造 | `quickEffectOnExit`／`userQuickEffectOnExit` 起始undefined，effects maps为空；power／腕托／外接电源字段有软件初值。configure按引用赋manifest退出目标；这些赋值均不产生硬件snapshot |
| 物理与逻辑身份 | 发现记录、DeviceInfo、实际协议实例分别存在。`_createRzDevice [857267,859293)` 以真实USB／BLE对象和rzDeviceConfig创建协议，之后可另改逻辑PID；snapshot helper直接调用传入对象，不能擅自替换成另一个helper选出的LampArray协议 |
| 14份继承实体 | 基类25定义`getChromaEffectCommon`；13份派生实体均未覆写或构造赋值此getter。Matrix／Batch／Skip改SET或构帧不等于改GET。4份Linker、2份Glitter各自保留实际factory／base绑定，不把同名manifest当作每份副本均由name loader直接选择 |
| 电源值 | 无hasPowerButton时直接置isPowerOn=true。基类以getEdition是否SUCCESS得到开关值；两份Glitter以外接电源查询得到值，失败也包装成SUCCESS／false，不能当作实读关机。GlitterWireless有电时还查询无线状态并执行`setActiveWirelessConnectionState(2.4G,1)`；这是有写入副作用的读取链 |
| edition与SKU | getEditionWhenPowerOn最多3次读取，后两次前各等2000ms，仅成功才采用edition／layout，缺字段补0。736／737另按主机SKU选择virtual edition129；已核实真实字符下标、四个129候选的两轮404、同PID0消费者及原生注册表来源，详见下节。serial／deviceId辅助读取的包装success未核对底层getter status，身份形成与设备成功应答须分开 |
| edition替换 | ready可新建DeviceInfo并复制电源／driver标志；新对象snapshot仍undefined。它调用`hookRzDeviceMethods [678280,679116)`，未重建实际协议对象；当前loader是arrow函数、该helper却查其prototype，14份实体也无`*_Hook`，不能声称已经换成新协议类 |
| 固件限定覆写 | ready中的overwrite循环只处理PID171、`ignoreSetDeviceModeTimeoutError`且扩展固件恰为1.2.1.0。现有215/{0,128,129}、227/0、234/0、45067/0虽声明同名配置，也不能认作已应用。引擎版本4.0.174／darwin的另一个gate控制HID mutex能力，与硬件effect选择不同 |
| 本地profile | 缺身份时先取deviceId，再按localStoragePId、可选device-specific profile／metadata和activeProfile读本地值；connectedDevices补ports与阻止连接灯效标志。空effect map时转manifest目标或软件spectrum，均不是硬件effect读回 |
| 腕托／侧板 | capability存在才GET HWModule，并仅在SUCCESS更新连接字段。外接电源capability为false时直接置true；为true时使用返回jsonData.status却未检查命令status。614/0与706/0声明腕托且该外接电源capability=false，不能据初值判定附件已连接或已供电 |
| 软件同步 | applyRegionQuickEffect可进入syncHWEffect；非advanced engine下按supportHWEffects选择或换成defaultHWEffect，同时仍保留所选setting对象。即时分支await设备setter，写入结果仍须按真实传输／应答判定；helper `[730954,731819)` catch异常而不再抛出，await完成也不能视作确认。syncExit只换退出目标并重配host包。141个唯一gate分支中的3940／736／737属于软件目标路径，useUser=false，不生成user snapshot；它们在当前物理输入中共有5份副本 |

按真实十名称gate过滤当前235份物理manifest，精确命中 **143份物理输入／141个不同PID／edition**。唯一分支为普通25=107、DualLinkKeyboard=14、DualLinkMouse=8、OLED=4、Matrix=3、BatchSender=2、Linker=1、GlitterWireless=1、Glitter=1；物理统计仅BatchSender增加到4份，其余不变，另92份物理输入在gate之外。唯一分支的manifest version为139份0.0.0.1、2份0.0.0.5；物理计数为141份0.0.0.1、2份0.0.0.5，版本字段不是设备固件。`manifest_projection`分别保存物理与唯一统计，不能把141个分支改记为143个。3871/0现在明确声明hasGlitter=true、maxGlitterPorts=6；没有直接命名Skip的manifest，141个唯一gate分支也未声明hasMultiRegionQuickEffect，不能据通用类或六端口配置推断附件已连接。

两个退出消费者还有**坐标与补值差异**：即使选用user snapshot，它们仍取退出目标的`profileId||1`／`regionId||0`，忽略snapshot.regionId；snapshot本身没有profile。批量构帧对snapshot effect使用`||3`，JS direct未做同样补值，且普通target分支另要求effectId为truthy。它们不能统称为“把原始读回按原坐标完整写回”。153份JS的有界字面扫描只在main和PID0默认chunk找到退出字段，没有名为getQuickEffect的实际调用；真实getter仍称getChromaEffectCommon，动态字段或其他未取得部署不在这一扫描结论内。

对应独立期望须覆盖无电源能力的软件true、getter失败包装、无线getter写入与恢复、edition／SKU替换后旧协议和新DeviceInfo、固件精确比较、附件命令status／jsonData差异、空本地effect map、软件同步与真实snapshot、目标坐标与观测region不一致、effectId=0及失败重读保留旧snapshot。无线“getter”不得直接登记为只读测试；执行前须核实原无线状态、可恢复写入及清理链。以上静态结果没有替代Rust实现、真实应答或实际DLL／设备对照。

### Glitter／Matrix 的当前产品选择与引擎备用 manifest

[协议选择专项结果](../../../.analysis/results/frontend/synapse-lighting-engine-current/protocol-selection-audit.json)独立保留134个完整AST owner、27份原件及获取收据。当前设备目录精确包含下列四个productId；实际MW入口先调用`useFeature("rzDeviceType",名称,参数)`，连接owner再从**同一模块导出的injector单例**读取`getFeatures("rzDeviceType")`，沿实际loader选择default导出并调用构造函数，之后才执行`connectDevice()`。专项保留useFeature删除原有默认flag、写入新flag、getFeatures、连接重试及异常返回的完整owner，没有执行厂商JS。

| 当前目录productId／位置 | 当前MW名称 | 实际MW选择和导出 | 引擎edition0配置 |
| --- | --- | --- | --- |
| 3871／203 | Razer Chroma Addressable RGB Controller | `main.5015581d246fd93c58c4.js` `[162963,163013)`选Glitter；连接模块87887的K表加载chunk9045／module66829，经getter与别名得到普通Glitter类 | `rzDevice25Glitter`，claimInterface=0；显式hasPowerButton=true、hasGlitter=true、maxGlitterPorts=6 |
| 515／33 | Razer BlackWidow Chroma | `main.8342a5d11f2dd0d29680.js` `[138713,138795)`选Matrix，参数regionIdEnum=BacklightLED；连接模块42057加载chunk6474／module62634，使用`t.default=类`的直接赋值 | `rzDevice25LedMatrix`，claimInterface=2；hasPowerButton=false |
| 521／40 | Razer BlackWidow TE Chroma | `main.ebd2a2feb18d105d65f2.js` `[138971,139059)`选Matrix并传同一region参数；实际loader同为6474／62634，但逐份核对本产品字节与直接default赋值 | `rzDevice25LedMatrix`，claimInterface=2；hasPowerButton=false |
| 545／34 | Razer BlackWidow Chroma V2 | `main.7502fdd350e035b6c3de.js` `[151934,152016)`选Matrix并传同一region参数；连接模块34938加载chunk5634／module71513，经getter与别名取得类 | `rzDevice25LedMatrix`，claimInterface=2；hasPowerButton=false |

目录位置为原JSON数组的零起始索引。四个产品的实际DeviceInfo导出分别来自module26061／21503，`productId`与`claimInterface`数值也已核对，未凭设备显示名生成身份。3871的Wireless chunk另含同ID的Glitter依赖副本；本次按真正K表和chunk9045的文件名表选择普通Glitter chunk，不能因module号或`this.name`相同就合并。MW代码与引擎代码分别保留自己的完整class、constructor、base import及default导出链，名称一致不表示两个部署的实现字节或行为天然相同。

[本轮获取收据](../../../.analysis/inputs/frontend/lighting-protocol-selection-current/acquisition.json)记录20次GET及16次HEAD：对四个PID各核两轮MW与引擎manifest地址，另以两轮GET核引擎HTML及其实际main身份。`products/<pid>/mw/lighting-manifests/DeviceManifest_<pid>_0.json`四个地址均返回404；`lighting-engine/manifest/DeviceManifest_<pid>_0.json`四个地址均返回200，无重定向。六份成功实体前后字节一致；404的错误响应正文分别保留，不把响应正文的变化隐藏为同字节。以上均是官方静态HTTP观察，没有执行原版HEAD／fetch helper。

这四个备用件的内部PID分别保持3871、515、521、545，version均为0.0.0.1，实际rzDeviceConfig名称逐项与所追MW类名一致；每份配置都显式给出IN／OUT重试30／20及三个5ms间隔。引擎`getManifestData`先尝试MW，再尝试自己的路径，并仍可能由较新IndexedDB或后续Cache API结果决定最终内容。观察到MW404、引擎200，证明此备用内容当前可取得；本机运行时究竟使用哪份缓存、具体硬件edition及连接应答仍未验证。引擎`_createRzDevice`使用实际USB／BLE对象和该配置的名称，经main的loader与`init`真正构造其引擎副本，不能把MW连接的成功或类名直接当作引擎设备已打开。

四份退出目标均为`{effectId:3,params:{}}`，未声明useUserQuickEffectOnExit／syncHWEffectWithSWEffectWhenExit／isHardwareEffectsOnly。当前configure按各自原规则将这些缺省flag设为false，故这四份本身**不请求userQuickEffectOnExit硬件snapshot**；退出目标不能描述为原有硬件设置。3871的供电getter虽真实查询外接电源，失败仍包装SUCCESS／false；三键盘没有hasPowerButton时的软件true也不是供电读回。3871的六端口上限是manifest配置；Glitter构造仍将deviceMaxPortNumber置undefined，灯帧首次以eventData.numRow填入，末行头0x84按这个帧行数判定，不能擅自固定为第六行或把六端口解释为六组已检测LED。

Skip的实际module269仍由引擎名称表加载，其constructor调用Matrix基类后覆盖为`rzDevice25LedMatrixSkipSetEffect`、设置skipSetCustomEffect=true。Wireless的基类为Glitter；BatchSender的基类为Matrix。这些是真实继承消费者，最终runtime名称仍不同，不能填成普通Glitter／Matrix／Skip的直接产品manifest。当前235份manifest未给出Skip的具体产品／edition，其他未取得manifest、动态别名及扩展分支也未被排除。专项只补清上表四项；端口观察、配置变化、真实应答、恢复、取消及释放仍需逐功能的独立Rust与官方实际DLL对照，不因本次134个静态owner通过而升级为功能完成。

### 736／737 的 SKU 129、manifest 可用性与原生来源

[SKU完整静态结果](../../../.analysis/results/frontend/synapse-lighting-engine-current/sku-129-audit.json)保留19份原件、40个完整AST所有者及11个原生完整函数；[专项获取收据](../../../.analysis/inputs/frontend/lighting-sku-736-737-current/acquisition.json)记录14个明确URL的两轮GET，共28次请求，其中8份实体字节稳定、6个候选两轮404且无重定向。另对下面8个manifest URL独立执行两轮HEAD，共16次，结果与GET一致。这是限定候选的当前可用性观察，没有执行厂商JS或其HTTP helper，也没有重取整个依赖闭包。

| 候选地址，`<pid>`分别为736、737 | 两轮GET与两轮HEAD |
| --- | --- |
| `products/<pid>/mw/lighting-manifests/DeviceManifest_<pid>_129.json` | 两个地址均404，未取得129内容 |
| `lighting-engine/manifest/DeviceManifest_<pid>_129.json` | 两个地址均404，未取得129内容 |
| `products/<pid>/mw/lighting-manifests/DeviceManifest_<pid>_0.json` | 两个地址均200，完整字节及字段与原保留件相同 |
| `lighting-engine/manifest/DeviceManifest_<pid>_0.json` | 两个地址均404 |

表内地址均相对于`https://apps.razer.com/synapse/`。目录将736、737各自作为primary；实际6191.IS选择器不改写这两个PID。两个0 manifest均开启`hasSKUVirtualEditionId`、映射`[{M:129}]`，使用BatchSender、`useUserQuickEffectOnExit=false`、`syncHWEffectWithSWEffectWhenExit=true`及退出effect3。MW元数据中的736_129／737_129 Mercury译名仅证明名称声明，不能补出缺失manifest的几何、颜色或运行时选中结果。

`getEditionIdFromSKU [1322240,1322952)`先检查映射表，再读取实际2010.A适配器的`systemSKU`。空映射、falsy值或`length<=12`返回0；长度大于12才用 **`charAt(11)`，即第12个UTF-16 code unit**，与每项第一个键严格比较。没有trim、大小写转换或Unicode规范化；当前`M`得到129，未匹配或匹配值undefined得到0。适配器`[273649,273778)`在Electron发`{action:"systemSKU"}`，RzAppEngine读`window.systemSKU`，其他分支返回字符串`"false"`。ready仍先要求电源路径取得非null的edition／layout；`E.editionId!==a && a`才为SKU变化新建DeviceInfo，0不触发这一重建。新对象复制电源／driver标志并调用hook，不重建实际协议类，也不保留旧对象snapshot为新观测。

`fetchManifestFromURL [1333740,1334329)`先HEAD，timeout=5000ms、retry=2；HEAD成功后执行GET，失败时仅405仍放行GET，其他HEAD失败由外层catch返回undefined，GET自身retry=2。上表404明确记录了实际候选当次不可用，已超出“保留范围没有该文件”的结论；独立GET观察没有执行这个HEAD门槛。`getManifestData`仍可从IndexedDB或Cache API取得内容，缓存未读，不能据HTTP404断言某台运行中的主机一定没有129数据。

在Razer的`configure`分支，首次返回falsy且请求edition非0时才再次调用同PID的`getManifestData(...editionId:0)`；**第二次正常resolve的continuation执行`e.editionId=0`并返回其结果**，不保留129。第二次返回undefined也先赋0，随后configure返回false；Promise拒绝是另一条路径。可用结果`C`提供productName、version、ledConfig、svg、rzDeviceConfig、退出效果及SKU标志等字段，经完整readiness检查才返回isDataReady；这里没有采用`C.editionId`恢复129，也没有改变发现的productId。IndexedDB getter实际按实例`productId_editionId`取值，并可从`fallbackEditionId`修改实例edition；writer在请求edition与提供edition不同时记录该字段，所以名为736_129／737_129的缓存记录也可能装着0内容。请求0、缓存键含129和configure成功须分别记录，均不能单独证明选中了129 manifest或读到了设备edition129。

主机链已追到`electron/main.js → FFISysUtils.callDLL → ffiMain.callDLLMain → SysUtilsNative.systemSKU`。当前DLL的Image Base为`0x180000000`，`systemSKU [RVA0x70540,0x706d3)`无参数并返回由调用方释放的窄字符指针。它实际读 **HKLM `HARDWARE\DESCRIPTION\System\BIOS` 的`SystemSKU`**：RegOpenKeyW、RegQueryValueExW探长度、再次读值；不是WMI或设备固件查询。先分配并清零128字节，普通注册表错误留下空串，初始分配失败返回null；转换使用WideCharToMultiByte代码页3（CP_THREAD_ACP），随后lstrcpyA复制。原件没有封顶第二次查询的cbData、校验返回类型或限制窄串复制长度；打开失败后仍到RegCloseKey且句柄未见预初始化，转换错误另可抛HRESULT。独立Rust须保留真实错误与编码边界，不能照搬这些内存不安全行为或把空串当设备成功。

实际同步FFI消费者`ffiMain.js [231,2766)`把有`readCString`方法的结果先转换字符串，**转换完成后**才置释放标志，finally调用同库`FreeMalloc`；该导出`0x6b9d0 → 0x97ce0 → 0xa98e0`最终HeapFree。空字符串转换成功也会释放；readCString抛错时标志仍false，不保证释放。FreeMalloc抛错可被外围catch记录而仍返回已读字符串，没有独立清理状态。缺handler／导出可返回undefined。没有传编码参数的readCString委托实际ref-napi原生addon，当前未把它的解码语义当作已核实UTF-8；原生转换临时buffer、注册表句柄及返回指针各有不同清理所有者。

后续测试须覆盖长度12／13、`M`分别位于下标11／12、大小写、缺映射／缺SKU、changed但结果0、HEAD405／404、两级HTTP失败与缓存版本选择、129缓存保存0内容、0重试返回undefined／拒绝、readCString／FreeMalloc异常及清理。纯逻辑和模拟HTTP／存储可在无设备时先完成；实际主机SKU、原生编码、运行时缓存、设备edition、Rust对应结果及官方DLL差分仍未执行／未验证，不能用上述静态一致代替。

### 实际设备目录、primaryPid 顺序与页面资源

已从代码中的实际地址获取 [AvailableDevices.json](../../../.analysis/inputs/frontend/synapse-device-catalog-current/raw/apps.razer.com/synapse/dashboard/AvailableDevices.json)：HTTP200、无重定向、11041字节，首取2026-10-22T11:30:11Z，第二轮截至11:35:30Z字节稳定。完整来源、请求头／响应头、30次HTTP记录、15个URL的前后身份及旧Service Worker差异保留于[获取收据](../../../.analysis/inputs/frontend/synapse-device-catalog-current/acquisition.json)；实际映射见[静态目录分析](../../../.analysis/results/frontend/synapse-device-catalog-current/catalog-analysis.json)。

表有335行、334个不同productId；691在0-based第63／331行出现两次。数据含productId、标量／数组dongleId、bleId、isNotChromaDevice、xBoxId、psModeIds、monitorPortIds和wiredId，不能为了统一schema把数组展开或删除重复行。灯光引擎真实helper `f [1072572,1072716)` **按原行序逐项**判断：productId严格相等即返回输入本身，否则仅标量dongleId／bleId严格相等才返回该行productId；未命中保持输入。它不读取其余alias字段，也不展开数组。

| 有序映射分支 | 实际结果 |
| --- | --- |
| 标量输入域 | 459个product／dongle／BLE数值输入，其中125个映射为其他primary；完整结果逐项保留，不能只用命中数代替映射 |
| 复用dongle2593 | 先命中第29行primary2595，后面的2594不胜出 |
| 复用dongle2308 | 先命中第46行primary574，后面的117不胜出 |
| dongleId数组 | 241仍由productId自身命中；2676的数组内2680／2681不会自动归2676 |
| 其他alias字段 | 全部481个数值ID中，22个只存在于其余alias域的ID不被该helper改写；不能由Xbox／PS／显示器／wired能力补映射 |
| 已保留manifest | 原86个PID均落在表的primary或标量alias域；新增3871／515／521／545均有精确productId条目，当前共90个PID。原47文件／22PID是alias的范围未增加；1378出现在1406／1407获取目录不能反推1378归该primary，helper仍返回1378 |

真实producer是 main `init [794461,796030)` → `getAvailableDevices [796031,796548)` → singleton别名 `de=ce [959148,959153)` → DeviceInfo.configure默认第二参 `[1307968,1308049)` → 上述helper。configure对 `RAZER_BLE_VENDOR_IDS=[5426,1678]` 的vendor分支计算mwPrimaryPid，**不是仅按BLE传输类型**；USB vendor5426也在其中。其结果既用于manifest路径，也用于 `loadSvgContent [1321469,1322239)` → SVG fetch `R [1305716,1306612)` 的 `/synapse/products/<primaryPid>/mw/lighting-svgs/`。因此映射差异会同时影响设备协议配置和页面图片／几何资源，不能只验证功能getter。

getter先请求表，失败／空值再读Cache API `DeviceDataJson`，有内容才更新内存availableDevicesList。新Dashboard Service Worker的完整配置 `[67074,67573)` 使用同名缓存，默认可先返回缓存并后台更新；只有显式 `X-SW-Strategy: network-first` 才进入另一策略，当前getter没有设置该头。线上表两次一致不能证明客户端缓存／内存也相同；一次读取失败也不能当成空设备目录。原件还保留每轮HTTP和客户端缓存边界，未执行厂商JS或真实窗口。

后续独立测试须固定原行序，覆盖重复productId、两个复用dongle、数组、未知／错误类型PID、vendor5426／1678及其他vendor、网络表／CacheAPI／内存差异、manifest与SVG同时选路、文件身份冲突和失败取消。本项目运行仍使用本地目录库；这张官方在线表用于获取与逆向证据，不能变为产品启动联网或静默回退路径。

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

### Added／Removed 的真实目标、身份来源与页面消费者

Watcher 的回调不是依据反编译函数名猜测：`0x159be0` 实际把 Impl 与 `0x2329 → 0x14fe20` 绑定给 Added，把 Impl 与 `0xa0d3 → 0x150f10` 绑定给 Removed。后者被分析器误命名为 `CMFCRibbonPanel::Reposition`，它的真实调用方、参数与函数体均属于设备移除。delegate 构造 `0x147860/0x147910` 写入的原件虚表 `0x305190/0x3051f0`，在 Invoke 槽 `+0x18` 分别指向 `0x4d2c → 0x1544c0`、`0x3de1 → 0x154500`；完整指令将 `this+0x10` 的绑定记录交给 `0x13b840/0x13b8c0`，后者 AddRef 两个 COM 参数，再调用保留的 Impl callback。这两个 Invoke wrapper 的原始 Hex-Rays 失败仍保留，语义依据是完整汇编与下游伪代码，不能登记成全库反编译无失败。

Added 的 `0x14fe20` 依次读取 DeviceInformation 的 Id、Name、IsEnabled、Properties。发现时的 VID／PID 来自 `System.Devices.DeviceInstanceId`：`0x155c30` 找 `VID_`／`PID_`，各截后续最多四字符，以 base 16 的 `stoul` 转换；缺属性或标记时为零。ContainerId 由 `0x1557a0` 单独读取 `System.Devices.ContainerId`，格式化带花括号的 GUID 后逐字节大写；缺失／类型不符不会成为已确认的 container。这些是**发现阶段的属性来源**，与 open 后向 LampArray 读取 VID／PID 分开。

随后在 mutex `+240` 内按完整 ID 插入或替换记录，设置这些基础字段，清除 optional 元数据；同 ID 若已有 LampArray COM 引用也会释放并置空。解锁后才调用保留的 Added consumer。重复 Added 因而可能重置已打开记录，不能实现成无条件忽略重复 ID，或沿用旧 optional 状态。

Removed 的 `0x150f10` 经 `0x153b50` 读取 DeviceInformationUpdate.Id，在同一 mutex 内删除精确 ID 的记录。记录析构 `0x14cb20` 释放 LampArray COM、optional 灯元数据和字符串；解锁后标记 changed，并调用 Removed consumer，**即使没有找到记录也会调用**。这条路径才删除发现记录，不能和仅释放 COM 的 `device_close` 合并。

API Startup `0x88410` 把两种 consumer 绑定到 `0x10023/0x10a0a → 0x874c0/0x87eb0`，序列化为 `DeviceAdded{device}`／`DeviceRemoved{device_id}`，进入既有反向 RPC 队列。已经排队、callback 已返回和 JS 的异步消费完成是不同相位。

前端也有两套各自保留的消费者，不能交叉拼接结果：

| 完整 owner | Added／Removed 的行为 |
| --- | --- |
| `RzWDL` `[278509,287000)` | PID=0 且有 container 时按大小写无关的 container 查本机 USB；无匹配时将 VID/PID 设为 `61439/61439` 的兼容身份，不能称硬件实测。新 ID 建 adapter 并发 new；已存在 ID 发 update，但**不把新 event 替换进保留记录**。Removed 仅找到保留 ID 时移除并发 disconnect |
| `LampArrayEventHandler` `[1138049,1147614)` | 按 device ID 的 mutex 串行处理；useWDL=false 时跳过 Razer vendor 设备。新增先移除同 container 的普通 Chroma 项，再建立 LampArray adapter、prepareDevice、发布集合并保存 WindowStorage。移除先删除同 container 的 LampArray Chroma 项，再按参数清保留集合／灯信息，删除文件 cache 并保存 WindowStorage；找不到 adapter 则提前返回。finally 释放 mutex，catch 只记日志 |

这两段完整 owner 没有连接代次校验，response/emitter 的异步 handler 也没有统一等待全部 Promise 的完成屏障。测试应覆盖重复 Added 重置 native open、旧 JS 记录不替换、Removed 先于排队 Added 被消费、同 ID 重新加入、USB 兼容身份、useWDL=false、prepare／移除／存储失败、Stop 后迟到回调，以及注销后已排队工作的清理。独立 Rust 要有自身的连接归属与完成结果，但不能把源码没有的确认或自动回滚写成官方行为。

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

设置挂载根 `Br` `[256127,256325)` 建立 Provider，经 `Vr → kr → mo` 进入实际设置视图。`mo` `[215572,216046)` 在 general/default 分支挂载 `ho`；`ho` `[215186,215412)` 把动态照明放在右列 `Un、Ta` 之后。`$=Q` 是 `body-widgets flex` 容器，`te=ee` 产生 `widget-col col-left/col-right`，没有额外的异步设备读取。`kr` 注册 beforeunload 和 languageChange；卸载只移除前者，不能声称该 owner 也注销了 languageChange。

`ia` 经直接别名 **`qs=Js`** `[82820,82825)` 挂载双向模式控件；这里没有 memo 包装。完整 `Js` 在同一 chunk `[81362,82814)`。两个标签都调用同一 toggle，点击已选标签也反转模式；内层回调先 stopPropagation，避免冒泡重复切换。`wdl-devices.isSwitching` 给外层添加 disabled 并挂 spinner，它是后台切换状态，不能用当前选项或本地保存状态推导。

该 widget 的模式、OS 可见性和 switching 初始值均为 false。effect 内先 await `getOsVersion()`，再读 `isDynamicLighting`；没有预读 `wdl-devices`，只注册后续 windowstorageevent。localStorage 包装器 `2478` 本身已解析 JSON，widget 又执行一次 `JSON.parse(t)`；只有 truthy 值才更新。原生 storage 监听同样忽略空 newValue，因此删除该键不会将当前控件清回 false。windowstorageevent 则直接访问 `newValue.isSwitching`，不做字符串解析或空值保护。完整 owner 没有为 OS／解析异常增加错误页、恢复默认值或回滚；这些事实描述原版 UI，不把 false 解释为设备已确认状态。

当前 CSS `720.dbc9cca5.chunk.css` 的字符区间 `[224300,224649)` 保留边框 `#5d5d5d`、背景 `#111`、外层高 36px／圆角 18px／padding 5px、标签高 26px／圆角 13px／padding `5px 10px`；按下背景为 `#0000001a`。`[371398,371515)` 的两向容器 gap 为 14px，spinner 使用 auto background-size。`[368793,368933)` 的 modes-area 默认隐藏、active 才显示，顶部 padding 20px；`[259842,259935)` 的说明行顶部 margin 10px、内容顶部对齐。这里的字符区间不能当作 JS 的 UTF-8 字节区间，也不能只由数值接近认定 Rust 的颜色、字体、焦点及缩放一致。

### 页面资源、交互状态与实际词条

所用四份 SVG 均从当前 settings 获取收据及输入索引逐字节复核，并在查询的 `settings_media` 保存原文与身份：警告图 `exclamation_icon.53d6d40b.svg` 为 14×14，圆底 `#5c5c5c`、图形 `#f4f4f4`；系统入口图 `wdl_large_icon.e610fd8c.svg` 为 44×44.001、填充 `#737373`。警告图加 `mr10` 与 margin-top 3px，入口图加 `mr10`、文案再加 `ml10`。spinner 原件 `spinner.ef2d0235.svg` 声明 26px，但 CSS 实际呈现 20×20；帮助图使用 `tooltip_questionmark.96138d2f.svg`，不能用警告图替代。

| 状态／层级 | 当前完整 JS／CSS 的行为 |
| --- | --- |
| 卡片与标题 | `.body-widgets .widget` `[71113,71342)` 的背景为 `#111`、圆角 5px、min/max-width 600px、padding `30px 40px`；标题沿用 RazerF5、绿色 `#44d62c`、uppercase，settings 的 `[260101,260145)` 再覆盖为 18px。widget-container 是 flex 容器。实际父级还有 `[259935,259990)` 的 display:block 覆盖，不能只照通用两列名称布局 |
| 选中与悬停 | 普通标签文字 `#ccc`，选中底色 `#44d62c`、文字 `#111`；dual 标签未选中 hover 保持 `#111`，选中 hover 保持绿色。容器 hover 边框变绿，按下时未选中底色 `#ffffff1a`，选中仍绿色；dual 标签按下 opacity 为 1。完整覆盖顺序在 `[367963,369638)` |
| 切换中 | 外层 `wdl-setting disabled` 通过全局 `.disabled` `[150368,150409)` 设置 opacity .3、pointer-events:none；spinner 是该外层的兄弟元素。它没有传入原生 disabled 属性，也没有在 click handler 内再次判断 switching |
| 系统入口 | `.img-text .external` `[10544,10676)` 为 14px、行高 44px、下划线、capitalize；实际动作只打开本机系统设置。入口是带 onClick 的 div，没有本 owner 定义的 tabIndex、role 或 Enter／Space handler |
| 帮助与提示 | `ue` `[11395,15497)` 的帮助 div 有 role=button 和已翻译 aria-label，以 mouseover/mouseleave 控制显隐，没有 tabIndex／键盘 handler。提示 portal 挂到 document.body；黑底、边框 `#5d5d5d`、14px／18px、max-width 300px、padding `8px 10px`、z-index 10001，规则 `[77650,78156)` |
| 提示定位与离开 | 初次 hover 后固定定位到 widget 右边内缩 14px、top+34px；按 body-wrapper 左右边界夹取。底部溢出时尝试帮助图右侧 8px，再改左侧 8px，仍溢出则上移溢出量+10px。mouseleave 直接移除 portal；完整 owner 没有 scroll／resize 重算监听或单独的异步取消任务 |
| 键盘／焦点 | 双标签与外层均为 div；`Js` 没有 tabIndex、ARIA tabs、keydown、焦点转移或恢复逻辑。当前页面没有为这一切换挂载独立确认弹窗。不能将其他组件的键盘行为写成此 owner 已有行为；真实窗口焦点结果仍未验收 |

显示词条从 `4693` 的九个 key，经 `264` 的 Translator、`1778.getTextItem`、`7248` 的实际语言 importer，追到 **10 份**语言模块：en／de／es／fr／ja／kr／pt-BR／ru／zh-CN／zh-TW。`settings_locale_exports` 为每个语言保存九项实际导出、词值完整 AST owner 与区间，未执行厂商 JS。下表保留中文与英文原词，大小写再受上述 CSS 影响。

| key | zh-CN | en |
| --- | --- | --- |
| DEVICE_LIGHTING | 设备灯光 | DEVICE LIGHTING |
| DEVICE_LIGHTING_TIPS | 设备的灯光可由 Chroma 幻彩 RGB 或 Windows 动态照明功能进行控制。使用切换开关在两种灯光控制模式之间切换。 | Your device’s lighting can be controlled by either Chroma RGB or Windows Dynamic Lighting. Use the toggle to switch between the lighting controls. |
| CHROMA_RGB | CHROMA 幻彩 RGB | Chroma RGB |
| DYNAMIC_LIGHTING | 动态照明 | Dynamic Lighting |
| CHROMA_RGB_TITLE | 通过 Synapse 雷云允许 Chroma 幻彩 RGB 功能控制你的设备。 | Allows your device to be controlled by Chroma RGB via Synapse. |
| DYNAMIC_LIGHTING_TITLE | 让你可以通过操作系统使用 Windows 动态照明功能。 | Allows you to use Windows Dynamic Lighting via your Operating System. |
| CHROMA_RGB_MSG | 为避免冲突，我们建议使用下面的链接在你的操作系统中禁用 Windows 动态照明。 | To avoid conflict, we recommend disabling Windows Dynamic Lighting on your Operating System using the link below. |
| DYNAMIC_LIGHTING_MSG | 你还必须使用下面的链接在 Windows 个性化设置中启用此功能。 | You must also enable this feature in Windows Personalization settings using the link below. |
| OPEN_WINDOWS_DYNAMIC_LIGHTING | Windows 动态照明设置 | Windows Dynamic Lighting Settings |

英文值集中在 `trans-en.f424ab82.chunk.js` `[271130,271743)`，中文值在 `trans-zh-CN.76d1e5ef.chunk.js` `[360818,361855)`；均为 UTF-8 字节区间。`1778` 初始化同时加载英语与当前语言，语言状态先为 unknown；字典未建时 `getTextItem` 返回空串，当前词缺失才按其原规则取英语或 key。Translator 订阅语言 reducer；这里没有动态照明专用插值、HTML、网络业务或设备 getter。其他八个语言不能由中文或英文推定，逐项以当前保留的导出为准。

| 边界 | 当前生产所有者 | 核对结果 |
| --- | --- | --- |
| 显示门槛 | [Windows system.rs](../../../crates/razer-platform/src/platform/windows/system.rs) 的 `supports_dynamic_lighting`，由两个设置入口调用 | 读取 CurrentBuildNumber／UBR，采用 build ≥22631 或 22621 且 revision ≥2506，数值条件对应所读设置控件；注册表读取失败返回 false。它只决定页面显示，不证明原生 LampArray 能力已初始化 |
| 普通设置布局与编辑 | [settings_lighting.rs](../../../crates/razer-settings/src/preferences/settings_lighting.rs) → [settings_page.rs](../../../crates/razer-settings/src/preferences/settings_page.rs) | 已有两个模式、说明／警告图、系统设置链接和 switching spinner；两个按钮都反转模式，保留已选标签也切换的行为。renderer 接受 Option，但实际 caller 固定传 Some；页面从 `saved.dynamic_lighting_draft.unwrap_or(false)` 起步，没有真实控制权初值和切换观察消费者，不能把这个本地选择显示为已观测模式 |
| 普通设置事件 | `SettingsEvent::DynamicLightingRequested` → [Shell 分发](../../../crates/razer-shell/src/shell.rs) | 只更新本地选择并显示尚未实现的状态说明，没有设备控制权转移、LampArray 枚举／开关、`wdl-devices` switching 发布及确认；`observe_dynamic_lighting` 存在但当前没有生产调用者 |
| Chroma 设置入口 | [chroma_settings.rs](../../../crates/razer-app-pages/src/chroma/chroma_settings.rs) → [状态 owner](../../../crates/razer-state/src/chroma_settings.rs) → [本地存储](../../../crates/razer-storage/src/chroma_settings.rs) | Change::DynamicLighting 进入串行本地文件保存；有双模式、文案／图像和系统链接，但尚无本页后台控制权消费者。本地文件保存与本机控制权已转移须分别记录 |
| 产品灯效页的 WDL 观察 | [product_chroma.rs](../../../crates/razer-shell/src/shell/devices/product_chroma.rs)、[controller_chroma.rs](../../../crates/razer-shell/src/shell/gamepad/controller_chroma.rs) → [chroma_profiles.rs](../../../crates/razer-storage/src/chroma_profiles.rs) | 已有真实本机存储读取与缺失／错误区分，产品页可消费已读的 `isDynamicLighting`；手柄 watcher 带身份／guard 并后台重查。这是存储观察，不能当作 LampArray 原生 watcher 或两个设置入口的写入确认 |

上述两入口的同类界面都需要沿各自当前官方 root 验收；不能因为共用本机系统设置链接就跳过一方的状态、资源、关闭和失败语义。当前 OS 链接是真实 `ms-settings:personalization-lighting`，但打开系统设置不等于已完成本程序的控制权切换。

## 验证范围

统一查询入口为`python -X utf8 tools/workspace.py audit-host-local --query dynamic-lighting`，覆盖完整source owner、所用CSS／SVG、10份语言模块、14份恢复实体、目录／七产品重连／原生函数／数据区和稀疏包。[查询结果](../../../.analysis/results/host-current/local-audit.json)已按最新输入索引严格重跑：动态照明保留722个完整source owner，宏查询保留3778个；两者均failures=[]、snapshot_stable=true、changed=[]。当前manifest范围为229份主输入、2份同字节观察及4份引擎备用件，共235份物理输入／215个唯一PID-edition、68个manifest获取目录；值源结果另含引擎收据，共69份。原144个DeviceInfo语义owner未改，两份副本和四份新分支的完整字节／字段及当前235份范围均通过维护工具原有精确检验函数，未放宽集合、SHA或投影比较。manifest文件名／内部PID的真实差异独立保留。SKU专项另核40个AST所有者和11个原生函数，协议选择专项另核134个完整AST owner及27份原件；专项与统一查询有重叠，均独立记录，不能相加作为总覆盖。`python -X utf8 -m unittest discover -s tools -p test_host.py` 的39项维护工具测试通过；IoT帧长度／独立XOR及parser静态向量仍按静态期望记录，不是实际DLL输出。以上不将未决原生间接目标、所有产品协议或实际DLL对照升级为已闭合。

收据分别保留完整汇编与伪代码状态。WDL当前选定函数有五处原始Hex-Rays失败：`0x1544c0/0x154500`是上述Added／Removed delegate；`0x156f30/0x157530`各将RDX指向的16字节闭包描述复制到栈、将RCX解引用为Impl，再经`0x4467/0x31c5`尾跳`0x156a90/0x156f60`；`0x158c60`将RDX／R8／R9三个16字节描述复制到栈，再经`0x1f50`尾跳`0x1585d0`。后三者均按完整汇编核实，返回寄存器沿原调用直接传回，没有额外确认。五处仍列入decompiler_failures，不能写成伪代码已恢复。HID的`0x30888`基础及早期566补捕获失败保持原样；当前查询另引用独立c98d成功补捕获和上述编码语义结果，不能据此改写旧失败。各捕获结果与产品语义完成度分别记录。

后续纯／模拟验证须覆盖 OS 边界、已选项点击、切换禁用、缺失／错误存储、两个窗口并发、设备加入／移除顺序、readiness、失败后 finally 清除 switching、旧连接拒绝及完整注销释放；独立实际对照继续比较官方 DLL 的本机调用结果和 Rust 的控制权／设备集合／事件顺序。像素、图标、文案、键盘／焦点和真实窗口须另行验收。本次未运行相关 Rust 行为测试、应用、官方 DLL 或设备，遵守[验证规则](../../../.agents/rules/verification.md)。

## RzAPIChromaSDK.dll：共享事件、注册状态与元数据边界（静态捕获）

本批同时核对 RzAPIChromaSDK.dll 的既有 IDA 捕获、导出、调用、字符串和导入 xref。分析使用四线程静态汇总；没有加载 DLL、打开 Chroma 事件、访问设备或运行服务。

| 原件（SHA-256） | 架构／大小 | 捕获收据 | 导出／函数／失败 |
| --- | --- | --- | --- |
| RzAPIChromaSDK.dll 78a7454edc13006e5c80d5d05d76c8262b1473cb4da6066e6422b1e32d48cef0 | x86，228,552 B，ImageBase 0x10000000 | [capture.json](../../../.analysis/results/native/78a7454edc13006e5c80d5d05d76c8262b1473cb4da6066e6422b1e32d48cef0/capture.json)，SHA-256 0e269a459a87e2d0af6c8e8008adfb9deba9829d27decde84c9aad32eb821432 | 29／544／3 |

29 项导出覆盖 Init (0x22820)、UnInit (0x22840)、应用目录、属性、设备支持、Invoke、Smart Lighting/Philips Hue、EnableDeviceLighting 以及事件通知。三个 Hex-Rays 失败位于 RVA 0x12c0、0x12d0、0x12e0；对应原汇编仍保留，不能写成伪代码已闭合。

### 已由静态调用和 xref 确认的边界

- 这是 Chroma SDK 的进程间状态/事件包装层。导入及 xref 包含 OpenFileMappingW/CreateFileMappingW、OpenEventW/CreateEventW、PulseEvent、WriteFile、CloseHandle、LocalFree 和任务调度运行库；没有在该 DLL 内找到直接 HID 传输或设备读回。共享内存/事件的名字、所有者和对端协议仍需继续从服务端与消费者闭合。
- Init 是到 sub_10001530 的 thunk，UnInit 到 UnInit_0。字符串与注册表访问指向 Software\Razer Chroma SDK\Apps、...\Devices\SmartLighting 和 ...\Devices\IoT；应用列表与元数据由这些本机状态驱动，不能把注册表存在当成设备已连接。
- IsDeviceSupported (0x22c20) 对 16 字节 GUID-like 值逐 32 位比较，身份不符返回 0。RegisterEventNotification (0x22c50) 身份不符返回 5，空回调返回 87，已有回调返回 1242，成功保存回调并返回 0；UnregisterEventNotification (0x22d50) 身份不符返回 5、未注册返回 85、清除回调成功返回 0。
- SetSynapseStatus (0x22f50) 先做同一身份检查，保存状态后若共享事件句柄存在则 SetEvent；句柄为空只记录 g_hEventSynapseStatusChanged NULL，仍静态返回 0。这个返回值不等同于消费者收到事件或设备状态已确认。
- GetSDKVersion (0x22da0) 读取 HKLM\Software\Razer Chroma SDK 的 MajorVersion、MinorVersion、RevisionNumber，格式化成 major.minor.revision，通过 CoTaskMemAlloc 写出并关闭注册表句柄；注册表失败沿 LSTATUS 返回。GetAttribute (0x22be0) 在身份不符时返回 5，仅对属性 69 静态返回 120，其余返回 50。

### Rust 接线和验证边界

本 DLL 的静态结果只定义了 SDK 进程间注册/事件/元数据边界，不能替代设备灯效协议、服务端共享区或 Rust 的真实写入。实现时必须分别建模回调注册代次、重复注册/未注册、身份不符、共享事件为空、UnInit 释放、注册表缺失和跨进程句柄关闭；纯模拟可先覆盖这些状态，但实际官方 DLL 与 Rust 的事件顺序、共享内存内容、设备结果和释放仍保持 **未验证**。

### 三个事件线程、共享环与停止结果

同一[灯光生命周期收据](../../../.analysis/results/native-bulk/chroma-lifecycle-verification.json)的 `api_event_threads` 继续核对了本 DLL 的 10 个完整函数、2,125 条指令和 6,785 字节。16 个静态核验 worker 逐项比对原件、全部 chunk、指令字节和捕获身份，未列出指令的块字节为 0。`0x12c0/0x12d0/0x12e0` 的原始 Hex-Rays 失败仍保留；它们均为六指令线程入口：从唯一栈参数取对象到 ECX，分别调用 `0x62a0/0x6720/0x6ae0`，保留 EAX 后 `ret 4`。这组汇编足以追踪实际线程目标，不等于三个伪代码已恢复，也不新增猜测的导出签名。

`0x62a0` 等待对象 `+44` 的状态事件及 `+24` 的停止事件。状态事件到来时，按 `+64` 的状态及 `+52` 的角色，创建／打开或释放 `+20/+68` 的命名 mutex；随后对 `Global\\{89811F96-91C2-4C19-8E0A-54469F491550}` 创建事件、调用 `PulseEvent` 并关闭本次句柄。状态传播发生在进程间事件层，没有在此读取设备应答，`PulseEvent` 返回也没有形成确认契约。停止索引为 1 时返回 0；其他等待结果走实际日志错误链，仍需与初始化、消费者和停止结果分别判断。

`0x6720` 创建 `Global\\{190E28CB-BB73-46B1-8FFD-62FF0EE273A0}` 计数事件，从 `+56` 所持映射读取应用数量；初值大于 0 且 `+60` 有回调时直接调用 kind `1`。随后等待计数／停止事件：数量改变与数量不变分别进入 `0xada0/0xae40` 的 PPL 调度，不能将“不变”分支当作未发生工作，也不能由释放 task 引用推定异步回调已经完成。PPL action 到页面的完整消费者仍须继续跟踪。

共享环的实际分配者 `0x10e0` 使用 `Global\\D8B2B77C-040B-4941-A944-769CB6924E1F`，映射长度为 `0x7c`（124）字节：4 字节 header 与 10 项、每项 12 字节记录。Open 成功时直接映射；进入 Create 路径且映射成功后清零 124 字节，没有单独检查 Create 是否返回已存在映射。`0x6ae0` 使用 `Global\\E099001D-500E-457D-97CD-380568F5DCBB` 事件，先取共享 header 作为起始 index，再按 `header + 4 + index*12` 复制 8＋4 字节；处理后以有符号 `cmp index,9 / cmovge` 决定下一 index 是否归零。入口没有先把 header 校验或规范化为 `0…9`，不能擅自改写成无条件 `%10` 后声称同等行为。

该环消费者的伪代码保留 **bad sp value** 警告，字段解释以完整汇编为准：记录第一个 u32 与入口保存的 this 指针低 32 位比较，不相等才打包到 `0xaef0` 的 PPL action。不能将这些不透明字段直接命名成 PID、设备 ID 或设备确认。映射失败的空指针分支与事件／停止处理分别存在，不能由对象分配成功推定共享区已可用。

初始化 `0x1530` 在后续句柄与线程创建前发布全局身份，线程创建链失败会短路并返回 `GetLastError`，没有把前面所有资源完整回滚；停止 event 的创建结果也没有单独进入最终成功判断。`UnInit → 0x1990` 发出停止信号后，对四个线程句柄各等待 **1,000 ms**，没有消费四次等待结果，再关闭线程、event、mutex 并清全局身份；不能套用另一份 SDK 的 10,000 ms，不能把返回 0 当成所有线程、PPL action、事件消费者与设备链已经排空。此节没有加载 DLL 或执行共享事件，实际并发、回调顺序及 Rust／官方行为差分仍保持 Unverified。
## Chroma 原生连接与效果引擎的下一批静态捕获

本节记录 2026-10-24 以四个 worker 并行提取的三个 Chroma 原生模块。分析只读取已校验的 PE、IDA capture、导入、字符串和 xref；没有加载 DLL、启动 Chroma 服务或访问设备。

### RzChromatic64.dll：效果句柄和 CoreStream 边界

| 原件 SHA-256 | 原件大小/架构 | capture SHA-256 | 函数/Hex-Rays 失败 | 导出 |
| --- | --- | --- | --- | --- |
| `30e64d0f10401a2c847476dfaf024d06550028b34d8775f84695d9713b0d9521` | 2,835,656 B, x64 | `b1b40b40b9ab379fc01fdd6158da45f25712266c1f475ffa9e85dc206446c6c4`（[`capture.json`](../../../.analysis/results/native/30e64d0f10401a2c847476dfaf024d06550028b34d8775f84695d9713b0d9521/capture.json)） | 10,120 / 0 | 588 |

- `Init`（RVA `0x134fc0`）、`InitSDK`（`0x134fd0`）和 `UnInit`（`0x134fe0`）是转发桩。`CreateEffect`（`0x134ff0`）先检查 `byte_1802A34CE`，再检查 `PluginGetLibraryLoadedState_0`，通过后把 16 字节效果描述复制到内部函数；`QueryDevice`（`0x1351f0`）走同一加载状态门禁。门禁失败统一可见 `0x80004005`，不能据此推导设备不存在。
- `SetEffect`（`0x135120`）在效果核心为空或停止标志存在时返回 `0x80004005`；否则在对象中写入当前 tick 加 1000 的截止值，再调用内部发送函数。`DeleteEffect`（`0x1351c0`）将描述复制到删除函数。`IsConnected`（`0x135260`）只在库加载状态允许时调用内部查询；`SetEventName`（`0x135290`）没有名称时使用内部默认窗口名。
- `PluginCoreStreamBroadcastEnd`（`0x135670`）需要互斥锁，检查 `byte_180277280` 后构造 `CoreStreamBroadcastEnd` 消息，再释放锁；未初始化或未启用时返回 0。`PluginCoreStreamReleaseShortcode`（`0x135960`）复制调用方字符串、构造消息并释放临时字符串。`PluginClose*`/`PluginUnload*` 包装器在停止标志时返回 `-1`，否则转发关闭或卸载动作。
- 379 个导入中包含映射文件、事件、线程、等待、文件 I/O 和 `CloseHandle`，但这份模块的导入摘要没有直接 HID 传输。`RzChromaSDK64.dll`、`_Keyboard.chroma`、`_Headset.chroma`、`_Mouse.chroma` 等字符串只证明插件/效果命名，不证明效果已写入硬件。`RegisterEventNotification`（`0x272c2a`）和 `UnregisterEventNotification`（`0x272ccb`）保留为导出，但 capture 没有可反编译函数体，调用参数、回调时序和释放仍未闭合。

### RzChromaConnectAPI64.dll：进程间映射和回调生命周期

| 原件 SHA-256 | 原件大小/架构 | capture SHA-256 | 函数/失败 | 导出 |
| --- | --- | --- | --- | --- |
| `babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f` | 223,432 B, x64 | `bae39fcffaf59a8af8299a5ecad4c0f69f14d9c7382c69b70c77a70ec1571a73`（[`capture.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/capture.json)） | 840 / 0 | 6 |

- `Init`（`0x20940`）只允许一个全局对象；重复初始化返回 `1247`。首次调用分配并清零约 `0x4570` 字节对象，建立 `CChromaBroadcastAPI` 虚表并进入内部初始化。`UnInit`（`0x20b40`）未初始化返回 `5023`；成功时先执行内部停止，再删除 boundary descriptor、关闭 private namespace、清理两个内部子对象并释放全局对象。
- `RegisterEventNotification`（`0x20c20`）和 `RegisterKeyboardEventNotification`（`0x20c40`）只有在全局对象存在时才转发；否则返回 `5023`。`UnRegisterEventNotification`（`0x20c60`）在锁保护下清空两个回调槽，未初始化同样返回 `5023`，成功返回 0。回调槽清零不等于已经收到设备或服务确认。
- 导入和字符串显示它通过 `CreatePrivateNamespaceW`/`CreateFileMappingW`、`MapViewOfFile`、事件、互斥量、线程和注册表组织跨进程通道；可见 `broadcast.dat`、`cbevent%d`、`ns-cbevent%d`、`SYNAPSE_NOT_ONLINE`、`BROADCAST_SUCCESS`、`BROADCAST_DISABLED` 和 `BROADCAST_APP_DISABLED`。这些是静态协议线索，映射区布局、消息校验和服务端应答仍需继续闭合。

### RzChromaBroadcastAPI64.dll：兼容加载器

| 原件 SHA-256 | 原件大小/架构 | capture SHA-256 | 函数/失败 | 导出 |
| --- | --- | --- | --- | --- |
| `21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55` | 53,448 B, x64 | `88ca1fc8a4cb43695974ef2488c551dda22bf6e9e75eb83d667c9403bdb1adde`（[`capture.json`](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/capture.json)） | 171 / 0 | 6 |

- `Init`（`0x4b00`）按已加载的 32/64 位模块选择 `RzChromaConnectAPI.dll` 或 `RzChromaConnectAPI64.dll`，调用 `LoadLibraryW` 和五个 `GetProcAddress`；加载失败保留 `GetLastError`，未解析到 `Init` 时返回 `5023`。因此这个 DLL 是兼容转发层，不是设备传输实现。
- `UnInit`（`0x4d20`）先调用已解析的 `UnInit`，再 `FreeLibrary` 并清空句柄。四个事件导出（`0x4d60`、`0x4d80`、`0x4da0`）在对应函数指针为空时都返回 `5023`。字符串和导入还显示 `Global\\RzChromaConnectManager.event.start`、`Global\\RzChromaConnectServer.event.start`、`RzChromaConnectManager` 和 `RzChromaConnectServer`，但未捕获服务端处理。

### Rust 迁移边界

上述三份捕获只闭合了转发门禁、进程间资源、效果消息包装和释放顺序；没有证明 `SetEffect`、CoreStream、广播事件或回调已获得硬件应答。Rust 实现必须分别建模加载失败、未初始化、重复注册、停止/释放、映射/事件异常和服务端应答，并用静态回放后再做显式官方 DLL 对照；当前所有 DLL、服务和设备结果仍为**未验证**。
## RzChromaSDKProxy_v1.2.3.2.dll：设备代理、JSON Invoke 与轮询事件（静态捕获）

原件 [RzChromaSDKProxy_v1.2.3.2.dll](../../../.analysis/inputs/official-native-current/blobs/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/RzChromaSDKProxy_v1.2.3.2.dll) 为 x64 PE，359,112 字节，SHA-256 e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e，Image Base 0x180000000。IDA 捕获 [capture.json](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/capture.json) SHA-256 00b5bf90b995f1381e55be65150eb67433600fcbbef00e9009d45f0d44bfcbe9，1,479/1,479 函数、0 个 Hex-Rays 失败，14 个导出入口。

- [Init 0x358b0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/358b0.json) 先清理旧的全局对象，等待约 100 ms，再创建约 0xA0 字节 native 状态；底层设备数量结果作为返回值。若已有对象，Init 会先调用内部释放。 [UnInit 0x359f0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/359f0.json) 调用内部销毁并把全局指针置零，返回同一错误/数量语义。
- [Invoke 0x35ac0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/35ac0.json) 解析 JSON 的 func、message、result/return 字段，已静态确认分派 AddDevice、SetDeviceState、MoveDevices；未知函数返回 ERROR_FUNCTION_FAILED，未知特性返回 ERROR_UNKNOWN_FEATURE，对象不存在返回 ERROR_OBJECT_NOT_FOUND 或 ERROR_NOT_FOUND，JSON 异常返回 ERROR_BAD_FORMAT。成功或错误都会序列化 response，不能把字符串生成成功当设备写入成功。
- [AddDevice 0x35ab0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/35ab0.json) 的导出桩本身恒定返回 -1；真正添加分支由 Invoke 调到底层状态对象。 [RemoveDevice 0x36ac0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/36ac0.json) 以设备 id 调用内部移除并返回其错误码。
- [PollEvents 0x372f0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/372f0.json) 在轮询模式下锁住事件树，把队列节点搬入临时 JSON 数组，序列化为新分配的 C 字符串并销毁临时节点；调用者必须使用同一分配器释放。 [SetOperatingMode 0x371d0](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/371d0.json) 以原子值切换模式：值为 2 时启动约 40 ms 周期的 threadpool timer，否则停止 timer；UsePollingMode 直接转发该入口。
- [SetAllocator 0x36c50](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/36c50.json)、[FreeMalloc 0x37930](../../../.analysis/results/native/e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e/functions/37930.json) 定义跨边界内存所有权；RzChromaSDKProxy/RzChromaSDKProxyNoReturn 分别返回需释放字符串和直接释放输入/临时 JSON。SetNodeFFIEvent 只保存回调指针，捕获未证明回调线程停止与 join。

该批只从完整捕获读取导出、伪代码、字符串和 xref，没有加载 DLL、启动 Chroma 服务或连接设备。底层 Add/Set/Move 的实际 HID/pipe 应答、事件顺序、取消和恢复仍需独立 Rust 回放与显式官方对照，当前为 Unverified。
## RzChromaBroadcastManager.dll：广播效果、应用注册表与事件回调（静态捕获）

原件 [RzChromaBroadcastManager.dll](../../../.analysis/inputs/official-native-current/extracted/d884ffbdf2dd0fdab2a7dd6b09a6c68b425996c0ded3d67e504e52871ef6d100/$PROGRAMFILES/Razer/ChromaBroadcast/bin/RzChromaBroadcastManager.dll) 为 x86 PE，226,504 字节，SHA-256 13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6，Image Base 0x10000000。IDA 捕获 [capture.json](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/capture.json) SHA-256 bfb1e7f7e6dcd1662abd46128264418a138f6b5298dc8c2cedd2f4c001104338，561/561 函数、0 个 Hex-Rays 失败，23 个导出入口。

- [Init 0x21940](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21940.json) 和 [InitGlobalBroadcastData 0x22930](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/22930.json) 都以全局对象为空作为初始化门禁；重复初始化返回 1247，未初始化调用返回 5023。UnInit 路径销毁内部对象并把全局指针置零，失败时保留原对象。
- SetBroadcastEffect/SetKeyboardEffect 及其 OnApp 变体先检查全局对象，再按产品 ID 选择内部并行设备集合；无效 ID 记录并返回 5，未初始化返回 5023。入口的 PPL RunAndWait 只等待内部任务集合，不代表硬件 writer 已确认读回。
- [Enable 0x21e90](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21e90.json)、IsEnabled、IsRunning、GetNumApps、GetApp、GetAppTitle、GetAppPath 和 GetAppIcon 均把查询转发给全局对象；未初始化查询分别返回 0、-1 或 5023，这些值不能被页面当作设备关闭或空列表的实测结果。
- GetAppIndex/GetDeviceSupported 读取 HKLM 下 Software\Razer\ChromaBroadcast\Apps\<name> 的 Index 或 DeviceSupported DWORD；空名称返回 -2，注册表打开/查询失败返回 -1，并在所有分支关闭 HKEY。该路径只证明本机注册表元数据，不证明应用当前运行或设备已支持。
- RegisterEventNotification 仅允许一个非空回调；未初始化返回 5023，无效产品 ID 返回 5，空回调返回 87，重复注册返回 1242，成功返回 0。UnRegisterEventNotification 使用相同产品 ID 门禁，空回调状态返回 85；UnInit/GlobalUnInit 负责释放内部回调资源。
- 字符串和 xref 还确认 cbevent%d/ns-cbevent%d 事件命名、应用刷新及 MonitorAppEnableEvent/MonitorNumAppEvent/MonitorApiEnableEvent 线程；捕获未闭合这些事件到页面的实际消费者和回调 join 顺序，Rust 端必须分别建模注册失败、重复注册、停止和释放。

本节与既有 Chroma Connect/动态照明捕获可能共享调用集合；SHA、函数和字节覆盖不重复累加。当前只完成静态读取，没有加载 DLL、修改注册表、启动广播服务或访问真实设备，效果写入、事件读回、取消与恢复保持 Unverified。


## RzChromaSDK.dll / RzChromaSDK64.dll 的导出参数与生命周期边界

本节从两份既有 IDA 9.1／Hex-Rays 9.1 捕获继续核对 SDK 公开调用链。Python 使用 `ThreadPoolExecutor(max_workers=4)` 并行核实原件 SHA-256、Image Base、函数记录 SHA-256、全部 chunk 与逐指令原始字节；没有加载或执行 DLL、启动服务或访问设备。

| 原件 | SHA-256 | 大小／Image Base | capture SHA-256 | 本批选定函数／唯一 chunk／指令／原字节 |
| --- | --- | --- | --- | --- |
| RzChromaSDK.dll（x86） | `8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038` | 253,128 B／`0x10000000` | `7d2de3c1ceb4b9860b3eec84bd045a29b4a7d5886cd1d0adc155b675e66c2e32` | 15／15／2,273／6,883 B，全部字节均有列出的指令 |
| RzChromaSDK64.dll（x64） | `7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7` | 273,096 B／`0x180000000` | `35aabd2f3285e856d762889213e7ec73630b69b116a5ff8b33d76250766540d1` | 15／15／1,666／6,822 B，其中指令 6,484 B，chunk 内未列出 338 B |

原件路径与版本归属见下方两份 capture 的 `input`；这些 SHA 用于证据身份，不是产品运行时兼容门禁。x86 完整基础捕获为 733 函数，原样保留 `0x6500`、`0x6510` 两项 Hex-Rays 历史失败；下节的私有补查已独立恢复这两项，本批所选 15 函数本来均有完整汇编及伪代码。x64 完整捕获为 943 函数、0 项 Hex-Rays 失败；本批不是整个 SDK 的语义完成声明，也不与其他文档复用的函数重复累加。

| Export | x86 RVA / function record | x64 RVA / function record |
| --- | --- | --- |
| Init | [`0x26cc0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/26cc0.json) | [`0x28c50`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/28c50.json) |
| InitSDK | [`0x26cf0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/26cf0.json) | [`0x28c90`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/28c90.json) |
| UnInit | [`0x26dd0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/26dd0.json) | [`0x28d90`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/28d90.json) |
| CreateEffect | [`0x26f60`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/26f60.json) | [`0x28e00`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/28e00.json) |
| SetEffect | [`0x287d0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/287d0.json) | [`0x2a530`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a530.json) |
| DeleteEffect | [`0x28780`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/28780.json) | [`0x2a4d0`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a4d0.json) |
| RegisterEventNotification | [`0x28830`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/28830.json) | [`0x2a590`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a590.json) |
| UnregisterEventNotification | [`0x288e0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/288e0.json) | [`0x2a660`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a660.json) |
| CreateKeyboardEffect | [`0x27ea0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/27ea0.json) | [`0x29c30`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/29c30.json) |
| CreateMouseEffect | [`0x28080`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/28080.json) | [`0x29de0`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/29de0.json) |
| CreateHeadsetEffect | [`0x283c0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/283c0.json) | [`0x2a100`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a100.json) |
| CreateMousepadEffect | [`0x28490`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/28490.json) | [`0x2a1b0`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a1b0.json) |
| CreateKeypadEffect | [`0x285b0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/285b0.json) | [`0x2a2f0`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a2f0.json) |
| CreateChromaLinkEffect | [`0x286f0`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/286f0.json) | [`0x2a450`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a450.json) |
| QueryDevice | [`0x28960`](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/functions/28960.json) | [`0x2a6f0`](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/functions/2a6f0.json) |

[x86 capture](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/capture.json)
[x64 capture](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/capture.json)

### 初始化与释放

`Init` 在全局对象已经存在时直接返回 0；首次调用分别分配 x86 `0x1574`、x64 `0x1780` 字节对象，再进入内部初始化。公开入口的成功不能证明底层共享区、服务或设备已经可用。

`InitSDK` 在检查全局对象之前先验证输入：指针非空，偏移 `0`、`512`、`2560`、`3072` 的四个 UTF-16 字符串非空，偏移 `3584` 的 DWORD 为 `1..63`，偏移 `3588` 的 WORD 为 `1..2`；失败返回 87。字符串检查沿 NUL 扫描，本层没有独立长度参数，不能把固定偏移误写成安全边界校验。通过验证后，已有对象仍返回 0，没有重新提交新描述。

x86 `UnInit` 先调用应用 `DevicesSupported` 写回链，将访问标志清零，触发并关闭停止事件，然后对三个线程各等待 10,000 ms，再关闭线程句柄和另外两个句柄、销毁对象并清空全局指针；本层丢弃每次等待结果，最终返回 0。x64 公开 `UnInit` 只有在内部停止返回 0 后才销毁及清空对象，否则保留对象并返回内部错误；下节核实其实际内部函数固定返回 0，不能把这个外层条件理解成已检查线程退出。不得用公开返回 0 证明线程已经退出或全部回调已经结束。

### 效果参数与错误边界

所有已选效果／设备入口在全局对象不存在时返回 5023。`SetEffect`、`DeleteEffect`、`QueryDevice` 都把输入 GUID 与全零 GUID 比较，**相等时返回 87**，其他 GUID 继续进入内部函数；x86 常量 RVA `0x2c2a0` 与 x64 常量 RVA `0x2e590` 的原件 16 字节均为零。这是拒绝空 GUID 的门禁，不能反向理解为只接受某个固定 GUID。

通用 `CreateEffect` 先拒绝大于 9 的类型（87），再把 `1/2/3/4/5/9` 返回 50；余下 `0/6/7/8` 继续按静态产品 GUID 分支验证。多个分支要求类型 6／7 的参数非空，还有产品分支检查首 DWORD 的 LED 值或首 QWORD 的结构大小（可见 `16` 与 `3616`）；不能合并为所有产品都接受同一种结构。完整分支仍保留在两份函数记录，产品 GUID 到内部编码的实际转换继续沿内部消费者核对。

六类专用入口先做各自参数验证，再传入下面的类别／产品选择值。表中的数字是当前原件的效果类型和选择值，未用推测的枚举名覆盖实际语义。

| 入口 | 本层接受的类型 | 参数规则 | 传入内部的选择值 |
| --- | --- | --- | --- |
| Keyboard | `0,1,2,3,4,5,6,8,9,50` | 0／5 强制传空参数；1／6 首 DWORD 为 1 或 2；3 为 1..3；2／4／8／9／50 非空；其他返回 50，参数错误 87 | `0x1ffff` |
| Mouse | `0..9` | 0／5 参数可空，其余非空；0／1／2／4／5／6 的首值为 `1..17` 或 `0xffff`，2／4 第二值为 1..3；7 首值为 0 或 1；9 转为类型 6，并构造首值 `0xffff`、第二值为输入首 DWORD | `0x2ffff` |
| Headset | `0..4` | 1／2／4 非空；0／3 保留输入参数；其他返回 50，缺失参数 87 | `0x4ffff` |
| Mousepad | `0..6` | 1／5 非空且首值为 1 或 2；2／4／6 非空；0／3 保留输入参数 | `0x8ffff` |
| Keypad | `0..6` | 1／6 非空且首值为 1 或 2；3 为 1..3；2／5 非空；0／4 保留输入参数 | `0x10ffff` |
| ChromaLink | `0..2` | 0 强制传空参数；1／2 非空；其他返回 50，缺失参数 87 | `0x80ffff` |

### 通知窗口与未验证边界

`RegisterEventNotification` 的输入是 HWND：未初始化返回 5023，第一次 `IsWindow` 失败返回 6，已有有效通知窗口返回 1247，否则第二次验证成功后保存 HWND 并返回 0；第二次验证失败返回 87。x86 通知槽位在对象 `+5484`，x64 为 `+6000`。`UnregisterEventNotification` 只在当前槽位仍是有效窗口时清零并返回 0，否则返回 5023；本层没有等待消息接收方、回调或硬件完成。

这些导出已经给出独立 Rust 纯／模拟测试的输入边界：未初始化、重复 Init、无效描述、空 GUID、类型 7 的键盘拒绝、鼠标 LED 17 的有效边界、类型 9 的转换、窗口失效和注销，以及停止等待超时。实际效果缓冲区编码、内部集合、共享帧／服务通信、设备写入及读回仍继续追踪；本次没有执行 Rust 行为测试或官方 DLL 实际对照，实际设备、事件顺序、取消、恢复和释放验收均保持未验证。

### SDK 内部线程、通知消息与停止结果

[生命周期逐字节复核](../../../.analysis/results/native-bulk/chroma-lifecycle-verification.json)继续核实同一 x64 原件的 22 个函数与 x86 原件的 7 个函数。x64 为 2,732 条指令、11,661 B chunk，其中指令字节 11,641 B、未列为指令的 chunk 字节 20 B；x86 为 1,430 条指令、4,714 B，所有字节均有列出的指令。完整 chunk、函数记录身份和每条指令均重新与对应原件比较；选择集合与上一节有交集，不能把两节函数数直接相加。本节没有执行 DLL 或其原生工作线程，也未创建应用窗口。

x64 导出 `Init 0x28c50`／`InitSDK 0x28c90` 均先发布新对象到全局指针，再调用内部 `0x81f0`／`0x86f0`。内部的提前返回 `1647` 分支没有清除该全局对象；随后再次调用 `Init` 会命中已有对象并返回 0，而不会重试内部初始化。正常分支创建手动复位停止事件 `+1080`，再启动下列三个线程；没有检查 `CreateEventW`／`CreateThread` 的失败结果就走最终返回 0。对象存在、公开初始化返回与后台观察是否工作必须分别建模。

| 线程及对象句柄槽 | 完整入口／消费者 | 等待与失败的实际分支 |
| --- | --- | --- |
| 应用通知，`+2368` | `0x7060 → 0x140b0 → 0x134f0` | 打开或创建应用事件 `Global\{89811F96-91C2-4C19-8E0A-54469F491550}`；等待顺序为停止事件、应用事件。索引 0 退出，1 刷新应用状态，其余记录错误并退出；停止句柄为空时仅日志后仍进入等待 |
| 本机在线状态，`+2376` | `0x7070 → 0x14310 → 0x134f0` | 创建在线／离线两个事件 `Global\{A84AF9C8-EFE0-430D-871C-10DA760C2CCD}`／`Global\{0F9297E6-E80C-47E4-9A8B-1237E50484B7}`，停止事件位于第三项。索引 0／1 设对象 `+0` 为 1／0 并刷新，2 退出，其余记录错误；退出关闭本线程持有的前两个事件 |
| 会话状态，`+2384` | `0x7080 → 0x6e70 → 0x14600` | 入口持有 `Global\{821AA2A2-8215-4A16-BE9D-7CD8CEBDC398}` 的 `0x54` B 共享区，再等待会话事件 `Global\{B8B918C0-9790-47F2-AC7A-F36B8414140C}`、停止事件。共享区为空直接返回当时 `GetLastError`；缺停止事件返回 6；正常 stop 索引 1 退出，包装入口再 unmap／close 自己的共享区资源 |

`0x134f0` 的通知消费者是已登记的 `HWND`，通过 `PostMessageW(hwnd,0xa000,wParam,lParam)` 发布：全局注册表 `Apps\Enable` 变化使用 `wParam=1`，当前应用 `Apps\<exe>\Enable` 变化使用 3，访问权变化使用 2，`lParam` 为对应状态值。注册表查询失败时不把该字段改为 0；不是每次唤醒都发送这三种消息。访问权还比较本进程、共享区发布的 active PID、在线与会话状态；在线但 active PID 尚未发布时保留既有访问权。`PostMessageW` 没有同步设备应答、接收者确认或队列排空契约，注销槽位也不能撤回已排队的消息。

会话读取 `0x13300` 有原版特有的失败策略：`ProcessIdToSessionId` 或连接状态查询失败会按 unlocked 返回 1；已知连接非 active 返回 0；后续扩展会话查询失败则采用已取得的 active 结果。该布尔是软件访问策略，不能当作会话实测成功。会话事件 `0x14600` 处理 lock／unlock、console disconnect／connect 时还存在 500／1,000 ms 等待及恢复调用；其具体效果编码不在本节的已闭合边界内，不把等待结束当作灯光恢复确认。

x64 `UnInit` 的实际停止链为 `0x28d90 → 0x8c20 → 0x7e10 → free → 全局指针清零`：

1. `0x8c20` 先调用 `0x13140` 合并写回本应用 `DevicesSupported`；该函数的注册表错误没有被停止链采用。
2. 将访问字段 `+12` 清零，调用 `SetEvent(+1080)`，随即关闭停止事件；只有 `CloseHandle` 成功才清零槽位。`SetEvent` 结果不检查，关闭发生在等待线程之前。
3. 对 `+2368/+2376/+2384` 中非空线程句柄分别 `WaitForSingleObject(...,10000)`；三次调用后都直接重新读取句柄，没有检查等待返回。随后关闭句柄，关闭 `+1064/+1072` 的同步句柄。
4. 调用 `0x13000(pid,0)` 把进程状态写入另一共享环并 pulse 通知事件；返回值同样不进入停止结果。尾部汇编 `0x8d8c xor eax,eax`，本函数固定返回 0。
5. 外层因此继续析构。`0x7e10` unmap／close 应用共享区、八个拥有对象和各设备映射；对象清理 `0x5f00` 还关闭事件／映射／私有命名空间并删除边界描述。失败后的条件、线程实际退出和消息处理并没有另一次确认。

因此该静态链证明“尝试停止、等待并释放”，不能证明 timeout／wait failure 后线程已退出，或消息接收端已停止使用相关状态；这里只记录未检查结果与释放顺序，没有执行并证实并发故障。独立实现与官方行为对照须分别验证停止信号失败、等待超时、句柄关闭失败、初始化部分失败、重复初始化、注销后的已排队消息与在途观察，不能照外层返回 0 统一宣告清理成功。

x86 的两项 Hex-Rays 历史失败也不意味着机器码和线程语义丢失：`0x6500`／`0x6510` 各有完整六条指令，均从 `[ebp+8]` 取唯一线程实参到 ECX，调用 `0x14260`／`0x14460` 后 `ret 4`。两个实际调用者 `0x7530`／`0x7a20` 均将它们传给 `CreateThread` 并传入同一对象；完整 callee 分别等待应用事件与在线／离线事件，再调用 x86 自己的状态刷新 `0x13490`。这条 ABI 依据来自该 x86 原件，没有套用 x64 伪代码。

#### x86 精确补查与没有被外层消费的错误

现有 `supplement-native --no-refresh-gaps` 已对 `0x6500/0x6510/0x14260/0x14460` 使用单个私有 IDB 补查。完整汇编中，两个 callee 在 `0x14288/0x1448b` 将 ECX 保存到 ESI／EDI，正常路径没有调用者栈参数，结果经 EAX 返回，正常退出为裸 `ret`；额外 chunk 的栈读取属于 SEH／安全清理 funclet。因此只对两个 callee 固化 `unsigned int __thiscall(void *context)`，保留 wrapper 的单参数 `__stdcall`、原始函数边界与全部代码字节。

[补查 capture](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/supplements/e40c25b84e7ca202/capture.json)与[分析器 session](../../../.analysis/results/native/8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038/supplements/e40c25b84e7ca202/session.json)保留实际声明、应用前后类型、私有／基础 IDB 身份和完整四函数。独立 `native_reanalysis_resolution` 核对源码身份、原始与补查函数、chunks、逐指令、xref、计划及 session 后接纳两项恢复：该模块剩余 Hex-Rays 失败为 0，补查核验错误为 0。基础 capture 的两条错误与原函数 JSON 均未覆盖；复核结果在本页既有生命周期收据的 `x86_type_reanalysis` 中。

根因仍保持准确边界：基础函数 JSON 的 callee `type` 字段为 null，但本次私有 IDB 记录的 `previous_type` 已是 `DWORD __thiscall(int *this)`，其调用约定与此次声明一致。固化已核实类型并清除反编译缓存后成功恢复，不能据此宣称原来的 ABI 一定错误，原始类型推断／缓存失败的具体内部原因尚未定位。成功生成伪代码和严格字节核验，也不等于已运行观察线程或验证 DLL 功能。

额外四个完整函数 `0x1a70/0x1950/0x1df0/0x13490` 的 680 条指令已并行与原件复核，结果在 `failure_consumer_extension`，不与前文函数数量相加。实际错误消费继续区分：

| 分支 | 线程实际返回／状态变化 | 初始化、注销及后续消费者 |
| --- | --- | --- |
| App 事件无法打开／创建 | `0x14260` 在清理安全资源后取当时 `GetLastError`，保存到 ESI，调用 `0x1950` 日志后将保存值作为 EAX 返回；并未保存每一个较早操作的原始错误 | `CreateThread` 创建成功不表示线程进入等待；初始化不读取这个退出码 |
| Synapse 安全属性准备失败 | `0x1df0` 返回非零时跳过两个事件的创建；`0x14460` 的结果寄存器 EBX 初始为 0，此路径清理后仍返回 0 | 与“两个事件已创建并正常 stop”会出现相同线程退出码，不能据 0 声称已观察在线状态 |
| Synapse 创建某个事件失败 | 每次显式创建失败均捕获当时 `GetLastError`；后一次失败可替换前一次保存值，再清理已取得的事件与安全资源 | `0x1a70` 记录后返回输入 DWORD，没有额外归一化；外层 Init／UnInit 均没有取线程退出码 |
| 两个 monitor 的等待异常 | 非已登记索引转 `GetLastError → 0x1a70`，并将该 DWORD 返回为线程结果；正常 stop 返回 0 | 外层 `UnInit 0x26dd0` 只等待并关闭线程句柄，丢弃等待结果，不调用 `GetExitCodeThread`，最后仍返回 0 |
| 刷新字段后窗口无效／`PostMessageW` 失败 | `0x13490` 已先更新相应状态，再尝试通知；没有因为通知失败恢复旧值。同一字段之后仍相等时不重发该项变化消息 | `0x14260/0x14460` 均丢弃刷新函数返回值并继续等待；发送失败不会成为其线程错误结果 |

两个 monitor 内部没有 `ResetEvent`；本节未核实命名事件所有生产者何时复位，不能把一次事件置位解释成一次且仅一次刷新。静态／模拟验证还须专门覆盖安全属性失败但退出 0、后次创建错误覆盖前次、等待错误被外层丢弃、通知前字段已更新以及相同值不重发。真正事件生产者、窗口接收端、线程退出和消息排空仍须独立确认；本次恢复了缺失伪代码，没有把这些运行时项改记为通过。

前端边界仍按真实路径区分：前述两设置页的控制权链走 LightingDriver／LampArray，不因本页存在 SDK HWND 通知就视为已接到它们。此次对三个已有前端目录的 209 份 JS 重新计算字节身份并进行有界字面扫描，未命中 `RzChromaSDK64`、`RegisterEventNotification` 或 `UnregisterEventNotification`；这只说明该范围内没有这些字面量，不能排除动态绑定，也不能证明窗口消息已经到达任何页面。实际消息接收 owner、全设备效果写回、Rust 独立实现、行为测试和官方实际 DLL／设备对照继续保持未验证。

#### 两份 COM 库的异常复制与线程 ABI 补查

既有[生命周期收据](../../../.analysis/results/native-bulk/chroma-lifecycle-verification.json)的 `native_failure_review.com_library_review` 保留两库完整静态审阅，`formal_reanalysis` 记录统一队列实际执行的 IDA 9.1／Hex-Rays 9.1 补查及独立复核。静态审阅使用四线程核验 48 份完整函数记录（两库 46 份及两份当前 x86 CRT provider 导出），共 6,317 条指令、23,157 B 完整 chunk；其中 23,114 B 列为指令，另外 43 B 是五处原件多字节 NOP，完整字节和位置均保留，没有因未列为指令而丢弃。这个集合不是两个 DLL 的完整功能语义审计。

| 原件 SHA-256 | 正式补查 | 已恢复的原始 Hex-Rays 失败／仍缺伪代码 | 保留的其他限制 |
| --- | --- | --- | --- |
| `94dbc59390605e36ecea1fbc36ccf41b19d1ef078f85f9b5274c3f2cc5d3cf4f` | [29 函数 capture](../../../.analysis/results/native/94dbc59390605e36ecea1fbc36ccf41b19d1ef078f85f9b5274c3f2cc5d3cf4f/supplements/8ee8425080e5784b/capture.json)、[session](../../../.analysis/results/native/94dbc59390605e36ecea1fbc36ccf41b19d1ef078f85f9b5274c3f2cc5d3cf4f/supplements/8ee8425080e5784b/session.json) | 19／0（异常构造 11、线程入口 8） | 原先被拒补查的核验错误 1 条；新伪代码 `0x1a4d0` 仍有 bad-sp 警告 |
| `d7781beebf438339b82085e9140609fbdd549d573617c8d71134754afeb58f8b` | [17 函数 capture](../../../.analysis/results/native/d7781beebf438339b82085e9140609fbdd549d573617c8d71134754afeb58f8b/supplements/c793d3795fb0092e/capture.json)、[session](../../../.analysis/results/native/d7781beebf438339b82085e9140609fbdd549d573617c8d71134754afeb58f8b/supplements/c793d3795fb0092e/session.json) | 13／0（异常构造 10、线程入口 3） | 原先被拒补查的核验错误 1 条；新伪代码 `0x168c0/0x17750` 仍有 bad-sp 警告 |

两库的 21 个异常构造失败都调用真实 named IAT `VCRUNTIME140.dll!__std_exception_copy`，槽位分别为 `0x2f2dc`、`0x2528c`。调用者先压目标异常数据指针、再压来源指针；一次复制清理 8 B，两次复制统一清理 16 B。当前保留的两份 x86 provider 导出 `0x4320/0x4720` 完整 102 B 机器码与[保留的 CRT 头文件](../../../.analysis/inputs/platform-abi-current/msvc-14.44.35207/include/vcruntime_exception.h)共同核实 `void __cdecl(source,destination)`；外层构造函数自己的 thiscall／`retn 4` 保持不变。正式补查记录两条精确 named IAT 的导入类型应用；这两库没有额外的 import-thunk 类型恢复记录，没有按相似函数名替换其他入口。静态 provider 一致不证明运行时 loader 会选中哪份 DLL。

两份 `Init`（`0x14850/0x129a0`）先发布全局对象，再以空 `lpParameter` 创建 11 个线程入口。外层入口不向内部 worker 压入参数，也不建立 ECX，仍保留 `DWORD __stdcall(LPVOID)` 和 `retn 4`。12 个内部 worker 从真实全局 `Block` 读取对象，唯一普通出口为裸 `retn`；补查仅固化无参数 cdecl 和 32 位 EAX 返回载体，不声称恢复了原始 C++ 返回类型或有符号性。四个对齐栈序言先 `push EBX`，故其后的 EBX 等于原始入口 ESP 减 4；`[EBX+4]` 是原始返回地址，不是普通业务入参。原始推断、SEH 共享 chunk 和 bad-sp 警告全部保留。

独立双线程重新执行现有 `native_reanalysis_resolution`，逐项核对实际原件、基础捕获、补查、完整函数、逐指令与 xref、计划、导入及分析器 session，接纳 19＋13＝32 条原始失败恢复；46 个选定函数均有补查伪代码。基础失败、两条历史补查核验错误和三处新伪代码 bad-sp 警告没有删除，因此这里不能写成“整个 DLL 无错误”或“完整功能已验证”。实际调用结果、间接业务消费者、页面接线、独立 Rust 行为和官方 DLL／设备对照继续分别保持其未验证边界；本次复核未启动新 IDA 队列、运行 DLL 或运行应用。

#### CRT 寄存器参数与无参异常抛出

既有生命周期收据的 `native_failure_review.remaining_call_abi_review` 保留两份 x86 CRT 和一份 Chroma 库的完整调用点、原件字节、精确类型及正式补查复核；这里区分补查产生了伪代码与严格证据核验实际接纳。

| 原件 SHA-256／失败目标 | 当前严格复核结果 | 原始失败的当前边界 |
| --- | --- | --- |
| `5695560a50ed9746696c0d647e55d77459f5981907c177d086df36656a978b19`／`0x4b5b` | [当前裁决](../../../.analysis/results/reverse-pending/review-sources/e8e1fe220f8fee1fba31c945e4a68313ef63ca2943f01f248039bc355c833a33.json)接纳完整 `0x4b5b/0x4bb7` 及独立 NLG 补查 | `0x4b5b/0x4bb7/0xb7b5` 均有完整补查函数；旧冲突及失败保留 |
| `56e6c202747c00c99b812c236b47d065c475baa8bded1dec1d55c338eeaf85e8`／`0x46e2` | [当前裁决](../../../.analysis/results/reverse-pending/review-sources/60970a1039467de50ac0050341c820f6493a7a614d2db2afa0ec1b279d7a981b.json)接纳完整 `0x46e2/0x473e` 及独立 NLG 补查 | `0x46e2/0x473e/0xafc5` 均有完整补查函数；旧失败保留 |
| `9fa56120e438276d9e212f09738b9c60848ac0876229c6dccd835aeb40891f97`／`0xecf0` | [当前裁决](../../../.analysis/results/reverse-pending/review-sources/3c429794beda2b7ba76fb08433d637851d2ceef3e60339e639ea0eae460e8009.json)接纳两份完整函数 | `0xecf0/0x21e60` 均有完整补查函数；旧失败保留 |

两份 CRT 的内部 `FlsSetValue` worker 分别为 `0x4e28/0x49a8`，均有完整 32 条指令。入口 EDX 先保存到 ESI，FLS index 只从原件全局槽位 `0x10028/0x10024` 读取；入站 ECX 仅通过 `push ECX` 保留局部栈空间，不作为输入。动态解析 `FlsSetValue` 与 named `TlsSetValue` IAT（`0x1106c/0x11064`）两支均先压 value 再压 global index，并原样返回完整 EAX，worker 自己以裸 `retn` 返回。每份原件的五个真实调用点均显式建立 EDX，传入 startup PTD、`-1`、新分配指针或零，没有为 worker 压普通栈参数。因此精确内部声明为 `int __usercall ___vcrt_FlsSetValue@<eax>(void *fls_data@<edx>);`，不能保留虚构的 `fls_index` 栈参数或补造 ECX 入参，也不能把这一优化内部 ABI 当成 Windows 公开 API。

`569556…` 历史重叠冲突的两份 `0x4bb7` 记录，其完整 chunks、汇编、类型及伪代码文本相同，差异位于伪代码局部变量 `location`：旧捕获器保留了带进程地址的 IDA SWIG `vdloc_t` 对象表示。当前裁决选择重新完整捕获的局部变量、真实寄存器命名空间及栈坐标转换，逐份保留所选记录和差异；没有忽略字段、删除旧捕获或放宽比较，同类历史 `0x473e` 冲突也保留。生命周期收据的 `followup_plans[0]` 为当时的补查计划，其目标已由上表当前完整结果覆盖，不再作为待运行任务。

`9fa561…` 的调用链为 `0xecf0 → 0xf420 → 0xf2f0 → _CxxThrowException`。完整九指令 `0xf420` 不读取入站业务参数；它构造本地 12 B 异常对象，压入真实 ThrowInfo 与对象指针后调用 `0x25a1f` 的六字节跳板，跳板指向 named `VCRUNTIME140.dll!_CxxThrowException` IAT `0x281f0`。六指令构造函数写入 `bad array new length` 与相应 vtable。精确 helper 声明为 `void __cdecl __noreturn sub_1000F420(void);`；不返回语义同时有[保留头文件](../../../.analysis/inputs/platform-abi-current/msvc-14.44.35207/include/ehdata_forceinclude.h)第 190–191 行异常契约佐证。当前 provider 导出机器码仍有物理 `retn 8`，完整原件保留，不能把 C++ 异常契约描述成机器码没有返回指令。

上述三个目标及上表其余原始失败目前均有明确接纳的完整补查依据，所有旧失败、冲突及核验错误继续保留。这里闭合反编译恢复与具体 ABI，不表示实际执行了 CRT 或 Chroma 功能，也没有完成整个 DLL 的业务语义和行为对照。

#### Stream 格式化操作符的精确导入边界

`RzChromaStreamPlugin.dll` 原件 SHA-256 为 `4fc03c7540c4917529b2f416c88cb8f3703b98b225d8f2f02eeddceb4b364393`。其失败选择器 [`0x1b350`](../../../.analysis/results/native/4fc03c7540c4917529b2f416c88cb8f3703b98b225d8f2f02eeddceb4b364393/functions/1b350.json)保留完整 40 条指令；调用失败地址为 `0x1b3e2`，真实 named IAT 是 `0x71138`，名字对应 `MSVCP140.dll` 的 `basic_ostream<char>::operator<<(ios_base& (__cdecl *)(ios_base&))`。这里按原 PE 导入表的精确修饰名绑定，不以相似函数名替换。

单回调分支压入一个函数指针；双回调分支在 `0x1b3d0/0x1b3d5` 预压两个指针，在 `0x1b3da/0x1b3e2` 调用同一导入两次，两次之间将 EAX 送入 ECX，调用后没有 caller 栈清理。当前保留的两份 x86 `MSVCP140.dll` provider（SHA-256 `463930767aaeb5603ebca7a068a17ca9482e64662a806942c4f48cbc6e1a8507`、`f656f0f98cf2510f4e0fe5d5666643028a6b8bac50bf553c0a464456c0e82934`）中，精确同名导出分别落在 `0x31d00/0x32640`，各有完整 20 条指令：ECX 保存为 this，唯一栈参来自 `[EBP+8]`，依 vbtable `+4` 调整到 ios_base 子对象，以 cdecl 调用 callback 并清理 4 B，EAX 返回原 this，最后 `retn 4`。合并函数地址还承载其他导出别名，不能用反编译器显示的 wide istream 名称覆盖实际 char ostream 导入。

因此只对这一精确 IAT 使用 `void *(__thiscall *stream_insert_ios_base_manipulator)(void *stream, void *(__cdecl *manipulator)(void *ios_base));`；每次调用消费一个栈回调指针，ECX 接收者不是额外栈参数。[保留的当前编译器头文件](../../../.analysis/inputs/platform-abi-current/msvc-14.44.35207/include/__msvc_ostream.hpp)第 200–203 行给出相同的 ios_base cdecl callback 重载；该头文件只是 ABI 佐证，静态 provider 一致仍不证明运行时 loader 选中了哪份库。

四个已有完整 callback 记录 `0x1a7d0/0x1a7f0/0x1b080/0x1b470` 均从普通栈输入取得指针、EAX 返回该指针并裸 `ret`。另两个实际目标 `0x1b060/0x1b450` 在原 capture 中没有独立函数入口，故仅保存原件支持的两个 23 B inline 代码跨度及其六指令解码，没有捏造新函数边界。新收据 `native_failure_review.followup_call_abi_review` 使用四线程独立核对 9 份完整函数、165 条指令与 493 B chunk，全部 chunk 字节均有列出的指令；两个 inline 跨度另列，不充当新增完整函数。

`followup_plans[1]` 的 `0x1b350` 已完成正式补查，[当前严格结果](../../../.analysis/results/reverse-pending/review-sources/6607ea59c241408ab3822554e599f1215bc742531c1d8a110e4c87afe198ddfa.json)接纳[完整函数](../../../.analysis/results/native/4fc03c7540c4917529b2f416c88cb8f3703b98b225d8f2f02eeddceb4b364393/supplements/a07924a61225875a/functions/1b350.json)。原件 40 条指令、精确 IAT、两次单栈参调用和完整 locals 的逻辑字节 SHA 已重新核对，原失败继续保留。该选择器只改变流的格式状态，没有设备 SET、应答或读回；CRT／Stream 的实际 provider 执行及完整业务消费者仍按各自边界保持未验证。
