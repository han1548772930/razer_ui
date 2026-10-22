# 本地固件与安装生命周期

本机固件操作使用已在本地的固件文件、产品配置和真实设备传输；安装生命周期管理本程序的文件、驱动/服务动作、共享依赖、版本记录与清理。文件存在、哈希匹配、准备成功、设备传输完成和安装记录发布各有独立的成功条件。

## 当前源码和原生所有者

安装行为来自 [installer/index.325f8759.js](../../../.analysis/inputs/frontend/installer-current/raw/apps.razer.com/installer/assets/index.325f8759.js)及其[获取收据](../../../.analysis/inputs/frontend/installer-current/acquisition.json)。固件本机动作和事件消费者来自 [update-fw/main.69cc5fbd.js](../../../.analysis/inputs/frontend/synapse-update-fw-current/raw/apps.razer.com/synapse/update-fw/static/js/main.69cc5fbd.js)及其[获取收据](../../../.analysis/inputs/frontend/synapse-update-fw-current/acquisition.json)。下列前端区间均指各自原件的 UTF-8 字节。

| 所有者 | 原件字节区间 | 本地职责 |
| --- | --- | --- |
| 固件包装器 `rzFWUpdate` | `[543895,549620)` | 原版调用声明、事件注册、SDK 初始化与退出 |
| 固件 API `Wr` | `[1867558,1878953)` | 设备/连接分支、本机文件准备、开始传输、失败恢复 |
| 固件事件管理器 `Br` | `[1860436,1866222)` | 当前设备事件关联、进度、完成和错误 |
| 安装文件核对/发布 `Rr/Xa` | `[134646,137251)` | 实际本机文件清单、内容写入、hash 核对 |
| 本机安装动作 `Ka` | `[137252,139690)` | 本地可执行文件/驱动/服务动作与结果 |
| 安装执行 `yl/$l` | `[181450,182795)` / `[183691,184260)` | 优先级、并发、取消、错误、重启与安装记录 |
| 卸载 `Bl/Dr` | `[192589,193737)` / `[194321,195353)` | 产品版本、共享使用者、真实删除与待清理记录 |

对应原生原件为 [RzFWSDKWrapper_v1.0.3.0.dll](../../../.analysis/inputs/host-current/blobs/0c6137afee7b08c1ebf18eaa37cd99db350a0765f3f5abc318aabb18c5ac6e76/RzFWSDKWrapper_v1.0.3.0.dll) 的[静态捕获](../../../.analysis/results/native/0c6137afee7b08c1ebf18eaa37cd99db350a0765f3f5abc318aabb18c5ac6e76/capture.json)，以及 [FirmwareUpdaterSDK_v1.0.1.031300.dll](../../../.analysis/inputs/host-current/blobs/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/FirmwareUpdaterSDK_v1.0.1.031300.dll) 的[静态捕获](../../../.analysis/results/native/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/capture.json)。原版的业务 DLL 加载和导出转发只用来确定独立实现的边界，不能进入本项目产品后端。

## 固件目标和准备

固件 API 读取当前设备的 productId、vendorId、serialNumber、连接方式、supportUsb/supportDongle、现有版本和目标版本。普通分支与 Windows UMA 分支选择不同的原版包装器。USB、dongle 和 BLE 的产品身份不能互换；缺少真实连接身份或必要版本时，本次操作失败，不能从本地记录补成已确认读数。

标准准备 `[1869071,1870963)` 先核对目标版本和 API 初始化状态，按连接方式选真实 PID，再读取已缓存的本机产品配置。提交内容是 `pid/vid/SN/targetVersion/deviceConfig`，deviceConfig 在通信边界序列化。返回必须能解析，且 `code===SUCCESS`；缺配置、异常、无效 JSON 或其他 code 均不算准备成功。此时还没有证明设备已经更新。

SDK `initSDK` RVA `0x1c0d0` 解析本地 `localResourceRootPath` 与 log 配置，执行字符串/JSON 校验。`prepareUpdate` RVA `0x1c8d0` 的真实本机链为：

1. `0xf7c0` 解析产品身份、序列号、目标版本和 deviceConfig；后者可以是 JSON 字符串或对象。
2. 读取所选版本的 `resourceSha256` 等固件包信息，形成配置根目录下的本地 ZIP 路径。
3. `0x1a850` 读取文件计算 hash，长度和实际 hash 字节均须匹配；不匹配返回 code 2001。
4. `0x1ae30` 解压本地归档；失败返回 2002。
5. 打开解压目录中的设备配置 JSON，由 `0x11320` 解析 VID/PID、reportId、reportLength、deviceType、协议和各 IC 的 mcuIndex、targetVersion、updateType、fwFileName、delay、bcdPid 等字段；文件打不开返回 2003。
6. 准备函数在上述文件读取路径返回 code 1，但未检查 `0x11320` 的 bool 返回；该解析器 catch 后可以返回 0。因此原件的 code 1 连“全部 IC 配置有效”都不能独立证明，更不能代替后续版本读取、IC 选择、传输应答或最终事件。异常路径的返回与错误记录也须单独保留。

原版配置键实际拼写包含 `supportProrocol`，在外部格式边界按原件解析；内部仍应使用具名类型。配置中固件包 hash 属于实际文件完整性约束，与逆向原件的证据 SHA 各有用途。产品不得拿工作站路径或证据文件 hash 判断设备兼容性。

UMA 分支 `[1871666,1874303)` 从实际本机 Common 文件清单中找到两个芯片文件，按目标版本和产品 ID 选择，传入 `chip1UpdatePath/chip2UpdatePath`、当前/目标版本与设备身份。源码当前使用 `some` 判断两个路径的存在结果，至少一项存在就可越过该判断；不能把它记为“两个固件文件已全部校验”。原件的 UMA 初始化还会跳过标准 setUpSDK，因此不能套用同一套准备状态。

## 实际传输、事件和成功

标准分支 `[1874303,1875528)` 对参数进行校验后调用实际传输，并通过事件 promise 等待结果；`runFWUpdater` 的 void 返回本身没有成功含义。接收器版本需要独立读取 currentDongleFWVersion 和目标 dongle 版本，不能使用设备主固件版本冒充接收器读数。

原版 wrapper 的 `runFWUpdater` RVA `0x33c30` 复制输入字符串的所有权，再交给工作队列；callback RVA `0x32ee0` 构造 `callBackId/callBackType/cmdType/arg/event`，上层由事件频道消费。SDK `runFWUpdater` RVA `0x1d8e0` 解析运行参数后进入 `0x1a340`，后者复制已筛选的 IC 列表，按 mcuIndex 依次处理，每项之间等待 500 ms。没有需要更新的 IC 时发布 code 2004，而非伪造一笔传输成功。

实际 IC 执行 `0x19b80` 先按协议需要处理本机 HID，再用 VID/PID/SN 和 report 条件查找目标句柄。找不到句柄发布 3003；找到后关闭查询句柄，再按 updateType 分派实际更新器。已读到的分支为 0…4 经 `0x180f0`、10 经 `0x19020`；不能把其他值推断成已支持。两条链分别管理本地固件路径、真实更新调用、进度回调及成功/失败，加载下层更新库失败发布 3004，实际更新失败发布 3002。下层产品协议仍需分别独立迁移，不能因调度器和少量 getter 已读完就宣称固件引擎完成。

MessageMgr 按 CommandName 路由；携带序列号时必须与当前设备匹配。USB 枚举/打开/关闭事件单独消费，不算 DFU 完成。DFU payload 必须具有 `updateCode="0000"` 和有效的 totalProgress，进度仅接受 0…100 的数值；totalProgress 达到 100 才完成成功 promise。非成功码、无效 payload 和执行异常进入失败；stepProgress/stepName 用于阶段显示，不能用计时器把进度补满。

操作登记前，`jr` 对已有在途事件任务最多等待五轮，延迟从 500 ms 倍增、上限 5000 ms；这是等待事件通道空闲，并非重新刷写五次。标准与 UMA 调用均未提供该事件 promise 的 timeout，源码打印的 “no timeout” 不能描述成自动取消或已实现超时恢复。

原版还通过本机 window storage 关联正在更新的 productId/serialNumber，阻止另一个窗口同时占用；当前代码的序列号键名为 `serialNUmber`，需按实际 producer/consumer 匹配。该记录是窗口协调状态，不是设备处于 bootloader 的实测结果。

## 固件失败、恢复与退出

失败处理 `[1877290,1877861)` 先刷新当前版本信息，再允许窗口/宿主退出，并请求重新打开对应设备 MW。重连检查按 productId 和 deviceContainerId 查真实本机窗口服务客户，循环间隔从 3000 ms 降到最小 500 ms，另有 10000 ms 边界；存在设备页不等于每个设置已读回。当前恢复函数的计时器和异步回调需由实际生命周期所有者承接，不能把日志中的 cleanup completed 当作设备恢复证明。

前端 destroy 会清任务 map、timers 和 onstorage；它没有展示对所有 Electron response 注册的同身份注销。原版 rzFW 包装器的 terminate 调用 uninitSDK；wrapper 的 `DeviceTerminate` RVA `0x325d0` 只记录并返回 1，另一个 `Terminate` 才进入库资源清理。SDK 的 `uninitSDK` RVA `0x1d770` 释放 logger，而 `endFWUpdater` RVA `0x1da80` 是空函数；`removeSdkIotCallBack` RVA `0x1d830` 的已读逻辑压缩空回调项，不能证明已注销非空回调或等待工作线程退出。不得据此对用户声称可安全中止刷写或已完整释放。

## 本机安装文件与执行结果

安装入口要求 type、id 和 location，申请 `installation_<type>_<id>` 的本机锁。锁通过 `navigator.locks.request(...,{ifAvailable:true})` 获得，保存 resolver 维持租约，统一结束路径释放。拿到锁只证明本轮安装协调权，不证明任何文件或驱动已安装。

`Rr` 从 `simpleGetUserApps` 的真实文件清单同时匹配 filename 和 sha256。`Xa` 要求 action 具有 saveToDisk 和实际 uint8ArrayData，按 app/path/filePath/fileName 发布本地文件；现有同 hash 文件可以跳过重复写入，否则调用 `simpleAddUserAppFile` 并读取真实 bool/result/reason。纯存储清单项、资源名和目标版本都不能代替实际文件存在与内容核对。

`Ka` 的本地执行行为保留以下分支：

| 条件 | 原版行为与结果边界 |
| --- | --- |
| 缺 action 或 saveToDisk | 本次动作失败 |
| 文件后缀为 .dll | 不作为可执行安装器运行；文件替换/写回另由本地文件发布和清理链管理 |
| manifest 设置 NO_INSTALL | 跳过执行；原件把跳过记作成功，不能据此认定驱动/服务安装完成 |
| Windows 且有 stop-service 动作 | 先执行已声明的服务停止，再进入安装动作 |
| 非 ASCII 路径且满足驱动/文件/服务版本分支 | 通过实际 ASCII 复制路径启动；不能泛化为所有平台都需要提权 |
| manifest 明确要求 elevated | 使用对应提升入口；其他分支使用普通本机进程入口 |
| 实际进程返回 | 保留 result、exitCode、reason 并广播 `install-fileExitData`；异常属于失败 |

安装资源按 priority 分桶，桶之间顺序执行，桶内 Promise.all 并行。`yl` 先写 INSTALLING，再检查空列表，仅在进入整个安装循环之前检查一次 isCancelling；桶内和后续桶没有再次检查。文件发布完成和 runOnInstall 的可执行安装动作是两种阶段。`$l` 要求实际调用结果不是 false/installerError 后才维护安装资源记录；manifest 要求重启或 exitCode=3010 时设置 restartRequired。安装后按 keep 标志决定是否删除安装文件，删除使用实际路径和 hash。

最终发布产品版本前检查 installErrors；源码的 ignoreErrors 分支可以在仍有错误时完成原版流程，这个状态不能在本项目中被解释为所有本地能力成功。completedResources、expectedResources、upgradedResources、真实进程结果、资源记录与产品记录分别维护。

## 安装原生队列、退出码与服务状态

当前主机 `main.js` 的实际 Windows 发布路径选择宿主版本目录中的无版本 `simple_service.dll`；调试 CommonDLL 分支另行选择本机副本。这里只分析该选择规则，不将厂商路径迁入产品。用于下列主链核对的是[当前主机原件](../../../.analysis/inputs/installed-current/files/app-engine/CommonDLL/simple_service.dll)及其[静态捕获](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/capture.json)，Image Base 为 `0x180000000`；另保留版本化原件的独立身份，不能因名字相近便当作同一个二进制。

导出对参数非空和线程有效性的核对先于真实工作。AddUserAppFile 复制输入字节至拥有自身生命周期的容器；普通、提升和 ASCII 启动复制 app、filePath、params，并把完成回调排入队列。实际虚调用已经按构造函数和原始 PE 核对：vtable RVA `0x1f2040` 的 `+0x48/+0x68/+0x70/+0x78` 分别落到 `0x2d10/0x3720/0x3a00/0x3ce0`。未发现可从本次 installer cancel 广播传入的取消参数、任务 ID 或进程句柄。

| 动作 | 导出 → 主队列 → Apps 队列 → 实际工作 |
| --- | --- |
| 发布文件 | `0x10660 → 0x2d10/0x2ee0 → 0x221de/0x223a0 → 0x274f4` |
| 普通进程 | `0x12280 → 0x3720/0x3920 → 0x23052/0x23250 → 0x1d5e8/0x284d0` |
| 提升进程 | `0x13140 → 0x3a00/0x3c00 → 0x236cc/0x238d0 → 0x1d74a/0x29570` |
| ASCII 路径进程 | `0x14000 → 0x3ce0/0x3ec0 → 0x233a6/0x23580 → 0x1d8ac/0x2a040` |

主队列缺失返回 `Invalid thread`；Apps 线程或实例缺失分别有独立失败，进程回调可带 exitCode=-1。Apps 实际工作先确认目录、文件及文件校验，再进入本机启动或 elevation COM 分支。普通进程的等待标志为真，ASCII 路径另包含是否已为 ASCII、复制目录及完成后清理的分支；提升入口进入 COM，不能据这个名字假定每次都由当前进程直接 CreateProcess。下节记录当前客户端及独立保留服务副本的接口、启动、事件与释放；服务副本的最新部署身份和真实运行结果分别保持未验证。

直接进程路径的 `0x79da0` 实际以 `WaitForMultipleObjects` 同时等待进程句柄和 AppsService 的停止 event，无固定超时；只有进程对象先满足时调用 GetExitCodeProcess。停止 event 先满足返回 1，等待／读取失败返回 2。调用方在处理这些返回之前已经排队 bool=true 和当前 exitCode，因此 true 不保证取得了真实退出码；初始 exitCode=-1 也不能自动视为安装器失败码或成功码。必须区分启动、进程实际退出、等待被释放和安装器业务结果。

Electron 的[Windows 适配器](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/simple_service/win/index.js)对普通／提升启动保留 `void(bool,string,int)` 和 `{result,reason,exitCode}`，但 ASCII 启动只声明 `void(bool,string)` 并返回 `{result,reason}`。原生 ASCII 完成包装器 `0x13de0` 实际调用三参数回调，第三项为 exitCode；这条桥接丢失退出码，`$l` 因而不能从该结果识别 3010，除非 manifest 另声明 restartRequired。这里是静态已证实的结果字段丢失，未据此猜测或执行 ABI。适配器用 cbMap 保留回调，收到回调才删除，所读方法没有超时和取消。

### 提升 COM 的客户端、类型库与事件释放

[客户端与安装提权证据](../../../.analysis/results/host-current/installation-elevation-client.json)保留 16 个完整 AST 所有者、99 个客户端原生函数、11 个客户端 PE 数据区域、14 个客户端虚表槽位及逐项字节身份。客户端 DLL 已核对当前官方 `4.0.827` 宿主包；服务端则是另一条来源：[本机保留的 razer_elevation_service.exe](../../../.analysis/inputs/installed-current/files/elevation-service/razer_elevation_service.exe) 由[独立获取收据](../../../.analysis/inputs/installed-current/service-acquisition.json)记录版本 `1.1.0.8`、x64、注册的 `LocalServer32` 来源及签名观察。该 EXE 尚未与最新官方 `4.0.827` 部署物逐字节核对，当前性及部署依赖闭包保持未验证。其 `REGISTRY`、`TYPELIB` 及下述选中代码分别核对了实际 PE 字节；服务副本补充保留 75 个完整 `.pdata` 范围、10 个显式 leaf／thunk 区域、7,409 条指令、10 个数据区域及 4 个真实业务槽位。分析使用 LLVM 静态反汇编，没有服务端的完整 IDA／Hex-Rays 捕获；线性解码和选中范围不代表整个服务或当前官方服务语义已闭合。

客户端 `0x2dc9a/0x2e152/0x2e89a/0x2ed1c` 使用 `CoCreateInstance`、`CLSCTX_LOCAL_SERVER=4`。PE 内实际 CLSID 为 `0edeaf3c-d36e-4e7e-9467-900b977dc4ff`，IID 为 `bfe24d59-6568-4179-8ae5-d9d53869a3e3`；二者与保留服务端副本的类型库及注册收据一致。该类型库 `RzUtilityLib` 的 LIBID 为 `f24a9e29-fc07-4277-9bbe-8315a7735bf0`，其 `IElevator` 方法为：

| 虚表字节偏移／DISPID | 方法 | 类型库参数与客户端使用 |
| --- | --- | --- |
| `+0x38`／1 | `LaunchProcess` | 输入 `BSTR filePath`、`BSTR params`、`UI4 cbId`；输出 `INT* exit_code`。等待版 `0x2e152` 及 ASCII 专用 `0x2ed1c` 的真实反汇编均调用此槽位 |
| `+0x40`／2 | `GetVersionInfo` | 输出 `BSTR* versionInfo`；客户端 `0x2dc9a` 读取后 `SysFreeString` |
| `+0x48`／3 | `CopyRazerFile` | 输入两个 `BSTR` 路径，输出 `BSTR* error`；保留服务副本的真实槽位 `0x3180` 直接返回 `E_NOTIMPL`，不复制、不写输出，不能从声明推定具备文件复制实现 |
| `+0x50`／4 | `LaunchProcessNoWait` | 输入 `BSTR filePath`、`BSTR params`、`UI4 cbId`；`0x2e89a` 实际调用，没有退出码输出参数 |

这些方法的结果类型为 `HRESULT`。等待版导出 `0x13140` 传入 wait=true，沿主队列和 Apps 队列到 `0x29570`，后者分配 job ID、将退出码初始化为 `-1`，再把该变量地址交给 COM。只要 COM 方法返回非负 HRESULT，外层便排队 `result=true` 及该整数；客户端没有另行确认服务端确实写了输出参数、进程已退出或安装器业务成功。不等待版 `0x138a0` 选择 `+0x50`，内部完成结果仍可带初始 `-1`，不能当成真实进程退出码。前端 `simpleLaunchUserAppElevatedJson` 还有 engine `4.0.531` 版本边界；较早宿主走普通进程入口，不能假定所有版本都调用提升接口。

连接器由 AppsService 的 I/O 线程持有；`0x280a0` 复用有效实例或经 `0x2db6c` 构造。`0x760c0` 使用 `CoInitializeEx(NULL,6)`，析构 `0x761e0` 仅在记录的 HRESULT 非负时配对 `CoUninitialize`。连接器销毁先使 weak pointer 失效；线程、COM apartment、服务对象及其事件订阅分别有所有者，不能把一个导出调用当作整个服务的租约。

每次启动方法都在栈上构造 `0x2e5e4` 的 176 字节临时 sink。`0x2e66e/0x2f326 → 0x30956/0x309a1` 读取 COM 类型信息，优先用 `IProvideClassInfo2::GetGUID(1)` 得到默认事件 IID；备用分支经 `IPersist::GetClassID`、coclass 类型信息和 implemented-interface flags=3 找默认 source。`0x30e5c` 再查询 `IConnectionPointContainer`，调用 `FindConnectionPoint`、`Advise`，将 cookie 保存在 sink `+44`。保留副本类型库中的默认 source 确为 `_IElevatorEvents`，IID `c6237515-dd5f-4acc-b6f0-f45569adf965`，与当前客户端 QueryInterface 条件一致。

该事件接口只有 DISPID=1 的 `ElevatorEvent(UI4 eventId,BSTR eventData,UI4 cbId)`；客户端的 `0x30410`、真实 `FUNC_INFO` 和 `DispCallFunc` 对应同一参数顺序。这里 `cbId` 是 job ID，BSTR 是事件文本。`0x30450` 将 eventId=0／1／2 分别交给连接器 `+8/+16/+24`，实际落到 `0x2f3d0/0x2f710/0x2f9f0`，再排队 AppsService 的错误／已启动／已退出事件。未知 eventId 不触发这些事件。退出事件将文本转为整数；`0x79870` 即使解析失败也写出解析器的当前／截断／饱和值，其 bool 被调用方忽略，不能把该消费者描述成严格有效整数校验。

AppsService 随后经 `0x1cc60/0x1ce80/0x1d090` 通知观察者；当前主机虚表的实际目标 `0x9650/0x99d0/0x9cf0` 仅在注册标记及外部 callback 均有效时序列化 `jobID` 和对应字段，再发布原生事件 4／5／6。JS 将其映射为 `onsimplelaunchprocesserror`、`onsimpleprocesslaunched`、`onsimpleprocessexited`，通过 `appsserviceevent` 广播给有效 webContents。这条状态事件链与单次启动的 `{result,reason,exitCode}` 完成回调是不同通道。

COM 方法返回时，无论同步等待还是 NoWait，客户端都调用 `0x2e804`：若 cookie 有效，经 `0x30fcc` 尝试 `Unadvise`，再清 cookie、释放保留的接口和 BSTR。该析构不汇总 Unadvise 的 HRESULT，也没有显式等待任意在途的外部回调；sink 的 AddRef／Release 是恒定计数返回，并未将栈对象转成独立生命周期。**NoWait 的临时订阅并不保留到未来的真实进程退出**；只能记录调用期间实际到达的事件，不能由方法提交成功推断之后仍能收到退出通知。

ASCII 专用 `0x2ed1c` 使用同一等待接口，并让 sink 保存实际选择的 ASCII／复制路径；eventId=0 或 2 时尝试删除该路径，eventId=1 不删除，删除结果未参与成功判断。原有直接启动回退及其清理是另外的分支。当前客户端的等待 COM 方法没有 stop-event 参数、取消 token 或本机超时；`0x79da0` 的直接进程／停止 event 等待机制不能套到 COM RPC 上。设置 AppsService 的停止 event、停止线程或关闭窗口，都没有证明服务端等待被打断、外部进程被终止或其副作用已恢复。

### 保留服务副本的实际启动、事件与停止

以下结论仅适用于上述 `1.1.0.8` 保留副本。构造函数 `0x7ace` 将 `IElevator` 放在对象 `+40`，安装真实虚表 `0x143af0`；业务槽位 `+56/+64/+72/+80` 分别为 `0x6490/0x7510/0x3180/0x7320`。`GetVersionInfo 0x7510` 分配 BSTR 文本 `1.1.0.8` 并交给调用方，分配失败进入已有异常路径；`CopyRazerFile` 的未实现返回与类型库声明必须分别记录。

`LaunchProcess 0x6490` 和 `LaunchProcessNoWait 0x7320` 先做路径／stat 核对；非零结果统一发布 eventId=0 的 `File not found` 并返回 `E_FAIL`，不能把该文本当成所有失败均已准确分类。之后才做签名允许检查，失败发布检查器的错误文本并返回 `E_ACCESSDENIED`，两者均不会启动进程。

签名入口 `0x15464` 按扩展名选择 `.ico` 与普通 PE 两条链，允许名字表的两个字符串为 `Razer USA Ltd.`、`THX LTD.`。普通路径 `0x130cd → 0x12bf8` 使用 `WinVerifyTrust`、通用 Authenticode action、无 UI、VERIFY 状态和 provider flags `0x1000`，取得证书链、复制编码证书后检查解析出的名字；有 provider 数据时先 CLOSE 再比较，provider 为空的错误分支没有这次显式 CLOSE。它遍历返回链中的证书，不能描述成仅比较可执行文件末端签名证书。`.ico` 路径另检查六字节头、条目数、签名数据、重建内容与签名，缺签名、读取失败、重建失败、名字不允许和签名无效有不同结果。选中所有者均保留完整指令；下层证书 principal 解析、ASN.1、摘要与密码算法的全部库分支仍未闭合，不能将字符串命中当作完整签名策略证明。

签名通过后，`0x61e0` 选择枚举到的第一个 `WTSActive` 会话并释放枚举内存。返回 session ID=0 与“没有活动会话／枚举失败”在调用方都会跳过 WTS 启动，这个零值分支不能省略。

| 启动路径 | 实际参数、状态与失败行为 |
| --- | --- |
| `0x6a40` WTS | 查询用户 token，建立继承环境，获取 linked token 并复制为 primary token；desktop 为 `winsta0\default`，当前目录传文件父目录，`CreateProcessAsUserW` 的 applicationName 为空、flags 为 `0x420`。命令行直接从 filePath 复制，只有参数 BSTR 长度至少 2 才拼接空格和参数，单字符参数被省略；任一准备／创建失败返回 `E_FAIL` 后继续 fallback |
| `0x6f8a` fallback | 为 filePath 加双引号并拼非空参数。先调用 `0x41800 → SetCurrentDirectoryW` 改服务进程的当前目录，忽略结果，所读 owner 没有恢复旧目录。`LaunchOptions +0x21=true` 先经 `0x3f2e0 → 0x3efe0` 调 `ShellExecuteExW`，verb 为 `runas`、保留进程句柄；失败后再清该 flag 走普通 `CreateProcessW`。两条分支均未传独立的非空目录参数，不能把服务当前目录修改当成已确认的每任务目录 |

成功创建先发布 eventId=1。等待版 WTS 分支直接 `WaitForSingleObject(INFINITE)`，不检查 wait 返回；退出码先置 0，随后也不检查 `GetExitCodeProcess` 的 BOOL，仍发布 eventId=2、写 out 并返回 `S_OK`。fallback 的 `0x3ee70 → 0x3ee90` 本来只有 wait 返回 `WAIT_OBJECT_0` 且退出码读取成功才写 out，但 `0x6f8a` 忽略这个 helper 的 bool，也可能把初始 0 发布为退出码。因此 **`S_OK + exitCode=0` 不足以证明真实进程已退出并取得退出码**，必须连同 wait／read 原始结果比较。

NoWait 传 `wait=false`，两条服务端路径都跳过等待、退出码写入和 eventId=2，然后关闭所持进程／线程句柄；没有后台退出观察者。该副本自身不会在未来补发 NoWait 的退出事件，加上客户端方法返回时释放临时 sink，不能承诺后续退出通知。关闭句柄也没有终止进程。

事件生产者 `0x667e` 对非空 sink AddRef，按 DISPPARAMS 倒序构造 `UI4 cbId`、`BSTR eventData`、`UI4 eventId`，通过 `IDispatch::Invoke(DISPID=1, LCID=0x400, DISPATCH_METHOD)` 发送，再清 VARIANT、Release。空槽跳过；启动调用方忽略其 Invoke HRESULT，所以 `S_OK` 不证明消费者收到了 started／exited／error。

订阅 `Advise 0x8200` 验证指针、查询 source IID、对 sink QueryInterface，保存取得的接口至首个空槽，将一基槽号交作 cookie；接口不匹配和容量分配失败分别返回对应连接错误，失败时释放取得的接口。`Unadvise 0x83a0` 对有效非空槽先清空再 Release，空槽／无效 cookie 返回 `CONNECT_E_NOCONNECTION`；cookie 可随空槽复用，只在当前订阅生命周期内有意义。对象零引用经 `0x7cc0 → 0x8040 → 0x8dbe` 释放剩余 sink 和向量。这些选中方法没有显式在途回调屏障或向量锁，不能虚构等待全部外部 callback 后才释放的保证。

服务注册与启动也有不同路径：`0x17b2` 依据经过类型与终止 NUL 校验的 AppID `LocalService` 值，选择 SCM dispatch 或本地 COM server；`0x53ea` 的创建参数是 own-process、demand-start、RPCSS 依赖、带引号的模块路径、空 account/password，并配对关闭 SCM／service 句柄。命令解析保留 RegServer／UnregServer／per-user／Service 分支；这些只是静态参数证据，没有执行注册或安装。

`ServiceMain 0x5a90` 建立 COM 后关闭本地 idle-monitor flag；`0x5d32` 在 service 模式初始化 COM security、注册 `CLSCTX_LOCAL_SERVER=4`／`REGCLS=5` 并 resume class objects，临时 class factory 引用由 `0x60a2` 释放。控制处理器 `0x5cc2` 收到 STOP 时设 `STOP_PENDING` 并给服务线程投递 `WM_QUIT`，没有子进程句柄、取消参数或 `TerminateProcess`。消息循环退出后 `0x5f38 → 0x6048` 撤销 class cookies；第一次非 `S_OK` 撤销结果会使循环停止，不能记成无条件全部注销成功。本地 server 的 idle event／monitor 另有独立关闭链。SCM stop、消息退出、RPC 返回、子进程退出和恢复原系统状态必须分别验收。

服务停止由 installer `Ce.stopService` → 主进程 `StopService` → [serviceFunction.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/serviceFunction.js) → simpleLaunchRazerApp → 当前宿主目录的 [RzPowerTool.exe](../../../.analysis/inputs/installed-current/files/app-engine/CommonDLL/RzPowerTool.exe) 完成。文件缺失返回 undefined。helper 的[静态捕获](../../../.analysis/results/native/5d04be921ae8f9f2338fc4b69afbfdb5ac3435dd1d95511bc0c9f82c3f305c72/capture.json)使用 Image Base `0x140000000`；`WinMain 0x36430` 对 `--stop-service` 返回停止 bool 本身作为退出码，即成功为 1、失败为 0，不能套用常规退出码 0 成功规则。前端确实用 `!!exitCode` 转换，但 `Ka` 等待后丢弃这个 bool，仍继续执行安装器。

PowerTool 的 `0x356e0` 只接受固定服务名；`0x36360` 打开 SCM，`0x36290` 打开服务并先停止活动依赖，随后 `0x35eb0` 查询／请求停止并核对 SERVICE_STOPPED。已有 stopped 返回 true；pending 等待按 waitHint 取 1–10 秒间隔，停止请求后的等待直接采用 waitHint，30 秒边界只在等待后检查，不是严格的单次调用时限。递归依赖停止没有逐项合并 bool，父函数也不检查依赖停止结果；部分成功日志先于／独立于真实返回，故日志不能代替状态核对。正常失败返回关闭服务／SCM 句柄，依赖枚举缓冲区也释放；这不恢复停止前的服务状态。

完整 `Ka`、`$l`、取消回调、正常结束及异常关闭消费者均没有对应 `StartService` 或初始服务状态保存，因此不能声称安装失败／取消会恢复被停止的服务及其依赖。产品的可恢复写入对照仍必须先记录真实原值并恢复，当前静态链没有为执行官方安装器提供授权。

### 当前官方升级工具的不同实现

同一[证据报告](../../../.analysis/results/host-current/installation-elevation-client.json)另保留两套 PowerTool 的 21 个完整 IDA 函数及逐指令／chunk 字节校验。当前宿主包选中的无版本 helper 与后台资源清单声明的 `1.0.18.0` helper 是不同输入，升级动作不能按同名工具合并。

当前宿主 `WinMain 0x36430` 的 `--upgrade-elevation-service <filename>` 进入 `0x36dd0`，但忽略其结果，正常继续返回初始退出码 0。升级 owner 在固定 elevation-service 目录组合源文件与 `razer_elevation_service.exe` 目标；源存在时请求停服务，却不检查 stop bool，再进入最多 5 次、失败 false 之间间隔 500 ms 的复制。复制错误异常与 false 重试是不同分支，不能一概声称异常也会重试。无论是否复制 EXE，随后都会调用 `0x37570` 检查客户端 DLL；EXE 复制成功才删源文件，复制成功或目标不存在时再尝试删除升级计划任务。

`0x37570` 读取并校验实际服务与当前 `simple_service.dll` 的四段文件版本，读取失败或当前 DLL 不存在便停止；编译数据和构造函数 `0x1010/0x1040` 证明以下选择：

| 服务版本 | 客户端版本条件与文件来源 |
| --- | --- |
| 恰好 `1.1.0.7` | 必须精确为 `1.2.2.9`；来源 `simple_service_v1.2.2.9.dll`。降级前先将 outgoing DLL 备份为 `simple_service_new.dll` |
| 其他版本 | 四段版本至少为 `1.2.2.10` 时不动；否则从 `simple_service_new.dll` 恢复。复制成功才删除已消费 backup |

这些文件位于 helper 自己的目录。DLL 复制重试仍失败后，原件会枚举 `RazerAppEngine.exe`、打开并终止匹配进程、逐个等待至多 5 秒并关闭句柄，随后再复制；没有完整原始进程／服务状态恢复。任务删除 `0x36b00` 使用固定系统 schtasks 路径，等 30 秒，超时请求终止这个任务删除进程，其他 wait 结果仍尝试读退出码且忽略 BOOL；结果只写日志，不能汇总成可靠升级成功。

版本化 `1.0.18.0` 的 `WinMain 0x36200 → 0x368d0` 则直接做存在检查、复制、删源、system 任务删除；该 owner 没有停服务、复制重试、上述客户端版本选择、备份／恢复或杀宿主进程分支，正常返回 1 也被 WinMain 丢弃。两套 helper 均未执行；外层退出码、日志和文件版本分支都不能证明实际安装、替换或恢复成功，更不能由此证明本机服务副本已匹配当前官方部署。

## 共享依赖、卸载和取消

共享资源有 usedBy 所有者。更新或卸载时仍被其他产品引用的文件只移除当前 owner，不能直接删除；不再共享的资源先查实际本机文件，再以 app/name/sha 删除。删除失败进入 pending-remove，后续仅在删除成功或确认文件已不存在时清除；其他错误继续保留。

产品卸载要求当前安装版本与请求版本一致。驱动分支先用真实 `getDriverInfo` 确认 isInstalled 和版本，再运行对应本地卸载路径；仅修改 installedResources 不能代替驱动卸载。部分原版调用方只记录驱动失败警告，必须保留真实返回，不能把最终记录删除当作系统组件已移除。

取消频道名为 `installation_<type>_<id>`。`install-cancel` 回调立即写 CANCELED、设置 isCancelling，再调用 `$r`；后者只收集当前 We 中的 fetch AbortController，等待 abort signal 回调计数达到控制器数后删除注册项。这个 Promise 没有等待响应流消费、文件写入、资源记录、进程退出或服务恢复。已经 abort 的控制器计入完成，没有活动控制器时直接 resolve；日志中的 All downloaders cancelled 只对应此信号阶段。

`Xl` 下载完成后检查取消；进入 `yl` 后直到全部 priority 处理结束，`Xl` 才再检查。取消在安装期间到达时，原件可能继续启动后续桶、写 installedResources／completedResources、更新单项记录和删除安装文件，最后抛 USER_CANCELLED；最终产品版本发布可以因此没有执行，但之前的副作用没有回滚。不能描述成“取消会阻止所有尚未开始的安装动作”。`yl` 在入口检查 flag 之前先写 INSTALLING，也会出现 CANCELED 被后续状态覆盖的时间窗口。

正常结束 `gn` 依次清 INSTALL_RESOURCES／瞬态状态、清该 manifest 的浏览器缓存、释放锁、等待 service worker offlineReady，最后延时关闭。缓存的 forEach 调用 cache.delete 没有 await 每个删除，锁释放前也没有加入外部进程确认；offlineReady 等待最多 60 轮各 1 秒，未读取取消标记。正常流程先执行 En 的 pending-remove 和 Tl 的未使用记录清理；En 对缺响应／其他错误保留 pending，仅成功或明确文件不存在才移除，Tl catch 返回错误对象却没有调用 handleError，调用方也未检查该返回值。

`unhandledrejection` 的生产关闭分支只在延时后移除五类进度／错误状态、释放锁并关窗，没有调用完整 `gn`、En 或 Tl，未清 INSTALL_RESOURCES，也没有启动服务、恢复文件或结束外部进程。`beforeunload` 仅发起 `$r().then(...)`；既没有阻止卸载，也没有证据证明宿主等待其异步完成。`navigator.locks` 的辅助函数保留 resolver 维持租约，但成功回调调用工作函数时不 await／返回它；请求与实际工作异常不构成同一个 Promise 完成链。

原生 SimpleService shutdown 与此窗口取消也是两条链。`0xf090` 在停队列之前清全局初始化标记；完成包装器 `0x12060/0x12f20/0x13de0` 只有标记仍真才调用外部回调，因此停机中的迟到结果可能不再回到 JS。shutdown `0x19c0 → 0x1cf0 → 0xb270 → 0x218ac/0x21a40` 释放 AppsService；析构 `0x26a9c` 设置上述停止 event、清 COM 引用、停止 I/O 线程并关闭 event。该释放 event 可使等待退出码提前结束，但所读析构没有 TerminateProcess。不能把线程释放／cbMap 删除或窗口已关当作外部安装器已经退出，更不能由此推断服务状态恢复。

本项目的目录库、缓存、安装文件名和清理边界统一来自 [installation.json](../../../crates/razer-resource-db/installation.json)。普通用户必须能写入安装缓存；卸载只清本程序的安装/缓存目录并保留用户配置语义。原版更广范围的历史文件扫描、驱动/产品目录清理不能直接照搬为本项目删除权限。独立 SQLite 目录库随安装发布为单独文件，必要核心图标离线可用；缺失目录库且无有效缓存时应显式失败。

## 验证边界

安装扩展重新校验 installer 原始脚本实际 bytes／SHA-256 和 tree-sitter 完整解析；主机 main、serviceFunction、SimpleService 适配器对照 ASAR 提取收据核对字节。上述无版本 DLL、版本化 DLL 和 PowerTool 的实际原件分别核对各自捕获身份；所读函数 JSON 按函数清单核对 hash，虚表从原始 PE 读取。无版本与版本化 SimpleService 的 selected owners 虽多数相同，提升调用的函数边界定位仍不同，本文主链 RVA 以当前宿主选中的无版本捕获为准。

| 安装前端所有者 | UTF-8 半开字节区间 |
| --- | --- |
| fetch abort 与取消消费者 | `$r [92243,92609)`；频道初始化／onmessage `[109376,110022)`／`[109645,109906)` |
| 服务调用与进程桥接 | stopService `[65417,65711)`；普通 JSON `[97868,98282)`；提升 JSON `[107581,108346)`；`Ka [137252,139690)` |
| 锁、执行与单项提交 | `wn [142461,143032)`；`yl [181450,182795)`；`$l [183691,184260)` |
| 清理与结束 | `Tl [187465,188075)`；`En [190014,190510)`；`gn [191612,191912)`；`Fl/bl/Ul [191913,192573)` |
| 最终消费者与异常 | `Xl [200311,202973)`；beforeunload `[203065,203199)`；unhandledrejection `[203204,203785)` |

全功能测试要求见[验证规则](../../../.agents/rules/verification.md)。下列为独立 Rust 实现需要覆盖的可测试条件，尚未执行或据其生成通过记录：

| 条件 | 必须比较的输出与副作用 |
| --- | --- |
| 下载前、两桶之间、进程等待中取消 | flag／状态次序、fetch abort、实际启动动作序列、已写资源项、最终产品记录和清理完成分别比较 |
| 文件发布成功后进程失败／记录失败 | 文件实际字节、真实 result/reason/exitCode、记录部分提交、keep 分支及 pending-remove；不以最终窗口状态代替 |
| 普通／提升／ASCII 返回 0、3010、其他值或无回调 | 原生返回、桥接字段丢失、restartRequired、超时／取消及 callback 生命周期；3010 的缺失字段保持未观测 |
| 服务不存在、拒绝停止、依赖失败、pending 超时 | helper 实际退出码 0／1、真实服务状态、Ka 是否继续、原值与依赖是否恢复；日志不作为独立期望 |
| 关窗与 shutdown 竞争 | 原生初始化标记、迟到回调、等待 event、句柄关闭和仍存活外部进程各自记录，不把静默回调当成功 |
| 等待／NoWait 的 COM 边界 | 实际槽位、BSTR 所有权、job ID、HRESULT 与 out 退出码分别核对；没有写输出、仍为 `-1`、0、3010 及业务失败分别覆盖 |
| 事件订阅及注册失败 | 类型信息／source IID／FindConnectionPoint／Advise 失败时不得继续启动；cookie、接口释放、Unadvise 失败、晚到回调和 NoWait 提前释放分别比较 |
| COM 事件与 ASCII 清理 | DISPID、eventId、cbId、错误及退出码文本、未知／非法／溢出输入、weak pointer 失效及 eventId=0／2 的复制文件删除副作用分别核对 |
| 保留服务副本的方法结果 | CopyRazerFile 的 `E_NOTIMPL` 与未改 out sentinel、版本 BSTR 所有权、路径核对／签名拒绝与未启动副作用分别验证；当前官方服务尚未绑定时不提升为当前官方实测 |
| 服务的 WTS／fallback／等待 | session 0／非零、token／环境失败、含空格路径、参数长度 0／1／2、服务当前目录修改与未恢复、runas 失败后的普通启动、wait／exit-code 失败仍发 0 各自比较 |
| 服务订阅与停止 | cookie 复用、QI／分配／Invoke／Unadvise 失败、空槽、析构 Release、STOP 期间阻塞 RPC 和继续存活的子进程分别记录；NoWait 的未来退出事件应保持未产生 |
| 两套升级工具 | 版本 1.1.0.7／其他、DLL 精确／最低版本、缺源／缺目标、stop false、复制 false／异常／五次边界、备份／删除、杀进程和任务删除分别用纯／模拟接口验证，不执行 helper 充作可恢复 DLL 对照 |
| 正常收尾、全局异常、cache.delete 延迟／失败 | INSTALL_RESOURCES、五类瞬态键、pending 清理、缓存、锁、service worker 等待及关窗顺序分别验证 |

无设备时可先用纯逻辑和模拟文件／传输／进程／服务接口验证上述状态机与失败注入；回放的期望须来自已核实的原件。纯逻辑、模拟验证、当前官方实际 DLL 对照与真实设备／系统验收各有独立状态，不能以两端都未连接判为一致。安装器执行、主程序运行和固件刷写不在本轮授权边界内，不能为了对照擅自执行。

已静态核对本机安装文件链、固件准备／调度／事件消费者、当前客户端 COM／直接进程／停止链、保留服务副本的选中启动／事件／订阅／释放代码及两套升级工具差异。服务 EXE 的最新官方部署字节与依赖闭包仍未核实；当前产品清单和宿主包 ZIP 已留目录核对，该 ZIP 没有服务 EXE，不能据此认定外层安装或另行下载依赖不存在。完整服务捕获、下层签名／密码库、固件协议、其他平台和全部产品分支仍需继续。报告保留原始分析快照，并另外核对并行获取结束后的当前输入索引及全部不可变来源。完整捕获、包装器初始化和文件写入都不能证明独立 Rust 实现完成。本文没有运行应用、厂商 JavaScript、安装器、helper、DLL、服务或设备，也没有执行本节纯／模拟测试或实际 DLL 功能对照；这些验证及固件刷写、安装运行验收保持未验证。
