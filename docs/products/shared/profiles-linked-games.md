# 配置档管理与关联游戏

配置档保存一组设备功能设置；关联游戏把本机应用身份、设备身份和配置档 GUID 联系起来。产品页的配置栏、全局 Profiles 页和关联游戏弹窗是不同调用方，共用部分数据与命令，但不能把“弹窗中选中一个配置档”直接理解为“设备已经切换到该配置档”。

本文先说明当前全局关联游戏弹窗与已追到的 PID 756 导入消费者。键盘配置栏的排序、默认配置与编辑限制见 [键盘配置档](../keyboard/profiles.md)。其他产品的配置档格式和设备写入继续按各族说明区分。

## 弹窗选择的对象

全局弹窗从 `profileLinkedGamesReducer.devices` 中同时匹配 `productId` 与 `serialNumber`；显示设备的 category、subCategory、container ID、profiles 和 activeProfile。它复制设备数据用于显示，并从本机 `synapse_<PID>` 补入宏映射摘要。

当同一设备同时出现名为 `General` 和 `Factory Default` 的两项时，这份弹窗代码从显示集合中移除后者。它是这一调用方的列表处理，不是一个可以推广到所有设备配置的固件规则。

初始配置选择跟随设备的 activeProfile；随后在下拉框选择会更新弹窗的 `activeProfileIndex`、`selectedProfileGuid` 和卡片的目标配置。这个 `Y` 回调本身没有发送设备配置切换命令。

## 游戏列表的过滤、排序与添加

过滤分支分别使用全部游戏、具有可见关联设备的游戏、已移除游戏。按名称搜索使用小写名称和 `searchKey`。排序有四种，含义并不相同：

| 排序 | 当前比较规则 |
| --- | --- |
| 名称升序 | 小写名称按 `<`／`>` 比较 |
| 名称降序 | 同一比较方向反转 |
| 最近使用 | `gameSettings.ATT_UserLastInteractTime` 较新者在前；缺值用该字段代码中的最早日期 |
| 使用次数 | `ATT_UserTotalGameplayCount` 较大者在前 |

这些是游戏卡片的排序，不能代替产品配置下拉框自身的排序。卡片的关联目标继续按 PID、序列号和 `isVisible` 匹配。添加应用入口切到 Add App 界面；文件拖放逐项把 `path` 交给 `addGame`。返回只改变弹窗显示分支。

## 新建、复制、重命名与删除

全局弹窗把命令发送到对应设备 MW 的 BroadcastChannel。频道由 vendor ID、product ID、container ID 和 `mw` 组成；IOT 类别使用 `iot` container。消息结构是 `msgName: "userCommand"`，内部为 `{ type, payload }`。

| 操作 | 原件实际提交的内容 |
| --- | --- |
| 新建 | `ON_ADD_PROFILE`，生成 GUID；名称经去重函数处理，基础名为用户名称加 `-Default` 或 `Default` |
| 复制 | `ON_DUPLICATE_PROFILE`，新 GUID、新去重名称、`sourceProfileGuid` |
| 重命名 | 输入最大长度 32；trim 后检查非空／非全空白及名称有效性；提交 `newName` 和 `renameProfileGUID` |
| 删除 | 确认后提交 `deletedGUID` 和 `newSelectedProfileGUID` |
| 导入 | `ON_IMPORT_PROFILE`，携带选中配置、`selectedMacroData: []`、`isCloud` 和 `fileType` |
| 导出 | `ON_EXPORT_PROFILE`，携带选中配置 |

删除代码为 `newSelectedProfileGUID` 赋的是数组最后一个配置对象，字段名与实际值形态不一致；不能仅根据字段名把它改述为必定是 GUID 字符串。重命名会先修改弹窗中的配置名称再发命令；是否最终持久化、广播或改设备由 MW 接续。

## 导入／导出对话框

对话框在导出模式复制现有配置，扫描直接宏映射或嵌套 `mapping[0]` 的宏引用，并按 GUID 去重。固定保留 GUID `af06d371-861f-4b98-8d78-bfaef5cfdebf` 不显示为可导出项。选中某个配置会把其宏子项全部选中；改变宏子项只改变对应子项。全选／全不选操作更改配置选中状态。

本地导入解析器返回 `fileType`、profiles、macros、warning 和 deviceName 后更新列表。`synapse3` 与 `not-supported` 的错误文案不同。确认按钮调用上游处理函数后立即关闭；它没有等待 MW 或设备完成才关闭。

全局调用方给 `ON_IMPORT_PROFILE` 的宏参数是空数组，产品页调用方可以提供新宏。不能因两者打开了相似对话框，就假定它们有相同的宏输入。原版云分支独立存在，属于本项目范围排除项。

## PID 756 导入如何落到本机状态

756 的 MW `ON_IMPORT_PROFILE` 状态机转交完整 `ay` processor。它根据动作来源选择产品 UI 或全局 UI 的反馈 action，并分别处理 Analog V3 板载键盘和普通分支；仅允许 `synapse4`／`synapse3` 文件类型。

处理顺序是读取现有宏与设备配置、复制导入参数、处理 GUID 冲突与去重名称、生成宏 app-engine 事件、重写配置中的宏引用、移除未选宏的映射，再依据布局和 `disableHypershiftMapping` 过滤不适用映射。有警告的导入会删除源码列出的三个特殊输入 ID；缺 commandDial 且产品默认定义存在时复制该产品 commandDial。

Analog V3 分支遍历其配置映射容器并使用专用 `generateAppEngineFromProfileMapping`；普通分支处理 `mappings`，也修订 keymaps 中的宏引用。两者都为导入配置生成新 GUID、追加到配置集合并增加设备存储 version。

随后先提交宏存储，再写设备本机缓存，发布 window-storage runtime data，刷新配置／宏摘要并发出 completed。这个 processor **没有选中新导入配置，也没有直接调用 `loadActiveProfile`**。存储写成功、映射引擎本机投影和物理设备设置分别由不同消费者处理。

## 同窗口与其他窗口的后续消费者

这条链的 `LocalStorageHook → RzLockableStorage → window.localStorage.setItem` 在导入所属浏览器窗口执行；浏览器 storage 事件不会返回给执行该次写入的同一窗口。不能为了补全图而虚构“导入后同窗口必定收到 storage 并恢复硬件”。

其他窗口的 `97161.ie` 接收存储事件，比较 version、原／新 activeProfile 和差异，再进入 `loadActiveProfile` 或已有更新队列。空 diff 可以跳过配置字段，但通用 feature 的设置／清理仍有分支，不能把它概括成“一律不恢复”。

宏存储在独占锁中提交 IndexedDB 后广播。当前对象只对收到的频道消息发 change；宏变更比较器检查已存在 GUID 的交集，因此纯追加宏与修改已有宏不会自动走同一条重写链。

## 当前证据与后续阅读

| 所有者 | 当前原件和 UTF-8 字节区间 |
| --- | --- |
| 关联游戏与配置命令弹窗 | [9449.8f17b519.chunk.js](../../../.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9449.8f17b519.chunk.js)，`[187966,197223)` |
| 设备命令频道 | 同一原件，`[187466,187965)` |
| 导入／导出对话框 | 同一原件，`[182427,187277)` |
| 756 完整导入 processor | [6737.90f8e2e80827f3801f8e.js](../../../.analysis/inputs/frontend/keyboard-mw-756-current/raw/apps.razer.com/synapse/products/756/mw/6737.90f8e2e80827f3801f8e.js)，`[1776685,1786604)` |
| 756 其他窗口存储事件 | 同一原件，`[2080780,2082896)` |
| 756 loadActiveProfile | 同一原件，`[1308593,1330096)` |
| 756 宏存储 | 同一原件，`[428695,430712)` |

20 个完整 owner 的原件、body SHA、任务、存储锁、窗口发布与消费者保存在 [keyboard-mw-audit.json](../../../.analysis/results/frontend/keyboard-mw-audit.json) 的 `queries.profile-import-processors`，本次重新核对这 20 项的实际原件与完整 body 字节一致。

这只闭合了上述 UI 操作和指定产品导入链。全局 Profiles 主页面、游戏扫描服务、前台切换／恢复、每族配置格式和全部写入分支继续分别阅读；这些内容不因本文存在而被标记为完成。本次没有执行厂商代码或设备操作。
