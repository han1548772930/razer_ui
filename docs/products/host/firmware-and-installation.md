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

主队列缺失返回 `Invalid thread`；Apps 线程或实例缺失分别有独立失败，进程回调可带 exitCode=-1。Apps 实际工作先确认目录、文件及文件校验，再进入本机启动或已识别的 elevation COM 分支。普通进程的等待标志为真，ASCII 路径另包含是否已为 ASCII、复制目录及完成后清理的分支；提升入口进入 COM，不能据这个名字假定每次都由当前进程直接 CreateProcess。COM 的完整接口、服务端任务与退出确认仍需继续连读，本段不把连接 COM 成功提升为安装成功。

直接进程路径的 `0x79da0` 实际以 `WaitForMultipleObjects` 同时等待进程句柄和 AppsService 的停止 event，无固定超时；只有进程对象先满足时调用 GetExitCodeProcess。停止 event 先满足返回 1，等待／读取失败返回 2。调用方在处理这些返回之前已经排队 bool=true 和当前 exitCode，因此 true 不保证取得了真实退出码；初始 exitCode=-1 也不能自动视为安装器失败码或成功码。必须区分启动、进程实际退出、等待被释放和安装器业务结果。

Electron 的[Windows 适配器](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/simple_service/win/index.js)对普通／提升启动保留 `void(bool,string,int)` 和 `{result,reason,exitCode}`，但 ASCII 启动只声明 `void(bool,string)` 并返回 `{result,reason}`。原生 ASCII 完成包装器 `0x13de0` 实际调用三参数回调，第三项为 exitCode；这条桥接丢失退出码，`$l` 因而不能从该结果识别 3010，除非 manifest 另声明 restartRequired。这里是静态已证实的结果字段丢失，未据此猜测或执行 ABI。适配器用 cbMap 保留回调，收到回调才删除，所读方法没有超时和取消。

服务停止由 installer `Ce.stopService` → 主进程 `StopService` → [serviceFunction.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/serviceFunction.js) → simpleLaunchRazerApp → 当前宿主目录的 [RzPowerTool.exe](../../../.analysis/inputs/installed-current/files/app-engine/CommonDLL/RzPowerTool.exe) 完成。文件缺失返回 undefined。helper 的[静态捕获](../../../.analysis/results/native/5d04be921ae8f9f2338fc4b69afbfdb5ac3435dd1d95511bc0c9f82c3f305c72/capture.json)使用 Image Base `0x140000000`；`WinMain 0x36430` 对 `--stop-service` 返回停止 bool 本身作为退出码，即成功为 1、失败为 0，不能套用常规退出码 0 成功规则。前端确实用 `!!exitCode` 转换，但 `Ka` 等待后丢弃这个 bool，仍继续执行安装器。

PowerTool 的 `0x356e0` 只接受固定服务名；`0x36360` 打开 SCM，`0x36290` 打开服务并先停止活动依赖，随后 `0x35eb0` 查询／请求停止并核对 SERVICE_STOPPED。已有 stopped 返回 true；pending 等待按 waitHint 取 1–10 秒间隔，停止请求后的等待直接采用 waitHint，30 秒边界只在等待后检查，不是严格的单次调用时限。递归依赖停止没有逐项合并 bool，父函数也不检查依赖停止结果；部分成功日志先于／独立于真实返回，故日志不能代替状态核对。正常失败返回关闭服务／SCM 句柄，依赖枚举缓冲区也释放；这不恢复停止前的服务状态。

完整 `Ka`、`$l`、取消回调、正常结束及异常关闭消费者均没有对应 `StartService` 或初始服务状态保存，因此不能声称安装失败／取消会恢复被停止的服务及其依赖。产品的可恢复写入对照仍必须先记录真实原值并恢复，当前静态链没有为执行官方安装器提供授权。

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
| 正常收尾、全局异常、cache.delete 延迟／失败 | INSTALL_RESOURCES、五类瞬态键、pending 清理、缓存、锁、service worker 等待及关窗顺序分别验证 |

无设备时可先用纯逻辑和模拟文件／传输／进程／服务接口验证上述状态机与失败注入；回放的期望须来自已核实的原件。纯逻辑、模拟验证、当前官方实际 DLL 对照与真实设备／系统验收各有独立状态，不能以两端都未连接判为一致。安装器执行、主程序运行和固件刷写不在本轮授权边界内，不能为了对照擅自执行。

已静态核对本机安装文件链、固件准备／调度／事件消费者及上述原生捕获，进一步闭合取消、进程等待、服务停止和释放的主要消费者。完整 COM 服务端、下层固件协议、其他平台和全部产品分支仍需分别核对；完整捕获、包装器初始化和文件写入都不能证明独立 Rust 固件协议或安装生命周期完成。本文没有运行应用、厂商 JavaScript、安装器、DLL、服务或设备；固件刷写与安装运行验收保持未验证。
