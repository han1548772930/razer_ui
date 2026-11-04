# 鼠标：按钮自定义、Hypershift 与侧板映射

Customize 将设备上的输入身份映射为输出动作。当前配置保存普通 `mappings`；可换侧板产品另保存按侧板名称区分的 `sidePanelMappings`。Hypershift 是同一物理输入的另一层映射，由 `isHyperShift` 区分，不是另一个按钮编号。

## 页面真正编辑的内容

选择按钮打开映射面板，按 `inputID` 与 Hypershift 层查找现有配置。界面按钮文案、按钮位置及列表下标不参与该匹配。物理布局、edition、当前侧板、左／右手模式和接收器连接会影响可选按钮及显示；源码的 disabled 或 `disableHypershiftMapping` 条件会直接拒绝打开相应输入。

映射面板按产品允许的 group 挂载：默认、键盘／组合键、鼠标、灵敏度、宏、多媒体、Windows 快捷键、文本、配置切换、设备间动作、灯光、禁用、Hypershift 等。通用组件还含应用启动、音频、旋钮和双触发等分支；组件代码存在不代表每只鼠标都启用。板载可保存的 group／动作由该产品 `OBMSpecs.supportedMappings` 限定，页面会分别标示需要主机软件的动作。

PID190 例中，板载灵敏度映射明确允许 DPI_Clutch、DPI_CycleUp、DPI_CycleDown、DPI_Up、DPI_Down；循环滚动等扩展鼠标动作另由 `extendSupportMappings` 描述。PID158 的映射引擎表解出 150 个键／输入条目和 38 个默认 turbo 项，这些是该 factory 的数据规模，不能当作所有产品的物理按钮数。

## 保存与放弃

PID158 映射面板把当前功能编辑器的 `saveMapRef` 作为“生成映射”回调；外层把面板的 `saveMapping` 注册为 Save／Don't Save 弹层回调。当前 editor 的草稿与配置中的 mappingList 因此不是同一个对象。

Save 时，`saveChanges` 先检查双触发 joystick 选择是否合法，再从编辑器取得一个或多个 mapping。它按 inputID 和层合并或替换，默认映射会移除相应自定义项；把 Hypershift 键改为其他动作时，还移除同一输入的附属 Hypershift 项。宏映射另维护绑定关系。最后调用 `setActiveKeyMapping`／`setMappingList`，清除 changed 状态、关闭面板并触发适用的 Windows 登录提醒。

Don't Save 调用 `saveMapping(false)`，不执行 `saveChanges`，然后执行之前记录的跳转／切换回调；关闭确认弹层的 dismiss 路径可携带另一个参数，保留原来的后续动作语义。左／右手切换若碰到尚未保存的映射，也使用这套 Save 提示和回调，不是另一个独立设置页。

面板的 window-click 关闭规则明确排除按钮、菜单选项、特定提示及容器节点；滚动和窗口缩放会重新定位键配置弹层。不能仅按“点击面板以外任意位置就无条件丢弃”概括它。

## 本地保存、主机映射与板载写入

PID158 中间件 `30525.bt` 读取当前配置，并按 `viewIndex` 和当前侧板合并普通／侧板映射。需要主机映射时，选中的 `32226` wrapper 导出 `77898.RzAppEngineMapping_default`，将映射及 turbo 规则转换为 appEngine 数据、计算其 hash。配置保存随后更新 root version、`mappings`／`sidePanelMappings`、appEngine 和宏投影，并刷新页面。

本地 completed 事件发出后，若动作还允许 deviceAccess，会另排 `NORMAL` 板载同步任务。该任务先处理宏的增删与映射，再对实际 GUID 匹配的板载槽位工作。源码还包含 2／6／7／12 键侧板集合和侧板 edition 的选择；保存当前侧板不能抹掉其他侧板的映射。

主机运行映射、板载写入和 UI 配置保存各有消费者。改变键盘组、宏或文本等输出时，不能仅凭本地 mappingList 改变就称物理设备完成了全部动作。`taskMakerSetKeyMapping` 的异常会记录 oldMapping、newMapping、message 并发布 error/post-process；其完整函数没有通用的设备失败后 UI 回滚处理。

Chroma 相关映射还有实际事件差异：PID70／158 的 CycleUpChromaEffect 消费者选择 previous，CycleDownChromaEffect 选择 next；跨设备切换配置的目标消费者会按条件执行导航和关联 Chroma 选择，可能形成两次有序本地变更；这不是高级灯光帧已写入的确认。

## 五款接收器的描述符发布与存储观察

PID158、192、194、211、222 的实际 `PreSetLocalStorage`／`PostSetLocalStorage`、publisher、双设备合并及主机调用方，集中登记在[当前五款审阅](../../../.analysis/results/frontend/mouse-main-audit.json)的 `receiver_legacy.mapping_descriptor_contract`。[不可变源码审阅](../../../.analysis/results/reverse-pending/review-sources/e63e78b8e726ac3bb8706e4cfaefae244ff5055f65dabe8ae02e19a651b539fa.json)保留 255 个完整 owner 与 76 个完整 factory 引用，并绑定[共享原生描述符审阅](../../../.analysis/results/reverse-pending/review-sources/21fe84d134926df8d0aecdcd5bbc215e298bd2a4709a9a964d7e344624b27960.json)。两者复用完整捕获；共享 chunk 中存在其他分支，不表示实际启用。

| PID | 无线物理身份 | 当前 `isDualLinkDevice` | PreSet 可读取的描述符导出 |
| --- | --- | --- | --- |
| 158 | dongle 159 | false | `REPORTIDS`：4 为按键，5 为硬件事件 |
| 192 | dongle 193 | true | 无 `REPORTIDS`／`BLE_REPORTIDS`，保留文档已有 map |
| 194 | dongle 195 | false | 无 `REPORTIDS`／`BLE_REPORTIDS`，保留文档已有 map |
| 211 | dongle 212、BLE 213 | true | BLE 的 `BLE_REPORTIDS` 只保留按键 4；其他连接保留已有 map |
| 222 | dongle 223 | true | 无 `REPORTIDS`／`BLE_REPORTIDS`，保留文档已有 map |

五款官方初始本地文档模板都是 `version=1`、`vendorId=5426`、逻辑产品 PID、`category=MOUSE`，初始 map 为按键 4／硬件事件 5。这是本地文档的初始内容，不能据此判断硬件首次初始化或设备实测值。`FIRMWARE_REPORT_IDS` 即使导出也不被这个 PreSet hook 读取；PID211 的 BLE 分支另外选择 Protocol30。

PostSet 按实际运行设备的 NONE／MASTER／SLAVE 角色分流：NONE 直接发布，只有 Mika 才先合并；MASTER 合并后发布；SLAVE 不在此 hook 发布。这五款当前 feature 输入都不选择 Mika。合并复制 master 文档，只有 slave 配置和序列号条件满足时才覆盖 slave map，最后设置 slave 按键 report ID；普通分支的 master／slave report ID 为 4／8，unit ID 为 0／43981。合并后的逻辑 `document.productId` 与运行对象中的物理 `rzDevice.device.productId` 必须分别保留。

publisher 的抑制条件是 `controllerDevice`。只有启用 `useMappingEngine.config.setMappingEngineLocalStorage`、未选择该 feature，且默认 appEngine hash、activeProfile、profiles 数量／GUID／hash／gamemode，或存在的 analogData hash 改变时才发起存储。`reportIDs` 单独变化不触发发布。hash 的当前算法复制对象、排除顶层 `hash`／`gamemode`、稳定排序 JSON 对象键、转义 `<`，然后计算 UTF-8 MD5；数组保持顺序。

缓存先更新，之后才等待 `localStorageSetItem`。该返回布尔值被丢弃，失败没有缓存回滚；因此一次 false 之后，同样内容的下一次调用可能被缓存门禁抑制。PostSet 也没有等待或返回所创建的 NONE／MASTER Promise。这些是源码的完成语义，不能把本地 completed 当作原生解析、活动映射或硬件写回成功。

真实 key 是 `synapse_<rzDevice.device.productId，缺失时取 DeviceInfo.productId>_<CONTAINER_ID>`。后续链为当前产品桥接器 → `apiElectron.doMappingEngineAction` → `mappingEngineAction` IPC → 实际 `FFIMappingEngine` singleton → Windows `callDLL` → `localStorageSetItem`。原生层用发现时的物理 productId／editionId／serialNumber／containerId 选择订阅 key，`DeviceData` 不从文档的 vendorId／productId／category 推导设备身份。存储成功只证明存储操作；原生 SetData 解析成功后，还须满足 change bit 才替换已发布对象，`reportIDs` 独改同样不会置该位。

master 存储观察还要求 `isDualLinkDevice`、物理 PID 在 `[dongleId, bleId].filter(Boolean)` 中，且初始化器核对实际 PID 等于 dongleId。因此当前只有 192／211／222 的各自 dongle 分支能到达；158／194 以及 211 的 BLE 分支不据此启动 master observer。该分支先处理已有 slave 文档，再监听 `/^synapse_\d+$/` 的窗口 storage 事件，读取 master、合并、转换并调用同一 publisher。序列号的 window-storage 服务回调在取得序列号后移除自身；匿名 `window` storage listener 没有保留移除句柄，不能称所有观察已经完成注销。

复核入口为 `python -X utf8 tools/workspace.py audit-family-function --family mouse --function main --query mouse-receiver-mapping`。本范围查清描述符生产、身份、门禁与观察调用方；全部映射执行器、独立 Rust、UI／后台接线和实际官方 DLL／设备对照仍分别保持未验证。

## 面板退出与释放

`clear` 清除 active button、收起面板、清 changed 状态；若当前输入为 AnalogInput，另向中间件发送停止相关输入观察的消息。componentWillUnmount 移除 window click、resize、capture scroll listener，断开 observer 并更新显示值。`saveAll`／`discardAll` 是共享事件源，不能把 DOM listener 移除概括为所有事件源已解绑。

按钮来源的布局和侧板改变后会重新加载按钮信息、合并该层映射，再清 mappingUpdated。PID158 的完整 `loadFilteredMappings` 还区分 SYSTEM／KEYBOARD、手柄和 attachment 分支；这些分支只在对应配置条件成立时运行。

## 侧板身份与型号分支

当前 `getButtonSideName` 使用设备／页面提供的领域编号；编号 0 返回空字符串，不能把默认 `sidepadLayout=3` 当作已连接 12 键板。静态 `hasAttachement=true` 只决定 attachment 结构，固定侧面也沿用该结构。

| PID | 编号 1 | 编号 2 | 编号 3 |
| --- | --- | --- | --- |
| 80 | `7ButtonSide` | 空 | 空 |
| 83、141、150、180 | `12ButtonSide` | 空 | 空 |
| 103、147 | `2ButtonSide` | `7ButtonSide` | `12ButtonSide` |
| 143、167、231 | `2ButtonSide` | `6ButtonSide` | `12ButtonSide` |

PID167 的设备 reducer 把真实 `SET_SIDEPAD_LAYOUT` 投影为 layout 及 `sidepadConnected=layout!=0`；Customize reducer 另保留当前浏览 `viewIndex`。保存普通按钮传 viewIndex=0，侧板保存按 `getButtonSideName(viewIndex)` 修改该名称的数组，其他侧板保留。重置按键才显式清空已知侧板集合。硬件当前侧板、用户正在浏览的侧板和配置保存的侧板映射是三个不同对象。

主机 appEngine 生成时，把普通映射与当前实际侧板名称对应的映射合并，再转换输出和计算 hash；切换侧板还需重新选择这组映射，不能只更换鼠标图片。配置导出收集宏时遍历全部 `sidePanelMappings`，所以非当前浏览侧板引用的宏也必须保留。

## PID199／208：折叠映射功能列表

这两款的当前 DeviceInfo 显式启用 `isHidingKeyMapping`，默认前缀长度为 5。`getDisplayFunctionList` 先依据当前输入、Hypershift／secondary 和所选配置过滤功能；折叠时取过滤后列表的前 5 项再拼接最后一项，不能把它写成总共仅显示 5 项。`RazerAI` 输入直接返回完整列表；secondary 使用自己的允许集合。

只有总功能数大于 6、产品启用折叠、`isShowHideMappingUI` 成立且当前输入不是 `RazerAI` 时，才显示 Show All／Show Less。点击只翻转局部 `hideMappingUI`，不删除 profile 的已有映射，也不产生设备命令。列表折叠、鼠标悬停的功能栏收缩和具体输出编辑器的 Save 是不同状态。

## PID182：本地旧版配置导入与宏坐标

实际导入组件按当前页面 origin 创建 `/profile-migration/worker.js`，名称为 `s3FileWorker`。此动态路径不在主机 manifest 的静态引用中。[两轮官方 HTTP 复核收据](../../../.analysis/results/frontend-revalidation/29e00387b8e4afa73f25c320303d7d38e1e56064225af2a2176a36543e54612d.json)证明该 worker 的当前部署字节与已保留原件一致；此次复用原件，未新增 raw 副本、临时目录或原生分析会话。该收据只覆盖实际动态 worker，不代表旧页面入口或整个应用部署已经重新核验。

worker 先回传输入，再异步调用 `9113.loadSyn3File`、同步调用 `5167.toSyn4` 并返回转换结果或错误。ZIP 全文件加载后，按可选精确文件名列表过滤，各条目并发解码文本与 XML；单条 XML 异常记录后继续，外层 ZIP／分组异常记录后返回 undefined。`DeviceInfo.xml` 必须精确匹配；重复 XML 子标签聚为数组，非空 content 直接返回。Features、Macros 的正斜线前缀分支不要求 XML 后缀，反斜线分支要求，不能自行补括号改变原式优先级。其余条目可合并到顶层，覆盖顺序仍受真实条目处理次序影响。

转换器按文件内原始 `Product_ID` 分派，再通过 `swapPID` 规范返回身份；导入目标 PID182 只参与类别／警告判定，不能用它裁掉其他源产品。同类别其他源产品仍可导入，例如源 183 的返回身份可变为 182，但与目标 182 的原始 PID 比较仍可产生警告。共享转换异常会终止本次 worker；all／源产品批量转换逐项捕获，且原式在首次结果 truthy 时调用同一转换函数两次，部分结果可能保留。

本批核对九项共享计算：DPI 档位与 active 加一、普通／高速轮询的覆盖、亮度、灯效及 PID780 方向、休眠秒转分钟、滚轮、智能调暗、动态灵敏度；同时核对 all 分组的电源／功耗成对输入。52 个共享入口及 26 个源产品入口按实际注册顺序保留完整 AST 范围、原件／函数哈希、源 PID、特征选择表达式与真实完成状态；其余转换器保持待逆向，不因入口已找到而标为完成，也不添加无源码依据的版本门禁。

完整旧版宏转换保留单个／数组事件、键盘扫描码与 extended 标记、按键／鼠标／循环配对栈、非移动事件之前的秒单位延迟、未知事件抛错、文本／分组／命令，以及键盘的 `Id` 和鼠标的 `id` 大小写差异。移动模式映射为 1→2、2→1、3→3，worker 仅解析坐标与毫秒时间，累计秒数保留三位小数；最后一段移动模式写入 actionBar。宏数组没有逐项 catch，一个异常宏可使整个 worker 回复错误。

MW 导入另外读取 monitor／virtual-screen 信息，旧版文件还会再次刷新这些全局值。模式 1／2 先用主屏尺寸把 65535 坐标还原，再由宏翻译器按主屏尺寸换回；相对坐标按选中 monitor 的 DPI scale 换算。当前选择只处理第一个／第二个 monitor，缺少主屏会抛错，时间残差按严格 `>1`／`<-1` 校正，hash 先于 `mouseMoveType` 更新。virtual-screen 信息被保存，但不参与这段坐标公式；全局可用性只查 truthiness，空数组同样可通过。

monitor 回复沿实际 `window.top.apiElectron`、preload IPC、当前主机 sysutil 与 FFI 指针链返回；当前 4.0.896 主机实际实例为 `po`，来源经版本收据、ASAR 原件及提取别名逐字节核对。前端 truthy monitor 文本解析失败返回 undefined，falsy／无支持路径返回空数组；非 Electron 路径的真实 gate 是 `window.setBounds`。FFI 读 C 字符串后按成功标记调用 FreeMalloc。该静态结论不覆盖原生 monitor 枚举计算、ABI 实际执行或 OS／设备对照。

SharedWorker 以显式 portId 保存端口，消息转发先回复 echo；页面忽略 ready／echo，并在每次收到消息后新建两秒 timeout。完整函数中没有 request ID、Map 删除、port.close、abort 或连接代次保护，不能凭关闭弹窗宣称在途工作已取消。

统一复核入口为 `python -X utf8 tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query mouse-182-legacy-worker-chain`；只有显式 `--prepare` 才重新获取 worker。正式结果归[现有鼠标中间件收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `mouse_182_legacy_worker_chain`。后续从该段四项 remaining 接续：其余共享转换（含 Mappings8061 → input62／output3405）、未闭合源产品转换、当前原生 monitor 枚举，以及独立 Rust／UI／真实 DLL 与设备对照；本批未修改或重新验收产品代码。

定向验证首次 15 项中有 1 项因默认选择了旧 Host 输入而失败；已改为显式选择当前版本收据，并保留当前版本与 ASAR 身份断言。`Mouse182LegacyWorkerChainTests` 修后实际复跑为 15／15 通过；测试仅验证静态收据及维护流程，不执行厂商 JS／DLL 或主应用。

## 原件与复核位置

- PID158 映射面板：`.analysis/inputs/frontend/mouse-158-current/raw/apps.razer.com/synapse/products/158/ui/static/js/567.9db2e44b.chunk.js`，windowClick `[34171,35400)`、clear `[36620,36990)`、saveMapping `[36991,37131)`、saveChanges `[42138,44558)`、主要退出 `[69469,69741)`；直接复读原始字节。
- PID158 外层 Customize：`.analysis/inputs/frontend/mouse-158-current/raw/apps.razer.com/synapse/products/158/ui/static/js/main.d9d086de.js`，loadFilteredMappings `[5641383,5642860)`、各组投影及默认映射 `[5648100,5650020)`、Save／Don't Save／后续动作 `[5650020,5651700)`。
- PID158 中间件：`.analysis/inputs/frontend/mouse-mw-158-current/raw/apps.razer.com/synapse/products/158/mw/6259.1944c2392b18ba3f8282.js`，wrapper 模块 32226 `[151883,152427)`，`30525.bt` `[1505570,1509301)`；直接复读完整函数。
- 全键／turbo factory 与回调差异：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries["mapping-engine"].products[product_id=158]`、`queries.shared_mapping_chroma_callback.products`／`semantics`；PID190 的当前 `DeviceInfo` 位于 performance 文档所列模块 8193。
- 侧板名称 getter：各 PID 当前 UI `main.*.js`，80 `[4862625,4862750)`、83 `[4710936,4711062)`、103 `[45237,45435)`、141 `[143843,143969)`、143 `[4770502,4770698)`、147 `[45206,45404)`、150 `[32747,32873)`、167 `[168925,169121)`、180 `[4783370,4783496)`、231 `[115527,115723)`；完整文件身份见 [鼠标原件表](README.md#产品定义原件)。PID167 UI reducer `[5063858,5065821)`、Customize reducer `[5114494,5117644)`；同目录 `main.05d42904.js` 的全侧板宏收集 `[5824135,5824468)`。
- PID199 UI `5107.34b4b1e1.chunk.js`：折叠长度 `[31013,31156)`、过滤 `[51917,54227)`、toggle `[65569,65631)`、render `[73330,76892)`；PID208 对应 `5107.c6818f1c.chunk.js` 为 `[32723,32866)`、`[53627,55937)`、`[67279,67341)`、`[75040,78602)`。

本页是原版实际前端和中间件语义说明；主机映射服务、物理按键事件、板载写入和真实窗口行为未在本次执行验证。
