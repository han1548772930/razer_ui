# 全局 Profiles、游戏扫描与本机关联

全局 Profiles 管理本机游戏列表、设备配置档与游戏的关联，以及配置档命令弹窗。设备摘要、关联记录、选中项和设备实际激活的配置档是不同数据：在设备卡片里选择配置档，只改变关联目标；新建、删除或导入则发给相应设备中间件，由其持久化和设备任务继续处理。

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

Games 页支持全部、具可见关联设备、已移除三种过滤；搜索使用 trim 后的小写名称。排序分别为名称升／降序、`ATT_UserLastInteractTime` 和 `ATT_UserTotalGameplayCount`。已移除页点击项目打开 Add App；常规页点击项目打开游戏详情。

## 添加、删除与封面

Add App 调用本机已安装应用列表和图标接口，按 name＋path 去重，展示时按大写名称排序；搜索不区分大小写。浏览支持平台对应的 app／exe 及 url，拖放路径也进入同一 addGame action。动作发送 `{action:"addGame",payload:{filePath}}`，页面不凭请求生成一条游戏记录。

代理把手动路径转换为 GameSignature，标记 MANUAL 并关闭 overlay；lnk 先经本机接口解析。服务若报告条目已存在，代理按 launch exe／helper、各架构默认 exe 或本地 url 快捷方式目标查已有项；隐藏项通过 `SetHiddenState(false)` 恢复。若服务正在扫描，路径进入单独互斥保护的 pendingActionList，扫描完成后重试。实际列表仍依赖服务事件或后续读取。

删除时，MANUAL 项调用 `RemoveGameInUserGameList`，自动发现项调用 `SetHiddenState(true)`；成功隐藏后读取所属列表并发布更新。代理另把实际结果发布到 `gms-remote-action-result`，携带 action、requestId、游戏签名或路径及 result。当前 Profiles 页面只写入删除请求，随后就从 reducer 移除游戏；它没有在该 action 中等待此结果频道。isMissing 项可直接本地移除，因此不能把卡片消失写成服务删除已确认。

游戏详情的删除确认同时维护关联和 removedGames。removedGames 内部按 id 去重；持久化只保留非本地项，原件判断为 `length<=20` 后追加，实际最多 21 项。游戏恢复后会从 removedGames 移除。

封面编辑接受 jpg／png，读取本地文件、转换并缩放为 300×150，先更新本地预览，再向代理发送设置／移除封面请求。代理调用服务的 `SetCustomGameCover`／`RemoveCustomGameCover`，只在成功时刷新所属游戏列表。预览更新与服务确认属于不同步骤，取消文件选择不发设置请求。

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
| Add App 的正常、刷新、搜索、浏览和关闭 | `f` 挂载后取得已安装程序和 80×80 图标；刷新有 loader，浏览、拖放和行点击进入真实添加 action；添加拒绝只记录错误，成功受理后才关闭；卸载清除 click 监听和刷新 timer | AddGameDialog 保留搜索输入、焦点及返回焦点、关闭事件和两类弹层几何，但程序列表是空容器；刷新与浏览固定 disabled，未接程序枚举、文件选择、行点击添加及拖放。搜索输入能编辑不代表添加功能已实现 |
| Devices 卡片与产品图片 | `Ga` 检查 activeProfile/profiles；普通设备与 IOT 的可见子设备各有分支，图片使用 edition/layout/sidepad/wristrest，名称含 editionName | [devices.rs](../../../crates/razer-app-pages/src/profiles/profiles_page/devices.rs) 仅从当前 workspace 生成普通设备 tile；没有上述 IOT/subDevices 输入，editionName 行没有实际值，附件图像分支仍需补齐 |
| 设备关联游戏弹层 | `Ua` 的配置档下拉只改变关联目标；游戏 tile 按所选 profile 显示 active/busy、缺失状态、封面和最多 6 个可见设备图标，更多项有 tooltip | DeviceGamesDialog 保留 profile/filter/sort/rename/菜单状态与焦点；切换选择不激活设备 profile。游戏来源只遍历各 workspace 已保存的 `profile_linked_games` 并按 exe 归一化，仍会保留解绑后的已知项；没有 GMS 目录、真实 missing/removed、封面、运行频次及多设备图标数据。最近／频次排序没有对应数据，Removed 结果固定空，只保留 Add tile |
| 新建、复制、重命名、删除 | 原版菜单发送设备 `userCommand`，删除在关菜单后延迟 100 ms 显示确认；各产品消费者继续决定存储／设备操作 | Rust 已有菜单禁用、延迟删除、点击外部／Esc 关闭确认、32 UTF-16 单元名称输入、blur/Enter 提交及返回焦点。动作进入 ProductWorkspace 的配置集合／元数据变更；成功说明文本明确仍是本地草稿，并通过 Shell 顶部未保存入口继续保存，不能据弹层更新登记设备已写入 |
| 配置导入／导出、宏子选择和取消 | `Ia/Da` 为每个配置保留自己的宏选择，文件兼容、损坏、warning 与提交分别处理；最终由对应设备命令消费者工作 | [transfer.rs](../../../crates/razer-app-pages/src/profiles/profiles_page/transfer.rs) 和 [transfer_view.rs](../../../crates/razer-app-pages/src/profiles/profiles_page/transfer_view.rs) 已有真实文件选择／后台读取、busy/error/warning、配置与宏勾选。关闭会取消 Transfer、推进 generation，父弹层还校验 workspace、上下文、所选 profile 及事件实体；导入／导出仅交给 Source owner，Existing owner 返回明确错误，各产品后续写入仍需独立核对 |

当前配置转移 Dialog 明确设置 `on_cancel=false`、`on_ok=false`，因此 Esc／Enter 不会通过组件默认决策关闭，关闭按钮与 Cancel 才调用 `dismiss`。设备和添加弹层使用 `on_close` 恢复保留的返回焦点，设备切换或父弹层关闭会取消转移并丢弃旧上下文结果。这些结论核对了 Cargo.lock 锁定的 GPUI Kit revision `1676826d9d49b90f4d7b6808b69924f58796f6f1` 的 base Dialog：Cancel 只有在 `on_cancel` 返回 true 时才调用 `on_close`；焦点陷阱与业务取消是两条独立机制。文件读取取消不代表已经提交到产品所有者的导入任务被撤销，后者仍需沿 SourceProfileImportRequested 和实际收据消费者核对。

视觉依据还包括当前 [9449 CSS](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/css/9449.b8e7f39a.chunk.css)：详情设备卡宽 240、高 230，设备图高 110，edition 12px/#707070；游戏封面 290×150，普通图标 60×60；详情最小高 500，局部分支最大宽 800。active、hover、pressed 的绿色边框／阴影、missing 红色提示和封面刷新 disabled 都是不同状态。Rust 只有设备卡和部分弹层的对应布局，尚无完整游戏详情树；本轮没有窗口、截图或像素验收，不能把部分尺寸相同写成 1:1 还原已完成。

后续自动化矩阵必须分别覆盖导航恢复／草稿继续、真实游戏列表和缺失分支、扫描中取消、添加失败不伪造列表、各删除确认路径、重定位冲突、封面取消与保存失败、同 PID 不同序列号、IOT／附件图片、配置菜单、逐配置宏选择，以及弹层切换后的旧结果拒绝。纯组件测试、服务模拟和实际 DLL／设备对照分别保留结果；当前页面静态存在不计作这些矩阵已经通过。

## 原件与范围

所有来源读取均校验 acquisition 的实际 bytes 和 SHA-256；以下为半开 UTF-8 字节区间。

Profiles 和 gms-proxy 的获取收据均于 2026-10-21 重新核对入口、asset-manifest 和 manifest，三端点在各自获取前后均保持同字节，主脚本仍分别为 `main.776df2b1.js` 和 `main.fd0c50a8.js`。recorded 时间记录本次核对，文件 acquired 时间保留首次日期；两套 `declared_references_fetched` 仍为 false，缺失 map 和动态依赖未决项继续保留。

| 来源 | 阅读范围 |
| --- | --- |
| [Profiles main.776df2b1.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/main.776df2b1.js) | 游戏请求 actions `[80386,83251)`；扫描 action `[1810110,1811054)`；GMS 桥接 `[2054899,2059998)`；数据所有者 `[2060409,2067675)`；本地 reducer／持久化 `[2072585,2076984)` |
| [9449.8f17b519.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9449.8f17b519.chunk.js) | 设备关联卡 `[3921,7485)`；封面 `[7696,11011)`；游戏详情 `[11820,17940)`；游戏页面 `[21322,27177)`；Devices 页 `[197233,198773)`；根导航 `[198774,201343)` |
| [9473.f9ed3133.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9473.f9ed3133.chunk.js) | Add App `[13735,17924)`；关联游戏卡 `[52540,57613)`；过滤工具条 `[58769,62307)` |
| [gms-proxy main.fd0c50a8.js](../../../.analysis/inputs/frontend/gms-proxy-current/raw/apps.razer.com/synapse/gms-proxy/static/js/main.fd0c50a8.js) | 本机服务 SDK 列表／修改 `[93906,100096)`；API 消费者 `[164292,166232)`；连接／事件／扫描 `[166243,178365)`；列表读取 `[178388,180294)`／`[181340,183291)`；添加 `[183860,189276)`；运行集合／删除 `[189308,193421)`；封面 `[197654,200682)`；订阅、发布与释放 `[203444,212245)` |

服务内部扩展使用当前静态提取的 GameManagerServiceLib 与 GameScannerCommon，二者文件版本均为 3.14.0.1109。重新核对官方包实际字节、提取与已安装 DLL 别名、所读 C#／IL 输出的 SHA-256 和 [托管捕获收据](../../../.analysis/results/host-current/managed/capture.json)，未运行服务或程序集。PE32 标记和 CLR IL-only 需分别解释，不能仅据 PE machine 字段推定其所有实际进程架构。

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

这里闭合页面到服务客户端，并扩展到连接门禁、主要本地列表／设置持久化及扫描调度。全部扫描策略、URL／UWP 分支、运行进程识别和各设备自动配置档写入仍须各自连读；没有运行厂商 JavaScript、启动服务测试、操作真实设备或据此声明 Rust 独立实现／实际 DLL 对照／运行验收完成。
