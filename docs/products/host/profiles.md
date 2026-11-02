# 全局 Profiles、游戏扫描与本机关联

全局 Profiles 管理本机游戏列表、设备配置档与游戏的关联，以及配置档命令弹窗。设备摘要、关联记录、选中项和设备实际激活的配置档是不同数据：在设备卡片里选择配置档，只改变关联目标；新建、删除或导入则发给相应设备中间件，由其持久化和设备任务继续处理。

当前已接续核验 Windows 的 Add App 枚举／普通图标、游戏列表／封面 Alt 图标、GOG 扫描到前端列表，以及全部 22 个实际注册扫描策略的所选方法版本差异。相关专题核对 Host 4.0.896 与当前稳定前端，GOG 及其直接共享扫描链、已选策略方法采用 GMS 3.15.0.1200；详见下方专题及[正式收据](../../../.analysis/results/host-current/local-audit.json)。注册策略版本清单已复核不代表所有消费者或整个 Profiles 语义完成；其余 GMS 与设备配置档段落仍对应注明版本的既有证据，Rust 独立实现、UI／后端接线和实际 DLL 对照分别保持未验证。

## 页面与设备来源

独立入口来自 `synapse/profiles`。完整窗口挂载模块 43，内嵌窗口挂载模块 8120；完整窗口标题为 `LINKED_GAMES`，导航依次为 `GAMES_HEADER` 和 `DEVICE`。导航保存到 sessionStorage，切换、返回和前进使用映射草稿的离开确认；首次挂载追加当前导航记录。

页面从 window-storage 的 `connectedDevices` 取得设备摘要，保留 setupStatus=ready、有序列号、有名称且 powerStatus 不为 off 的项。profiles 的 GUID 转为列表使用的 id。产品图片另按 edition、layout、sidepad 和 wristrest 选择，不以图像加载结果判断设备存在。

游戏详情和 Devices 页匹配 PID，并优先使用序列号；旧记录缺少序列号时才检查 container。IOT 复合设备使用 `isShownOnUI` 子设备。普通设备只有同时具有 activeProfile 与 profiles 才挂载配置档卡。已存关联的 `isVisible` 更新却只按 PID，与卡片的 PID＋序列号匹配粒度不同，不能将其归并成统一身份算法。

## 游戏扫描的真实消费者

游戏列表桥接器挂载后先发扫描请求，再订阅 memory-storage，最后读取 `gms-data` 快照。完整窗口向 `gms-api-request` 写 `{action:"scanGame"}`；名称符合 `profiles-\d+` 的内嵌窗口不重复扫描。请求发布不是扫描完成，页面的 isSyncing 由请求和后续游戏列表分别更新。

本机 `gms-proxy` 订阅该键，消费 addGame、deleteGame、hideGame、unhideGame、scanGame、setCustomGameCover、removeCustomGameCover。扫描在未连接时只设置 `hasPendingScan`；连接成功后按本地列表、用户添加列表、当前运行游戏的顺序恢复，再根据空列表／尚未扫描或 pending 条件启动扫描。

代理通过当前服务信息取得本机端口，连接 `127.0.0.1:<port>/GameManagerServiceHub`。SDK 的本地／用户列表分别流式读取 `GetLocalGameListPartial` 和 `GetUserGameListPartial`，累计每段 gameSignatureList；流错误拒绝整个调用。设置列表另调用 `GetGameSettingsList`，按 INFO_EntryGUID 合并到对应游戏，排除空 GUID。这里是本机服务协议边界，不是浏览器自行扫描安装目录。

StartScan 使用互斥锁，分别拒绝未初始化和重复扫描。扫描结束事件重新读取两类列表、发布 `gms-data`、刷新运行游戏，再处理扫描中排队的添加动作。列表／设置事件在扫描或清理中有忽略条件，其他更新根据签名和所属本地分区合并；不能把某次请求的成功写入 memory-storage 当成服务结果。

## 本机服务的连接身份与分段返回

当前服务端不仅检查 loopback。`GameManagerServiceHub.OnConnectedAsync` 先取得 HTTP 连接的远端 TCP 端口，再据 TCP 表找调用进程 PID；随后核对该进程主模块的 Authenticode 信任结果和签名证书 Subject 中的 `Razer` 字符串，最后检查远端地址是否为 loopback。无 HTTP context、PID 无效、签名失败或非 loopback 都会 Abort。签名核对实际调用系统 WinVerifyTrust，并非比较文件名或 DLL 版本。

因此同样的 SignalR 端口、方法名和报文字段不足以证明独立 Rust 客户端能复用已安装服务。服务是已识别的本机协议证据，其实际连接能力仍受上述身份条件约束；本项目需要独立实现对应本机功能，不能把绕过签名校验作为迁移路径。静态发现该门禁也不代表本轮执行过服务连接测试。

连接后的 `ConfigSetUniqueUserIdWithClientVersion` 建立 ConnectionId 到 client instance 的映射，记录 appName、分区 ID、clientVersion 和 session number，并配置 Windows SID。空分区 ID 使用服务的当前本地 ID；ID 先经路径穿越修正。SID 从 impersonation 身份取得，失败时使用服务当前身份，再检查 HKEY_USERS 中是否存在。这里的分区 ID、Windows SID 和设备身份是不同对象，不要求本项目保留网络账户流程。

`GetLocalGameListPartial` 和 `GetUserGameListPartial` 每次最多返回 16 项，TotalSize 为完整列表长度；do/while 使合法空列表也返回一段空数据。两者只在建立 partialList 时写入传入的 Offset，循环只推进局部 offset 和 SignatureList，后续段仍携带初始 Offset。SDK 的累计消费者与服务广播事件中的 offset 算法必须分别测试，不能据名字统一成常规分页。

这两个流在 client instance 缺失时先 yield null，却没有 yield break，之后仍访问 clientInstance.Id；列表读取异常则被 catch 并继续以初始空列表构造返回。它们既不能证明“未配置”是合法空列表，也不能证明空结果必然是磁盘中确实没有游戏。读取链没有接受显式 CancellationToken，Hub 断开只移除 ConnectionId 映射，没有在该方法中停止全局扫描任务。

## 服务端列表、隐藏与封面写入

服务的本地持久化按本机分区和 MachineId 生成 UserGameListEnc、LocalGameListEnc、GameSettingsListEnc XML 文件；UGL 的部分路径另外写无 Enc 的 XML。文件、服务内存字典、浏览器 IDB 与页面草稿互不等价。`GetUserGameListCached` 缓存缺失时调用文件读取；后者默认检查每项 LaunchExe 是否存在，移除缺失项并重写文件，再发列表事件。因此这一 getter 可能带真实磁盘修改，不能把所有查询入口一概登记为只读对照测试。

手动添加的主要 `AddGameToUserGameList` 路径先拒绝扫描中，再核对目标文件存在及本地／用户列表重复项；匹配范围包括 LaunchExe、URL、默认 exe、运行 exe、launch helper 和备选 exe。它结合本机已有 master list 归一化签名、GUID、架构路径与图标；未匹配项根据来源信息生成 GUID，并设置 MachineID、DetectingMethod 和 LastInteractTime。新增 GameSettings 会单独调用设置写入，其返回码没有与 UGL 保存组成共同事务。

随后先向内存 UGL 追加条目，再序列化加密 XML；通知标记在序列化前就设为真，故本层捕获保存错误并返回 GS_FAILED 后仍可能发送列表更新。更外层 catch 只记日志，没有统一把初始 GS_SUCCESS 改为失败。添加调用返回、列表事件、服务内存及实际文件必须分别核对，不能因任一处出现条目便判定整个动作成功。

`RemoveGameInUserGameList` 在非扫描状态按 EntryGUID 从内存列表移除，依次调用加密和普通 XML 写入，解锁后才通知；没有在文件失败时把内存条目加回。Hub 后续尝试清理设置时，却重新从已经移除后的列表查同 GUID，并据查找结果构造 GameInfo；未找到时得到空 GameInfo，设置删除返回信息不足且结果未纳入 UGL 返回码。因此 UGL 删除成功不等于相关 GameSettings 已完整清理。

隐藏动作把 ATT_IsHidden 传入 `SetGameSettings(HIDDEN_STATE)`。封面动作先在本地分区的 CustomGameCover 目录按 EntryGUID 文件名复制／删除文件，再写 ATT_CustomGameCover。文件不存在的检查只记录日志，实际错误由 File.Copy 捕获；文件操作已经完成后，设置写入仍可能因扫描中或其他错误失败，完整方法没有恢复原文件。文件副作用与设置结果不能当成一次原子提交。

`SetGameSettings` 先校验身份字段、拒绝扫描中，在写锁内读现有加密设置文件，匹配或新增 GameSettings，再仅修改指定属性并调用加密序列化。异常置 GS_FAILED，finally 释放锁；之后只要 notifySettingsChanged 为真便发送设置事件，未按 result 或实际写入成功过滤。signal 参数在该完整方法体中没有控制此通知。事件经列表管理器转给 Broadcaster，再发 `Clients.All.OnGameSettingsListUpdatedEvent`，不是向请求方返回持久化收据。

还需注意下层实际文件序列化器的行为：CryptoXmlSerializer 和 XmlSerializer 都按 DataContractSerializer 写文件，最多重试 10 次 IOException、每次等待 250 ms；耗尽后 void 方法正常返回。写后尝试独占打开文件的核对也会重试，但耗尽没有独立失败返回；非 IO 异常才重新抛出。写入直接创建目标文件，没有跨 UGL／GSL／封面文件的事务或原值回滚。故即使上层返回 GS_SUCCESS，仍需对实际文件和后续读回做比较，测试应包含文件占用、不可写、写入中断与部分副作用，不能只对比枚举返回值。

### 加密设置文件的读取、重试与释放

扫描结束后的设置保存实际使用 [IoUtil.CreateAes](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/IoUtil.cs) 和 [CryptoXmlSerializer](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/CryptoXmlSerializer.cs)。`CreateAes 0x0600091e@0x2d53d/60` 调外部 Aes.Create，显式设 BlockSize=128、KeySize=256，再从本类静态字段解码 Key／IV；`.cctor 0x06000924@0x2d938/43` 中的两个 #US 字符串已从原元数据核对，解码长度分别为 IV 16 B、Key 32 B。这个工厂没有设置 Mode／Padding，也没有本地 catch；其实际 provider 与框架默认行为属于外部依赖，不能只凭函数名声称全部密码实现和所有运行时分支已核。这里保留的是厂商文件兼容语义，不将这两个字段用作设备或用户身份。

文件反序列化主体 `0x06000377@0x8ab8/386` 先 File.Exists，再最多尝试十次。每次创建 DataContractSerializer，以 Open／Read 打开文件，依次包 CryptoStream(Read)、StreamReader 和 XmlReader；finally 按反方向释放这四层对象，全部释放结束后才置完成标志。仅 IOException 在内层记录后 Sleep(250) 并重试，包括第十次失败后仍有这次等待；其他异常进入外层 Exception catch 并返回 null，文件不存在也返回 null。这里的 htmlDecode 参数没有读取消费者。缓存加载 `0x060000ca@0x641c/202` 在读锁内调用此链，若返回 null，后续访问 SettingsList 会异常，catch 再将该分区缓存替换为空 GameSettingsList；文件不存在则直接新建空列表。两条空缓存来源与磁盘真实为空并不等价，此处也没有证明设备无配置，不能据此覆盖设备设置。

文件序列化主体 `0x0600037d@0x8f98/345` 在重试 try 之外先构造 DataContractSerializer，再用 FileMode.Create 直接打开目标；`IL_001c/001d` 的参数与原 PE 字节一致。XmlWriter → CryptoStream → FileStream 依次通过 finally 释放，随后才开始另一个最多十次的独占打开探测：Open／ReadWrite／FileShare.None，立即关闭；失败只等 250 ms。这个探测没有读取、比较或同步文件内容，十次耗尽后仍在 `IL_00bc` 无条件置外层完成标志。外层 IOException 最多重写十次，耗尽正常 void 返回；非 IO 异常记录后 rethrow，DataContractSerializer 构造异常则根本不进入重试。写入前已截断目标、关闭／探测失败、文件部分更新和设置通知可以分别发生，没有临时文件交换、备份恢复或耐久性确认。

字符串入口是另一份契约：加密 Serialize `0x0600037e@0x9158/252` 先通过 StringWriter 生成 XML，UTF-8 编码后加密，显式 FlushFinalBlock，再对密文 ToBase64String；其 serializer 构造在 try 外，主体 Exception catch 返回空字符串。反向 DeserializeString `0x06000379@0x8cec/331` 却直接 UTF8.GetBytes(data) 后解密，没有 FromBase64String；htmlDecode 为真时才对解出的 XML 做两次 amp 替换及 HtmlDecode。不能把这两个公开方法假定为可直接互逆的一对，也不能把字符串、字节和文件入口统一成同一种错误或编码规则。所选完整方法和 EH 均保留在 [Common 完整 IL](../../../.analysis/results/host-current/managed/e07813af888b/il.stdout.txt)；GSL 的读取／写入消费者在 [Lib 完整 IL](../../../.analysis/results/host-current/managed/955d4bbd372f/il.stdout.txt)。

本节独立核 Lib 5 个、Common 10 个所选 MethodDef 身份／方法头及原 EH sections，包含旧集合复用；try／handler 范围、catch 类型、关键文件参数及无条件完成字节另读原 PE，不把方法头核验扩写成全 IL 独立解码。行为测试须覆盖目标缺失、坏密文／XML、持续占用、最后一次 IO 失败、Dispose 抛错、探测全失败但序列化正常返回、非 IO 中断、空缓存重建，以及字符串 Base64 与原始 UTF-8 输入差异；逐项比较官方 DLL 实际结果、原文件字节、缓存及事件。当前没有执行程序集或行为测试，框架加密／XML 内部、全部数据契约与实际落盘／恢复仍未验证。

## 扫描调度、停止与发布次序

服务的 stopped／scanning 两态由独立 state 对象分派：scanning 时再次 StartScan 返回 GS_SCAN_IN_PROGRESS，stopped 时 StopScan 返回 GS_SCAN_IDLE。Controller 启动前读取本地／用户列表并检查本机 master list；缺失或空时发 scanCompleted=false、设 stopped 并返回对应错误。正常路径先切到 scanning，把 stopScanFlag 清零，再排队执行 OnStartScan，立即返回 GS_SUCCESS；这只确认任务受理。

OnStartScan 建立按 GUID 去重的输入集、当前扫描字典及设置缓存。排除 Immediate 优先级后，按优先级降序处理：同组策略并行运行，逐组 `Task.WaitAll`，每组最长 5 分钟。每个策略有独立 CancellationTokenSource，取消回调调用策略 StopScan，GameFound 事件填入 CurrentScanningGames 并通知消费者。WaitAll 的超时布尔结果未检查，所以等到超时不等于全部策略已经退出。

Controller 在策略分组和等待期间持有 cancellationTokens 的锁。StopScan 先把 stopScanFlag 设为真，再获取同一锁来 Cancel 所有已登记 token，随后直接设 stopped 并发 scanCompleted=false，没有 join 各策略任务。锁竞争可能延后取消回调；停止回包或停止事件不能证明全部旧任务已经释放。token 注册句柄没有保留，清理列表时也没有逐一 Dispose，因此本轮静态链不能作为可靠任务取消的实现证明。

常规扫描尾部先发布 `NotifyScanFinished(list, true)`，之后才设 stopped、修正并保存 GSL、保存 LGL、刷新运行游戏集合，再清空 token 列表、重置各策略停止标志、清理映射／当前集合和缓存。通知消费者时最终持久化尚未执行；尾部没有以 stopScanFlag 把 true 改成 false。因而停止后的在途任务和正常完成事件需要按实际顺序区分，不能把任意 finished 事件升级为本次扫描完全落盘／清理完成。

Broadcaster 对列表和扫描结束事件另外生成每段 16 项的新对象，并推进 Offset，空列表也发一段；这些字段与上述 Hub 流复用 partialList 的行为不同。Broadcaster 的事件处理是 async void，发送失败记录日志；Hub 的请求返回不会等待全部广播完成。初始化／析构存在成对事件注册和解绑调用，但不能据其存在推断客户端断开时已经停止扫描或释放所有策略。

## 列表合并、图标与重定位

页面收到 `gms-data` 后分别处理两类列表，过滤 `ATT_IsHidden`；本地隐藏项进入 removedGames。用户添加项缺 gameSettings 时，尝试沿用相同 id 的旧条目。图标通过本机应用图标接口读取，列表图标为 80×80，自定义封面为 300×150。

桥接器为每次数据处理递增代次，只有最新代次进入最终 `addNewGames`；已经开始的图标请求和内部数组更新仍可能继续，这不是底层取消。合并按游戏 id 维护已有封面；来源列表缺失项可标为 isMissing，同 id 具有真实项时移除 missing 副本。不同事件 action 对缺失项保留方式不同，不能简单把一次缺失等同删除。

重定位通过本机文件选择器取得 exe／url，UI 后续只接受匹配 `.exe` 的首项；若该 location 已在列表中，显示重定位冲突提示。新旧游戏替换时迁移 linkedGames 的 gameID、更新选中项并清空重定位字段。最终列表用临时 `Profile_GameListUpdates` BroadcastChannel 发布，发送后关闭频道。

重定位的选择器虽然允许多选，消费者只检查第一项；正则是区分大小写且未锚定结尾的 `/.*\.exe/`，冲突检查则是 location 字符串完全相等。选择后调用的 `updateRelocateGamePath` 只向 reducer 保存旧 id 和新 path，再关闭详情；完整 action 没有添加请求、服务调用或设备写入。不能把文件对话框关闭登记为游戏已经重新定位。

后续 `_replacingGame` 只在另一次目录合并中，从本次新增 id 的集合寻找完全相同的 path。旧项和新项同时存在时，无论有无关联记录都会移除旧列表项；只有找到旧 gameID 的关联记录，才迁移 gameID、更新选中项并清空 relocation id/path。没有关联的分支会保留上述 relocation 字段。因此这里是依赖后续目录事件的替换条件，不是具有独立返回值、失败恢复和取消句柄的重定位事务。

Games 页支持全部、具可见关联设备、已移除三种过滤；搜索使用 trim 后的小写名称。排序分别为名称升／降序、`ATT_UserLastInteractTime` 和 `ATT_UserTotalGameplayCount`。已移除页点击项目打开 Add App；常规页点击项目打开游戏详情。

## 添加、删除与封面

Add App 调用本机已安装应用列表和图标接口，按 name＋path 去重，展示时按大写名称排序；搜索不区分大小写。浏览支持平台对应的 app／exe 及 url，拖放路径也进入同一 addGame action。动作发送 `{action:"addGame",payload:{filePath}}`，页面不凭请求生成一条游戏记录。

安装程序枚举与图标提取在当前 Windows 主机实际走 SysUtils 的异步 FFI，返回字符串指针经 `readCString` 复制，成功识别为指针字符串后在 finally 调用 `FreeMalloc`。文件选择则直接走 Electron `dialog.showOpenDialog`，没有在这条完整主机分支调用业务 DLL。原件同时保留非 Electron 的 window API 与 macOS 声明；这些分支的接口存在不等于其原生实现已全部确认。

刷新先设 refresh=true，只有取得非空程序路径、完成图标返回及列表合并后，才安排 300 ms 计时器清除此状态。枚举返回空、路径列表为空、JSON 解析或图标请求失败时，没有共同的 finally 清理，也不会在这些早退路径清空旧列表。Add App 卸载只移除 click 监听和既有计时器，没有取消已开始的枚举、图标请求或拒绝旧结果的代次判定。

Browse 的多选结果通过 forEach 分别调用 addApp，没有等待所有添加结束；Games 的拖放同样逐个发布 file.path，没有合批完成确认。Electron 取消选择通常转为 JSON 字符串 `"[]"`，因此不会发布添加；主机根本没有返回对象时，桥接器却返回真正的空数组，调用方仍可能继续 JSON.parse 并拒绝，不能把两种结果统一写成成功取消。拖放还要求目标 class 为 dragging、game-tile、list-box 或 game-footer，完整回调没有扩展名校验。容器监听正确解绑；覆盖层 effect 的清理对 drop 误用 addEventListener，不能将其记为全部监听已释放。

addApp 的 then 等待的是 `addGame` action 的 Promise。该 action 捕获发布到 memory-storage 的异常后只记录日志，不重新抛出；因而发布失败也可能正常 resolve 并关闭 Add App。请求没有提供 requestId，当前页面桥接器也没有消费 `gms-remote-action-result`。FFI 异步桥在带参数调用的回调里直接取返回值，外层异常也会被捕获；其 Promise 返回不能作为可靠的原生错误拒绝。弹窗关闭、请求发布、代理实际结果、目录事件和持久化必须分别核对。

代理把手动路径转换为 GameSignature，标记 MANUAL 并关闭 overlay；lnk 先经本机接口解析。服务若报告条目已存在，代理按 launch exe／helper、各架构默认 exe 或本地 url 快捷方式目标查已有项；隐藏项通过 `SetHiddenState(false)` 恢复。若服务正在扫描，路径进入单独互斥保护的 pendingActionList，扫描完成后重试。实际列表仍依赖服务事件或后续读取。

Games 的刷新调用不等待扫描 action 返回，随后直接设 isSyncing=true；页面桥接器也只依据扫描请求设 true。所有 `updateGameList`／`setGameList` 进入的列表提交 reducer 都会设 isSyncing=false，而非等待与本次请求相关的扫描成功结果。页面和代理 API 请求 switch 没有 stopScan 动作；代理另有 stopScan 方法，不能据它存在虚构页面扫描取消。代理 startScan 在设置 isScanning=true 后，调用失败或返回失败码的完整路径没有复位该字段；finished 消费者则不检查 scanCompleted 的真假就清 isScanning、重新读列表并处理排队添加。测试必须区分失败后停留的状态、其他列表事件清掉页面 loader，以及真实扫描任务停止。

删除时，MANUAL 项调用 `RemoveGameInUserGameList`，自动发现项调用 `SetHiddenState(true)`；成功隐藏后读取所属列表并发布更新。代理另把实际结果发布到 `gms-remote-action-result`，携带 action、requestId、游戏签名或路径及 result。当前 Profiles 页面只写入删除请求，随后就从 reducer 移除游戏；它没有在该 action 中等待此结果频道。isMissing 项可直接本地移除，因此不能把卡片消失写成服务删除已确认。

游戏详情的删除确认同时维护关联和 removedGames。removedGames 内部按 id 去重；持久化只保留非本地项，原件判断为 `length<=20` 后追加，实际最多 21 项。游戏恢复后会从 removedGames 移除。

封面编辑接受 jpg／png，读取本地文件、转换并缩放为 300×150，先更新本地预览，再向代理发送设置／移除封面请求。代理调用服务的 `SetCustomGameCover`／`RemoveCustomGameCover`，只在成功时刷新所属游戏列表。预览更新与服务确认属于不同步骤，取消文件选择不发设置请求。

## 程序枚举与图标的 Windows 原生链

本条查询使用当前官方 4.0.896 包内 SysUtilsNative 1.2.3.0，x64、Image Base `0x180000000`，以及 [当前 Profiles 主脚本](../../../.analysis/inputs/frontend/synapse-profiles-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/profiles/static/js/main.bceb4af9.js)、[Add App 所属 chunk](../../../.analysis/inputs/frontend/synapse-profiles-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/profiles/static/js/9473.fa0bcd61.chunk.js) 和 [Games 挂载 chunk](../../../.analysis/inputs/frontend/synapse-profiles-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/profiles/static/js/6360.49064bb7.chunk.js)。不复用旧 4.0.827 Host 或旧前端的字节身份。统一入口为 `python tools/workspace.py audit-host-local --query profile-local-applications`；完整证据写入上述正式字段，选择范围为 57 个完整 AST 所有者中的 49 个角色、40 个完整原生函数和 35 条经汇编交叉引用确认的调用边，不代表整个 Profiles 或 SysUtilsNative 已完成。

当前 [主机 main.js](../../../.analysis/inputs/host-current/asar/e7a524aca177/electron/main.js) 实例化 `FFISysUtils(ffiMain)`，平台选择器在 Windows 加载 win 分支。打包路径使用 `app-<appVersion>/CommonDLL`，调试分支只在本机 Apps/Common 副本缺失时复制；`initDll` 检查文件后配置 `sysUtilsNative`。相关声明仍为 `pointer()`、`pointer(string,int,int)` 和 `FreeMalloc:void(pointer)`；配置失败每次删掉一个尾部 API 组，最多尝试三次，不能把它描述为无上限重试或另换 DLL。来源核验包含当前获取收据、官方包、实际提取 PE、ASAR 父容器及每个成员的真实偏移字节。

| 导出／主体 RVA | 实际输入和返回 |
| --- | --- |
| `getInstalledApplicationList` ordinal 65，`0x6d250` | 无参数；返回本 DLL 分配的窄字符 JSON，条目只有 `name`、`path`。初始值为 null，第一次追加才转为 array；无合格项返回字符串 `"null"`，分配失败返回空指针 |
| `getApplicationIcons` ordinal 61，`0x6f070 → 0x41bd0` | 本页面传入路径字符串数组的 JSON 文本和 80、80；主体另保留对象 path 分支。返回 `path/width/height/image`；失败可以保留条目但 image 为空，没有追加时为 JSON null |
| `FreeMalloc` ordinal 5，`0x69d10` | 非空时经 `0x98c30 → 0xaa840` 调用本 DLL 堆的 HeapFree；不能换用调用方分配器 |

程序枚举按 HKLM、HKCU 的顺序，每个根先查 WOW6432Node 下的 Uninstall，再查普通 Uninstall。RVA `0xe40b0` 的 16 字节根表已与真实 PE 核对。每项先取得 DisplayName 和 DisplayIcon；只在 DisplayIcon 末三个字符内找逗号并截掉图标编号。路径长度大于 5、末四字节小写比较为 `.exe` 或 `exe"` 后，`0x6d060` 去首尾双引号并以 ifstream 检查可打开性。成功项按原枚举顺序追加，没有原生去重、排序、环境变量展开或从其他字段补路径。

这条完整枚举链没有遍历开始菜单、解析快捷方式或枚举 UWP。独立的 `GetPathFromShortcutLnk` 导出及 GMS 的 UWP／URL 策略属于其他调用链，不能把它们并入 Add App 的程序枚举。两个注册表字符串实际读取的返回码没有二次判定，RegEnumKeyW 只以 ERROR_NO_MORE_ITEMS 终止；读失败不能据最终 null 推断“机器没有安装程序”。固定缓冲区、截断规则、注册表项重复及路径不可打开都应在隔离夹具中单独验证。

实际消费者由 Games 的 lazy 模块 3137 挂载；Add App 在 componentDidMount 发起枚举，通过词法导入 `u=a(37226)` 找到真实平台单例。它解析枚举结果、提取 path，调用普通图标接口，按精确 path 双循环合并，再按 name＋path 去重；空 image 使用页面内 SVG。结果交给 `setInstalledApplicationList`，沿模块 68548 的 action、模块 13254 的真实动作常量、`addGameReducer` 和 connect props，最后由 componentDidUpdate 更新 installedAppList／filteredAppList。仅写入的 `state.appList` 并不是列表渲染所读字段；renderAppRows 对 filteredAppList 排序后呈现。该结果不代表添加游戏、服务持久化或设备写入成功。

`window.top.apiElectron` 经当前 preload 的 `doElectronAction → ipcRenderer.invoke("electronAction")` 到 main 的实际 ipcMain handler；三个相邻 case 最终调用同一个 `po.callDLLAsync(n,i)`，Windows 对象再将 action 和参数交给 ffiMain。完整语法所有者保留了实际导出 getter、词法绑定、声明、处理器和异步结果消费者，没有仅靠同名字符串串接。

原生 `0x3bbb0` 按末四个 UTF-16 字符分派；exe／dll 经 `0x3b1b0` 的 PrivateExtractIconsW 从索引 0 提取一枚 128×128 HICON，`0x3a380` 写 ICO，随后 DestroyIcon，再由 `0x3aeb0` 按请求尺寸缩放 PNG，插值模式为 7。普通位图分支为 `0x3b7b0/0x3b580`。本页面固定请求 80×80；主体保留的零尺寸分支会把缓存键和返回尺寸改为系统尺寸，却仍向提取器传原请求，不能据返回尺寸断言处理成功。

临时路径 `0x3a7e0` 使用系统临时目录和 `RazerTempPicture_<thread id>_<counter % 20>.png`；exe 第一份实际是 ICO，第二份才是 PNG。`0x3ac90 → 0x3a920` 读取并编码，`0x3ba00` 仅检查数据前缀和 Base64 允许字符。读取缓冲及文件在正常路径释放，成功生成的临时文件没有统一删除；GetIconInfo 成功后 GetObjectW 失败可能遗留位图；部分 WriteFile／GetBitmapBits 结果没有参与失败判定，读取循环也没有短读或零进度退出；PNG codec 查找 `0x3a6b0` 始终返回 0。正式收据保留这些失败分支，未把原版缺口改写成已正确释放。

缓存链为路径 `0x3ec30`、key `0x3ed20`、加载 `0x3ef50`、命中 `0x41120`、批次剔除 `0x41770`、磁盘保存 `0x40490`。位置仍是 Local AppData 下的 `Razer/RazerAppEngine/User Data/State/icon_cache.json`，格式含 version=1、savedAt、entries；key 为 `path|altPath|widthxheight`。只有成功读到非零且变化的修改时间才淘汰；文件缺失也可能命中旧缓存。每批剔除不属于本批 key 的全局项，再直接写文件，没有原子发布或批次事务。图标接口会修改磁盘；未来实际 DLL 对照不能将其默认归为无副作用的只读调用。

返回串由 `0x9b120 → 0xaa8a0` 分配，枚举预留 `2*JSON长度+10`，图标预留 `2*JSON长度+1`，清零后经 `0x9dae0` 复制。ffiMain 只在 readCString 成功后置标志并在 finally 调 FreeMalloc；复制抛出时没有补救释放，async callback 的 error 也没有可靠转换为 Promise rejection。FreeFFI 仅删 Map，不确认在途结束或卸载。DLL attach/detach `0x39240` 保留缓存保存 `0x410c0` 和 GDI+ 生命周期 `0x72080`，不据此声称主机关闭已等待所有调用完成。

页面无返回、无 path 或解析失败时不会共同清空旧列表；refresh 只在非空枚举和图标合并完成后由 300ms timer 结束。卸载移除 click listener、清已保存 timer，没有取消原生请求或旧代次保护；原生入口也无取消参数。这里闭合的是这些实际源码行为，不是运行成功。Alt 的实际消费者和 Windows 主体见下一节；macOS 原生、扫描策略、游戏写入与设备自动配置档继续按各自缺项推进。9 项维护器反例和严格 JSON 往返验证不替代 Rust 功能测试、实际 DLL／设备对照或窗口验收。本子任务复用完整捕获，没有创建临时副本或分析会话。

## 游戏图标、封面与 Alt 平台边界

`python tools/workspace.py audit-host-local --query profile-alternate-icons` 从普通图标收据接续，正式字段为 `profile_alternate_icons`。当前仍使用上节的 Profiles 主脚本、Host 4.0.896 和 SysUtilsNative 1.2.3.0。此专题核对两个实际 Alt 调用、平台 JS 和 17 个原生函数、15 条真实调用边；普通 ICO／PNG 提取、缓存持久化及指针释放的完整深链复用 `profile_local_applications`。Add App 的普通图标消费者保持上节已核实的独立调用。

| 实际消费者 | 请求与结果消费 |
| --- | --- |
| `P.getApplicationsIcon` | 本地／用户游戏列表映射为 `{path:launch_exe,altPath:display_icon_path或空串}`，请求 80×80。按返回的原始 path 精确匹配 launch_exe，重复 path 的后项覆盖前项；空 image 使用页面 SVG。原 gameSignature／gameSettings 继续保留 |
| `P.setCustomGameCover` | 只处理存在 `ATT_CustomGameCover` 且当前封面仍等于 signature 封面的项；输入只有 path，请求 300×150。返回 path 精确匹配自定义路径；image 为 null／undefined 才保留旧封面，空字符串会实际覆盖封面 |

`P` 经真实 connect 后由父组件 render 挂载。挂载先注册 GMS 事件，再注册 iframe／memory 监听并读取快照；本地和用户列表分别经过隐藏项过滤及图标请求，隐藏的本地项另交 removedGames。`addNewGames` 合并列表并处理自定义封面后，调用模块 70399 的 `updateGameList`；模块 13254 的 `GAME_LIST` 进入 `linkedGamesReducer` 的真实提交 helper，过滤特定空名称项、去除已恢复的 removedGames 并清 isSyncing。它另创建 `Profile_GameListUpdates` BroadcastChannel，发布后立即 close。列表消费不证明服务或设备写入成功。

`gmsDataCount` 只保护最终调用 `addNewGames` 的入口；较旧的在途图标结果仍可能先修改 `localGames`／`userGames`。该完整桥类没有 componentWillUnmount，也没有原生取消参数。两个图标消费者的 JSON.parse 不在方法内捕获；封面空串、请求异常、较旧结果和未取得的结果不能统一改写为成功回退。

Windows 导出 `getApplicationIconsAlt` ordinal 62、RVA `0x6f080` 跳到 [完整主体 `0x431c0`](../../../.analysis/results/native/bc0387a0913c882a9cd82aa1a8e147d6408fc7bb7f77519f4f8860f16c5b17c8/functions/431c0.json)。它解析对象数组，字段存在却不是 string 时保留真实 JSON 类型错误；无追加项时返回字符串 `"null"`。主 path 长度大于四个 UTF-16 单元时先走普通路径分派器 `0x3bbb0`，只有所得 image 为空且 altPath 长度大于四时才走 `0x3b7b0` 位图提取，不能把 altPath 也描述成 exe／dll 图标分派。输出保留原 path／altPath，附 width／height／image；备用路径成功不会替换输出 path。任一请求尺寸为零时，返回及缓存键的两维改为系统尺寸，实际提取仍收到原请求尺寸。

缓存键仍为 `path|altPath|widthxheight`。命中检查优先使用已保存的成功 source_path；只有读到非零且变化的文件修改时间才淘汰，文件缺失仍可能命中。`0x41590` 只保存非空图像及实际成功源路径。每批经 `0x45be0` 登记 key，结束时 `0x41770` 剔除其他全局项，`0x40490` 直接落盘，因此 Alt 也有磁盘副作用。返回串分配 `2*JSON长度+1`、清零并复制，调用方沿相同 FFI 在 readCString 成功后才置释放标志并于 finally 调 FreeMalloc；复制异常前的指针、忽略的 async error 及 FreeFFI 不等待在途的边界均保持上节事实。临时文件、短读、位图失败释放等公共缺口没有因选择 Alt 而消失。

当前平台选择器对 win32 选择 win，其余平台都选择 mac；只有 darwin 的主机加载器把文件名改为 `libSysUtilsNative.dylib`，打包路径在 `HOME/Library/Razer/RazerAppEngine/razerappengine.app/Contents/CommonDylib`。当前 Mac JS 声明普通 `getApplicationIcons` 和 FreeMalloc，没有 Alt 声明，generic FFI lookup 因而返回 undefined。前端 Electron Alt 适配器只对 null／undefined 结果回退：解析原对象数组、丢弃 altPath，再以原尺寸调用普通图标接口。空字符串或字符串 `"null"` 不触发此回退；非 Electron Alt 没有直接 window Alt 分支。

当前保留输入中没有 Mac 原生原件；这里只闭合 Mac JS 声明和回退路由，Mac 原生枚举／提取仍待取证，不能据此断言功能不存在。当前来源断点、32 项声明、初始化、事件订阅与释放边界统一见[macOS SysUtils 专项](window-lifecycle.md#macos-sysutils-来源事件订阅与释放)。10 项维护器测试覆盖新增／重复消费者、动态尺寸、错误回退假设、调用边及失败事实篡改，并通过严格 JSON 往返和原件快照核验；这些静态结果不代替独立 Rust、UI／后端接线或实际 DLL／设备对照。专题复用既有完整捕获，没有新增临时副本、数据库或分析会话。

## 当前 GOG 注册表扫描与结果消费

`python tools/workspace.py audit-host-local --query profile-gog-scanner` 从 `profile_local_applications` 和 `profile_alternate_icons` 接续，正式字段为 `profile_gog_scanner`。Host 当前清单已指向 GMS **3.15.0.1200**，因此本批只增量捕获当前包的三个程序集，没有将旧 3.14 收据改标为当前。当前完整 C#／IL 位于 [ServiceLib](../../../.analysis/results/host-current/managed/7d2563f13d79/)、[Common](../../../.analysis/results/host-current/managed/1848c257176f/) 和 [Service](../../../.analysis/results/host-current/managed/d667d986ff57/)。本批选定 78 个 AST 所有者，正式收据含绑定／导出共 86 个完整所有者，另核对 174 个完整托管方法及 41 条原字节调用边；这些计数只描述选定链，不代表整个 Profiles 完成。

真实入口是完整页面的首次挂载与两处游戏刷新按钮。模块 38301 的 `initiateGameScan` 向 `gms-api-request` 写 `{action:"scanGame"}`；名称符合 `profiles-\d+` 的内嵌窗口抑制此请求。主机 memory-storage 保存值并通知其他已注册窗口，不返回扫描完成结果。当前代理连接前只置 pending，连接后通过 SDK 的 `HubConnection.invoke("StartScan")` 调 Hub → GameScanner 状态对象 → Controller；SDK StopScan 调用与代理包装器存在，但当前页面请求 switch 没有 stop 动作。未初始化、重复扫描和调用失败各有实际分支；失败 StartScan 没有统一复位 isScanning。

当前 [GOG 策略](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/GameScannerStrategyGogRegistrySearch.cs) 注册为 scoped、优先级 High=4；注册类型参数从原 PE 的 MethodSpec 解码核对。它先整批读取 `HKLM / Registry32 / SOFTWARE\GOG.com\Games\` 下的 `exe`、`launchParam`、`gameName`，随后才检查停止标志。注册表包装器遇异常返回已完成前缀，没有显式 Dispose／Close；策略逐项异常继续，外层异常返回已收集前缀。文件存在性检查删除双引号，保存的启动路径保持原值；标题缺失取文件名并去除 ™／®。身份先按当前主表的注册表 key 匹配，否则以 `gog + platformId` 和源码固定 key 经 Encoding.Default／HMACMD5／大写十六进制构造 Guid；冲突再按实际来源和标题处理。LaunchExe 映射与 helper 映射继续影响最终 GUID，TryUpdate 返回值未检查。

ScanAsync 构造 GOGGALAXY／GOG_REGISTRY 签名，经 CanSkipGame 和共享 AddFoundGame 发布；它没有逐项 catch，正常末尾 Clear 不在 finally，完整状态机没有 await 挂起点。公共层仍会正规化路径、按主表覆盖字段、处理 helper／图标及再次去重，不能把原注册表字段当成最终结果。扫描后的 RemovePostScanDuplicates 在内存列表设置后、图标任务与结束通知前运行：按非空启动路径的当前文化小写值分组，优先主表与路径映射项；单 Steam 候选组可把 Steam 来源／图标／注册表信息合并进保留项，非 Steam 重复项、与 UGL 同 GUID 项及作为其他项 helper 的候选分别移除。对应设置只在此处修改缓存，返回码没有形成事务；异常被外层记录后继续，已完成的修改不回滚。

Controller 正常受理即返回 GS_SUCCESS。每组 WaitAll 的超时 bool 被原 IL 的 pop 丢弃；超时与 StopScan 都不 join 策略，停止只取消回调并设置策略标志。正常尾部先广播 `scanCompleted=true`，再保存 GSL／LGL，LGL 返回码同样被丢弃；随后清 token 集合、重置停止标志和清共享映射，没有旧代次保护。停止通知可先到达，正常尾部仍可能随后到达。发现广播是 async void，调用方不等待发送；通知发出、服务缓存改变、文件保存和实际设备写回是不同完成点。

完成事件按最多 16 项分段，空列表也发一段，没有请求 ID。当前 SDK 累计段后异步派发，代理不检查 scanCompleted，重新读取本地／用户列表并通过零延时 timer 发布 `gms-data`。Profiles 依次消费两组列表，复用既有图标／封面链，最后由真实 Redux 提交 helper 更新 gameList 并清 isSyncing。timer 发布没有被 await，logout 不等待 disconnect，桥组件没有 unmount 取消；较旧分段、在途结果与定时器没有完整代次隔离。

Service 的完整捕获仍有 **10 条 C# 诊断**，其中本批 RegisterService 的 2 条位于 `IL_00d5/00dc` 的 analytics 构造。诊断原样保留，GOG 注册另据完整 IL 与原 MethodSpec 独立确认，没有把整个 Service 标成反编译无缺项。12 项定向测试核对真实 AST／IL 调用、错误目标／RVA／原字节、被丢弃返回值、诊断保留、边界篡改、严格 JSON 往返和输入快照。其余扫描策略、框架内部、Rust 行为实现、实际 DLL／服务／设备对照和窗口验收仍未完成；本批不执行服务、程序集、应用或厂商 JS，也未创建待清理的临时副本。

## 七个扫描策略的当前版本差异

`python tools/workspace.py audit-host-local --query profile-scanner-version-differences` 把已审的 Arc／Bethesda／EA／GOG／Mihoyo／Riot／Uplay 策略接续到 GMS 3.15.0.1200，正式字段为 `profile_scanner_version_differences`。GOG、共享调度／停止／去重及当前列表消费者直接复用上一专题；本批只比较其余六个策略及直接 Reader、Parser、注册表和路径依赖，不重做图标或扫描全库。两版完整方法、原字节、异常区间、实际调用及 #US 字符串均保留于收据，当前原件来自 [3.15 ServiceLib](../../../.analysis/results/host-current/managed/7d2563f13d79/) 与 [3.15 Common](../../../.analysis/results/host-current/managed/1848c257176f/)。

329 个两版完整方法形成 180 个比较项：146 个所选方法在元数据令牌重排后保持相同，3 个改写、29 个新增、2 个移除。比较同时要求非令牌字节、完整 IL 声明／局部类型／异常范围、指令、已解析调用目标及原 #US 常量一致；只遮盖经真实 opcode 和四字节范围核实的令牌操作数，不因类名或 RVA 相同判等，也不把字符串变化遮掉。该结论仅限所选方法，完整字段／TypeSpec 签名及框架运行结果没有被宣称独立验证。

| 分支 | 当前版本核对结果 |
| --- | --- |
| Arc、Bethesda、EA、Mihoyo | 完整策略 C# 字节相同，所选方法及实际依赖按两版原字节和元数据重新绑定；下面原有的预读、逐项停止、部分结果、映射副作用和非 finally 清理边界继续适用 |
| GOG | 复用 `profile_gog_scanner` 的当前策略及共享链，不重复提取和分析 |
| Riot | ScanAsync 与路径读取调用者保持所选方法一致；正则、解码委托及新的解码函数有实际变化 |
| Uplay | 改为已安装游戏优先，并增加配置匹配、路径解析、递归 exe 候选排序和空身份字段门禁；旧版仅遍历配置项的描述不能作为当前行为 |

Riot 当前正则从原 #US 堆核实，接受成对的 `\u` 代理项、`\x` 两位、`\u` 四位及 `\U` 八位十六进制。代理对用两个 UTF-16 char 构造字符串，其余走 Convert.ToInt32 → Char.ConvertFromUtf32；ArgumentException／OverflowException 返回原匹配文本，没有捕获所有异常。故孤立代理项、无效标量和旧版单 UTF-16 单元转换须分别比较；不能把失败统一变成空路径。文件读取 catch 仍可能返回已收集文本而跳过后续正规化，停止与释放机制没有因新增解码而改变。

Uplay 仍先读取 `GetUplayData(false)`，随后读取已安装列表和主表注册表索引，以上均先于停止检查。已安装列表非空时只遍历该列表；为空才遍历配置项。每个安装项按 GameId、Name、GameNameIdentifier 的精确匹配，再按 Name、GameNameIdentifier 的双向包含匹配，共五轮 FirstOrDefault，均为 CurrentCultureIgnoreCase；同一配置可以被多个安装项复用。未匹配时以安装项 DisplayName／GameId 构造本地候选，不要求配置文件里已有该游戏。

配置路径仍只试第一条 offline，再试第一条 online；有安装项时用安装目录对象的 GetFullPath，缺 registry key／value 时回填安装项字段。配置路径为空或不存在，且此前未抛异常时，才递归枚举安装目录的 `*.exe`。明确排除 unins 前缀、含 uninstall，以及精确的 UbisoftConnect.exe／Uplay.exe；其余按“正规化显示名＋game、正规化显示名、非 EAC 名称包含显示名、其他非 EAC、EAC／EasyAntiCheat”给出原 IL 常量 0–4，再按全路径分隔符数、字符串长度排序，同分保留原枚举顺序。正规化只保留 Char.IsLetterOrDigit；EAC 是排序条件，不是全部拒绝条件。递归枚举没有停止 token，DirectoryExists 检查位于该方法 catch 外，异常和读后路径失效仍须按上层实际消费者处理。

当前 CreateUplayEntryGuid 在查询注册表索引前检查空 key／value，空 LaunchUrl 不再进入共享映射。GetUplayDisplayName 的原 #US 常量却是重复的 `?`，不是旧段落的 ™／®；四处文字已从原堆字节核实，不能在记录官方结果时自行修正。helper TryUpdate 返回值仍被忽略，配置列表只在正常尾部 Clear；路径与身份／发布各有 catch，但共享映射没有事务或回滚。

当前 Service 的 10 条 C# 诊断保留，RegisterService 的所选 IL 与原 MethodSpec 注册单独核对。Common 的 GetAllPossibleFilenames 及消费者另由[共享 helper 专题](#共享-helper-与实际消费者)复核；这里实际选用的 GetComponents 已独立比较，不将邻接方法并入六策略的完成范围。`ProfileScannerVersionDifferencesTests` 的 11 项定向维护器测试通过，覆盖令牌遮盖范围、原常量、声明差异、五轮匹配、排序原字节、失败边界篡改、JSON 往返及输入快照；没有执行厂商代码或新增临时副本。独立 Rust、行为测试、实际 DLL／服务／设备对照和窗口验收仍未完成。

## Steam、Battle.net 与 Epic 的当前版本差异

`python tools/workspace.py audit-host-local --query profile-store-scanner-versions` 使用 `profile_store_scanner_versions` 正式字段接续 GMS 3.15.0.1200 的四个实际注册策略：SteamLibrarySearch、SteamShortcutsSearch、BattleNetSearchV2 和 EpicLauncherInstalledList。仅核这组及直接 Reader、模型、本地序列化和 SQLite 方法，复用既有七策略、注册入口、路径／注册表与 Mihoyo 边界、图标和共享调度／去重／列表消费者，不重新捕获程序集。

当前完整 RegisterService 的 22 次实际 AddScoped 分别保留 MethodSpec、调用字节、类型和顺序；21 个无条件，UWP 仍要求 OS major≥6。此前七策略与本节四策略的分组事实保留；后续两个专题分别接续[本机目录／注册表七策略](#本机目录与注册表七策略的当前版本复核)和[剩余四策略](#feveritchiotwitch与uwp的当前版本复核)，不能因已经注册或旧版有段落便计为已完成。Service 的 10 条 C# 诊断继续保留。

两版分别保存 298／315 个完整方法，共 613 个，形成 318 个比较项：286 相同、9 改写、20 新增、3 移除。比较复用前述完整 IL、原字节、异常区间、已解析调用及原 #US 常量规则。Steam ReadAsync 的生成所有者由 `<ReadAsync>d__13` 改为 `d__14`，原始比较仍保留新增／移除，不解释成相应功能被删除。Steam 两个策略与主要 Reader 的 C# 字节相同，不代表其调用的 appinfo 查询仍按旧语义执行；JSON 序列化器的完整文本有 cast 呈现差异，本批实际选中的 DeserializePath／DeserializeString 及构造方法没有相应 IL 变化。

### Steam：递归启动项与异常传播

当前 [SteamAppInfoData](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/SteamAppInfoData.cs) 的 GetAppType 先检查 common、type 的 ContainsKey；GetLaunchExe 同样先检查 config、launch，再进入新 FindLaunchExe。查找按 KV Children 的实际枚举顺序深度优先遍历：键以 OrdinalIgnoreCase 匹配 executable，值非空且以 `.exe` 结尾才返回，后缀也使用 OrdinalIgnoreCase。当前项不满足时递归其值，再处理下一项；不限制 0–6 槽位，不按 OS／架构额外筛选，也不在此检查文件存在。

旧 GetLaunchExe 捕获 Exception 并返回空，当前 `0x06000821@0x2a20c/104` 没有异常区间；新递归方法 `0x06000822@0x2a280/137` 仅有释放枚举器的 finally，没有 catch 或停止参数。缺少 config／launch 键可正常返回空，null 数据、坏 KV 子项及其他访问异常则交已有 ScanAsync 的单游戏 catch，不能继续当作“查询空后尝试 SQLite”。appinfo 39／40／41 解析、已核的库／manifest 读取顺序和其他既有异常边界保持原证据范围；KV 依赖自身和实际文件／DLL行为仍需分别验证。远程 SteamKit 补全继续属于剔除的联网业务，本批没有重审下游网络包装器。

### Battle.net：不可变黑名单

当前 [BattleNetSearchV2](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/GameScannerStrategyBattleNetSearchV2.cs) 的黑名单由 HashSet 改为 FrozenSet，实际构造器 `0x060003b8@0x1b268/76` 调 ToFrozenSet，过滤 lambda 随字段类型变化。原 #US 中的 bna／agent／clnt 和 CurrentCultureIgnoreCase 比较器均保留。实际日志 Parser 的完整 C# 和所选方法无变化，不因此重做日志、停止或共享发布分析；集合分配失败和框架运行结果未被静态同源结论覆盖。

### Epic：候选增加与前置字段门禁

当前 [Epic 策略](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/GameScannerStrategyEpicLauncherInstalledList.cs) 仍先读取已安装列表、再读取本地 manifest，改为两表均空才提前返回；仅 manifest 列表为空时才从已安装项合成 manifest，并不把全部未匹配安装项并入非空列表。每个 manifest 在停止检查之前按 AppName、InstallLocation、ProductId 对 ArtifactId／AppID 三轮 FirstOrDefault 匹配，均为 CurrentCultureIgnoreCase。匹配安装项允许 IsApplication=false 的 manifest 继续，并可补 InstallLocation；UE_ 与精确 plugins／plugins/engine 过滤仍保留。

新增路径不能绕过入口的 LaunchExecutable 门禁。完整 [EpicGameManifest](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/EpicGameManifest.cs) 的 getter `0x0600049b@0x210a9/7` 直接读取 backing field，构造器 `0x060004ba@0x211d2/7` 只调用 Object 构造器。合成方法 `0x060003e1@0x1c1ec/112` 仅设置 AppName、DisplayName、InstallLocation、IsApplication、MainGameAppName、InstallSize 和 BaseURLs，未赋 LaunchExecutable；后续匹配只补安装目录。MoveNext `0x06000b6a@0x3cf24/2963` 在 `IL_0225` 读取 LaunchExecutable、`IL_022a` 检查空、`IL_022f` 以原字节 `3ab9080000` 跳到下一项 `IL_0aed`，早于 GetLaunchUrl、主表与 fallback。因此新合成项的正常路径仍会被跳过；不能凭新增 helper 宣称此安装列表分支已经找到游戏。

通过门禁后，原有主表／顶层／递归候选先执行；候选为空或不存在才进入 `TryFindEpicFallbackLaunchExe 0x060003e3@0x1c2d0/452`。依序尝试安装根加去前导分隔符的 LaunchExecutable、递归找主表 DefaultExe、最后递归 `*.exe`。后两段按现有 blacklist 过滤；最后一段按原常量 0–3 排序：GTA5／GTAV、正规化显示名或 AppName 精确相等、文件名包含其一、其他，再按路径分隔符数、字符串长度与原枚举顺序。同分不额外按文件名排序；只有候选多于一个才返回完整 RunningExe 集合。此方法没有独立 catch 或取消参数，枚举失败仍可使策略 fault，没有新增逐 manifest 恢复链。

四个旧身份 URL 比较消费者改为 ToLowerInvariant 后 Trim；UGL／LGL 两阶段和后者可覆盖前者的顺序、共享映射副作用、未消费 LaunchCommand、正常尾部仅 Clear installedGames 的事实保留。模型 ProductId 与两个实际 Reader 的所选方法无变化，不扩大其成功或释放语义。

`ProfileStoreScannerVersionsTests` 的 11 项维护器测试通过，检查注册分组、所选方法比较、Steam 异常区间与原常量、Epic 属性生产者／门禁原字节、匹配与排序，以及失败边界与收据篡改；不执行厂商程序集、不扫描本机游戏目录，也没有新建临时副本或分析会话。独立 Rust、UI／后端接线、功能行为测试、实际 DLL／服务／设备对照及窗口验收仍未完成。

## 本机目录与注册表七策略的当前版本复核

`python tools/workspace.py audit-host-local --query profile-local-scanner-versions` 使用 `profile_local_scanner_versions` 字段复核 DefaultExeSearch、DefaultRegistryKey、GameUXInfoSearch、GameUXRegistrySearch、RegistrySearch、UninstallSearch 和 UserFolderExeSearch。范围包含直接 GameUX Reader／模型、注册表搜索、路径适配和实际路径展开方法；复用前述 11 策略、22 个实际注册、共享调度／去重／列表消费者和图标结论，不重新捕获程序集。

两版各保留 124 个完整方法，共 **248 个方法／124 个比较项：123 相同、1 个变化**，无所选方法增删。七策略的完整 C# 字节均相同，GameUX 模型的 8 组 getter／setter 及实际路径辅助方法仍按原始字节、完整 IL、调用目标、异常区间与 #US 常量逐项比较，不能仅以源码同名或文本相同判等。GetAllPossibleFilenames 的变化与消费者归入[共享 helper 专题](#共享-helper-与实际消费者)，不重复纳入本组。

唯一变化位于当前 [RegistryInspector.SearchForRegistryKey](../../../.analysis/results/host-current/managed/1848c257176f/source/Razer/GameScannerCommon/RegistryInspector.cs) 的 `0x06000354@0x829c/354`。旧版两处 `IL_004c/007c` 调 Enumerable.Contains&lt;string&gt;；当前先在 `IL_0046/007b` 转换数组，再在 `IL_0051/0086` 调 MemoryExtensions.Contains&lt;string&gt;。两版 Contains 的原调用字节都为 `281c00002b`，但该令牌实际解析到不同目标；泛型签名字节 `0a010e` 保留 string 参数。转换的完整 IL 呈现 ReadOnlySpan&lt;string&gt;，MemberRef 名称与令牌已校验，TypeSpec 所有者未独立解码；没有据此声称框架运行结果已对照或两种实现完全等价。

此 Reader 确由两个注册表策略 → RegistryWrapper.SearchRegistryString → SearchRegistryKeyPath 消费。单组件先读 value names，再查子键名；优先返回匹配子键，否则返回含同名值的当前键，没有新增忽略大小写比较器。SearchForRegistryKey 和 SearchRegistryString 都没有局部 catch；前者异常进入外层路径搜索的日志／已积累前缀返回，后者还保留 `matchingName.Equals(DisplayName)`，不改写为统一大小写匹配。路径拆分／根格式化位于主 try 之前。指定 Hive 的 64 位进程重试仍在 `IL_017b` 打开 Registry32、`IL_0180` pop 掉返回值，再把原 Default-view 的 local 13 交给搜索；本批没有把它记成已正确切换视图，也没有虚构 RegistryKey 释放链。

| 策略链 | 当前保留的实际语义 |
| --- | --- |
| DefaultExe | 先取得除 Network 外的全部本机盘符，再逐游戏检查停止；并非仅 Fixed 盘。按主表路径候选补 Program Files 分支，首个存在路径发布；Steam／Epic／UWP／Mihoyo 排除继续适用 |
| DefaultRegistryKey／Registry | 非空 SystemId 时才把 HKCU 替换为 HKEY_USERS 下该 SID；直接值为空后才搜索同名安装项。两者的 DefaultExe64 门禁之后，原调用都仍读取 DefaultExe32 来组合候选，收据保留真实消费字段 |
| GameUXInfo／GameUXRegistry | Info 依次预读 GameUX、Origin、GOG、Uplay，之后才到扫描循环停止点；Reader 自己的停止检查仍单独保留。先 AppExePath 后 ConfigGdfBinaryPath；来源优先级为 Origin、GOG、Uplay、Direct。集合只在正常末尾 Clear，不是所有路径 finally |
| Uninstall／UserFolderExe | 前者读 HKLM 的 GUID 卸载项 InstallLocation，按原顺序尝试含／不含相邻目录修复的路径；后者只处理 userprofile 前缀，从当前 UserProfile 首字符推导盘符并枚举 Users，选第一个存在 exe。逐游戏异常和路径转换失败分支分别保留 |

共享路径链仍使用原 `C:\` 默认根；通配符候选先 FileExists，再按 CreationTimeUtc 降序选择。多架构候选使用 CurrentCultureIgnoreCase HashSet；64 位进程先把 ProgramFiles 环境变量替换为 ProgramFiles(x86)，再展开环境变量和 SpecialFolder，不自行改用另一套默认目录。没有新添取消 token、旧代次防护、失败恢复或资源释放。

`ProfileLocalScannerVersionsTests` 的 10 项维护器测试核对完整比较、模型字段、相同原令牌的不同目标、实际消费者与异常边界、原句柄丢弃／字段读取、注册分组及收据篡改，并检查 JSON 往返和输入快照。这是所选版本语义复核，独立 Rust、UI／后端接线、功能行为测试、实际 DLL／服务／设备对照和窗口验收仍未完成。其余四策略由下一专题接续；本批不执行厂商代码或枚举本机安装项，没有新增临时副本或分析会话。

## Fever、ItchIo、Twitch与UWP的当前版本复核

`python tools/workspace.py audit-host-local --query profile-remaining-scanner-versions` 使用 `profile_remaining_scanner_versions` 字段复核剩余四个实际注册策略及直接 SQLite Reader、UWP helper、缓存／manifest 模型、Xbox 本地发现、版本选择和 Impersonator。它复用前述 18 策略、共享调度／停止／去重、列表消费者、图标和已有完整捕获，不重做源码获取；当前原件仍来自 [3.15 ServiceLib](../../../.analysis/results/host-current/managed/7d2563f13d79/) 和 [3.15 Common](../../../.analysis/results/host-current/managed/1848c257176f/)。

两版各选 Lib 138、Common 6 个完整方法，共 **288 个方法／144 个比较项，全部相同**；相同要求仍覆盖原始非令牌字节、完整 IL、异常范围、解析调用及原 #US 常量。24 对完整 C# 产物字节相同；另保存两版共 20 个接口／Win32 导入声明，10 对完整 IL 相同，声明本身不证明实际 ABI 或 OS 行为。Twitch 路径拼接实际调用的两个 InlineArray helper 也纳入完整 IL；生成的 C# 文件真实为 0 字节，不能声称恢复出其 C# 源码。

| 策略链 | 当前保留的实际语义 |
| --- | --- |
| [Fever](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/GameScannerStrategyFeverPlatformSearch.cs) | 使用 HKCU／Default view 的 `Software\FeverGames\FeverGamesInstaller\game\`，不替换 SID；读 DisplayName、ShortcutPath，丢弃 StartupPath 返回值。完整状态机构造签名并处理跳过条件，却没有 AddFoundGame、停止检查或 RegistryKey 释放；不能登记为已发布游戏或已实际枚举本机 |
| [ItchIo Reader](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/ItchIoGamesDataReader.cs) | 在模拟用户上下文取 ApplicationData/itch/db/butler.db，异常后在自身上下文重取路径；查询 caves 与 games，实际消费列 0／1／4，签名身份是 caves.id。单候选直接返回，多候选才筛 Windows 并优先 OS 位数；64 位标识为 amd64，候选文件须存在 |
| [Twitch Reader](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/TwitchGamesDataReader.cs) | 读取 CommonApplicationData/Twitch/Games/Sql/GameInstallInfo.sqlite 的 Id／ProductTitle，再按 Id 读 HKLM 卸载项 DisplayIcon 并检查文件存在；保存 twitch://fuel-launch/＋Id，不在扫描器执行 URL |

ItchIo／Twitch 都在第一次停止检查前完整调用 GetInstalledGames，先修改 SpecialGuidMappings 再 Guid.Parse，之后才发布结果；坏 GUID 没有逐游戏 catch，已修改映射不会自动撤回。Reader 执行 SELECT，但连接串未显式设置只读；catch 返回已收集前缀。command／connection 有 finally，DataReader 无独立显式 Dispose；扫描集合 Clear 仅在正常尾部，不能把枚举器 finally 写成完整恢复／清理。

[UWP](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/GameScannerStrategyUwpSearch.cs) 的实际 DI 分别核对 helper 和策略两处 OS major≥6 门槛，低版本绑定 UwpHelperUnsupported。扫描按 GetUwpPackages(SystemId) → GetAllXboxPackages → GetCategoryCache → GetEntitlementsCache 预读，再进行无停止检查的整批分类，最后两组发布循环各自检查停止。已知 MGL family 优先；否则按两份缓存的 Game 条件或本地 config／GamePage marker 分类。包选择按版本降序，再选 OS 架构、Neutral、首项；只有一个包时直接返回。已知项先修改 MGL Version，其实际发布路径要求 DefaultFolderExe 含通配符；非 MGL 排除 Microsoft.XboxApp 和 StoreId `9PJ8266BHFWN`，禁用 overlay；两类均在 AddFoundGame 后才写 ProcessName。

[UwpHelper](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/UwpHelper.cs) 枚举指定 SID 的包，异常返回初始化空列表，不能据此确认本机无游戏。Category 按 OS 位数选择注册表视图，优先 InstallService 再 InstallAgent；旧缓存缺 ProductId 才解析 FulfillmentData，Entitlements 只读 Store LocalCache 顶层 JSON。AppxManifest 图标选择及 ms-resource 的 MrtCache 解析按实际消费者保留。GetPkgPath 在 InstalledLocation 为 null 时不会进入 InstalledPath／EffectivePath fallback。Xbox 只查 ready／Fixed 盘和 .GamingRoot，读取 Content/MicrosoftGame.config；组合的 ExecutablePath 不另查存在，WinGDK 为无排序、无停止参数的 BFS 和首个递归 exe。GetAllXboxPackages 无自身 catch，错误由 Scan 外层处理；未新增注册表释放、取消或代次防护。

[Impersonator](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/Impersonator.cs) 在 session=-1 时取 active console，WTSQueryUserToken 后以 level=2 DuplicateToken，复制失败返回原 token；SetThreadToken 失败只读取并丢弃 LastError。Dispose 的真实 IL 是 RevertToSelf → pop → ret，没有 CloseHandle；using 不能证明 token 已关闭或身份可靠恢复。

`ProfileRemainingScannerVersionsTests` 的 12 项维护器测试覆盖完整比较、原字节／EH 反例、两处 OS 门槛、无发布／无停止／无释放事实、声明与注册遗漏、完成状态篡改及 JSON 往返。至此 22 个注册策略的**所选方法版本复核清单归零**；三个共享 helper 和实际消费者由下一节接续。其余 Profiles／Service 分支、外部框架、整个扫描语义、独立 Rust、UI／后端接线、功能行为测试及实际 DLL／服务／设备对照仍未完成。本批没有执行厂商代码，没有新增临时副本或分析会话。

## 共享 helper 与实际消费者

`python tools/workspace.py audit-host-local --query profile-shared-helper-consumers` 使用统一宿主收据的 `profile_shared_helper_consumers` 字段，复用 GMS 3.14.0.1109／3.15.0.1200 原件及完整 C#／IL，只补两个 UWP helper、文件名 helper、返回模型和真实消费者。两版各选 Lib 82、Common 25 个完整方法，共 **214 个方法／107 个比较项：106 相同、1 个变化**；18 条实际 helper 调用均绑定到完整方法和原调用字节，每版分别为文件名 5 条、UWP 枚举 2 条、全名回填 2 条。已有 22 策略、缓存／manifest、Impersonator、路径与注册表链直接复用，不新增捕获。

[`GetInstalledUwpApps`](../../../.analysis/results/host-current/managed/7d2563f13d79/source/Razer/GameManagerServiceLib/UwpHelper.cs) 当前为 `0x06000637@0x249b4/653`。它先将 FindPackagesForUser(userId) 完整物化，再读 Category、以 Impersonator(-1) 读 Entitlements；模拟用户或读取异常后重新在自身上下文读取。随后逐包要求安装目录非空、非 framework、manifest 名称与 exe 非空且组合 exe 存在。结果包含名称、相对 exe、family／full name、publisher、安装目录及 logo；ProductKind 先经 family → category ProductId → entitlement 取得，仍空才按 MicrosoftGame.config 的存在置为 Game。**它没有只返回游戏、按架构筛选或按 family 去重**。逐包异常静默跳过，外层错误记录并返回已收集前缀；空表或字段缺失不证明本机无游戏，也没有停止或代次检查。using 与枚举器的 finally 只证明发出释放调用，不能改写上一节的 token 释放边界。

`GetUwpFullName 0x06000638@0x24ccc/288` 用 signature.GameInfo 调 FindPackages，没有传 userId。单包直接用 FullName；多包先选 OS 为 64 位时的 X64、否则 X86，再选 Neutral，不排序版本、不选 ARM64。三个实际架构谓词的原 IL 常量为 9／0／11。它只修改 ProcessName，没有成功返回；无包、无适用架构及普通异常保留旧值。catch 日志仍访问 signature.DisplayName，null signature 可再次抛出，不能概括成全部错误均被吞掉。低 OS 分支的 Unsupported helper 真实返回新空表／执行空回填，但这不是本产品其余平台功能已实现的证明。

唯一版本变化在 [`GetAllPossibleFilenames`](../../../.analysis/results/host-current/managed/1848c257176f/source/Razer/GameScannerCommon/GameSignatureHelper.cs) 的 `0x06000103@0x5e50`，方法体由 577 增至 582 字节。它依次取 DefaultExe、DefaultExe32、DefaultExe64、AlternativeExe、DefaultFolderExe、DefaultFolderExe32、LaunchHelper、RunningExe 的无扩展文件名；多值只按 `|` 拆分、不 Trim，不直接加入 LaunchExe，也不检查文件存在。LaunchHelper 在去扩展名前以原完整 component 比较 Excludables：旧 `Enumerable.Contains<string>` 改为数组到 ReadOnlySpan 后调用 `MemoryExtensions.Contains<string>`，没有显式比较器；当前转换在 `IL_01a9`，Contains 在 `IL_01b0`。两版 Contains 原调用字节同为 `281c00002b`，真实目标不同，泛型签名 `0a010e` 仍为 string。最终 Distinct 使用 CurrentCultureIgnoreCase，不能把该比较器提前套到排除项。八个文件名提取 catch 继续保留，后续异常区间随新增字节移动；字段读取、拆分和 Contains 不全在 catch 内。框架行为等价、TypeSpec 全量签名与实际执行没有由此得到验证。

实际消费者具有不同用途：GetRunningProcesses 在 MGL、LGL、UGL 的独立锁边界收集文件名到 OrdinalIgnoreCase 集合，随后枚举 Process.GetProcesses，先按名称／特殊名称筛选再提取路径；异常可保留前缀，本方法没有显式 Dispose 这些 Process 对象。CacheFileNames 将文件名累加到 HashSet&lt;Guid&gt;，不先 Clear；只有新文件名分支才删同名黑名单，已有 key 添加 GUID 后直接 continue。三种本地／用户列表事件的 RemoveGamesFromBlacklist 则删除 stem 和 stem.exe，锁外仅在正常末尾按 flag 重置 ignored names。枚举器与锁有 finally，部分修改没有回滚，也没有整项取消／join。

UWP 枚举的两个消费者分别是 AddDetectedGame 和 manager.GetUwpApps：前者只取首个 family 的 CurrentCultureIgnoreCase 匹配，把 LogoPath 写到待添加签名后继续添加；后者传当前 SystemId。Hub 的完整异步枚举器 `0x06000cad@0x475e4/398` 在 client 为空时先 yield 空表，**没有 yield break**，下次继续枚举仍会 Task.Run(manager.GetUwpApps) 并 yield 结果。DisposeAsync 的完整 86 字节主体保留，但 Task.Run 没有传取消 token，不能把停止消费视为底层枚举已取消。

两个 AddGameToUserGameList 变体仅在 SourceEx 为 UWP 时回填全名，调用分别在 `IL_0de4`／`IL_0e00`；都晚于 SetGameSettings，早于后续重复检查、列表持久化与通知。全名查询无结果不阻止添加。它们在序列化前就改变签名／列表并设通知 flag；序列化失败返回 GS_FAILED，仍可能发列表更新，外层 catch 也不保证改变初始 GS_SUCCESS。因此图标找到、ProcessName 已填、列表通知、文件保存和添加成功必须分别判断。

`ProfileSharedHelperConsumersTests` 的 11 项维护器测试核对实际调用清单遗漏、原令牌不同目标、架构常量、输出模型、错误／释放边界、原调用字节篡改和完成状态伪造，并核对严格 JSON 往返及输入快照。本节闭合所选 helper 和消费者的静态版本语义；其余添加／存储／进程监听依赖、外部 WinRT／OS／框架、独立 Rust、UI／后端接线、功能测试及实际 DLL／服务／设备对照继续保持未完成。本批无厂商代码执行、新临时副本、数据库或分析会话。

## GMS 服务发现、排队添加与任务退出

当前 [代理 SDK 和实际消费者](../../../.analysis/results/frontend/gms-proxy-current/modules/apps.razer.com/synapse/gms-proxy/static/js/main.fd0c50a8.js) 把 StartScan／StopScan 直接交给 SignalR 的 `HubConnection.invoke`，两者没有额外参数；真实目标是托管 Hub、Scanner 和 Controller。Windows 原生 `GetGMS3Info` 负责取得服务端口与版本，不能把它当成原生扫描实现。当前原生主体、托管程序集和代理原件的来源分别保存在 [原生捕获](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/capture.json)、[托管捕获及包来源](../../../.analysis/results/host-current/managed/capture.json) 和 [代理解析收据](../../../.analysis/results/frontend/gms-proxy-current/analysis.json)；本段只补充这些当前原件的静态语义。

| Windows 服务发现边界 | 完整函数与实际行为 |
| --- | --- |
| 导出与主体 | `GetGMS3Info` ordinal 8，RVA `0x73860 → 0x69f00`，ABI 为无参数、返回窄字符字符串指针；返回缓冲由本 DLL 分配，消费者须调用 FreeMalloc |
| 注册表打开与返回 | [主体 `0x69f00..0x6a46e`](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/functions/69f00.json) 固定读取 HKLM 的 `SOFTWARE\WOW6432Node\Razer\Services\GMS3`；首次以访问掩码 `0x20119` 打开，失败便返回 port=0、空 version、reason=`not found`。该 reason 同时覆盖打开错误，不能独立证明服务未安装。成功后关闭首个句柄，再经包装器以 `0x20019` 重新打开 |
| 值读取 | [`0x5c680..0x5c84d`](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/functions/5c680.json) 依次服务于 Version 和 ServerPort：先查询字节数，再申请 `(cbData+1)*2` 字节、只清零 `cbData+1` 字节；第二次查询返回码、实际注册表类型及重新打开结果没有共同的成功判定，随后按 UTF-16 NUL 扫描复制 |
| 端口与版本 | ServerPort 非空就经 `0xa13b8..0xa1468` 的十进制转换，未检查有效端口范围；为空时仅在 Version 非空且数值版本低于 3.1 时返回 5000，否则为 0。版本比较 `0x63340..0x6361a` 使用 `0x7a020..0x7a22f` 按点分段、给较短序列补字符串 `0`，逐段转换为整数再有符号比较，不能替换成普通字典序 |
| JSON、分配与释放 | [`0x69b70..0x69e74`](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/functions/69b70.json) 把版本转成 UTF-8，手工拼接 `serverPort/version/reason`，没有 JSON 字符串转义；申请长度加一的缓冲、清零并复制。正常主体释放临时字符串并关闭包装器中的注册表句柄；返回指针的释放仍由宿主承担 |

RVA `0xe2d70` 的包装器初值是空句柄和 HKLM，不能把其他注册表根表套进本入口。主机的 GetGMS3Info 分支使用 `io.callDLL`；默认 FFI 消费者在 readCString 成功后才设置释放标志，故字符串复制异常仍存在与图标链相同的释放缺口。代理平台层只在返回值存在时 JSON.parse；版本或返回格式异常不能降级为“服务不存在”。`createGmsClient` 先要求 port 为真，再仅对精确版本 `3.0.12.55` 使用 SDK 默认的 `127.0.0.1:55235`，其余使用发现的端口。SDK clientVersion 为 5；创建返回 true 只表示已经创建并发起初始化，连接配置成功须另外由事件确认。服务 JSON 协议配置了字符串枚举转换器，代理的 `GS_SUCCESS`／`GS_SCAN_IN_PROGRESS` 也是字符串；StartScan 中 `if (!result)` 不是对 GS_SUCCESS 的比较，失败字符串同样不会据此复位 hasScan。

排队添加具有独立的请求和锁边界。`addGameToUserGameList(path, remoteRequest=false, requestId="")` 在持有 main mutex 时调用服务；收到 GS_SCAN_IN_PROGRESS 后再取得 pending mutex，仅保存 `{action, args:path}`，没有保存原 remoteRequest／requestId，也没有去重。finished 消费者不检查 scanCompleted，重新读取 LGL、UGL、运行列表并发布后，持 pending mutex 逐项 await 同一个添加方法，正常遍历结束才清空队列。重试因此使用默认的 remoteRequest=false，不能把第一次“扫描中”结果当成随后添加成功的关联收据。扫描完成回调的前几次读取还处在队列 try/catch 外，异常会使队列根本没有进入重试。

这两把锁的完整实现以 Promise 链先后放行，释放函数必须由持有者调用，没有重入、超时或取消机制。添加的取锁顺序是 main→pending，而 finished 重放是 pending→main；并发添加可以形成逆序等待。若重放中的服务再次返回 GS_SCAN_IN_PROGRESS，重放会等待自己尚未释放的 pending 锁。这里确认的是条件等待环，实际触发次数和服务时序仍未运行验证。正常 finished 先于服务切 stopped；新的扫描、停止后的迟到事件和断开重连也没有共同 scanId，因此不能假定重放一定避开扫描中状态。

[当前服务注册](../../../.analysis/results/host-current/managed/3aa42301ef78/source/Razer/GameManagerService/Service/ServiceRegister.cs) 有 21 个固定扫描策略，OS major≥6 时再加入 UWP，共 22 个；相同 OS 条件也选择真实 UwpHelper。22 类均覆盖基类 ScanAsync，不能把基类内部递归调用自己的虚方法当作当前实际扫描默认路径。Controller 从新建 scope 取得这些 scoped 策略并保存复用，但完整构造器没有保留或 Dispose 该 scope。基类 StopScan／ResetStopScanFlag 只用 ReaderWriterLockSlim 修改停止标志；token 不进入各 ScanAsync 的参数，取消实际依赖策略主动检查标志。

已继续核对的策略说明停止粒度不能统一推断：[DefaultExe](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyDefaultExeSearch.cs) 和 [UserFolderExe](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyUserFolderExeSearch.cs) 只在外层游戏循环检查，内层驱动器、目录和路径检查并不逐步取消；[UWP](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyUwpSearch.cs) 先枚举包、Xbox 安装和本机缓存，再在两组游戏循环检查，文件递归没有同一停止参数。其 Impersonator 有 using 释放，不能据此推断整个扫描均可取消。[Fever](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyFeverPlatformSearch.cs) 的完整覆盖方法没有停止检查，也没有把构造的签名送入 AddFoundGame 或图标队列；注册表根、父项和子项没有 using／Close。不能为了凑齐来源覆盖，把这个分支描述成已经发布了扫描结果。

Controller 的异常和等待边界另由 [完整 IL](../../../.analysis/results/host-current/managed/955d4bbd372f/il.stdout.txt) 复核：OnStartScan 包装入口为 MethodDef `0x0600033f`／RVA `0x15720`，其 MoveNext 为 `0x06000ab8`／`0x38a28`。它使用 AsyncVoidMethodBuilder，但完整 MoveNext 没有 await 挂起点，正常主体仍同步执行到末尾；异常在 `IL_0b42` 进入 SetException，不能由外层 Action 调用的 catch 证明所有扫描异常已转成 finished=false。外层 worker 为 `0x06000aa4`／`0x38730`，finally 的 Unpause 不等于超时策略已 join。MoveNext 在 `IL_0583` 调用 WaitAll，`IL_0588` 直接 pop 布尔结果；之后没有因超时取消或等待旧任务退出。图标任务还会逐个 Start、无超时 Wait、读取 Result，停止标志只在开始每项前检查。末尾重置共享策略停止标志、清空映射也没有代次校验，不能证明迟到策略不会访问本次已清理的数据。

扫描结果的广播进一步分离了消费者和提交：`NotifyScanFinished` 为 `0x06000330`／`0x1519b`；Broadcaster 的异步消费者 MoveNext 为 `0x06000c11`／`0x443cc`，按 16 项向 Clients.All 发送，不携带请求 ID。代理只凭末段的 `offset + 本段长度 == totalSize` 分派完成，offset=0 时重置累计，没有检查连续 offset、累计总长或所属扫描；dispatchAsync 也没有被 SignalR 回调 await。服务内存 isScanning 由状态事件同步更新，添加消费者先读该状态，随后才可能修改设置、UGL 和文件；因此收到完整分段、排队动作返回、服务停止及文件保存必须分别比较。

本链测试应覆盖：注册表打开／二次读取失败与异常类型、空或非法端口及版本转义、特殊版本 URL、连接配置失败；分段重复／缺段／交错和合法空段；扫描请求失败后保留的标志；无主表、等待超时、停止与图标等待交错；排队请求关联丢失、重入等待和并发锁顺序；通知早于停止／落盘、写盘失败及旧任务迟到。顺序用例使用事件屏障控制，不能用延时或宽松比较掩盖差异。当前原生七个完整函数的 chunks／指令与原 PE 相符，托管方法 token、RVA 和方法体长度已从原件核对；这只增加静态证据。独立 Rust 扫描所有者、全部策略依赖闭包、实际 DLL／服务对照、取消后资源收束及窗口验收仍未由本段完成。

### 本机扫描的路径、去重与结果发布

本段继续读取同一当前程序集的七个策略及其 [公共基类](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyBase.cs)，并将 C# 输出与托管捕获清单的实际字节逐项核对。七个 `ScanAsync` 包装器和对应完整 `MoveNext`、一个 InstallLocation getter、十个基类方法共 25 个 MethodDef 的 token、RVA、tiny/fat 头和 code size 已由原 PE 元数据独立复核。这里没有执行程序集，也没有把反编译源码审阅算作全部 IL 指令已独立解码。

| 实际策略 | 路径与分支 | 停止、失败和发布边界 |
| --- | --- | --- |
| [RegistrySearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyRegistrySearch.cs) | 只处理有 RegistryKey 和 RegistryPathKeyName 的主表记录，排除带 LaunchParams 的 Steam 和 Riot；HKCU 在 SystemId 非空时替换为 HKEY_USERS。先读指定值，再按名称搜索；原路径有扩展名时也作为候选，随后加入默认目录、Game 子目录和架构候选 | 仅每个主表游戏开头检查停止。候选使用默认 HashSet 比较器，取第一个 FileExists 项；单游戏异常仅日志后继续。`DefaultExe64` 非空分支实际仍取 `DefaultExe32`，不能凭字段名修成另一条查找规则 |
| [DefaultRegistryKey](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyDefaultRegistryKey.cs) | RegistryKey 存在、RegistryPathKeyName 为空且非 Steam；先读 InstallLocation，搜索回退却传原来的空 RegistryPathKeyName。按双引号拆分后，现存文件且其大写路径包含大写 DefaultExe 即接受；目录分支仍使用拆分前原字符串拼路径 | 每游戏检查停止、每游戏 catch；候选 HashSet 使用 CurrentCultureIgnoreCase，与 RegistrySearch 不同。64 位字段门禁后的实际 getter 同样为 DefaultExe32。路径包含、扩展名判断和目录候选不能归一为通用 exe 后缀验证 |
| [UninstallSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyUninstallSearch.cs) | 主表 GameGUID 非空且非 Steam，读取固定 HKLM Uninstall 下带花括号 GUID 的 InstallLocation；依次加入普通／交叠路径修正及默认目录候选，再取首个存在文件 | 外层游戏检查停止，GetInstallLocation 自行捕获读取异常并返回 null，候选处理另有 catch。此链不是 Add App 的四路注册表枚举，不据此补出 HKCU 或全部 Uninstall 项扫描 |
| [GameUXRegistrySearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyGameUXRegistrySearch.cs) | 先取得完整 GameUX 信息表，再按主表 GameGUID 匹配。优先 AppExePath，其次 ConfigGdfBinaryPath，条件是大写文本包含 `.EXE` 且文件存在；复制主表设置后补 registry 字段 | 取得信息表发生在游戏循环与停止检查之前；每项异常日志后继续，正常末尾 Clear。这里的 Contains 不等于严格后缀匹配，GameUX 底层注册表及资源解析须继续单独核对 |
| [GameUXInfoSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyGameUXInfoSearch.cs) | 预先读取 GameUX、Origin、GOG、Uplay 四组数据，再遍历 GameUX。路径优先级与上述一致，但直接使用 File.Exists；来源按 Origin 路径前缀、GOG GUID、Uplay 路径前缀、Direct 的顺序选择 | 停止只在 GameUX 项开头检查，预读取没有同一停止参数。路径前缀用 CurrentCultureIgnoreCase，未核目录边界；GOG 身份分支实际把当前 SourceExId 传入 CreateEntryGuid，不可自行改成另一个 GUID 字段。正常末尾清四组集合，异常是否到达清理须按所在层判断 |
| [ItchIoSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyItchIoSearch.cs) | 先读本机数据库列表，再按标题建立 EntryGUID，经 SpecialGuidMappings 的 GameGuid 项 GetOrAdd；GameGUID 则为 Guid.Parse(item.Id)，两种身份不同；检测方式为 ITCHIO_DB | 每游戏才检查停止，读取与 Guid.Parse 不在每项 catch 内。无有效 GUID 的项可使该异步方法 fault，不是忽略该项继续；成功项进入共享 AddFoundGame，不直接保存 LGL |
| [TwitchSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyTwitchSearch.cs) | 同样先读本机列表、按标题及特殊映射形成 EntryGUID，再解析 item.Id；另保留 LaunchUrl、RegistryKey 和 RegistryValue，检测方式为 TWITCH_DB | 停止粒度和无每项 catch 与 ItchIo 相同。两者的 async Task 包装器及完整 MoveNext 均没有 await 挂起点；同步数据库／文件工作不会因为方法名含 Async 自动变成可取消 I/O |

两处架构 getter 的差异另外核对了 IL 原字节和 MemberRef 名称：RegistrySearch 的 `MoveNext 0x3e200` 在 `IL_02e4` 读取 DefaultExe64 作门禁、`IL_02f7` 调用 DefaultExe32；DefaultRegistryKey 的 `0x3b5b4` 对应 `IL_02fa/IL_030d`。四处均为实际 `callvirt` 指令，不能将其当作反编译器变量命名失误。两个完整方法分别为 MethodDef `0x06000b82/0x06000b22`，code size `1181/1220`；其余五个 MoveNext 为 `0x06000bba@0x40bb0/865`、`0x06000b61@0x3d20c/610`、`0x06000b5f@0x3cdbc/998`、`0x06000b77@0x3d7b0/532`、`0x06000bb8@0x40920/574`，均为 12 B fat header。对应七个包装器位于 `0x1d78c/0x1b5f4/0x1f06c/0x1d140/0x1d0cc/0x1d694/0x1eff0`，独立方法体仍在[完整 IL](../../../.analysis/results/host-current/managed/955d4bbd372f/il.stdout.txt)中保留。

本机列表读取也有不同失败粒度。[ItchIoGamesDataReader](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/ItchIoGamesDataReader.cs) 在模拟用户上下文取得 `%APPDATA%\\itch\\db\\butler.db`，退出该上下文后再打开 SQLite，查询 caves 与 games，解析 verdict 并只收 FileExists 的路径；切换上下文失败时以当前上下文重算路径。[TwitchGamesDataReader](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/TwitchGamesDataReader.cs) 从 CommonApplicationData 下 `Twitch\\Games\\Sql\\GameInstallInfo.sqlite` 查询 Id/ProductTitle，再查对应 HKLM Uninstall 的 DisplayIcon，并直接 FileExists，未采用 Add App 的图标编号截断规则。两查询均无 ORDER BY；一个数据库循环中出现异常会结束本次读取并返回已收集前缀，不能称整库为空或每个坏项都被跳过。连接和 command 有 using，reader 没有独立 using；底层 SQLite/provider 的全部关闭和故障路径不因这些包装器已读而自动完成。

公共 AddFoundGame（MethodDef `0x060003a6`、RVA `0x19cbc`、1411 B code）先按 EntryGUID 将签名放入 FoundGames。重复身份只补缺字段并立即返回，不再正规化路径、补图标或通知；新项随后才处理路径、主表覆盖、launch helper、图标、MachineID 与 LastInteractTime。路径正规化异常保留原路径；图标异常由独立方法记录。第二次去重使用 `(LaunchExe, Steam SourceExId 或空串)` 的原值 tuple，比较器由 Controller 的实际调用链注入：`MoveNext 0x38a28` 的 `IL_0329/IL_032e` 构造 [CaseInsensitiveTupleEqualityComparer](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/CaseInsensitiveTupleEqualityComparer.cs) 和带该比较器的并发字典，再经 GameScanListData 传给策略。其 Equals 对两个字段分别使用 CurrentCultureIgnoreCase；GetHashCode 将 null 按空串取哈希后 XOR，Equals 则直接比较原字段，不能把这条路径当成默认 tuple 比较或自行改为 OrdinalIgnoreCase。旧项属于主表时移除本次新身份并返回，否则移除旧身份后尝试更新路径索引；TryUpdate 失败只写日志，不能补成原子事务。原 IL 的 GetOrAdd、CopyMissing、两次 TryRemove、TryUpdate 和 NotifyGameFound 共七处调用原字节及方法名另已核实。FoundGames 是并发容器不代表插入后对可变签名的字段补齐、两张索引和通知构成一个原子提交，失败或迟到任务也未在这里回滚整条记录。

CreateEntryGuid（`0x060003a8@0x1a288`）把来源前缀和标题／平台 ID 拼接，经 Encoding.Default 与当前固定类型 key 做 HMAC-MD5，逐字节大写 X2 后交给 Guid(string)；不能替换为 Guid(byte[]) 的内存布局或任意 UTF-8／大小写正规化。清理标题只删除源码列出的九类字符，不做 trim、空白折叠或小写；空输入返回空 GUID。新方法含 Epic 专支，旧方法 `0x060003a9@0x1a434` 没有，调用方不可混用。NotifyGameFound 只同步转发事件；Controller 消费者把对象加入 CurrentScanningGames、标记设置改变并继续通知，随后 finished、stopped 和 LGL/GSL 保存仍按前节各自发生。单项发现事件不代表列表持久化、自动配置档切换或设备写回成功。

这批需要的独立用例包括两种注册表候选顺序与比较器、空值搜索参数、64/32 getter 差异、`.EXE` 文本包含与目录前缀、数据库前缀返回、无效 GUID、同身份补缺、同 exe 不同 Steam ID、路径大小写、两张索引更新交错、插入后异常及停止后的迟到通知。七个策略之外的源、底层 GameUX／路径与图标依赖、全策略并发、独立 Rust 所有者和实际 DLL／服务／设备对照仍须继续；25 个方法身份核验与十一份相关 C# 文件字节核验均不算这些功能测试通过。

### Steam、BattleNet 与 Epic 的实际本机输入

本段保留 GMS 3.14.0.1109 的 GameManagerServiceLib 与 GameScannerCommon 详细证据；当前 3.15 的变化以前文对应专题为准。前者两份原件及全部 246 个捕获输出、后者两份原件及全部 107 个捕获输出均已按实际字节长度和 SHA-256 对上现有 [托管捕获清单](../../../.analysis/results/host-current/managed/capture.json)，没有执行程序集。四个 ScanAsync 包装器和完整 MoveNext、Steam 公共基类、两个实际 Reader／Parser 及其直接文件处理方法已结合 C# 和完整 IL 核对。方法 token、RVA、tiny／fat header 和 code size 另从原 PE 元数据读取；关键分支的 call／callvirt operand 与 pop 也从原方法体字节复核。这里的边界是静态语义，不是 Rust 或实际 DLL 测试通过。

| 扫描策略 | 实际来源及身份／路径 | 停止、失败与发布 |
| --- | --- | --- |
| [SteamLibrarySearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategySteamLibrarySearch.cs)（Lowest） | 先读 libraryfolders.vdf，再读 appcache/appinfo.vdf，随后才枚举各库 *.acf。仅 Installed 位存在且 MissingFiles 位不存在的 manifest 进入后续；以 AppId、Name 为主，UserConfig 为缺值备选，显式排除 228980、250820、205790。安装目录为库目录下 common/InstallDirectory；身份由 SteamId 映射与 CreateEntryGuid(STEAM, AppId) 取得，再参与 LaunchExe 映射 | 库和每个 manifest 开头检查停止；单 manifest 主体异常日志后继续。appinfo.vdf 的打开／完整解析在逐项 catch 之前，失败可直接 fault 本策略，不能称坏 appinfo 会退回 *.acf 继续。正常候选经 SteamBase 的覆盖方法进入公共 AddFoundGame，检测方式 STEAM_LIBRARY |
| [SteamShortcutsSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategySteamShortcutsSearch.cs)（High） | 递归搜索 SteamPath/userdata 下所有 shortcuts.vdf，没有按传入 userId 只选一个 Steam 用户。实际解析要求 Name、ExePath、StartDirectory 三项均非空；策略构造 DIRECT 签名时只取 Name 与 ExePath，没有把 StartDirectory 接入启动参数。以标题创建 GUID，经 LaunchExe 映射及主表 launch-helper 匹配决定最终身份 | 文件枚举与读取阶段无停止参数；每个 shortcut 开头才检查。GetSteamShortcuts 有外层及逐文件 catch，签名处理另有逐项 catch；CanSkipGame 为真跳过，否则公共 AddFoundGame，检测方式 STEAM_SHORTCUTS。正常末尾 Clear，不在 finally |
| [BattleNetSearchV2](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyBattleNetSearchV2.cs)（Low） | 从 CommonApplicationData/Battle.net/Agent 递归取 Agent-*.log，按 LastWriteTimeUtc 降序取 3 个、Reverse 后并行解析，最后结果 Reverse、Take(20)。排除 bna／agent／clnt、无 Product／Binaries／InstallDir 项。实际扫描是“每个主表 BATTLENET 项 × 每个日志游戏”的两层循环；主表没有对应来源项时不会独立遍历日志列表 | 只有内层每个日志游戏开头检查停止，不能取消前面的递归日志枚举或两个 Parallel.ForEach。单游戏主体异常日志后继续；GetBattleNetData 的外层异常丢弃本轮数据。接受的路径必须存在，CanSkipGame 后公共 AddFoundGame，检测方式 BATTLENET_AGENT_DB_V2 |
| [EpicLauncherInstalledList](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyEpicLauncherInstalledList.cs)（Normal） | 先依次调用 GetInstalledGames 和 GetLocalInstalledEpicGames，再判断前一列表是否 Any；真正遍历的是后一组 *.item manifest。跳过 AppName 以 UE_ 开头、精确 plugins／plugins/engine 分类、IsApplication=false、缺失或不存在 InstallLocation 的项；主表按 LaunchUrl 匹配 | 每 manifest 开头检查停止；两个 Reader 预读取都在停止检查前。无逐 manifest catch，路径／集合字段／递归枚举等未被更内层处理的异常可 fault 整个策略。末尾只清 installedGames，非 finally。经公共 AddFoundGame 发布，检测方式 EPIC_LAUNCHER |

后三个策略的完整 MoveNext 没有 await 挂起点；它们的方法名与返回 Task 不会使同步注册表、文件和并行循环自动可取消。四种发现事件都继续沿公共发布链处理，不能将某个来源已找到解释为 LGL 已落盘、配置档已应用或设备已确认。

Steam 的 [SteamHelper](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/SteamHelper.cs) 同时读取 HKLM InstallPath 与 HKCU SteamPath，优先返回非空 HKLM 值；这一步不验证目录存在。GetLibraryFolders 先要求 steamapps 目录及 libraryfolders.vdf 存在，using FileStream／StreamReader 读取并经文本转换器处理；读／转换异常留下 null 内容。反序列化先取旧格式，结果为 null 或 ContentStatsID 为 null 时再取子对象格式；随后直接设置 MainLibraryFolder，没有再次检查备用结果是否为 null。所谓 GetLibraryFolders1To7 实际保留 0 到 7 全部八个槽位。GetAppManifests 先主库，再按槽位添加现存的 SteamApps 目录；每文件读／转换异常只置空，但反序列化调用位于该 catch 外、整库 catch 内，若该接口调用抛出则结束当前库剩余文件。实际绑定的字符串反序列化器会捕获通常的 JSON 格式错误并返回 null，所以不能把普通坏 JSON 一概写成结束整库。已打开的文件／reader 均有 finally Dispose；没有将同一个停止信号传进这些文件操作。

[SteamDataToJsonConvertSerializer](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/SteamDataToJsonConvertSerializer.cs) 是当前 DLL 的逐行转换算法：先丢第一行；引号数为奇数时继续拼下一行；替换空引号和被引号包围的花括号，按是否同时含左右括号选择分隔符，再用上一行缓存决定逗号。遇空行即结束，没有独立的完整 VDF 语法树或转义校验。读取 StringReader 有 finally Dispose，转换异常交调用方，不可在 Rust 中凭文件扩展名换成另一种解析算法后宣称已与当前 DLL 一致。

[SteamAppInfoData](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/SteamAppInfoData.cs) 从 FileShare.ReadWrite 的本地文件解析二进制缓存：UInt32 高 24 位必须为 480836，低字节只接受 39 到 41，随后读 Universe；41 版先按 Int64 偏移读取字符串表再回到原位置。每 app 读取 ID、块长、状态、Unix UTC 时间、token、20 B hash、change number，40／41 版另读 20 B hash 覆写前字段，再由二进制 KV serializer 解析，要求流位置精确等于块尾，ID=0 终止。外层 FileStream 和内层 BinaryReader 都在 finally 释放；内部名为 ReadAsync 的主体没有 await，也没有停止 token。GetLaunchExe 依序测试 launch 0、1、2、3、重复的 3、4、5、6，使用 else-if；前一项存在但不以小写 exe 结尾时，不会再试后一项。该方法捕获异常返回空字符串，GetAppType 则不包同样的 catch。

SteamLibrary 的可执行文件候选先尝试主表的 DefaultFolderExe／DefaultExe 路径修正，再尝试本机 appinfo 中 Type 精确等于 Game 的启动项及 Win64-Shipping 路径，最后才读本机 SteamGameInfo.db。路径修正会从第一个路径移除与第二个路径相交的组件，再合并；它不是普通 Path.Combine。缓存 LaunchData 按原顺序逐项试 File.Exists，首个命中才写 LaunchExe 与 LaunchParams，此方法不按 OSArch 再筛选。主表和本机 appinfo 的各分支与 SQLite 的失败边界不同，不能合并成“始终用一个可靠启动路径”。

本策略有三处实际丢弃的判断。MoveNext `0x06000bb4@0x3f2d4` 在 `IL_0334` 取 IsSupportedSteamAppAGame 的 bool 后 `IL_0339 pop`；`IL_0463` 调 CanSkipGame 后 `IL_0468 pop`。前者实际仅检查 228980 和 SQLite 是否有记录，不等于再次读取远程目录。SQLite 命中分支另在 `IL_08a0` 把局部 flag 置 false，于是 `IsLaunchExeExists || !flag` 在不抛异常时恒为 true，后面仍另有 LaunchExe 非空检查。上述实际字节不能改写成三个严格筛选门槛。LaunchExe／SteamId 冲突更新仅发生在源码相应分支，图标补提取在正常扫描循环中也只位于 guid != entryGuid 块中。

在线补全须和本机扫描分开。[GetSteamGameInfoFromSteamServer](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategySteamLibrarySearch.cs) 只有失败列表非空才把可解析的 SourceExId 转为 uint 列表，调用 SteamCmdProcessor 的 SteamKit 流程。库／manifest 循环因停止跳出后，仍到 `IL_0c87` 调用该方法，后续逐失败项的停止检查在 `IL_0dcc`，因此不能称停止能阻止已积累项进入网络阶段。SteamKit 列表重载创建 15 秒 CancellationTokenSource，仅将 token 交 Task.Run；委托内 Connect(3)、LoginAnonymous(5)、GetSteamAppInfoAsync 没有收到该 token；本包装链也没有显式 Disconnect 或 Dispose 该实例／token source。远程目录及匿名登录属于本项目剔除的联网业务；可保留有依据的本机 manifest／appinfo／本地 SQLite 能力，不能把远程缺失伪造为已扫描成功。

远程结果消费者同样不能按日志文案推断：`IL_0e52` 的 IsLaunchExeExists 为 false 时，TryRemove 之后仍落到 `IL_0ec8` 的 AddSteamInfoToSqlite，再提默认路径／图标并到 `IL_0fbd` AddFoundGame；没有立即 continue。SQLite 插入的返回 bool 在 `IL_0f20/0f25` 再被丢弃。SteamCmdProcessor.SteamAppInfoToDBAdapter 虽收集 Windows 或无 OSList 的 launch 数据，却在 `IL_0173` Serialize 后 `IL_0178 pop`，没有设置返回对象 LaunchData；不得在记录官方结果时自行补上。SQLite getter 用参数化 SELECT，按固定列读取并要求 SteamGameId 相等；打开／查询错误返回 null。插入是参数化 INSERT，没有 upsert，打开／执行异常返回 false；connection、command 和 getter reader 有各自 finally Dispose。正常末尾清 SteamApps、结果字典和两组列表，不是包住整个策略的 finally。

Steam 的 [公共覆盖方法](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategySteamBase.cs) 只对 STEAM 且 SourceExId 非空的签名重算身份，设置 steam://rungameid/Id 并补注册表／Publisher／图标，再进入 Base.AddFoundGame。读取 DisplayName 非空才返回注册表 key。复制注册表图标失败被吞掉后仍设置目标 DisplayIconPath，目标目录创建却位于这个复制 catch 外，所以不能把该字段赋值当作新图标复制成功。GameSteamInfo 另从 Registry32、64 的 HKLM Uninstall 收 Steam App 条目；64 位分支跨库路径替换可多次 Add 同一个可变对象。OpenBaseKey 和 OpenSubKey 返回的 RegistryKey 没有独立 using／Dispose，不能将枚举器的 finally 当成注册表句柄已释放。

延后图标提取 `ExtractIcon 0x06000411@0x1e490` 创建 LongRunning Task 后只 Enqueue（`IL_00cd`），不在策略内 Start；实际启动和结果消费由 Controller 接续。任务先用共享提取器，再递归 EnumerateFiles，经 AsParallel().AsOrdered() 过滤 exe／ico 后试标题、单 ico、版本信息、任意 exe。各阶段和逐 exe 的提取入口有停止检查，递归枚举本身没有同一 token。标题缩写 lambda `0x06000ba5@0x3ed95` 实际调用 Enumerable.Append<char> 后 pop，没有向外部 StringBuilder 添加；版本信息排序 key `0x06000bae@0x3ef40` 返回 num > 0 的 bool，不是 num。IconExtractor 与输出 FileStream 有 finally Dispose，但取得的 Icon 没有独立 using；DestroyIcon 仅在 Save 正常完成后的同一 try 内，Save 异常会绕过它。最终仍以目标文件存在决定 DisplayIconPath，不能据此证明本次生成成功或完整的原生图标资源链已验收。

[SteamShortcutsDeserializer](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/SteamShortcutsDeserializer.cs) 实际将整个文件用 StreamReader 读成文本，按 STX 分块，再按 NUL／SOH 分词；标签 AppName、exe、StartDir 使用 CurrentCultureIgnoreCase，值删除所有双引号。文件打开／读取异常记录后用空内容继续解析；reader 和 stream 均 finally Dispose。这是当前实现的分割算法，不能仅因文件名为 binary VDF 就描述成完备二进制 KeyValues 解码。

BattleNet 的实际 [LogParser](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/BattleNetAgentLogParser.cs) 以 FileShare.ReadWrite 打开日志，按文件 Length 分配缓冲，单次 Stream.Read 后 `IL_0044 pop` 丢弃实际读数。用 Request GET /game/ 之后文本作共享字典 key，日期先试 [I 与 ] 间的首段，再试首个空格段；都失败则 DateTime.MinValue。较新日期的 TryUpdate 返回值也被丢弃，不能把并行解析描述成严格选取最新记录。`IL_0171 Contains("{")` 后 `IL_0176 brtrue` 返回继续读行，实际为“含左括号就继续”，并非“读到左括号为止”；随后人工添左括号并按字符计数。外循环在 `IL_0219/0220` 比较底层 MemoryStream.Position 和 Length，未用 reader.EndOfStream；不能忽略 StreamReader 预读和单次短读的测试边界。

日志文件／片段读取 catch 会清本文件片段，所有文件 stream、memory stream 与 reader 有 finally Dispose。之后 Parallel.ForEach 位于该 catch 外；实际绑定的 JSON 字符串解析器将通常格式错误转换成 null，lambda `0x06000bd7@0x428a0` 在 `IL_0013` 明确跳过 null，正常对象才加锁入列表。GetBattleNetData 的 includedInstallDir.Contains 位于锁外，列表 Add 和 set Add 使用两个不同锁，故结果顺序及去重不能推断成原子且稳定的“最近 20 个唯一游戏”。主表匹配后的候选先试默认文件和递归 FindFile；检查 DefaultExe64 非空的 `IL_02b8` 后，`IL_02cb` 仍实际读 DefaultExe32。日志路径回退则 OS64 且 64 位相对路径非空便选它，文件不存在时不再自动改试 32 位。未知主表项按版本资源选标题、先创建 GUID 再去 ™／®，两步顺序不能交换；还会访问 Binaries.Game，Binaries 非空并不保证子对象非空，异常粒度仍为单游戏主体。

Epic 的实际 [Reader](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/EpicLauncherInstalledGamesReader.cs) 从 HKLM 的 AppDataPath 取得根路径。GetInstalledGames 去尾反斜线、上移两级，再读 UnrealEngineLauncher/LauncherInstalled.dat，经实际 IJsonSerializer 解析 InstallationList；方法自己的 catch 返回空列表。GetLocalInstalledEpicGames 则从 AppDataPath/Manifests 枚举 *.item，逐文件用 File.ReadAllText 与 System.Text.Json 解析并捕获该项异常；Path.Combine、Directory.Exists、GetFiles 位于逐项 catch 前，不能概称枚举错误只跳单项。扫描策略没有使用 manifest 的 LaunchExecutable／LaunchCommand，也未根据 bIsIncompleteInstall／bIsExecutable／LaunchElevated 构造额外门槛。

Epic 有主表项时，先核安装目录末组件与 DefaultFolderExe 首组件相等，再试父目录下 DefaultFolderExe／DefaultFolderExe32 组件；未命中才接本机游戏数据路径辅助方法或 InstallLocation/DefaultExe。无主表项时先看顶层 exe，再递归 exe，过滤当前黑名单；恰一个便选它，否则取枚举首个并保留所有候选，没有按名称排序。多个候选依次调用 LaunchExe 映射 GetOrAdd，GUID 可能被后续候选再次改写；RunningExe 用替换过安装根的相对形态加 | 拼接。

Epic 的 [ProductId](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/EpicGameManifest.cs) 在 BaseURLs[0] 存在时取按 / 拆分的倒数第二段，否则用 AppName；只读本地字段，不在该 getter 发网络。LaunchUrl 为 com.epicgames.launcher://apps/AppName?action=launch&silent=true。旧身份用 CreateEntryGuidOld(EPIC, AppName)，新身份通常用 CreateEntryGuid(EPIC, ProductId)，命中旧 LGL／FoundGames 冲突时按源码改用 AppName 或另一来源再造身份。OldEntryGUID 的 UGL 和 LGL 两阶段均先查旧 GUID、再查正规化的 LaunchUrl；后一阶段可能覆盖前一阶段赋值，不能当两个互斥迁移分支。最终再覆写 DisplayName、InstallSize、SourceExId=ProductId 后发布。

本批的最小独立用例应包含：双注册表来源优先级、两种 libraryfolders 格式与 null、槽位 0／7、VDF 空行／引号／反斜线；appinfo 39／40／41、未知头、块尾不符、短读／截断和 launch else-if；manifest flags、三个屏蔽 ID、各候选顺序、两个被丢弃 bool、GUID 冲突和图标任务未启动；SQLite 缺记录／坏 LaunchData／重复插入与返回值消费者；停止发生于读取前／库间／网络调用前后的具体边界；shortcuts 跨用户及缺 StartDir；日志短读、reader 预读、括号读取、同 key 日期竞争、set／列表两锁交错和空 Binaries.Game；Epic 两列表门槛不一致、分类大小写、未用 manifest 启动字段、exe 枚举顺序、多候选映射、ProductId 尾斜线、两阶段旧身份迁移和逐项／整策略失败。每个用例先保留当前 DLL 的原始结果，再与独立 Rust 实现比较；发现的官方异常行为和本地优先删除的联网行为须分别记录，不得暗中修正期望值。本段没有执行行为测试、DLL／服务对照或窗口验收，底层 KV／SQLite provider／原生图标叶及完整 Rust 生命周期仍不能因本段静态审阅而计为完成。

本段独立核对 Lib 的 39 个、Common 的 4 个所选方法身份与方法头；与前述集合有交集，不直接累加为覆盖总数。下表定位完整方法及其原 IL。

| 当前程序集 | 完整方法 | MethodDef / RVA / code bytes | 原完整 IL 行 |
| --- | --- | --- | --- |
| ServiceLib 955d4bbd372f | SteamLibrary ScanAsync 包装器 / MoveNext | 0600040f / 1e3a4 / 90；06000bb4 / 3f2d4 / 4649 | 99572；97729 |
| 同上 | SteamShortcuts 包装器 / MoveNext / GetSteamShortcuts | 06000421 / 1ee78 / 81；06000bb6 / 40640 / 629；06000422 / 1eed8 / 208 | 101316；100998；101370 |
| 同上 | BattleNetV2 包装器 / MoveNext / GetBattleNetData / 并行 lambda | 060003b6 / 1b26c / 81；06000b15 / 3a01c / 2114；060003b7 / 1b2cc / 248；06000b14 / 39ed4 / 274 | 81812；80829；81866；80662 |
| 同上 | LogParser.ParseLog / 并行反序列化 lambda | 06000453 / 1f9a4 / 619；06000bd7 / 428a0 / 63 | 107013；106933 |
| 同上 | Epic 包装器 / MoveNext | 060003dd / 1c128 / 97；06000b46 / 3bf54 / 2824 | 87863；86744 |
| 同上 | Epic Reader 两方法 / ProductId / LaunchUrl | 060004b5 / 2041c / 188；060004b6 / 204f4 / 170；06000480 / 20170 / 117；060004a1 / 20314 / 41 | 109979；110083；108734；109374 |
| 同上 | SteamBase.AddFoundGame / GameSteamInfo / GetAppManifests | 06000403 / 1da18 / 385；06000405 / 1dc34 / 889；06000406 / 1e020 / 438 | 95612；95845；96232 |
| 同上 | SteamHelper.GetLibraryFolders / GetSteamPath | 06000870 / 2a944 / 232；06000872 / 2aa8c / 161 | 158947；159125 |
| 同上 | SteamAppInfoData.ReadAsync MoveNext / ReadDataFromFile MoveNext / GetLaunchExe | 06000c92 / 4734c / 579；06000c94 / 475e0 / 218；06000809 / 29180 / 1043 | 148821；149142；149511 |
| 同上 | SteamAppManifest.IsInstalled / AppStateFlags | 0600055e / 2179c / 53；06000563 / 217f4 / 19 | 115068；115168 |
| 同上 | 延后图标入队 / ExtractIconFromExe / 缩写 lambda / 版本排序 lambda | 06000411 / 1e490 / 216；06000414 / 1e790 / 187；06000ba5 / 3ed95 / 15；06000bae / 3ef40 / 187 | 99691；100062；96839；97157 |
| 同上 | IsLaunchExeExists / GetPossibleExePath / Server MoveNext | 0600041b / 1ed18 / 140；0600041a / 1ec24 / 215；06000bb2 / 3f164 / 311 | 100767；100657；97499 |
| 同上 | SteamCmd GetSqlite / AddSqlite 的 MoveNext / DB adapter | 06000cc0 / 4943c / 1005；06000ca2 / 4792c / 1027；06000832 / 29d8c / 411 | 155369；151964；156810 |
| 同上 | SteamKit 列表重载 MoveNext / Task.Run 内 async lambda MoveNext | 06000cb8 / 48f40 / 446；06000cea / 4a8f0 / 319 | 154696；151108 |
| Common e07813af888b | Steam text converter / shortcuts deserializer | 060003f3 / 9ea0 / 558；06000401 / a240 / 454 | 29275；29911 |
| 同上 | 实际 JSON Path / String reader 主体 | 060003d8 / 9a20 / 121；060003da / 9ad4 / 133 | 28335；28439 |

### 解析依赖与发现通知的完成边界

当前 [服务注册](../../../.analysis/results/host-current/managed/3aa42301ef78/source/Razer/GameManagerService/Service/ServiceRegister.cs) 将 IJsonSerializer 绑定为 [NewtonsoftJsonSerializer](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/NewtonsoftJsonSerializer.cs)，但该类的实际实现使用 System.Text.Json。RegisterService 的 `MethodDef 0x06000033@0x2f88` 在 `IL_0104` 注册此绑定；Serializers 的构造器与 getter 只是保存和返回注入的对象，不能按类名另选 JSON 库。三个完整反序列化方法 `0x060003d6@0x9940/167`、`0x060003d8@0x9a20/121`、`0x060003da@0x9ad4/133` 均添加 JsonStringEnumConverter，knownTypes 参数没有消费者。字符串入口独有 `NumberHandling=AllowReadingFromString`；文件入口直接使用 FileStream，字节入口只读一次固定 4096 B 缓冲、丢弃实际读取长度，再以整缓冲和 isFinalBlock=false 构造 Utf8JsonReader。这些入口不能合并成相同的长度、数字字符串和失败语义。主体 catch 记录异常并返回 null，流通过 finally 释放；options／converter 的构造在该 catch 之前，不能称所有异常均已吞掉。解析 null 仍须由各调用者解释，不代表正常空列表。

Epic 与独立平台策略调用的 `GetMihoyoGamedata2` 是另一条本地文件链，不经过上述 serializer。其完整方法 `0x060003b3@0x1adfc/559` 以 Open／Read／ReadWrite 共享模式打开文件，再用 UTF-8 StreamReader 读取；打开和 ReadGameDataItem 位于逐记录 JSON catch 之外。`ReadGameDataItem 0x060003b4@0x1b084/361` 按字符扫描标题和对象：标题仅收 `[0-9A-Z_a-z]`，只在前一字符小于空格时接受起始 `{`，前一字符初值为 255；对象内部跟踪引号、反斜杠转义和花括号深度。因此文件开头直接是 `{`、普通空格分隔、截断对象及字符串内花括号，必须按实际状态机分别比较，不能改成任意文本中的 JSON 提取。记录解析读取 installPath、gameBiz、gameInstallStatus.gameExeName、packageName、gameShortcutName；四个必需字符串非空且目录存在才收集，packageName 非空时追加到 GameId，FullGamePath 由路径拼接得到，此层没有确认 exe 存在。每条解析／字段错误记录后继续，末尾按 GameId 的默认比较去重；文件读取异常则向调用者传播。此链没有停止参数，文件读取、解析、调用方 FileExists 和 AddFoundGame 的完成点分别保留。

发现通知的实际顺序是基类事件 → Controller `0x0600033e@0x156e8` → GameScanner `0x0600032e@0x1515b`。Controller 丢弃 CurrentScanningGames.TryAdd 的 bool，仍标记 gameSettingsChanged 并继续通知；GameScanner 先同步调用 GameFoundEvent，之后才调用 GSL 的 UpdateGameSetting（`0x060000b7@0x508c`）。[Broadcaster](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameManagerServiceBroadcaster.cs) 同时订阅扫描器和列表管理器的发现事件，其异步消费者 `0x06000c0f@0x44298/252` 向 Clients.All 发送 OnGameFoundEvent，实际存在 await 挂起点，但事件发布者不会等待此 async void 方法完成。因此消息可能已发出或仍在发送，GSL 更新尚未完成；一次发现事件也不等于单次持久化提交。取消订阅只删除后续事件入口，未持有这份发送任务作 join；注销方法的一个外层 catch 还意味着前面步骤异常时，后续注销不保证执行。

[GslDataManager](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GslDataManager.cs) 的 UpdateGameSetting 修改／创建缓存设置，并丢弃 SetGameSettingsCache 返回码。SetGameSettingsCache（`0x060000c0@0x5e10/249`）在缓存没有相应分区时仍可保持初始 GS_SUCCESS，而不添加记录；缓存分区存在时才设置通知标志。其异常 catch 可返回 GS_FAILED，但已设置的通知标志仍会在 finally 解锁后触发事件；返回码、事件和实际缓存变化须分别核对。SaveGameSettingsCache（`0x060000cb@0x651c/501`）在后续扫描结束链单独执行：先合并同路径项的 overlay 属性，再交 CryptoXmlSerializer 写文件，异常只记录，finally 解锁，返回类型为 void。其文件读取、重试与 Dispose 顺序见[加密设置文件](#加密设置文件的读取重试与释放)；耐久性与框架内部仍未验证，不能据 void 返回或发现通知推定写盘成功。此处的本地游戏设置也不是设备实测设置，不能触发未授权的设备默认值覆盖。

本段对 Lib 的 18 个、Common 的 9 个及服务 EXE 的 1 个所选 MethodDef 身份／方法头独立核验，包含与前述集合重叠的方法；完整 IL 与相关 C# 产物分别核对当前捕获摘要，关键构造／调用另核原 PE 字节，未声称逐指令独立解码全部 IL。测试须分别覆盖 4096 B 边界、数字字符串、解析 null 的消费者、文件共享与读失败、分帧的引号／转义／截断、GameId 大小写、路径比较器区域设置、重复发现、广播挂起／失败、缓存缺分区及保存失败；这些是待实现的行为验证边界，本段没有执行程序集、DLL、服务或设备，也不提升实际对照状态。

### Arc、Bethesda、EA、GOG、Mihoyo、Riot 与 Uplay 的实际本机扫描

本段保留 GMS 3.14.0.1109 中已注册七个策略的详细证据，不重复公共 AddFoundGame、GUID、事件消费者及 LGL 落盘链，也不重复 GetMihoyoGamedata2 的记录解析。该版本三份程序集输入、22 份直接相关 C# 和三份完整 IL 的实际长度与 SHA-256 均已对上现有 [托管捕获清单](../../../.analysis/results/host-current/managed/capture.json)。七个 ScanAsync 包装器与 MoveNext、Reader／Parser 和公共文件／注册表适配器结合源码及 IL 审阅；所选方法的方法头、大小、直接调用、分支和异常区间另核原字节。旧批没有独立解析泛型 MethodSpec 类型参数，其证据范围保持原样；当前版本差异与已解析的真实 DI 绑定见前文两个专题。Arc／Bethesda／EA／Mihoyo 的所选链已据新版复核，Riot／Uplay 的当前变化以前文为准；没有执行厂商程序集、扫描本机注册表或游戏目录。

实际 [ServiceRegister](../../../.analysis/results/host-current/managed/3aa42301ef78/source/Razer/GameManagerService/Service/ServiceRegister.cs) 将 IRegistryWrapper→RegistryWrapper、IPathWrapper→PathWrapper、IGameSignatureHelper→GameSignatureHelper，以及 BethesdaGamesReader、UplayConfigurationParser 和 UplayConfigurationDataReader 注册为 Singleton；七个策略均为 Scoped。它们通过 [GameScannerStrategyDependencies](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyDependencies.cs) 获得这些实际实现，不是仅根据接口名称猜测行为。Arc／GOG／Uplay 优先级为 High=4，Bethesda／EA／Mihoyo／Riot 为 Normal=3；完整 IL 的 Priority attribute blob 分别是 `01 00 04 00 00 00 00 00` 和 `01 00 03 00 00 00 00 00`。实际执行顺序仍由前述 Scanner 调度层决定，不能把 DI 注册顺序当作最终顺序。

| 策略 | 实际本机来源及发布内容 | 停止、异常与清理边界 |
| --- | --- | --- |
| [ArcRegistrySearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyArcRegistrySearch.cs) | HKLM Registry32 的 `SOFTWARE\Perfect World Entertainment\Core\`；读取 CLIENT_PATH、INSTALL_PATH、LAUNCHER_PATH、PATCHER_PATH。签名 LaunchExe=ClientPath、LaunchHelper=LauncherPath、SourceEx=ARC；经 CanSkipGame 后进入 AddFoundGame，检测方式 ARC_REGISTRY | 整组注册表读取和主表索引先于逐记录停止；GetArcData 有逐项及外层 catch，返回已收集结果。ScanAsync 的逐签名主体没有 catch；正常末尾 Clear，不在 finally |
| [BethesdaRegistry](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyBethesdaRegistry.cs) | 先经实际 BethesdaGamesReader 确认 launcher 和读取本机 Uninstall 记录，再按主表 LaunchUrl 或 exe 候选发现；检测方式 BETHESDA_REGISTRY | Reader 完整预读取先于停止，策略每个游戏开头才检查。exe 枚举／路径／映射没有逐游戏大 catch；只有图标复制有局部 catch。正常末尾 Clear，不在 finally |
| [EARegistrySearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyEARegistrySearch.cs) | 13 个 HKLM Registry32 产品树的 Product GUID 与同视图 Uninstall 子项名 join；使用 DisplayIcon 作为实际 LaunchExe，SourceEx=ORIGIN、GameGUID=解析值；检测方式 EA_REGISTRY | 两批注册表及主表索引先读完，GetEAData 每个产品项才检查停止，有逐项及外层 catch。ScanAsync 再逐项检查停止，但没有逐项 catch；正常末尾 Clear |
| [GogRegistrySearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyGogRegistrySearch.cs) | HKLM Registry32 的 `SOFTWARE\GOG.com\Games\`；exe／launchParam／gameName，签名 SourceEx=GOGGALAXY、SourceExId=注册表末组件，并保留 LaunchParams；检测方式 GOG_REGISTRY | GetGogData 先读整组、再逐项停止及 catch，外层 catch 返回前缀。ScanAsync 无逐项 catch；正常末尾 Clear |
| [MihoyoPlatformSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyMihoyoPlatformSearch.cs) | 先 GLOBAL、CN launch helper，再完整读取 GLOBAL、CN 的所有本机 gamedata 文件；只对主表存在的 MIHOYO GameId 继续，实际 exe 使用 InstallPath+主表 RunningExe；检测方式 MIHOYO_PLATFORM | 读取和第一轮全部预处理没有停止检查，第二轮逐游戏才检查。无逐项 catch、无末尾 Clear；两轮路径处理或主表字段异常可 fault 策略 |
| [RiotPlatformSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyRiotPlatformSearch.cs) | helper 取 CommonApplicationData 下 RiotClientInstalls.json 的 rc_live，metadata 则固定为 `C:\ProgramData\Riot Games\Metadata`；每个 *.live 目录只取首个 *.yaml。按主表 RIOT RunningExe 找本机 exe，检测方式 RIOT_PLATFORM | 所有文件读取和文件夹集合构造先于停止。每文件夹只在开头检查；局部 catch 仅包主表路径匹配，之后仍构造候选并调用 CanSkipGame。无匹配成功的独立门槛，最终是否发布须结合公共消费者；没有末尾 Clear |
| [UplayConfigurationSearch](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerStrategyUplayConfigurationSearch.cs) | launcher 安装目录的本地 `cache/configuration/configurations` 文本，辅以本机 Uninstall 的 Uplay Install 目录；只尝试第一条 OfflineExecutable、第一条 OnlineExecutable 及二者的目录补全。SourceEx=UPLAY、LaunchUrl 来自 GameId，检测方式 UPLAY_DB | 配置解析、目录和主表索引均先于逐项停止。路径提取及身份／发布各有独立逐项 catch；正常末尾 Clear，不在 finally。OnlineExecutable 只是本地配置字段名，此调用链没有发起在线请求 |

这七个完整 MoveNext 都没有 await 挂起点。方法名和 Task 返回类型不表示它们自动建立后台线程，也没有把 GetStopScanFlag 传入注册表枚举、文件读取或 Parser。Arc／EA／GOG 的 GetData 开头都先进行预读取，每项检查分别位于 `IL_00e9`、`IL_018f`、`IL_00de`；对应 ScanAsync 又在每个候选开头检查。Bethesda、Mihoyo、Riot、Uplay 的逐项检查分别在 `IL_012f`、`IL_0403`、`IL_01d1`、`IL_016f`。IEnumerator 的 finally 只释放枚举器；不能据此声称列表清理、注册表句柄释放或整个同步读取已取消。抛出未由更内层处理的异常会到 MoveNext 的 AsyncTaskMethodBuilder.SetException，先前已发生的映射修改和发布不因此自动撤销。

### 实际注册表与路径适配器

[RegistryWrapper.GetRegistryGroup](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/RegistryWrapper.cs) 直接按传入 Hive／View 打开根和子树，仅枚举下一层子项。每个请求字段调用 GetValue(name, string.Empty).ToString() 并写入 RegistryData，缺值通常成为空字符串；当前子项全部字段完成才 Add。它只有覆盖整次读取的 catch，异常后返回已添加的前缀，既不逐子项继续，也不因一次异常主动清空前缀。不存在的子项跳过；没有排序和停止参数。返回的 RegistryKey 字符串来自子键 ToString()。

GetRegistryGroupForKey 不枚举子项，而是读取指定 key 的 value names，按 CurrentCultureIgnoreCase 建集合，只为实际存在的请求字段写字典；不存在的字段保持缺项，不是统一补空。此方法先创建带传入 key 的 RegistryGroupData，再进入 try，catch 后返回该对象及已写字段。两方法都没有独立 Dispose OpenBaseKey／OpenSubKey 返回的 RegistryKey；原 IL 的 finally 对象是 `List<string>.Enumerator`，不是注册表句柄。Rust 所有者仍须按工作区要求保证资源释放，实际 DLL 对照应记录此官方边界，不得以枚举器清理推断句柄链完整。

ReadRegistryString 将空 key 直接变成 defaultValue，空 value name 或字面量 `string.Empty` 变成默认值名。第一次 Registry.GetValue 及显式 string cast 在内部 catch 外：读取异常、权限错误或非字符串类型可能向上层传播，并非全部静默返回 defaultValue。只有第一次结果为空且 key 以 HKLM 全名开头时，才进入异视图备选；比较采用 CurrentCultureIgnoreCase。32 位进程／64 位 OS 读 Registry64，64 位进程读 Registry32；这个备选内部吞异常，没有独立关闭新打开的 key。该分支也可能返回 null，不能把所有缺失路径归成同一个空字符串结果。

[PathWrapper](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/PathWrapper.cs) 的 DirectoryExists 对 null／empty 返回 false，否则直接 Directory.Exists；FileExists 同样先判空，但会删除路径中的全部双引号再 File.Exists。这个删除只用于存在性判断，不会把调用方保存的路径原地改成无引号形式。GetFiles(path, pattern) 直接 Directory.GetFiles；带 SearchOption 的重载先判断 Directory.Exists，不存在则返回 null。GetDirectories(path) 直接枚举，带 pattern 的重载则先判断目录，不存在返回 null。它们没有独立 catch 或停止 token，调用方的异常层级和 null 消费必须逐条保留。FindFileInDirectory 则按 CreationTimeUtc 降序取 FirstOrDefault，不等同按修改时间或文件名找首项。

[PathExtensions.CanonizePath](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/PathExtensions.cs) 对非空值尝试 `Path.GetFullPath(new Uri(path).LocalPath)`，转换异常返回原字符串，不是拒绝该值或返回 null。[GameSignatureHelper.GetComponents](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/GameSignatureHelper.cs) 对空值返回空数组，其余按 `|` Split，保留空组件，不负责路径分隔符／架构／存在性解析；Bethesda 的两个默认候选列表都继续依赖这一具体行为。

### Arc、EA 与 GOG 的候选和身份

Arc 先接受“LauncherPath 非空且存在”或“ClientPath 非空且存在”；只有两者均不合格时才要求 PATCHER_PATH 存在并把它赋给 ClientPath。这不会保证最后的 ClientPath 本身存在：仅 launcher 存在而 client 为空或为不存在的路径时，仍可进入后续。`IL_01f7/01fc` 只有 ClientPath 与 LauncherPath 相等才清空 LauncherPath；清空后 ClientPath 为空时，才将原 launcher 注册表值赋回 ClientPath。不能把这个嵌套分支改写为无条件的“缺 client 就采用 launcher”。

Arc 的标题先尝试 InstallPath 末目录里最后一个 `_` 之前的部分，且要求该分隔符索引大于零；没有下划线时不会直接采用该末目录名，而是依次试 client 文件名、launcher 文件名、Unknown，再去掉 ™／®。主表 ARC 用 RegistryKey 建 CurrentCultureIgnoreCase 字典，候选精确替换 `\WOW6432Node\` 后查找，否则 CreateEntryGuid(ARC, Title)；发现既有 GUID 属于其他来源时，以该来源和 Title 重建。client 存在时 RegistryValue 固定写 CLIENT_PATH，即使实际 client 来自 patcher。launcher 存在且尚无 RegistryValue 才写 LAUNCHER_PATH。

Arc 的映射必须保留字面行为：client 参与 LaunchExe.GetOrAdd；launcher 先查 LaunchHelperExe，却在命中冲突时对 LaunchExe 字典以 LauncherPath 调 TryUpdate，随后不论这个 bool 结果都采纳映射 GUID（`IL_0434` 取枚举 3，`IL_0456` 取枚举 0，`IL_0467/046c` TryUpdate 后 pop）。同样的“helper 查找→LaunchExe 尝试更新→采纳身份”出现在 EA、GOG 和 Uplay。这些映射修改发生在最终 CanSkipGame／AddFoundGame 之前，停止或后续失败不自动回滚。Arc、EA、GOG 传入的 egidNameGuids 参数在各自 GetData 中没有使用。

EA 固定按 EA Games、EA Sports、BioWare、Crytek、Origin、Sims、PopCap、PopCap Games、Respawn、Maxis、CCCP、Bullfrog Productions、Origin Systems 顺序取 Product GUID，再读取 Registry32 Uninstall 的 DisplayIcon／DisplayName。13 路中 null 接口结果被当作空列表，若调用本身抛出则由外层 GetEAData catch 结束收集；实际 RegistryWrapper 通常已在自身 catch 内吞异常并返回空或前缀，因此不能把普通坏子项一概说成剩余 12 路全部不读。Uninstall 索引按 key 末组件、CurrentCultureIgnoreCase 比较，随后用产品树的 Product GUID 字符串 join，不是扫描任何含 EA 名称的 exe。

EA 对 DisplayIcon 只删除双引号，不剥除常见的 `,0` 图标索引；要求所得路径 FileExists 且 DisplayName 非空。Guid.TryParse 的成功 bool 在 `IL_0241/0246` 被 pop，失败时 GameGUID 是 Guid.Empty；EntryGUID 则独立按主表或标题生成，不能把二者混为一个身份门槛。主表先按 Uninstall key、再按产品树 key 匹配，二者只做源码中的 WOW6432Node 精确替换；未匹配时 ORIGIN+去 ™／® 后的 Title，异来源冲突再按该来源重建。LaunchExe／helper 映射后才得到候选，策略最终把 DisplayIcon 对应路径放入 LaunchExe。

GOG 的 exe 必须非空且经 PathWrapper.FileExists 通过；标题缺失时取 exe 文件名，随后去 ™／®。PlatformID 来自 RegistryKey 按反斜线 Split 后的 Last，末尾分隔符可得到空组件；没有自行解析成数字。主表用 GOGGALAXY RegistryKey 匹配，普通未知项创建 GUID 时用 PlatformID（`IL_0285/028a`）；异来源冲突却用对应来源加 Title（`IL_02b1/02b6`）。不能把两分支统一为“始终用标题”或“始终用商店 ID”。签名继续保存原 launchParam，SourceExId=PlatformID；该层不验证参数是否能实际启动游戏。

### Bethesda 的 Reader、路径和图标

[BethesdaGamesReader](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/BethesdaGamesReader.cs) 先读 HKLM 的 `SOFTWARE\Bethesda Softworks\Bethesda.net` installLocation，读取 catch 置 null；非空后组合 BethesdaNetLauncher.exe 并 FileExists，路径组合及该存在性判断在这个读取 catch 外。GetInstalledGames 对 IsBethesdaInstalled 的调用也位于其主 try 之前，因此此处异常可以直接传播。安装门槛通过后，从 HKLM Registry32 Uninstall 读取 DisplayIcon／DisplayName／Path／UninstallString／ProductID；先对整批执行不锚定、IgnoreCase 的 `bethesdanet://uninstall/\d+` 正则过滤并 ToList，再进入逐记录 try。过滤谓词异常落到整批 catch；单记录字段／路径异常才日志后继续。

每条记录的 Path 去引号并 CanonizePath，要求目录存在、ProductID 与 DisplayName 非空；ProductID 并没有再校验为数字。[BethesdaGameData.GetLaunchUrl](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/BethesdaGameData.cs) 直接拼 `bethesdanet://run/`+ProductId。Reader 无停止参数；正常结果与“已成功完成整批注册表读取”分别判断。

策略按主表 BETHESDA LaunchUrl 建 CurrentCultureIgnoreCase 索引。有主表时，安装末目录与 DefaultFolderExe 的首目录组件进行区分大小写的相等判断，匹配才尝试“父目录+DefaultFolderExe 各个 `|` 组件”，再追加 DefaultFolderExe32 的组件；首个存在路径胜出。没有选到路径时先取顶层、再递归的非黑名单 exe：恰一个取它，多项则取原枚举首项并保留整组候选，没有按名字排序。最终仍要求实际 exe 存在。

命中主表时，CanSkipGame 检查的是该主表对象，再将设置 CopyGameSignatureSettings 到新对象。未命中时先以安装 Path 的末目录名创建 BETHESDA GUID，不用 DisplayName 或 ProductId；异来源冲突后先映射选中 exe，再逐一 GetOrAdd 所有 exe 候选，后面候选可再次改变最终 GUID。逐候选映射时会将保存的候选路径中的安装根替换成 `\`+末目录名，按 `|` 组成 RunningExe，再用 LaunchUrl 映射确定最终身份。不能从多个 exe 中选过第一项就停止核查其余身份副作用。

只有未命中主表的分支在这里赋本机 DisplayName／LaunchUrl／registry／icon／overlay／MachineID。图标先保存 Reader 提供的 IconPath；源文件存在且目标 `<EntryGUID>.ico` 不存在才 File.Copy，复制完成后才把 DisplayIconPath 切到目标。目标已存在时跳过复制，也不在本分支切到目标；复制异常被吞后仍保留原 IconPath。没有在这个复制分支创建目录。原 IL 的图标 catch 只覆盖 `IL_058e–05a7`，不存在覆盖整个游戏的 catch；目标路径组合及此前递归枚举的异常仍可 fault 策略。发布用单独选定的实际 exe 参数，不应把缺省签名字段或 icon 路径当作可执行文件已成功写回的证据。

### Mihoyo 与 Riot 的主表匹配

Mihoyo caller 按 GLOBAL、CN 顺序取得两组 helper，并按 GLOBAL 全部 gamedata 路径、CN 全部路径逐文件调用已核实的 GetMihoyoGamedata2 后 AddRange。之后先完整预处理一次全部游戏，按 `SourceExId.Trim()==GameId.Trim()` 精确匹配主表 MIHOYO 项，并设置 InstallPath+DefaultExe 到 FullGamePath、写日志；第二轮才开始逐游戏停止检查，第二轮又赋同样 FullGamePath。真正 File.Exists 的路径却在 `IL_04b2/04b7` 使用主表 RunningExe+InstallPath，不使用之前 FullGamePath，也不是 gamedata 记录的 GameExe。若 RunningExe 本身包含 `|`，此调用方不会先用 GetComponents 拆分它。

Mihoyo 命中主表后 CopyAllGameSignatureSettings，保留该项 EntryGUID／DisplayName；GameId.Contains("global") 时用 GLOBAL helper，否则 GameId.EndsWith("cn") 时用 CN helper，两个字符串判断都区分大小写。选中的 helper 同时赋 LaunchHelper 与 LaunchHelperExcludables，调用方没有再验证 helper File.Exists。没有主表匹配时仅日志，不按任意本地 gamedata 自主生成新签名。两轮均无逐游戏 catch；Trim 所需字段、路径组合、Reader 直接抛出的异常不能概称跳过一项。blackListedExes 在这里赋值，但没有被实际筛选使用。

Riot 的 GetLaunchHelper 忽略传入 launchHelperPath 参数；它读取 CommonApplicationData/Riot Games/RiotClientInstalls.json，取 JsonNode rc_live 的 ToString，解析／读取异常吞掉并返回空，不在该函数核实 helper 文件存在。主策略 helper 为空就返回。metadata 根目录却硬编码为 C:\ProgramData，不能把这两种路径来源合并描述为都遵循 OS 重定向。*.live 只查顶层，每目录 *.yaml 只取未排序的 FirstOrDefault；全部 settings 先转换为路径后再建默认 HashSet，区分大小写。主表 RIOT RunningExe 索引则使用 CurrentCultureIgnoreCase，二者去重规则不同。

GetFullGameFolderFromSettings 对现存文件 ReadAllLines；行包含 `product_install_full_path:` 且按 `:` Split 长度大于一时，实际却使用 `Split(": ")[1]`，再删全部引号和 Trim。最后匹配行覆盖之前值；格式错或读取异常返回已积累的 text，并在 catch 路径绕过 NormalizePathUnicodeEscapes。正常路径仅在包含 `\\u`+四位十六进制的匹配时替换，lambda `0x06000b87@0x3e788` 用 Convert.ToUInt16(base16) 后 Char.ToString，处理单位为 UTF-16 code unit。它没有提供完整 YAML 解码、一般反斜线转义或 Unicode scalar 校验。

Riot 每个 folder 先建立默认 GameSignature 和空路径，再遍历主表 RunningExe，Path.Join／GetFullPath 后取第一个 File.Exists 命中。若没有命中，默认签名仍保留，路径可能是最后一次失败候选；匹配循环的 catch 结束后无独立 continue，而在 `IL_0285` 新建签名并 CopyAll、赋 EntryGUID／DisplayName／LaunchExe、GetFullPath(helper)、SourceEx=RIOT 和 registry，继续 CanSkipGame→AddFoundGame。这里只证明候选能继续进入公共判断，不证明默认 GUID／不存在路径最终一定成为有效列表项；须沿已核实的公共发布及消费者逐项对照。GetFullPath(helper) 和后续消费者异常在局部匹配 catch 外，不能称逐文件夹失败均被吞。Riot 的 blacklist 同样只赋值、未在此使用。

### Uplay 的本地文本解析与四级路径选择

[UplayConfigurationDataReader](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/UplayConfigurationDataReader.cs) 从本机 launcher Uninstall 的 InstallLocation 定位 configurations 文件；存在时调用实际 Parser，读取／定位异常重置为空列表。getThirdParty=false 的过滤位于这个 catch 之外，只保留 ThirdPartyPlatform==null，空字符串不等于 null，不能写成“空第三方字段均保留”。实际策略固定传 false。

GetUplayGameDirectories 先枚举 HKLM Uninstall 的 Default view，64 位进程再加 Registry32，32 位进程／64 位 OS 再加 Registry64。key 必须精确 Contains("Uplay Install")，取 InstallLocation／DisplayName；末组件删除 Uplay Install 并 Trim 得 GameId。路径先经 Uri.LocalPath→GetFullPath→TrimEnd，再进入只包按 DisplayName 去重及 Add 的单项 try；坏 Uri／路径会到外层 catch，结束本轮而保留已有结果。DisplayName 字典使用默认比较器，首个完全同名项保留；此 Reader 不核实 InstallDirectory 存在，也没有停止参数。

[UplayConfigurationParser](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/UplayConfigurationParser.cs) 以 Open／Read／FileShare.ReadWrite 打开本地文本、ReadToEnd，按 `\nroot:` Split；是逐行自有算法，没有网络请求／解压或完整 YAML 语法分析。流、TextReader 和每块 StringReader 各有 finally Dispose；整个提取只有一个外层 catch，任意未被内部处理的异常都会把返回列表重置为空，而不是跳过坏块或返回此前前缀。CultureInfo.CurrentCulture 及 separator 的初始化在该主 try 前。

Parser 每行 Trim，首次 name 设置原始名字与名字 key；online／offline 切换正在积累的列表。`- path:`，或尚未建立首 path 时的 `path:`，会创建 Executable 并立即消耗下一行，仅识别 relative／register 填 RelativePath；不会直接从 path 同一行取文件路径。append 只填当前 Executable 缺失的 DirectoryAppend。working_directory 同样消耗下一行 register 填 DirectoryRegistry；third_party_platform 消耗下一行并 ExtractValue。localizations 用“两空格且非四空格”的行作文化键，四空格的属性行按冒号拆分且恰两项才收；内外两层字典 CurrentCultureIgnoreCase。内循环还会消耗第一条不再以两空格开头的行，外层不重新处理该行。

原始名字可再通过“名字 key: 值”替换；最终本地化优先当前 Culture 的该名字 key，其次 default，否则保留原名，GameNameIdentifier 单独记录。GameId 只先从所有 offline 的 DirectoryRegistry 中找 `Ubisoft\Launcher\Installs\[0-9]+\`，无结果才读 online 的 DirectoryRegistry；不是从 RelativePath 推导。ExtractGameId 直接 Regex.Match(registry)，null 会抛并触发整个列表清空。ExtractValue 只进行冒号分割、首冒号后截取和引号段选择，不是一般引号／转义解析。[GetLaunchUrl](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/UplayConfigurationData.cs) 对空 GameId 返回空字符串，其余拼 `uplay://launch/<id>/0`；这一层不发起 URL launch。

策略的候选顺序是第一条 offline 的 GetFullPath → 不存在才第一条 online → CanonizePath → 不存在且 Name 精确命中目录字典才以该目录组合第一条 offline → 仍不存在再组合第一条 online。不会遍历第二条 executable；失败时不能擅自扩大候选范围。策略选择要拆的 registry 字符串时采用 RelativePath.Contains("HKEY")，而 [UplayConfigurationExecutable.GetFullPath](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/UplayConfigurationExecutable.cs) 的分支采用 StartsWith("HKEY")，都区分大小写，这两种判断不等价。

Executable 按最后反斜线拆 registry key／value，拆分异常给双 null。先 ReadRegistryString，空结果才去根名前缀后按 HKLM Default、进程对应的另一个 view 调 GetRegistryGroupForKey；即使原 registry 字符串使用其他根名，该备选仍明确选 HKLM。RelativePath 注册表读到相对路径时再结合 DirectoryRegistry。目录组合时 DirectoryAppend 与 relativeFilePath 都先 TrimStart('\\','/')，因此这里不会让前导分隔符直接把根覆盖；异常返回 null，未进行 FileExists／停止检查。与它不同，[UplayRegistryAndDirectory.GetFullPath](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/UplayRegistryAndDirectory.cs) 组合 DirectoryAppend 不先 TrimStart，relativePath 含 HKEY 时读注册表，所得路径 rooted 就直接采用，否则 Combine；catch 置 null，之后再 CanonizePath。它的 ExtractFileName 没有自己的 catch，由调用者路径 try 处理。

策略在路径 catch 中置 text=null，之后独立要求 FileExists，才进入身份／发布 try。主表 UPLAY 按 RegistryKey、CurrentCultureIgnoreCase 建索引；候选 key 是去 WOW6432Node 的 registryKey，key2 却是同样替换后的 registryValue，不是拼回完整键名。`IL_051f` 首次 Dictionary.ContainsKey(key) 之前没有 null 检查，key 为 null 时会进入身份 catch，不会自动退到标题 GUID；首次未命中且 key2 为 null 时同理。否则优先 key→key2→UPLAY+去 ™／® 后的 Name，再依源码处理异来源冲突。

之后先 LaunchExe.GetOrAdd，再查 LaunchHelperExe 并 TryUpdate LaunchExe，最后 LaunchUrl.GetOrAdd 决定最终 GUID。GetLaunchUrl 可以返回空字符串，调用方没有单独拒绝它，空键可能参与共享映射；不能为了让结果看起来正常而在对照期望中自行补一个 GameId。该层只赋 LaunchExe／LaunchUrl／Source／registry 及 overlay／machine，没有从 Executable 增加未出现的启动参数。路径 try 与身份 try 外还有集合和读取阶段，正常末尾 Clear 不是全策略异常清理保证。

### 扫描行为测试边界与旧版方法定位

后续最小用例须覆盖相同本机输入、主表与共享映射初始状态，分别保留官方 DLL 的原始候选、映射变化、事件及其顺序、Reader 返回值、失败和释放，再与独立 Rust 结果比较：

- Registry32／64／Default 分支、空字段／缺字段／非字符串值、根或子项读取失败时的前缀，以及 ReadRegistryString 第一次读取与备选读取的不同异常边界；PathWrapper 的全引号删除、返回 null 与真实 File.Exists 的区别。
- Arc 只有 launcher 存在、client 不存在、仅 patcher 存在、client==launcher、标题含／不含下划线、CLIENT_PATH 归因，以及 helper 映射命中但 LaunchExe.TryUpdate 返回 false 的身份副作用。
- EA 的 13 树顺序、Product GUID 字符串 join、DisplayIcon 引号与 `,0`、Guid.TryParse 失败但独立 EntryGUID 继续，以及两个主表 key 的先后；GOG 的空／尾分隔符 PlatformID、标题清理、常规 ID 与异来源 Title 创建差异、LaunchParams 保存。
- Bethesda launcher 未安装／路径异常、正则未锚定和过滤整批异常、主表目录大小写、默认 `|` 中空组件、顶层与递归多 exe 的枚举顺序和后续 GUID 重写、图标目标已存在／复制失败及实际发布参数。
- Mihoyo GLOBAL／CN 全量预读先后、第一轮预处理发生停止、主表 Trim 大小写、DefaultExe 与 RunningExe 不同或 RunningExe 带 `|`、helper 后缀大小写、主表未命中及日志路径与真正 File.Exists 路径不同。
- Riot OS ProgramData 重定向与固定 C 盘根不同、多个 yaml／重复路径大小写、缺 `: `／重复属性／读取半途异常、UTF-16 单元替换、没有主表项／所有 exe 不存在／路径抛错仍进入公共消费者的完整结果。
- Uplay 当前 Culture 与 default 本地化、缩进及被消耗行、空 ThirdPartyPlatform、null DirectoryRegistry 清整批、GameId 只在第二条 executable 存在、只第一条候选被使用、四级路径选择、Contains 与 StartsWith 不同、rooted／TrimStart 差异、null registry key／value 和空 LaunchUrl 共享映射。
- 各策略预读前／逐项间／CanSkipGame 前后／发布后停止及异常；验证已发布前缀、共享映射副作用、枚举器释放、文件 reader／stream 释放与列表清理分别发生，不得仅比较最终记录数。

以上是由静态语义驱动的用例要求，尚未执行这些行为测试，也没有新增 Rust 独立扫描实现、UI／后端接线、官方 DLL 实际对照或运行时验收。当前原件已获取与这些方法已核语义分别记录；不能据此提升为整个 GMS、所有本机扫描或整个配置档功能完成。实际 DLL 只在显式对照入口中执行，产品仍由独立 Rust 实现承担。

| 3.14 程序集 | 完整方法 | MethodDef / RVA / code bytes | 原完整 IL 起始行 |
| --- | --- | --- | --- |
| ServiceLib 955d4bbd372f | Arc ScanAsync 包装器／MoveNext／GetArcData | 06000399 / 193d8 / 81；06000b09 / 39bb8 / 416；0600039a / 19438 / 1252 | 76924；76688；76978 |
| 同上 | Bethesda ScanAsync 包装器／MoveNext | 060003bb / 1b4b0 / 81；06000b1a / 3a9c0 / 1639 | 82865；82146 |
| 同上 | Bethesda Reader GetInstalledGames／IsBethesdaInstalled／过滤谓词／GetLaunchUrl | 0600047b / 1fe20 / 516；0600047c / 2007c / 69；0600047e / 20140 / 35；06000479 / 1fde0 / 17 | 108293；108543；108619；108195 |
| 同上 | EA ScanAsync 包装器／MoveNext／GetEAData／GetEaRegistryGroupDatas | 060003d8 / 1b7bc / 81；06000b34 / 3bbb4 / 477；060003d9 / 1b81c / 1108；060003da / 1bcf8 / 841 | 85495；85227；85549；86049 |
| 同上 | GOG ScanAsync 包装器／MoveNext／GetGogData | 060003ef / 1d1bc / 81；06000b75 / 3d574 / 491；060003f0 / 1d21c / 955 | 91841；91582；91895 |
| 同上 | Mihoyo ScanAsync 包装器／MoveNext | 060003f5 / 1d710 / 81；06000b80 / 3da88 / 1759 | 93526；92834 |
| 同上 | Riot ScanAsync 包装器／MoveNext | 060003f9 / 1d808 / 81；06000b88 / 3e7b8 / 1002 | 94850；94347 |
| 同上 | Riot GetLaunchHelper／GetFullGameFolderFromSettings | 060003fe / 1d96c / 97；060003fd / 1d8d0 / 125 | 95086；94990 |
| 同上 | Riot Normalize／ContainsUnicodeEscape／DecodeUnicodeEscapes／lambda | 060003fa / 1d865 / 27；060003fb / 1d881 / 22；060003fc / 1d898 / 53；06000b87 / 3e788 / 33 | 94904；94933；94956；94300 |
| 同上 | Uplay ScanAsync 包装器／MoveNext | 0600042a / 1f174 / 81；06000bbf / 40f9c / 2007 | 103418；102501 |
| 同上 | Uplay DataReader GetUplayData／GetUplayGameDirectories／过滤谓词 | 060005d5 / 22204 / 149；060005d6 / 222b8 / 512；06000bdf / 4293f / 10 | 118452；118533；118397 |
| 同上 | Uplay Parser ExtractUplayData／ExtractGameId／ExtractValue | 060005e7 / 22adc / 1508；060005e8 / 23130 / 53；060005e9 / 23174 / 151 | 119696；120412；120449 |
| 同上 | Uplay Executable GetFullPath／GetRegistryKeyAndValue | 060005e2 / 22581 / 51；060005e3 / 225b8 / 55 | 118949；118979 |
| 同上 | Uplay Executable ExtractFromRelativePathRegistry／ExtractPathRegistry | 060005e4 / 2260c / 355；060005e5 / 22798 / 780 | 119036；119233 |
| 同上 | Uplay Directory GetFullPath／GetRegistryKeyAndValue／ExtractFileName | 060005f4 / 23368 / 89；060005f5 / 233e0 / 55；060005f6 / 23434 / 317 | 120813；120892；120949 |
| 同上 | Uplay ConfigurationData.GetLaunchUrl | 060005d2 / 221b3 / 41 | 118279 |
| Common e07813af888b | RegistryWrapper GetRegistryGroup／GetRegistryGroupForKey／ReadRegistryString | 06000357 / 84a0 / 180；06000358 / 857c / 175；06000359 / 8654 / 225 | 24194；24319；24441 |
| 同上 | PathWrapper DirectoryExists／FileExists／FindFileInDirectory | 0600033e / 7dd8 / 17；0600033f / 7dea / 32；06000340 / 7e0b / 46 | 23008；23030；23055 |
| 同上 | PathWrapper GetDirectories 两重载／GetFiles 两重载 | 06000341 / 7e3a / 7；06000342 / 7e42 / 18；06000344 / 7e5d / 8；06000345 / 7e66 / 19 | 23087；23102；23141；23158 |
| 同上 | PathExtensions.CanonizePath／GameSignatureHelper.GetComponents | 0600033c / 7d44 / 37；06000104 / 6104 / 25 | 22821；12967 |
| Service3 3aa42301ef78 | ServiceRegister.RegisterService 的实际 DI 绑定 | 06000033 / 2f88 / 1406 | 3379 |


## 配置档关联和存储

关联动作保存 pid、serialNumber、profileID、category、subCategory、profileName 和 isVisible。新增／替换关联先按 PID＋序列号移除旧项，再追加；取消关联却只查同 PID 的第一项删除，这是原件的不同匹配粒度。切换卡片配置档后更新该关联，不直接调用设备的配置档切换。

关联列表总会写本地 `linkedGames`；原件另按 `currentUserId` 键将副本分区到 `linkedGamesData`。游戏列表使用 IndexedDB 的 localGames，只有该本地分区标识存在时执行写入，并发布 `finish-update-game-to-idb`。这些是现有外部格式及其条件，不是要求本项目增加账户入口。原件 IDB 读取失败的 catch 路径给空列表，也不能当成真实服务返回空列表。

全局配置档菜单的新建、复制、重命名、删除、导入、导出会向对应设备 BroadcastChannel 发送 `userCommand`。具体字段、对话框、宏选择和已核实的设备消费者见 [共享配置档与关联游戏](../shared/profiles-linked-games.md)。键盘默认配置档、板载槽和具体版本分支见 [键盘配置档](../keyboard/profiles.md)；配置迁移是 [独立入口](profile-migration.md)。

## 运行游戏、前台应用与退出

代理从真实服务 started／stopped 事件和当前运行集合维护 runningGames 与 runningGamesDetail，后者包含 entry_guid 及各架构 exe 名称。`gms-running-games` 发布 action、当前游戏 id／名称和完整运行集合；本机前台监控另向 `gms-foregroundWindow` 发布 exe_name 与 runningGamesDetail。真正的自动配置档切换由设备中间件消费这些键及 linkedGames 后执行，Global Profiles 本身没有设备写入任务。

例如 226／235 的消费者还会在游戏／前台事件上先处理 Perfect180，再处理自动切换；不能仅凭全局列表有一条关联，推断所有设备分支都采用同样触发顺序。设备掉线、切换、写入确认和恢复必须继续按对应产品链判断。

GMS 的显式 unsubscribe 移除服务事件并断开客户端；beforeunload 移除 storage／beforeunload 监听、停止前台监控并清空两个适配对象的监听。断开后的 30 秒重连和零延迟发布计时器没有持有可取消句柄，beforeunload 也没有调用该 unsubscribe 方法，不能宣称所有在途工作已终止。

Profiles 页本身存在可核实的释放缺口：游戏桥接类没有卸载方法；数据所有者的 memory-storage 在一个对象注册、却在另一对象解绑；storage 以匿名包装器注册、却以具名方法解绑。window-storage 监听和 Add App 的 click／refresh timer 则正确配对。文档记录当前行为，不把这些缺口改写成已有的可靠取消机制。

## 已挂载页面与 Rust 接线核对

本节把原版实际挂载的页面与本项目现有生产入口分别核对。Shell 在进入 Profiles 时创建并保留 [ProfilesPage](../../../crates/razer-app-pages/src/profiles/profiles_page.rs)，传入实际 ProductWorkspace 列表并观察页面；存在页面或弹窗结构不能替代游戏服务、配置档消费者和真实确认链。以下均为静态结果，未运行窗口或执行设备测试。

| 页面／状态 | 当前官方消费者 | 当前 Rust 生产路径与未完成部分 |
| --- | --- | --- |
| Games／Devices 导航、返回和前进 | 根组件 `Ba` 从 sessionStorage 恢复导航及当前位置，挂载 Games 或 Devices；有映射草稿时按原调用方继续 | 页面有保留的本地导航 Vec、焦点和弹窗阻挡判断，未在该页面实现原版 sessionStorage 导航恢复；Shell 工具栏刷新只调用 `refresh`，其行为是重置搜索／过滤并聚焦，没有发游戏扫描请求 |
| Games 正常、关联、移除、搜索和排序 | `re/oe` 从真实游戏列表渲染卡片；点击普通项打开详情，点击已移除项打开 Add App，排序有名称／最近／频次分支 | 当前 Games 网格只挂载 Add tile；移除过滤时连此 tile 也不挂载。过滤／排序／搜索控件本身存在，尚无真实目录、游戏卡、扫描中列表刷新或移除条目消费者；扫描按钮固定 disabled 并显示服务不可用提示 |
| 游戏详情与关联设备 | `W` 挂载封面、路径、缺失游戏提示、重定位冲突与删除确认，随后挂载普通设备、IOT 子设备或工作室关联卡 | 全局 Games 分支没有挂载这套详情界面；不能用 Devices 页的设备弹层替代游戏详情、封面编辑、重定位、隐藏／删除及对应失败状态 |
| Add App 的正常、刷新、搜索、浏览和关闭 | `f` 挂载后取得已安装程序和 80×80 图标；刷新有 loader，浏览、拖放和行点击进入添加 action；action resolve 后关闭，但发布异常被 action 吞掉，关闭不证明请求受理；卸载仅清除 click 监听和既有刷新 timer | AddGameDialog 保留搜索输入、焦点及返回焦点、关闭事件和两类弹层几何，但程序列表是空容器；刷新与浏览固定 disabled，未接程序枚举、文件选择、行点击添加及拖放。搜索输入能编辑不代表添加功能已实现 |
| Devices 卡片与产品图片 | `Ga` 检查 activeProfile/profiles；普通设备与 IOT 的可见子设备各有分支，图片使用 edition/layout/sidepad/wristrest，名称含 editionName | [devices.rs](../../../crates/razer-app-pages/src/profiles/profiles_page/devices.rs) 仅从当前 workspace 生成普通设备 tile；没有上述 IOT/subDevices 输入，editionName 行没有实际值，附件图像分支仍需补齐 |
| 设备关联游戏弹层 | `Ua` 的配置档下拉只改变关联目标；游戏 tile 按所选 profile 显示 active/busy、缺失状态、封面和最多 6 个可见设备图标，更多项有 tooltip | DeviceGamesDialog 保留 profile/filter/sort/rename/菜单状态与焦点；切换选择不激活设备 profile。游戏来源只遍历各 workspace 已保存的 `profile_linked_games` 并按 exe 归一化，仍会保留解绑后的已知项；没有 GMS 目录、真实 missing/removed、封面、运行频次及多设备图标数据。最近／频次排序没有对应数据，Removed 结果固定空，只保留 Add tile |
| 新建、复制、重命名、删除 | 原版菜单发送设备 `userCommand`，删除在关菜单后延迟 100 ms 显示确认；各产品消费者继续决定存储／设备操作 | Rust 已有菜单禁用、延迟删除、点击外部／Esc 关闭确认、32 UTF-16 单元名称输入、blur/Enter 提交及返回焦点。动作进入 ProductWorkspace 的配置集合／元数据变更；成功说明文本明确仍是本地草稿，并通过 Shell 顶部未保存入口继续保存，不能据弹层更新登记设备已写入 |
| 配置导入／导出、宏子选择和取消 | `Ia/Da` 为每个配置保留自己的宏选择，文件兼容、损坏、warning 与提交分别处理；最终由对应设备命令消费者工作 | [transfer.rs](../../../crates/razer-app-pages/src/profiles/profiles_page/transfer.rs) 和 [transfer_view.rs](../../../crates/razer-app-pages/src/profiles/profiles_page/transfer_view.rs) 已有真实文件选择／后台读取、busy/error/warning、配置与宏勾选。关闭会取消 Transfer、推进 generation，父弹层还校验 workspace、上下文、所选 profile 及事件实体；导入／导出仅交给 Source owner，Existing owner 返回明确错误，各产品后续写入仍需独立核对 |

当前配置转移 Dialog 明确设置 `on_cancel=false`、`on_ok=false`，因此 Esc／Enter 不会通过组件默认决策关闭，关闭按钮与 Cancel 才调用 `dismiss`。设备和添加弹层使用 `on_close` 恢复保留的返回焦点，设备切换或父弹层关闭会取消转移并丢弃旧上下文结果。这些结论核对了 Cargo.lock 锁定的 GPUI Kit revision `1676826d9d49b90f4d7b6808b69924f58796f6f1` 的 base Dialog：Cancel 只有在 `on_cancel` 返回 true 时才调用 `on_close`；焦点陷阱与业务取消是两条独立机制。文件读取取消不代表已经提交到产品所有者的导入任务被撤销，后者仍需沿 SourceProfileImportRequested 和实际收据消费者核对。

视觉依据还包括当前 [9449 CSS](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/css/9449.b8e7f39a.chunk.css)：详情设备卡宽 240、高 230，设备图高 110，edition 12px/#707070；游戏封面 290×150，普通图标 60×60；详情最小高 500，局部分支最大宽 800。active、hover、pressed 的绿色边框／阴影、missing 红色提示和封面刷新 disabled 都是不同状态。Rust 只有设备卡和部分弹层的对应布局，尚无完整游戏详情树；本轮没有窗口、截图或像素验收，不能把部分尺寸相同写成 1:1 还原已完成。

后续自动化矩阵必须分别覆盖导航恢复／草稿继续、真实游戏列表和缺失分支、扫描中取消、添加失败不伪造列表、各删除确认路径、重定位冲突、封面取消与保存失败、同 PID 不同序列号、IOT／附件图片、配置菜单、逐配置宏选择，以及弹层切换后的旧结果拒绝。纯组件测试、服务模拟和实际 DLL／设备对照分别保留结果；当前页面静态存在不计作这些矩阵已经通过。

新增的具体用例还须覆盖：刷新时枚举空返回及图标失败；多文件添加的不同完成顺序和发布失败；正常取消与主机缺失响应；拖放到无效目标及卸载后的监听；扫描失败后无 finished、无关列表事件结束 loader、finished=false；重定位首项不是 exe、`.EXE`／`.exe` 后缀差异、完全相同路径冲突，以及有／无 linkedGames 记录时的迁移与清空差异。正式静态收据的负例只验证上述证据和结论不可被篡改，不等同于这些 Rust 功能测试或实际 DLL 对照已通过。

## 原件与范围

所有来源读取均校验 acquisition 的实际 bytes 和 SHA-256；以下为半开 UTF-8 字节区间。

旧 Profiles 整页收据和 gms-proxy 收据于 2026-10-21 核对入口、asset-manifest 和 manifest，当时主脚本分别为 `main.776df2b1.js` 和 `main.fd0c50a8.js`。两套 `declared_references_fetched` 仍为 false，缺失 map 和动态依赖未决项继续保留。下面的旧页面范围只解释该收据；本机应用链已由前文的 2026-10-31 部署和独立正式字段接续，不能把旧前端身份推广为当前部署身份。

本轮静态收据由 `python tools/workspace.py audit-host-local --query profiles` 维护，使用 `--verify-only` 独立复核，结果写入 [local-audit.json 的 profiles 字段](../../../.analysis/results/host-current/local-audit.json)。复核读取实际原件、完整 AST 节点、导出 getter、词法绑定、官方 ASAR 容器和获取收据，不调用收集器，也不执行厂商脚本。当前获取收据与旧完整捕获所记录的 acquisition 摘要已经不同：Profiles 为 923234 字节／`c8e3592055161a57f004e87418a01006b931a4e2623e6a81e342cc7abaf335c2`，代理为 35208 字节／`a299c9cfc2f2c19b51dae78f8f87072be921e9c750f44be27568097e5b96d8e6`；捕获仍保留其旧摘要。查询重新校验部署原件与捕获输出的实际字节，保留原分析失败和获取未决项，不把修改收据摘要当成更新源码。

| 来源 | 阅读范围 |
| --- | --- |
| [Profiles main.776df2b1.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/main.776df2b1.js) | 游戏请求 actions `[80386,83251)`，添加 `[82077,82311)`、重定位 action `[83209,83250)`；扫描 action `[1810110,1811054)`；平台桥接完整类 `[1558931,1605004)`，文件选择 `[1603906,1604539)`；GMS 桥接 `[2054899,2059998)`，替换 `[2058722,2059119)`；数据所有者 `[2060409,2067675)`；本地 reducer／持久化 `[2072585,2076984)`，列表提交 `[2074306,2074703)`、重定位 reducer 分支 `[2076768,2076870)` |
| [9449.8f17b519.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9449.8f17b519.chunk.js) | 设备关联卡 `[3921,7485)`；封面 `[7696,11011)`；游戏详情 `[11820,17940)`；游戏页面 `[21322,27177)`；Devices 页 `[197233,198773)`；根导航 `[198774,201343)` |
| [9473.f9ed3133.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9473.f9ed3133.chunk.js) | Add App `[13735,17924)`，枚举 `[14053,14884)`、Browse `[14885,15099)`、添加关闭 `[16566,16753)`、卸载 `[17246,17366)`；关联游戏卡 `[52540,57613)`；过滤工具条 `[58769,62307)`；拖放完整所有者 `[121777,124156)` |
| [gms-proxy main.fd0c50a8.js](../../../.analysis/inputs/frontend/gms-proxy-current/raw/apps.razer.com/synapse/gms-proxy/static/js/main.fd0c50a8.js) | 本机服务 SDK 列表／修改 `[93906,100096)`；API 消费者 `[164292,166232)`；连接／事件／扫描 `[166243,178365)`；列表读取 `[178388,180294)`／`[181340,183291)`；添加 `[183860,189276)`；运行集合／删除 `[189308,193421)`；封面 `[197654,200682)`；订阅、发布与释放 `[203444,212245)` |

旧 profiles 收据中的添加／选择主机消费者来自 4.0.827 官方包。原件 [main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js) 为 60830 字节／`0e3b84c11dd3e6d06072f54fb3295404894afcd7d2f7cce814ac20d9cd7d074e`，文件选择分支 `[50590,50921)`、异步分派 `[53689,53746)`；[ffiMain.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/ffi/ffiMain.js) 为 5412 字节／`b6eff823bfb4e19b1cb341ef6a840e1004e57490c40e93f51789bcc67e4bcd9d`，完整异步消费者 `[2767,5375)`。该收据保留其实际来源，不改写为 4.0.896；当前应用枚举链另核前文的新 Host、ASAR 字节和返回消费者。

当前本机应用原生证据复用 [SysUtilsNative 1.2.3.0 完整捕获](../../../.analysis/results/native/bc0387a0913c882a9cd82aa1a8e147d6408fc7bb7f77519f4f8860f16c5b17c8/capture.json)，实际 PE SHA-256 为 `bc0387a0913c882a9cd82aa1a8e147d6408fc7bb7f77519f4f8860f16c5b17c8`。IDA 9.1／Hex-Rays 9.1.0.250226 保留全部 3882 个函数的汇编和 3881 个伪代码；`0xabcbc` 的 call analysis failed 原记录保留，所选 40 个函数没有伪代码失败。正式字段校验全部捕获输出、所选函数的完整字节区块、实际导出和 HKLM/HKCU 根表；RVA 与语义集中在前文，避免并列维护旧 Windows 进度表。全模块捕获完成与所选语义完成分别记录。

其余服务内部扩展段落保留 GMS 3.14.0.1109 的原静态证据、方法身份和链接。GOG 及共享扫描／去重／结果／停止链、22 个实际注册策略的所选方法版本差异已按前文接续到 3.15.0.1200；未选方法和未复核消费者仍保留旧版本边界。原件、提取别名与 C#／IL 输出身份由[托管捕获收据](../../../.analysis/results/host-current/managed/capture.json)分别记录，不因增量捕获覆盖旧结论。PE machine 与 CLR IL-only 标记分别解释，不能仅据 PE32 推定实际进程架构。本批没有运行服务或程序集。

| 托管所有者 | 完整方法与反编译行定位 |
| --- | --- |
| [GameManagerServiceHub.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameManagerServiceHub.cs) | 本地分段 349–383；用户分段 562–596；删除 701–716；Start／Stop 838–868；配置／连接映射 891–919、941–1007；连接／断开 1009–1058 |
| [ProcessValidator.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/ProcessValidator.cs) | 进程与签名核对 78–159；常量来源为同目录 GmsConstant.cs |
| [GameClientSessionManager.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameClientSessionManager.cs) | 本机分区、目录与 SID 128–197、219–307；封面目录 391–394 |
| [GameListAndSettingsManager.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameListAndSettingsManager.cs) | 手动添加完整主路径 711–1067；设置事件转发 1387–1396；其他 URL／UWP／内部签名消费者仍须按独立方法覆盖 |
| [UglDataManager.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/UglDataManager.cs) | 目录／读取与缓存 41–163；删除 200–269；保存 271–311 |
| [GsiDataManager.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GsiDataManager.cs) | 封面与隐藏 433–483；[GslDataManager.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GslDataManager.cs) 设置 320–465、缓存保存 755–802；[LglDataManager.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/LglDataManager.cs) 保存 184–237 |
| [GameScannerController.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameScannerController.cs) | 调度／停止／尾部完整方法 105–427；状态分派见同目录 GameScanner.cs、GameScannerScanningState.cs、GameScannerStoppedState.cs |
| [GameManagerServiceBroadcaster.cs](../../../.analysis/results/host-current/managed/955d4bbd372f/source/Razer/GameManagerServiceLib/GameManagerServiceBroadcaster.cs) | 列表／设置广播 88–197；扫描广播 381–420；注册和解绑 558–663 |
| [CryptoXmlSerializer.cs](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/CryptoXmlSerializer.cs) | 文件读取 50–110、文件写入 161–250；[XmlSerializer.cs](../../../.analysis/results/host-current/managed/e07813af888b/source/Razer/GameScannerCommon/XmlSerializer.cs) 文件写入 155–242；GameInfo.cs 38–65 核对删除后的空信息语义 |

这里闭合页面到服务客户端，并扩展到 Windows 程序枚举／图标、连接门禁、主要本地列表／设置持久化及扫描调度。全部扫描策略、URL／UWP 分支、运行进程识别和各设备自动配置档写入仍须各自连读；没有运行厂商 JavaScript、启动服务测试、操作真实设备或据此声明 Rust 独立实现／实际 DLL 对照／运行验收完成。
