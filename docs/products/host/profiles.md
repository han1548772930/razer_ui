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

## 原件与范围

所有来源读取均校验 acquisition 的实际 bytes 和 SHA-256；以下为半开 UTF-8 字节区间。

| 来源 | 阅读范围 |
| --- | --- |
| [Profiles main.776df2b1.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/main.776df2b1.js) | 游戏请求 actions `[80386,83251)`；扫描 action `[1810110,1811054)`；GMS 桥接 `[2054899,2059998)`；数据所有者 `[2060409,2067675)`；本地 reducer／持久化 `[2072585,2076984)` |
| [9449.8f17b519.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9449.8f17b519.chunk.js) | 设备关联卡 `[3921,7485)`；封面 `[7696,11011)`；游戏详情 `[11820,17940)`；游戏页面 `[21322,27177)`；Devices 页 `[197233,198773)`；根导航 `[198774,201343)` |
| [9473.f9ed3133.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9473.f9ed3133.chunk.js) | Add App `[13735,17924)`；关联游戏卡 `[52540,57613)`；过滤工具条 `[58769,62307)` |
| [gms-proxy main.fd0c50a8.js](../../../.analysis/inputs/frontend/gms-proxy-current/raw/apps.razer.com/synapse/gms-proxy/static/js/main.fd0c50a8.js) | 本机服务 SDK 列表／修改 `[93906,100096)`；API 消费者 `[164292,166232)`；连接／事件／扫描 `[166243,178365)`；列表读取 `[178388,180294)`／`[181340,183291)`；添加 `[183860,189276)`；运行集合／删除 `[189308,193421)`；封面 `[197654,200682)`；订阅、发布与释放 `[203444,212245)` |

这里闭合当前页面到本机服务客户端的静态调用链，服务内部实现和各设备完整写入分支另行核实；没有运行厂商 JavaScript、启动服务测试、操作真实设备或据此声明 Rust／运行验收完成。
