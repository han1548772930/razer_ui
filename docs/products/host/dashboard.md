# 本机 Dashboard 与设备卡片

Dashboard 汇总已发现设备、设备中间件公布的运行摘要和已安装模块，再把操作路由到对应页面。卡片不是另一套设备设置来源：配置档名称、电量、就绪状态和配件来自各自运行摘要，进入卡片也不代表设备读取或写入已经完成。本文依据页面部署 `0.0.86`／build `2610200951` 的当前根和实际引用 bundle；页面部署身份独立于宿主版本。

## 挂载与导航

Synapse 主页面按原件顺序挂载 `DASHBOARD_HEADER`、`GAMER_ROOM_HEADER`、`DEVICES_AND_MODULES_HEADER`、`GLOBAL_SHORTCUT_HEADER`，分别延迟加载模块 502、19388、44442、94608。Devices & Modules 与 Gamer Room 是两个入口，不能因共享主机外壳而合并。Global Shortcuts 的未保存映射通过共享保存提示阻止离开；完成回调后才更新当前导航。

历史记录从 sessionStorage 的 `tabNavigations_dashboard` 与 `currentTabNavigation_dashboard` 恢复。新增导航在当前位置截断前进历史，返回／前进只改变当前索引。外部 `focusSubTab` 消息可导航到 Gamer Room；全局广播另支持 Devices & Modules 和 Global Shortcuts。`dashboardDisabled` 是覆盖整个窗口的半透明阻断层。

根组件发布 `dependent-app-status`，管理模块窗口、设备窗口聚焦和后台窗口激活。窗口激活使用零延迟定时器合并连续请求；卸载会取消该定时器、终止窗口协调器、移除全局消息与 pointerdown 监听并移除运行标记。原件卸载时对 `broadcastMessage` 再次调用 `on`，并非配对的 `off`；两个 `apiElectron.response` 订阅也未在该类中展示释放，不能把它们描述为已完整清理。

## 设备集合与身份

本机设备组组合 `devices`、`validDevices` 和 `installedDevices`：先用 PID 与 container ID 查有效摘要，具有有效序列号时按 PID＋序列号去重；没有有效摘要、但尚未出现在已安装集合中的项保留安装中卡片。连接、安装与设备就绪是不同状态。

当某 PID 已有另一 container 的实际序列号且处于非 off 状态时，缺序列号或为 `NOSERIALNUMBER` 的离线副本被过滤。Xbox／PlayStation 分支有独立条件；兼容设备、Windows 10 设备、Xbox 耳机、Xbox 手柄和开发中设备分别使用分组卡片。空集合才挂载 noDevice。

普通卡片通过 `connectedDevices` 的 window-storage 事件匹配 PID＋container 更新摘要，展示 `loading`、`ready`、`waiting`、`error`、`canceled`、无 alive、`standby` 和 `off` 等状态。当前配置档从 `activeProfile` GUID 在 profiles 中查名称，自动切换另附 Auto；IOT／复合设备使用 `isShownOnUI` 子设备，不能拿主接收器身份代替子设备。

## 卡片动作与电量

卡片把点击动作交给拖动所有者：mousedown 开始会话，mouseup 且位移未超过约 10 px 才执行页面聚焦。需重启、低于最低固件、preset 正在加载、off、待机、固件更新和特定系统检查各有阻断路径。卡片上的固件入口只导航到 Devices & Modules；它本身没有执行固件刷写。

电量显示区分充电、未充电、暂停、警告和离线。`level=-1` 是查询失败值，不能改写为 0%。首次上报以及从 off 恢复时，原版先显示 `-`，约两秒后才公布电量。该计时器在普通卡片的卸载方法中没有取消；window-storage 与自定义 windowstorageevent 则有对应解绑。

Windows Dynamic Lighting 图标同时要求全局功能开启、设备支持、ready 且非 BLE；点击打开对应设置窗口。无 alive 的恢复动作向 background-manager 发 `resuscitate`，不是把卡片立即改为已连接。复合设备数量徽标和 `singleProfile` 的显示各有独立条件。

## 双连接卡片

当前实际挂载链为模块 502 导出的 `Vi → Gi → Hi.renderGroup → Ci(Di) → getProdItems → Ii`。分组 mount 异步查询 `global.multipairing`，只读取返回对象的 `provisionFlags`，先保持 false；查询拒绝仅记录错误。返回后重建卡片并重新计算顺序，该调用方没有取消已发查询的 guard。普通卡片与双连接卡片同时参与设备分组，并非用双连接卡片替换全部普通卡片。

候选来自 `validDevices`，必须同时满足 `isMultiPairingDevice`、`setupStatus === "ready"` 和接收器资格。`isOnlyWirelessMode` 直接通过资格；其余须在原 `devices` 中找到同 PID、同 container 的 `isDongle` 项。当前表达式在 `devices` 缺失或空集合时也放行，不能把这条源码路径解释为已经实测找到接收器。

随后读取本地 `duallink-devices` 并取 `Object.values`；解析失败按空记录处理。每条记录投影 container、`master.productId` 和 slave `productId`，实际筛选 slave 时只比较 container 与 slave PID。若找到 slave，按这些 slave 的序列号剔除候选，留下配对主卡，并按 container 收集 slave 的 `chargingStatus`；未找到则保留全部候选。这里的配对归组来自本地关系记录，不能推定当前实际连接，也不能改成只按 PID 合并。

普通卡片的排序身份是 `${productId}-${deviceContainerId}`，双连接卡片是 `${deviceContainerId}-${productId}`，分别由同一模块的 `LP`／`UU` 生成。配对主卡显示 off 警告要求本卡 off 且该 container 下全部 slave 状态都严格为 off；源码对空数组使用 `every`，结果也为 true。未配对时只检查本卡。警告与真实连接读取不是同一个状态源。

### 卡片导航、候选主设备与图片

卡片只向拖动所有者登记 `overrideAction`；鼠标释放、仍是短距离拖动时才执行。执行时 PID 和 container 必须都存在，构造该 PID 的原 UI 地址，附带 `containerId`、`displayMode=multiDevicePairing`，以及非空时的 `allMasters`。窗口身份优先为 `multi-device-pairing-<container>`；通用命名函数另外支持 PID＋非空序列号，最后才是公共窗口名。窗口打开器先按完整窗口名查询本机客户列表；已有窗口则 activate，没有才 `window.open`，使用 `policy=3,shouldFocus=1`。这里没有直接发送扫描、配对或解绑设备命令。

`allMasters` 的生产顺序和字段校验独立于卡片分组：

1. 依次合并 `connectedDeviceInfo`、`duallink-devices` 中的 owner／master／masterDevice／deviceInfo 候选和当前设备；旧记录只有 master PID／dongle 时单独构造候选。
2. 按数值 PID、dongle ID、edition ID 去重，先出现者保留；不会把后项字段补入前项。
3. 中间集合允许有效设备项或与当前设备 PID 相同的项；最终标准化仍要求有限正数 PID、非空序列号和非空 `productName.en`，不满足则不发布。
4. 对合格项仅保留具名身份字段，以及实际存在的 dongle、category、有效 layout／edition 和 container。不会把本地候选升级为设备观测，也不会生成序列号或名称。

双连接卡片的接收器图有有序候选链，不能套普通设备图或跨产品借图。仅 `canPairTwoDevices`、`isOnlyWirelessMode` 或 ACCESSORY 项可以先使用自带 `src`。PID 或 dongle PID 为 226／227 时选专用资源，edition 128 使用独立图，其他值使用 edition 0；该分支不再构造通用型号 URL。其余型号先请求本 PID／edition 的接收器图，非零 edition 才追加同 PID 的 edition 0。最后依据 productivity／newHyperspeed 标志选择对应通用图，按原顺序去重；出错只推进候选索引，最终失败不会创建成功状态。PID、edition、src 或 container 改变会重置索引。

### 打包代码与实际挂载的边界

同一 56 bundle 内还保留 `kt` 配对窗口类、`_t` iframe bridge 和配对状态机。`kt.renderView` 能组合 `Ot(_t)`，但模块 502 的默认导出和 Dashboard 卡片路径没有挂载 `kt`；其 `connect(...)(kt)` 表达式的结果未成为这里的导出或卡片子节点。因此不能把其中的 window-list watcher、iframe 握手或 `DUALLINK_*` 状态机直接计作 Dashboard 已执行的生命周期。实际导航目标是产品 UI 的 `displayMode=multiDevicePairing` 分支，须沿该入口及 multipairing 页面另行核对。

原件中的 iframe bridge 确实比较 `event.source === iframe.contentWindow` 和确切 origin，收到 `multiDevicePairingReady` 会发送 `multiDevicePairingInit`；同一发送函数也由 iframe 的 onLoad 与独立 effect 调用，不能建模为只在 ready 后发送。effect 清理移除同身份 message 回调。这仅说明保留函数本身的语义，不证明本 Dashboard 挂载了它。配对写入、事件完成与取消仍由真实目标页面和中间件拥有。

## 模块、分组和布局

本机模块从 `AVAILABLE_MODULES` 与安装记录筛选，部分本地模块有自己的额外入口条件。模块卡片按名称打开对应窗口；维护状态可阻止打开，安装中显示 spinner。当前实际模块卡类 `X` 定义了 `getDeviceStatus`，把 enabled／connected、disabled、unknown 映射到绿色、白色和橙色，但该类的 mount 仅调用 `checkUpdating`，连接包装器也没有调用此 helper。状态初值仍为空、disabled 初值为 false，不能把未消费的 helper 当作当前卡片已经接收状态的证据。updating 检查也只在 mount 调用，未展示对应的更新钩子。

普通卡片宽 290 px、高 220 px，横纵间隔均为 20 px；列数按实际容器宽度计算。每组以稳定 `boxId` 维护显示顺序、离线项和新增项，noDevice 不保留到正常排序，特定模块引导保持在末项。折叠状态按组标题保存，拖动重排同步更新组顺序；回位动画为 300 ms。

拖动会话临时注册 document 的 wheel、mousemove、mouseup；正常 reset 会移除三者。分组类没有 `componentWillUnmount`，不能声称中途卸载也执行 reset。Dashboard 容器的 ResizeObserver 以约 60 Hz 节流计算列数，卸载时取消观察，并正确移除自身 storage／memory-storage 监听。介绍横幅关闭后写入本地状态并发出 `show-profile-migration-toast`。

Chroma 的 Studio、Connect、Sensa 和 Visualizer 行为分别见 [Chroma Studio](chroma-studio.md)、[Chroma Connect](chroma-connect.md)、[Sensa](sensa.md)、[Visualizer](visualizer.md)。这些模块的设备任务和退出恢复不归普通 Dashboard 卡片拥有。

## 当前页面与 Rust 接线核对

页面的固定几何须与行为一起保留。当前实际加载的 55 样式中，`.dashboard` 常规上限为 1220 px、最小宽为 620 px，`.reflow` 上限 2460 px；1279 px 及以下上限变为 910 px、正文两侧 inset 为 30 px，600 px 及以下上限为 290 px。组件写入的 `width:unset` 又覆盖 1260 px 断点的 `width:min-content`，不能孤立翻译一条 CSS。分组上下 margin 为 10 px，内容与标题相距 10 px、卡片间距 20 px，标题为 14 px。普通卡片 padding 为 `10px 20px 9px`、圆角 5 px、底色 `#111`；无设备卡片保留虚线轮廓。普通／选中／悬停、loading 组的 `visibility:hidden`、折叠的位移／max-height／overflow、拖动层次是独立状态。依据为 [55.4e8559cb.chunk.css](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/css/55.4e8559cb.chunk.css) 的分组／断点 `[182467,186278)` 及实际 `.box-item`／`.box-no-device` selector，未将同包 Gamer Room 或未挂载配对窗口的样式混入普通卡片。

当前 Rust 的实际入口是 [dashboard_cards.rs](../../../crates/razer-shell/src/shell/navigation/main_pages/dashboard_cards.rs)，不能以注册表有某 PID、存在 `DeviceCard` 或导航可达替代页面完成。静态核查结果如下：

| 页面表面／动作 | 当前 Rust 所有者与实际接线 | 尚不能计为完成的边界 |
| --- | --- | --- |
| 布局、设备组与空状态 | [MainLayout](../../../crates/razer-dashboard/src/lib.rs)、`dashboard_device_group`、`dashboard_empty_devices` 保留卡片尺寸、断点和本地空状态；设备卡片来自 `self.devices` 中的 workspace，点击进入该身份的设备页 | 官方 `devices/validDevices/installedDevices` 的去重、占位安装项与 Xbox／PlayStation／Windows 分类组未由这一个遍历完整复现；真实发现与全部卡片分组须分别验证 |
| 普通设备卡片、图像与提示 | [DeviceCard](../../../crates/razer-dashboard/src/dashboard_device_card.rs) 消费 setup、alive、固件、WDL、sub-device、edition 与图像选择；[dashboard_device](../../../crates/razer-dashboard/src/dashboard_device.rs) 独立持有延迟电量观察并按 GUID 查配置名 | 原件的全部产品／系统分支与当前设备生产者仍须逐项对应；本地 preview 图片不能作为连接、电量或配置已读的证据 |
| 卡片排序、点击与取消 | [DashboardGrid](../../../crates/razer-dashboard/src/dashboard_grid.rs) 使用领域卡片身份、`DashboardState`、300 ms 位移动画、10 px 短拖动判定、wheel 与 release；原生 drag preview 的 release 取消只恢复本组顺序且不激活卡片。Base Button 提供独立键盘激活入口 | 分组标题当前只接折叠，未接官方整组拖动排序；跨页面卸载、失焦及没有进入原生 drag 的在途 press 清理仍需逐路径核验，不能由成功 mouseup 推断 |
| 折叠和本地保存 | [DashboardGroupContent](../../../crates/razer-dashboard/src/dashboard_group.rs) 分开处理高度、位移、箭头与 overflow；`DashboardChanged` 经 shell 的串行 workspace writer 保存，并以 captured snapshot 标记已存 | 这是本地布局持久化；不能据此覆盖设备状态、真实服务动作或所有组排序。静态动画实现不等于真实窗口像素／焦点验收 |
| 重试安装／设备恢复 | 卡片将动作交给 shell；当前 `retry` 回调只发出“安装／恢复服务未连接，尚未发送请求”的提示 | 尚未提交原件的 reinstall／resuscitate 服务请求、接收状态或完成释放；该项没有后端闭环 |
| 固件与 WDL 图标 | 固件点击导航到 Devices & Modules；WDL 点击导航到设置入口 | 页面导航分别核验，不代表固件传输、WDL 控制权切换或系统设置页实际执行成功 |
| 模块与附属入口 | Rust 依次构造 Macro、Linked Games、Add Wi-Fi Device、Profile Migration、Tour 五个本地入口；相应动作进入各 owning 页面或 popup | 当前列表按已编译本地功能构造，没有复现官方 installedModules／模块更新观察链；各弹窗和目标功能另行验收，不能因卡片可点击宣称完整实现 |
| 多配对组卡 | 官方候选、关系投影、`allMasters` 和产品 `displayMode=multiDevicePairing` 入口已在前文静态追明 | 当前 `dashboard_device_group` 没有消费这些关系或构造 `Ii` 对应组卡；单独存在配对路由不能补足 Dashboard 的组卡及全部目标分支 |
| 引导和介绍横幅 | [DashboardTutorial](../../../crates/razer-dashboard/src/dashboard_tutorial.rs) 保留教程已读状态、100 ms 定位等待、popover 与独立文案／媒体；介绍横幅的关闭和 Tour 进入 shell 导航／保存 | 须继续对照实际挂载位置、重开、焦点恢复、滚动与全部资源状态，不能以教程 flag 已保存覆盖这些行为 |

已存在的 `dashboard_grid_tests.rs`、`dashboard_group_tests.rs`、`dashboard_tutorial_tests.rs` 与卡片／电量测试是具体测试入口；本轮只静态读取其实现和用例，没有运行这些窗口测试，也没有新增设备或官方 DLL 功能对照。后续用例必须覆盖当前官方候选数据、源值与本地草稿不同、错误／断开／重连、图像失败、拖动取消及卸载，分别记录页面、本地存储、真实服务和设备结果。

## 当前原件

以下读取均核对 source set acquisition 中实际 bytes 和 SHA-256，并确认当前 asset-manifest／lazy loader 的引用；区间为半开 UTF-8 字节。当前 index、asset-manifest 和 manifest 的前后采样一致。本文只记录本机设备、模块和窗口语义。

| 来源 | 实际阅读范围 |
| --- | --- |
| [App.70b4a4e6.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/App.70b4a4e6.chunk.js) | 导航类 `[15370,19001)`；主机窗口根 `[21851,29095)` |
| [main 当前词条](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/main.d1d8c15b.js) | 导航词条声明，byte 137729 起；由 App 的实际四项导航读取对应导出 |
| [56.d9a580a0.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/56.d9a580a0.chunk.js) | 普通设备卡 `[3535,22650)`；模块卡 `[22971,25372)`；分组 `[100828,113893)`；Dashboard `[120446,129390)` |
| 同一 56 原件的双连接链 | 候选校验／标准化 `[48392,49365)`、`[52324,54875)`；图片候选 `[97762,98658)`、实际卡片 `[98659,100508)`；分组接收器资格／关系投影 `[106282,107711)`、实际挂载 `[107712,109268)`；排序身份模块 65153 `[152771,152937)` |
| 同一 56 原件中未由 Dashboard 挂载的配对子树 | iframe bridge `[54968,57579)`、保留的窗口类 `[57945,61885)`；不能以这些完整函数的存在替代实际调用方 |
| 同一 App 原件的窗口路由 | 模块 84058 `[89068,90459)`，窗口身份与已存在窗口聚焦；模块 88263 `[90466,91202)`，设备 UI／MW 频道身份 |
| [2280.e728a644.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/2280.e728a644.chunk.js) | feature 查询模块 42723 `[114374,114691)`，按 key 缓存原 `getAppFeature` 请求结果 |

上述原件区间已直接读取，并用项目管理的 JavaScript AST 解析器核对完整所有者和调用位置，未执行 React 或厂商代码。当前 acquisition 的 `unresolved` 是 chunk 枚举不可用所保留的动态依赖闭包缺项，map 获取失败由独立文件记录维护；二者是不同集合。恢复输入同时含保留的旧部署原件，完整 lexical factory 恢复量不能当作当前 manifest 可达模块数量。已取得原件字节一致、本页具体语义、Wakaru validate 的非零阶段与完整依赖闭包分别核对。

这是对应部署收据下官方前端的静态语义，页面部署身份与宿主版本独立。本文没有据此声明 Rust 后端、界面接线、官方 DLL 实际功能对照或真实设备验收完成，也没有执行厂商 JavaScript、主程序或安装动作。
