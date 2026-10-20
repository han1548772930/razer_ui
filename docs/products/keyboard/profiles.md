# 键盘配置档、板载身份与本地导入

本文记录当前已核对的官方前端与中间件行为。配置档目录、本机保存的配置档、当前活动 GUID、设备板载槽和界面选择是不同的数据来源；下述“发布到 UI”仅表示官方发送了该消息，不表示已经完成物理设备写入。

目前逐函数核对的主链是 PID 679；本地导入的完整消费者样本是 PID 756。两者的分支不能直接外推到所有键盘。各 PID 实际页面入口见本目录逐产品结构页。

## 证据定位

本文字节区间均为 UTF-8 原件的半开区间 `[start,end)`。获取版本、HTTP 部署信息与完整原件 SHA-256 见获取收据，不在业务说明中重复。

| 简称 | 原件与收据 | 本文使用的所有者 |
| --- | --- | --- |
| 679 UI | `.analysis/inputs/frontend/keyboard-679-current/raw/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js`；[获取收据](../../../.analysis/inputs/frontend/keyboard-679-current/acquisition.json) | `JU` `[8109259,8118712)`；`Ng` `[8132057,8134648)`；`createProfileDataSet` `[7619411,7621083)`；`changeProfile` `[7611946,7612281)`；排序模块 `35380` `[945729,946390)` |
| 679 MW | `.analysis/inputs/frontend/keyboard-mw-679-current/raw/apps.razer.com/synapse/products/679/mw/6737.d9169f212a8242741503.js`；[获取收据](../../../.analysis/inputs/frontend/keyboard-mw-679-current/acquisition.json) | 模块 `51765`、`17204`、`10745`、`89423`，详见下文 |
| 全局 Profiles | `.analysis/inputs/frontend/synapse-profiles-current/raw/apps.razer.com/synapse/profiles/static/js/9449.8f17b519.chunk.js`；[获取收据](../../../.analysis/inputs/frontend/synapse-profiles-current/acquisition.json) | 对话框 `Ua` `[187966,197223)`；命令发送 `ja` `[187466,187965)`；导入导出选择器 `Ia` `[182427,187277)` |
| 756 MW | 见[完整导入捕获](../../../.analysis/results/frontend/keyboard-mw-audit.json)的 `queries["profile-import-processors"].complete_owners`；[获取收据](../../../.analysis/inputs/frontend/keyboard-mw-756-current/acquisition.json) | `middleware_import_processor`、`local_device_storage_hooks`、`other_window_device_storage_observer` 等具名角色 |

679 的结构证据保存在 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `keyboard_page_layout.products[product_id=679].factory_customize_chain`；679 板载身份与本地存储捕获保存在 [keyboard-mw-audit.json](../../../.analysis/results/frontend/keyboard-mw-audit.json) 的 `queries["keyboard-local-profile-adoption"]`。本文核对了上述引用的原件字节及捕获函数体。

## 打开页面时先做什么

679 主页面 `JU.componentDidMount` 按源码语句顺序执行：

1. 把产品的 `DKMKEYS` 加入当前集合，恢复或建立本窗口的导航历史，并发布当前标签页。
2. 查询窗口服务客户端列表，寻找 URL 包含 dashboard 的窗口。该调用使用 Promise；源码并未等待其完成后再执行后续加载。
3. 加载产品名称、`OBMSpecs` 与板载槽数，再依据当前 `layoutId`、edition 构造按键及触发点显示表。
4. 调用 `loadActiveProfileSettings` 读取 `synapse_<productId>` 本机文档，之后读取模拟键教程的显示标记，注册 `storage` 监听。

`loadActiveProfileSettings` 首先发布 Factory Default 的忽略身份，再读本机文档。文档不存在，或 `profiles` 不存在/为空时直接返回 `false`。存在文档时，活动对象来自 `activeProfile` 对应 GUID；仅在没有 `activeProfile` 值时取第一个配置档。该 UI 函数没有直接调用 HID getter。

它向 `MW_SET_PROFILE_TO_UI` 发布由原文档投影出的 `guid`、`name`、宏映射以及 `selectedProfileGuid`。宏映射投影移除 `appEngine`、`deviceList`，避免把内部编译数据当成配置档菜单数据。随后按活动对象实际具有的字段发布亮度、Quick Effects、自定义色、熄灯设置、映射、触发点、Gaming Mode 和 Snap Tap。这里的 Quick Effects 含源码自身的显示兜底；它属于界面投影，不能单凭这个值认定设备已返回对应效果。

宏独立窗口的入口是 `Ng`。它先从 `DEVICE_DETAIL` 找当前 PID 的 edition/layout，加载对应按键，再在挂载时读取语言与 `synapse_<productId>`。该窗口只提取本链路所需的配置档和映射，不能把主窗口的全部初始化动作自动套入宏窗口。

## 配置档身份、排序和下拉菜单

配置档身份使用 GUID。`changeProfile(index)` 用当前菜单行反查 GUID，GUID 不同才调用 `changeSelectedProfile`；随后重新计算菜单数据。菜单行号只是当前排序结果，源代码没有把它当成持久化身份。

679 的 `createProfileDataSet` 同时消费 UI 配置档列表和 `obm.profiles`：

- 先以 `guid` 建立实际板载资料的 `Map`。同 GUID 多次出现时，后面的资料覆盖前面的资料。
- 预设身份来自 `obm.specs.presetProfiles`，提示图标依 GUID 命中，而不是依显示名称判断。
- `profileSwap` 分支把命中实际板载 `Map` 的行放在第一组，记录实际 `slotId` 并显示对应槽图；其余行放在本机组。
- 板载组按实际 `slotId` 数值升序；本机组保留上游配置档列表的顺序。只有两组均非空时才插入 `name:"divider"` 分隔行。
- 通用代码还含 `hasModeSwitchSlider` 分支，会按名称过滤 Factory Default；679 当前产品描述不启用该能力，因此这不是 679 的普通过滤规则。

名称排序由模块 `35380` 的函数统一执行。它按 JavaScript UTF-16 下标逐个比较：ASCII 标点优先，其次是 ASCII 数字，其余字符最后；同一类遇到第一个不同字符，立即返回该字符的 `localeCompare` 结果；共同前缀完全相同时才比较字符串长度。这不是自然数字排序，也不是对完整名称调用一次排序比较。`localeCompare` 的运行环境及孤立 surrogate 行为仍有独立边界，不能由静态源码推定各语言环境下的最终顺序。

菜单允许只有一个条目时打开。名称显示使用 `notLocalize`，不翻译或修剪保存的名称。槽图是前缀，预设说明图标独立；触发器所显示的前缀只含槽图。正在进行触发点调整时，菜单的鼠标按下处理会阻止切换并显示对应提示；这不是普通配置档永久禁用状态。

## 679 工厂预设与编辑限制

产品预设表位于模块 `78193`，变量 `r`，`[6297718,6298282)`：

| 名称 | GUID | 描述表中的槽 | 颜色 | 身份 |
| --- | --- | --- | --- | --- |
| Factory Default | `af06d371-861f-4b98-8d78-bfaef5cfdebf` | 2 | green | `isFactoryPreset:true` |
| FPS Rapid Trigger | `d363ba48-79a8-422c-b405-29a07eff7e8e` | 3 | red | 普通预设身份 |
| Analog WASD | `2c481cef-62e0-4fe6-85a1-4d134c279c23` | 4 | blue | 普通预设身份 |
| Racing | `474c90e3-7eab-4aa0-8030-34c43cbc707a` | 5 | yellow | 普通预设身份 |
| High Sensitivity | `177a20d5-f30a-4a52-923e-4fadb7db8392` | 6 | cyan | 普通预设身份 |

这是版本内固定的预设描述，实际连接的槽内容另由 `obm.profiles` 提供。表中的槽号不能替代一次设备读取。

`JU.componentDidUpdate` 在选中 GUID 变化时，精确比较 Factory Default 的 GUID，再更新 `isFactoryDefaultProfile`。Customize 标签仍能打开；`fh` 根据该标志选择静态 `gh/Uh` 或可编辑 `Ph/mh` 子树。静态子树只有当前 layout/edition 的产品图、暗角及说明，没有挂载映射编辑器。宏窗口 `Ng` 也单独按预设表的 `isFactoryPreset` 身份作同样选择。该规则的用途是限制该工厂档的编辑，不能概括为所有预设都不可编辑。

## 板载身份如何进入默认映射

679 MW 模块 `51765` 的导出 `od` 解析到 `Ne`，`[1044945,1045096)`。它读取当前 runtime `layoutId`，调用模块 `17204.q$` 取得该布局的完整默认映射，再调用 `Le` 采用当前槽身份，最后调用 `10745.qz` 发布默认映射。

`Le` `[1044415,1044944)` 复制输入映射，默认只检查槽 `3,4,5,6`。旧槽 GUID 来自模块 `17204.o8`，新槽 GUID 来自调用参数。只有槽 GUID 变化且找到的映射属于 `profileNavigationAssignment:"Specific"` 时才替换其目标 GUID。源码使用 `if(index)` 而不是 `index>=0`：下标 0 不进入替换记录，`-1` 虽为真但最终因不存在对应映射而跳过。这里保留原始条件含义，不把它改写成更宽的推测规则。

模块 `17204` 的 `E` `[330807,331163)` 同时列出 Factory、四个预设和 General；General 与 High Sensitivity 的描述槽均为 6。`T` `[331186,331613)` 根据当前 `isObm.yes.ignoreProfiles` 选择完整表或排除 General，并在 GUID/槽顺序变化时更新集合。`D` Proxy `[331624,331722)` 在每次访问前执行这次同步；因此这是受能力条件约束的描述集合，不是“同一实际槽中同时存在两份已观测配置”。

布局默认映射函数 `C` `[331743,333189)` 区分 US/Korean/Traditional Chinese、Nordic/French/German/UK、Japanese。后两组仅在对应 UK/JPN 表非空时采用专用表，否则使用该产品普通映射表。它还有 HyperBoost 系统的 fan-ready 分支；该分支依赖系统能力和真实 fan 状态，不代表普通键盘都会创建跨设备风扇映射。

## 默认映射的本机存储边界

`10745.qz` 解析到 `_o` `[211659,212466)`：

1. 读取 `DEVICE_LOCALSTORAGE_KEY` 与 `DEFAULT_TURBO_LOCALSTORAGE_KEY`。
2. 根据当前产品能力启用双字节触发值并更新 converter。
3. 有保存的 default turbo 时交给 converter；缺少该项时，由源码的 turbo factory 生成并写入该项。
4. 取得布局默认映射；本次传入采用后的映射时以其替代；调用 `setDefaultMappings`，保存 converter 生成的 `defaultMappings`。
5. 调用 `89423.Pf(document,false)`。

`Pf` 解析到 `M` `[1573892,1576311)`。它以当前 runtime serialNumber 为写存储的前置条件；没有 serial 时只记录不保存。在 serial 存在时，它补充文档 metadata/defaultMappings；仅在此 serial 无 metadata 且已有配置档时，把第一个现有配置档的 GUID 注册为该 serial 的活动身份。参数 `false` 保留原有 `isDefault`，不执行默认参数 `true` 才会进行的移除。

最终本机写入经过模块 `89423` 的 `setLocalStorageData` 包装 `[1573186,1573547)`。包装以排除 `version` 的内容哈希抑制重复写，并由既有 `PreSetLocalStorage`/`PostSetLocalStorage` hooks 承接副作用。本节描述的是本机文档写入；该函数本身没有提供物理设备写入确认。

## 新增、复制、重命名、删除和本地文件

全局 Profiles 对话框 `Ua` 按当前设备的 productId、category、deviceContainerId 发送命令：

| 动作 | 传入中间件的数据 | 对话框内的直接行为 |
| --- | --- | --- |
| 新增 | `newProfile:{name,guid}`；新 GUID 由 UUID 生成，名称经唯一名称函数处理 | 发送 `ON_ADD_PROFILE` |
| 复制 | `newProfile:{name,guid}`、`sourceProfileGuid` | 发送 `ON_DUPLICATE_PROFILE` |
| 重命名 | `newName`、`renameProfileGUID` | 先 `trim()`；空白或名称校验失败则结束编辑；通过后更新当前显示对象并发送 `ON_RENAME_PROFILE` |
| 删除 | `deletedGUID`、`newSelectedProfileGUID` | 先关闭确认状态，取消则不发命令；确认后发送 `ON_DELETE_PROFILE` |
| 导入 | `selectedProfileData`、`selectedMacroData`、`isCloud`、`fileType` | 发送 `ON_IMPORT_PROFILE`；全局 Profiles 路径给出的宏数组是 `[]` |
| 导出 | `selectedProfiles` | 发送 `ON_EXPORT_PROFILE` |

`ja` 构造目标设备 MW 的 BroadcastChannel 名称并发送 `{msgName:"userCommand",msgData:{type,payload}}`。IOT 使用专用 container 字段，不能把一般 USB 通道名称套到该分支。

选择器 `Ia` 将配置档与它引用的宏分成可勾选项；选中配置档时重新选中该配置档所有宏。没有选中任何配置档时提交按钮采用 disabled 样式。导出时过滤 Factory Default 的精确 GUID；导入展示也过滤该 GUID。点击取消/关闭只结束对话框；点击提交先调用消费者，再关闭对话框。本地文件解析失败会清空选择并保留 fileType/错误状态，源码区分 Synapse 3 和不支持文件的提示。原件含云路径；本文仅追踪本地文件与本机命令链。

以上菜单的后端新增、复制、删除、导出文件生成仍需逐产品消费者继续核对。不能以菜单已经发送命令为由认定这些动作的设备与文件副作用已闭合。

## 756 本地导入消费者和成功语义

完整捕获位于 `queries["profile-import-processors"].products[product_id=756]`；以下相关 owner 的原件及函数体均已逐字节核对。

`middleware_import_dispatch` `[1214569,1214821)` 取得当前事件中的所选配置档、宏和 fileType，交给 TaskMaker。`middleware_import_processor` 的 `ay` `[1776685,1786604)` 同时保留 Analog V3 OBM keypad 专用分支与普通分支，不是只处理一个无条件数据格式：

1. 依事件来源选择产品 UI 或全局 Profiles 的反馈 action。
2. 在允许处理的条件成立时，读取当前宏和本机设备缓存，复制输入；仅接受 `synapse4` 或 `synapse3` 文件类型。
3. 导入宏名称去重，GUID 冲突时创建新 GUID，并记住旧新 GUID 对应。宏事件编译为 appEngine 数据，更新宏配置。
4. 对配置档移除未选择宏的映射；把映射里的宏 GUID/名称对应到导入结果；按 warning、当前布局和禁用 Hypershift 键过滤不适用映射。专用分支会遍历其分层映射表示。
5. 源码在对应产品默认配置确实有 commandDial 时补充该字段，生成 appEngine，去重配置档名称并创建新配置档 GUID，追加到本机列表。
6. 更新源文档 version，先提交 MacroStorage，再写本机设备缓存和 runtime window storage，发布配置档及宏列表。发布的 `selectedProfileGuid` 仍为原活动身份；本函数没有调用 `loadActiveProfile` 来选择新导入项。
7. 正常完成时发送 completed 和完成 action；异常时发送 error 与原异常 message。TaskMaker 的外层事件反馈和处理器内部的 completed/error 是两层不同信号。

TaskMaker `[1398959,1401553)` 串行维护当前事件与队列，按当前条件使用存储 exclusive lock 或设备 shared lock 加存储 exclusive lock。预处理时检查事件的 `isCurrent`，过期事件可被移除。此捕获没有显示用户可取消一个已经提交的导入事务，不能把关闭选择器解释成中止已排队工作。

## 同窗口、其他窗口和宏广播

导入写本机缓存的链路是 LocalStorageHook → RzLockableStorage → `window.localStorage.setItem`。同一浏览器窗口不会因自己的这次写入收到 `storage` 事件，因此导入函数的本机写入不能直接推导成该窗口再次执行设备恢复。

其他窗口由 `other_window_device_storage_observer` `[2080780,2082896)` 消费变化：设备未明确 power-off 时，比较旧新 version；版本相同直接返回。版本变化时更新源内容 hash，解析 serial 对应的当前活动 GUID，计算旧新活动配置档的 diff，然后调用 `loadActiveProfile(document,diff)`。后续恢复处理器对空 diff、已初始化队列和普通恢复有独立路径；不能从“活动 GUID 没变”直接推出所有恢复和清理都不会发生。

`MacroStorage` `[428695,430712)` 在 `macros-idb` exclusive lock 内写 IndexedDB，等待事务完成后才 BroadcastChannel 广播。当前对象的 change 事件来自收到广播的 handler。`existing_macro_change_projection` `[2387197,2387785)` 只比较旧新 GUID 交集中的名称、appEngine hash、回放选项；单纯追加新宏不等于既有宏重写。

`PostSetLocalStorage` 还会生成本地映射元数据，并投影到已安装主机服务的 localStorage；runtime 数据写入 host window storage。两者属于服务/窗口投影，不是 HID 写回证明。准确的设备恢复、恢复失败及断开清理需结合相应产品的 restore runner，不能由本页的本地导入成功提示替代。

## 已定位的具体缺边

- 679 UI 的启动消费已核实；其上游 MW 如何依当前连接读取实际活动槽、处理首次初始化、完成写回/读回及断开释放，尚未在本页逐函数闭合。
- 全局菜单新增、复制、重命名、删除和导出已核对输入，但各产品消费者、文件格式转换和最终反馈仍需按 PID 继续追踪。
- 756 导入的本地事务、跨窗口观察和宏广播已核对；该产品其他恢复分支不能由此概括为已完成。
- 679、756 之外的预设身份、板载槽数、菜单分组与可编辑条件必须引用各自当前产品描述及实际挂载组件，本文不宣称全族一致。
- 本文为静态源码理解；未执行厂商 JavaScript、应用程序或真实设备验收。
