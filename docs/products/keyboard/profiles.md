# 键盘配置档、板载身份与本地导入

本文记录当前已核对的官方前端与中间件行为。配置档目录、本机保存的配置档、当前活动 GUID、设备板载槽和界面选择是不同的数据来源；下述“发布到 UI”仅表示官方发送了该消息，不表示已经完成物理设备写入。

目前逐函数核对的主链是 PID 679；本地导入的完整消费者样本是 PID 756。两者的分支不能直接外推到所有键盘。各 PID 实际页面入口见本目录逐产品结构页。

## 证据定位

本文字节区间均为 UTF-8 原件的半开区间 `[start,end)`。获取版本、HTTP 部署信息与完整原件 SHA-256 见获取收据，不在业务说明中重复。

| 简称 | 原件与收据 | 本文使用的所有者 |
| --- | --- | --- |
| 679 UI | `.analysis/inputs/frontend/keyboard-679-current/raw/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js`；[获取收据](../../../.analysis/inputs/frontend/keyboard-679-current/acquisition.json) | `JU` `[8109259,8118712)`；`Ng` `[8132057,8134648)`；`createProfileDataSet` `[7619411,7621083)`；`changeProfile` `[7611946,7612281)`；排序模块 `35380` `[945729,946390)` |
| 679 MW | `.analysis/inputs/frontend/keyboard-mw-679-current/raw/apps.razer.com/synapse/products/679/mw/6737.d9169f212a8242741503.js`；[获取收据](../../../.analysis/inputs/frontend/keyboard-mw-679-current/acquisition.json) | 模块 `51765`、`17204`、`10745`、`89423`，详见下文 |
| 679 产品与板载引擎 | 同一收据内的 `main.2e96b673a06ae6ca25a1.js`、`obmEngineKeyboardCalMaxMem.781facfafb8f31b96e18.js`、`obmEngineKeyboard.fdf63782d77cc1c9a909.js` | 产品 `OBMSpecs` `[25300,25693)`、`isObm` 挂载 `[230799,234785)`；派生类构造 `[655,792)`、`[821,972)`；实际读取与保存继承自基础引擎 |
| 679 HID | 同一收据内的 `rzDevice25.fc7bd21101285d902a76.js` | 配置档 getter/setter `[168057,171528)`；`sendCommand` `[120868,125087)`；崩溃恢复登记 `[235655,236864)` |
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

## 679 的连接、本地初建与设备初始化

以下流程来自 679 MW 的完整所有者，不能用 UI 的 `loadActiveProfileSettings` 代替：

| 阶段 | 所有者与字节区间 | 实际行为 |
| --- | --- | --- |
| 当前连接 | `10745.Qn` `[151575,152539)`；`fo` `[190692,191654)` | 按当前 container 和产品的 wired/BLE/dongle 身份选择设备；无匹配设备关闭窗口。连接后读取 serial，再由 `xn` `[152539,153737)` 读取 edition/layout 并发布 runtime 资料 |
| 正式 setup | `$n` `[154674,180232)` | 等待主机 `initializedDevices[productId]` 通知；防止重复 setup；再次核对连接；注册映射设备，建立本机文档，初始化 TaskMaker/TaskManager，进入 driver mode，建立默认映射与事件消费者，再创建板载引擎和排入 `ON_INIT_OBM_ENGINE` |
| 本机文档初建 | `qn` `[180803,182022)` | 仅当前本机文档不存在时调用本机默认数据工厂，设置 schemaVersion、`isDefault:true`。Analog V3 创建一个 preset 配置档并设本机 active GUID；随后经 `Pf(document,false)` 保存 |
| driver mode | `34938.X` `[633504,633951)` | 先 `getDeviceMode()`；模式不同才调用 `setDeviceMode(3,0)`。此包装捕获并记录异常，因此仅等待包装返回不能证明设备已经进入 driver mode |
| 引擎挂载 | `$n` 内的 `isObm && !useBle` 分支 | 产品选 `obmEngineKeyboardCalMaxMem`；传入当前布局的默认映射及实际 `supportedFeature`。初始化引擎后排队执行板载读取和同步 |

这里有三种不同的“初始化”：主机 `initializedDevices` 是主机协作流程的状态，本机 `qn` 处理文档缺失，板载任务读取并协调现有设备内容。`qn` 没有读取设备“首次使用”标志，不能从它的 `!document` 条件推导出设备为空或允许把整套默认值写回。它保留的 `isAnalogV3 && ignoreProfiles && presetProfiles` 特殊分支还会选择专用默认 preset、移除部分本机身份；679 当前 `isObm` 挂载没有传入 `ignoreProfiles`，不能把其他产品的这条分支套给 679。

`Qn` 中的 `restoreDeviceWhenCrash()` 也需要区分：679 当前产品启用该能力，但 HID 类的实现是向主机发送 `setRestoreFileWhenCrash`，登记 mode `0,0` 等恢复报文。无参数调用不追加 lightingEffect 报文。它不是在本次连接时立即恢复设备，也不构成恢复执行成功的证据。

679 的 `OBMSpecs` 明确启用 `profileSwap`、`hasHardwareProfileSwitch`、brightness、macros、Analog V3、resetProfile、N-key rollover 和 kill switch；`modTap:false`。CalMaxMem 派生类只调整宏内存计算方式和最大内存取值来源，没有另写一套配置档读取逻辑。共享引擎存在 DPI、polling、knob、modTap、Snap Tap 等条件分支，仍须按上述实际能力筛选。

## 679 板载读取顺序与活动身份

基础引擎 `init` `[47848,52797)` 先建立 inputID/fwID/HID 对应、正常与 Hypershift 的忽略键集合，再调用 `getEdition()` 保存实际 layout。`loadProfilesFromDevice` `[53521,56401)` 的完整调用顺序如下：

1. 先执行 `loadMacrosFromDevice` `[10064,12836)`，取得宏存储与宏身份；之后取得引擎 mutex 并清空本次配置档投影。
2. 依次读取最大槽容量、现有数量、实际槽 ID 列表。按返回列表顺序建立槽对象并读取 GUID/name/owner。通用非 Analog V3 分支跳过第一个列表项的身份；679 的 Analog V3 分支实际读取第一项，不能固定跳过 slot 1。
3. 按实际启用能力调用 DPI、亮度、polling、高速 polling、deadzone、debounce 读取阶段。679 的亮度阶段 `[56402,56959)` 先读取所有槽的亮度；单槽失败保留 `undefined`，不补成本机亮度。
4. 再逐槽执行 `getAllButtonAssignmentInfo` `[65389,66188)`。每槽先 Normal，再 Hypershift；各模式分别过滤 `ignoreInObm`，然后由 `[66189,70770)` 按每批 8 键读取 assignment、13 键读取 Rapid Trigger、15 键读取 actuation point。
5. 8 键 assignment 仅在返回的所有 `dataArray` 字节均为零时重试，总计最多两次；这不是任意异常都重试的规则。RT、actuation 没有 mode 参数，但源码仍在两种模式的各自键集合中分别读取，不能自行合并成一次。
6. 所有槽的 assignment/analog 阶段结束后，才进入 knob、modTap、kill switch、Snap Tap 等受能力控制的阶段。679 实际启用 kill switch，其顺序在全部槽的 assignment 之后。
7. 成功或抛错都释放本轮配置档 mutex；宏加载在此前拥有自己的读取/锁流程。此后上层消费者才补读本机尚无相同 GUID 的宏内容和各槽 continuous Rapid Trigger。

HID 的命令三元组为 `[dataSize,commandClass,commandId]`，不是完整的 91 字节报文：

| 操作 | 三元组 | 结果或参数 |
| --- | --- | --- |
| 最大容量 | `[1,5,138]` | 返回 `maxProfilesSupported` |
| 当前数量 | `[1,5,128]` | 返回 `numOfProfiles` |
| 实际槽列表 | `[80,5,129]` | 第一个字节为列表数量，随后为实际 slot ID |
| 槽身份读取 | `[80,5,136]` | 请求 slot、16 位大端 offset、size；读取器的 size 字段传 0，实际 packetSize 随本批长度指定 |
| 新建槽 | `[1,5,2]` | 参数为目标 slot ID |
| 删除槽 | `[1,5,3]` | 参数为目标 slot ID |
| 槽身份写入 | `[80,5,8]` | slot、16 位大端 offset、总长度 250，再接本批数据 |
| 独立活动槽查询/设置 | `[1,5,132]` / `[1,5,4]` | getter 返回原始 slot ID；setter 输入 slot ID |

身份读取由 `getProfileNameStructure` `[5490,7383)` 执行：当前 HID 分别从 offset `0,64,128,192` 读取 `64,64,64,58` 字节，总共 250 字节；临时延长 IN 时序，结束后恢复旧值。前 16 字节使用 GUID 混合端序，`[16,116)` 为 UTF-8 名称，先截到首个 NUL；没有 NUL 时源代码的 `slice(0,-1)` 会去掉最后一个字节。`[116,180)` 单独作为 owner 字节字符串读取并修剪两端 NUL。遇到空、全零、全 `ff`，或包含 `0000`/`ffff` 的 GUID，最多尝试十次；最终未知身份仍是未知身份，不能改造成默认配置档。分块缺失或通信失败也不能据此认定设备未初始化。

**活动槽查询并不属于这条挂载读取顺序。** 本次捕获的 MW JS 中，`getActiveProfileID`/`setActiveProfileID` 只有设备类定义，679 上述初始化没有调用它们。上层协调的是三个独立来源：serial 对应的本机 active GUID、实际 slot 1 的 GUID、其他槽中相同 GUID 的存在情况。不能把一次独立 `[1,5,132]` 查询自动接成页面启动前置条件，也不能把 slot 1 或本机菜单选中值冒充这次 getter 的实测结果。

## 679 板载合并、冲突和初始化差异写入

`ON_INIT_OBM_ENGINE` 经 TaskMaker → `42124.ki` `[866548,867596)` → `85225.kt` `[1436733,1452388)`。`ki` 先发送 taskMaker completed，再把本机 profiles、active GUID、宏及 owner 上下文排入设备任务；真实工作由 `kt` 执行：

1. 完成上节设备读取，克隆本轮板载资料。Analog V3 先按实际槽 GUID 采用默认导航身份，再补读本机没有同 GUID 的宏内容，不能把“已有同 GUID 所以跳过内容读取”写成内容已核验一致。
2. 转换设备配置档和宏，发布 `obm.profiles`、宏锁、可用内存；菜单板载列表先排除 slot 1。`initCompleted:true` 在这次投影时就可出现，后面仍有合并及设备同步，不能把这个字段当成全部写入已确认。
3. 按 owner、GUID 和 preset 身份建立新增/更新列表。板载转换的 mappings、brightness、continuous RT 等与本机保留的 quickEffects、switchOffLighting、gamingMode 等分别处理；这不是“整份本机配置优先”或“整份设备配置优先”的简单规则。
4. 本机缺少的 preset 可由源码 preset 工厂建立；它属于本机候选资料。`ON_ADD_OBM_PROFILES_TO_PROFILES` 的消费者 `Fi` `[876377,877450)` 去重追加、替换需更新的 GUID，必要时设置选中 GUID，更新 version/serial metadata，经 `Pf` 保存并调用 `loadActiveProfile`。不能跳过这条消费者而只显示板载槽图。
5. Analog V3 单独检查 slot 1 是否与其他板载槽共享 GUID；只有独有 slot 1、当前 active 匹配及 owner 等条件满足时，才继续检查它与本机当前档的冲突。通用冲突函数 `17204.P` `[342386,343743)` 排除 Factory，逐键比较 actuation 与 Rapid Trigger 的实际字段和源码默认规则，产生 `{obmProfile,uiProfile}` 对并发出 `MW_OBM_MAPPING_CONFLICT`。
6. 冲突处理器 `42124.ao` `[958993,964177)` 依据用户选择的 keyboard 或本机方向合并映射，编译本机 appEngine，更新本机文档，再排入宏、assignment、actuation、RT 的真实设备任务。它保留源代码的 Analog V3 字段合并规则，不是无条件把整个本机映射覆盖到设备。
7. 初始化协调还会检查每个本机配置档对应的实际同 GUID、同 owner 槽；当前 active 另可关联 slot 1，但独有 slot 1 的冲突会影响是否纳入。只有需变化的槽才进入亮度/映射等同步。亮度 helper `Pt` `[1455448,1456165)` 会先 GET，返回值不同才 SET。
8. 宏还需按引用集合、锁和 hash 协调：现有宏 hash 为空则跳过 hash 对比；有差异时删除并重建，导入缺失本机宏，删除无引用且未锁宏，添加需要的宏；转换 Worker 在正常及异常路径终止。随后才执行实际 assignment 差异。结束后的 OBM 再发布来自引擎当前对象，并不等同于重新 GET 全部槽内容。

这条链没有以“本地缺配置文件”“读失败”或“GUID 未知”为通用首次设备初始化判据。它确实包含必要的恢复与差异写入，因此本项目也不能长期用一次只读 inventory 代替它。复刻时必须保存各阶段的真实结果和官方消息相位，设备观测与本机候选、草稿仍各自保留。

## 679 的切换、板载保存和确认边界

本机菜单切换由 `42124.Di` `[853105,854925)` 消费 `newActiveProfileGUID`：保存前一 active，更新新 GUID/version/serial metadata，按 Analog V3 更新默认导航映射，经 `Pf` 保存并更新 runtime 投影；随后排入旧映射清理、新档的默认 actuation/RT 和 slot 1 的身份任务，再调用 `loadActiveProfile`。slot 1 身份参数为 `{profileId:1,name:"",guid:activeProfile,owner:""}`；`85225.Gt` `[1458637,1458887)` 使用引擎 `setProfileName`，这里没有调用 `setActiveProfileID`。

`loadActiveProfile` 的 `vo` `[192462,211643)` 按文档与活动档实际字段遍历并排队；会处理 brightness、effects、gamingMode、mappings、continuous RT、Snap Tap 等各自能力分支，再执行 generic feature 的恢复 hooks。它不是一次同步“把所有设备设置写完”的函数。通用 `restoreDeviceData` `[1188123,1188485)` 也逐个捕获 hook 异常；单个 hook 被调用过不等于全部功能都成功。

设备触发的导航走另一入口：`device navigation` handler `[501124,502730)` 将 `switchProfile` 的目标 GUID，或 Next/Previous/Cycle 操作连同原始事件排入 `ON_SWITCH_PROFILE_BY_DEVICE`；TaskMaker 检查 report 来源后调用 `42124.bi` `[861022,864183)`。Analog V3 先按实际板载列表组织候选，再按源码规则处理本机组和边界；GUID 不变时不会执行活动档变化链。它消费的是导航事件，没有在每次事件后额外调用活动槽 getter。

保存到板载槽的链为 `ON_SET_OBM_PROFILE` → `42124.Ri` `[867596,871423)` → `85225.lt` `[1417038,1421616)` → 引擎 `addProfile` `[90872,96989)`：

- `Ri` 对目标 slot 与 GUID 已一致的情况发送结束通知并返回；否则计算宏增删、正常映射、Analog V3 阈值和 RT，排入真实设备任务。它在任务完成前会更新内存中的 `obmData` 并发布默认导航身份。
- `lt` 从本机找到同 GUID 的源配置档，检查取消，按实际能力准备参数，再调用 `addProfile`。共享的 custom game mode、dual-key priority、Snap Tap 分支仍受该产品能力控制，不能因为共享函数中有代码就宣称 679 启用。
- `addProfile` 取得 mutex；目标槽已存在时先删除，再创建身份，写亮度与各自启用设置，再逐项设置映射、RT、actuation，最后刷新宏内存信息。每项映射有自身异常捕获；外层正常结束和部分子项成功不能合并成“全部映射已确认”。
- 身份写入 `setProfileNameStructure` `[27750,29048)` 生成 `16 + 100 + 128 + 6 = 250` 字节：GUID、定长 UTF-8 名称、补齐的 owner 和尾部保留字节。先 `createNewProfile`，再以 64 字节批写入，setter 的总长度字段为 250，临时延长时序后恢复。
- `setProfileName` `[41441,42042)` 在 helper 返回 false 时只记警告，仍可能更新引擎内存身份；`lt` 也捕获 `addProfile` 的异常后继续发 UI/OBM 消息。因此内存身份、slot 保存结束事件及 taskMaker completed 都不能替代设备应答。

`profileSwap` 的后继消费者 `42124.Li` `[871423,872217)` 不交换 Factory、不回写 slot 1；只在同 GUID 的其他可用 owner 槽存在、且目标槽也存在时，为旧位置排入 `dontSwap:true` 的第二个保存事件。测试需要覆盖这次后继写入，不能只检查第一次保存的目标槽。

HID `sendCommand` 的正常应答路径会等待传输 IN、检查协议状态，失败时按该设备的重试与 mutex 规则处理并抛错。上述身份 setter 的主链没有追加一轮完整身份 GET；`addProfile` 末尾的 `getMacroStorageInfo` 也只证明宏存储查询的结果。对照必须分别记录传输 ACK、官方上层异常处理、UI 消息和后续 GET 的结果；增加测试用读回可以核对实际落盘内容，但不能伪称它原本就是官方函数内的步骤。

## 679 的排队、取消和断开释放

当前 679 的 TaskMaker 是模块 `36305` 的类 `[671556,673895)`。它串行处理事件，根据 action/lockHeld 选择本机存储 exclusive lock，或设备 shared lock 加存储 exclusive lock；执行 Promise 的结果用于发送 SUCCESS/FAIL，finally 清除当前事件并调度下一项。它没有 756 导入样本中的同一套 `isCurrent` 预处理，不能混用版本行为。

真正设备任务使用模块 `81988` 的 Task `[1344889,1345800)`、队列 `[1345801,1348264)` 和 Manager `[1348265,1349862)`。每个任务获得 container 与任务名对应的 exclusive Web Lock，并创建 AbortController。NORMAL 队列按入队顺序；SKIPPABLE 类别取最新任务并对丢弃项发 skipped。取消等待项立即产生 canceled 反馈并释放任务记录；取消在途项发出 abort，实际函数仍须在其已实现的检查点观察 signal。`kt` 在遍历本机档时检查，`lt` 在任务前后检查；这不保证已经开始的一串 HID 写入可以逐包撤销，也没有通用自动恢复原槽的实现。

断开与退出入口是模块 `16737` `[319472,321599)`：USB disconnected、BLE remove/disconnect 按当前 container 与 PID 匹配后，停止投影并调用 `10745.Gn` `[147120,151410)`；exitApp 也进入这条清理链。679 可适用的主干依次涉及 Xbox 事件注销、关闭设备 BroadcastChannel、停止运行标记、删除 `DEVICE_RUNTIME_DATA`、主机 `removeUsbDevice`、停止 turbo/macros，最后关闭窗口。`Gn` 中直接 `setDeviceMode(0,0)` 受 systemPower 条件控制，不能把笔记本分支当作 679 已执行的还原步骤。

这里还存在可核对的完成边界：Xbox 注销及停宏调用没有统一 await；`Gn` 主干使用吞异常的 try/catch，前序失败可能跳过后续语句；断开入口没有调用 TaskManager 的 `cancelAllTasks`。`removeUsbDevice` `[1212945,1213578)` 与停宏包装 `[1228189,1228394)` 只把 action 转到主机 mapping engine，本页不推断原生服务已经归还设备模式、HID 与事件注册。另一清理入口 `Kn` `[151410,151575)` 会卸载窗口/内存存储观察、注销 unsupported mapping 通知并 removeUsbDevice；也不能以该函数存在就认定每次断开都会执行它。

## 当前 Rust 消费者与测试边界

| 范围 | 已定位的独立 Rust 代码 | 本页核对的实际边界 |
| --- | --- | --- |
| 实际槽列表/身份 | [keyboard_profile_inventory.rs](../../../crates/razer-device/src/keyboard/keyboard_profile_inventory.rs) | 直接 HID 读取 max/count/list/250 字节身份；679 保留第一槽实测值，分离未知 GUID、跳过身份、失败；独立 `read_active_hid` 不改变本机选中档 |
| 完整 679 读取 | [keyboard_onboard_profiles.rs](../../../crates/razer-device/src/keyboard/keyboard_onboard_profiles.rs) | 按宏→inventory→全部亮度→各槽 Normal/Hypershift 的 assignment/RT/actuation→kill switch 读取，并补读缺失本机宏内容及 continuous RT；保留 `Unobserved/Failed/Observed` |
| IPC/服务传输 | [keyboard_onboard_profiles.rs](../../../crates/razer-ipc/src/keyboard_onboard_profiles.rs)、[Windows 读取适配器](../../../crates/razer-service/src/runtime/windows/device_reads.rs) | 校验实际产品/传输/layout，按物理设备锁执行当前读取；完整 679 路径没有接成其他 PID/无线分支的通用实现 |
| Shell 请求与清理 | [worker](../../../crates/razer-shell/src/shell/keyboard/keyboard_profile_inventory_worker.rs)、[连接所有者](../../../crates/razer-shell/src/shell/keyboard/keyboard_profile_inventory.rs) | 679 必须使用 complete 请求；连接/serial/revision/request 约束结果归属。取消后拒绝旧连接结果，先 shutdown、finish_termination、drop，清理失败单独保留 |
| 页面板载结果 | [页面所有者](../../../crates/razer-pages/src/features/workspace/source_workspace/keyboard_profile_inventory.rs) | 保存完整观察并刷新配置档选择器；断开或代次变化清除旧槽装饰。当前检索 `keyboard_onboard_profiles()` 仅找到访问器定义，尚未找到消费完整内容执行官方 `kt` 合并、冲突和差异写入的调用者 |
| 本机档案 | [keyboard_local_profiles.rs](../../../crates/razer-shell/src/shell/keyboard/keyboard_local_profiles.rs)、[初始配置类型](../../../crates/razer-device/src/keyboard/keyboard_current_profile.rs) | serial 确认后独立读取本机库；文档不存在或失败有各自状态。本机导入及 `initial_profile()` 构造不提供设备已初始化或写入成功的证据 |

已有纯测试源分别覆盖读取顺序/批大小、第一槽真实身份、十次后未知 GUID、截断/异槽/重复 ID、全零 assignment 两次读取、后阶段失败保留前阶段实测、取消停止下一批、旧代次拒收，以及 worker shutdown/termination/drop 与清理错误。这些断言位于 [inventory tests](../../../crates/razer-device/src/keyboard/keyboard_profile_inventory_tests.rs)、[complete tests](../../../crates/razer-device/src/keyboard/keyboard_onboard_profiles_tests.rs)、[worker tests](../../../crates/razer-shell/src/shell/keyboard/keyboard_profile_inventory_worker_tests.rs) 和 [页面所有者 tests](../../../crates/razer-pages/src/features/workspace/source_workspace/keyboard_profile_inventory_tests.rs)。本页本次核对了测试代码，没有重跑 Cargo，不能据此写成当前工作树测试已通过。

后续逐功能对照必须固定独立期望，并记录同一输入下官方实际 DLL/服务链结果与 Rust 结果，至少覆盖：

| 用例 | 必须保留和比较的事实 |
| --- | --- |
| 打开/重连 | 实际 container、serial、layout、槽列表顺序；主机 ready、本机文档和板载读取的先后；不得插入不存在的 active getter 或把缺文档当设备空白 |
| 合并/冲突 | 同/异 owner、同 GUID 多槽、独有 slot 1、Factory/普通 preset、本机缺 preset、设备读失败；比较新增/更新/保留字段、冲突对、后继任务和实际写入 |
| 切换/板载保存 | 本机 active 与 slot 1 身份、旧映射清理、各 setter 参数/ACK、部分失败、同 GUID 跳过、profileSwap 第二次写入、保存结束消息与任务最终结果 |
| 查询及确认 | 原始 ACK 与测试追加 GET 分开记录；GET 无值或未知值不得从本机文档补出；只比较两个“未连接”错误不构成功能通过 |
| 取消/退出 | 排队前/等待锁/在途各检查点、停止新增传输、每项事件与宏停止、主机移除结果、锁与句柄释放，以及旧结果不得发布 |

无设备时可先做上述纯逻辑与模拟传输、已有依据的应答回放；恢复性写入对照必须保留完整初始槽/宏原值，执行写入、读回并恢复原值，逐项记录清理结果。模拟结果不能作为官方 DLL 或真实设备对照收据；实际 DLL 功能执行、设备保存/恢复、原生释放及运行时 UI 验收在本页均保持“未验证”。

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

- 679 当前连接、板载读取、slot 1/本机活动身份、合并/冲突、保存任务、应答与清理入口已按上文逐函数核对；完整板载观察进入 Rust 后的合并/冲突及必要初始化写入消费者仍未闭合，原生主机移除、设备恢复与实际 DLL/设备对照未验证。
- 全局菜单新增、复制、重命名、删除和导出已核对输入，但各产品消费者、文件格式转换和最终反馈仍需按 PID 继续追踪。
- 756 导入的本地事务、跨窗口观察和宏广播已核对；该产品其他恢复分支不能由此概括为已完成。
- 679、756 之外的预设身份、板载槽数、菜单分组与可编辑条件必须引用各自当前产品描述及实际挂载组件，本文不宣称全族一致。
- 本文为静态源码理解；未执行厂商 JavaScript、应用程序或真实设备验收。
