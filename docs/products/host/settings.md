# 全局设置：独立宿主、Synapse 与 Chroma

设置页管理应用的启动、通知、教程、迁移入口、灯光控制权、语言和版本信息。独立宿主设置另有 Software、Systray 和 General 三个页签。这里的大部分选项属于本机应用状态；它们不等同于某个设备配置档，也不经过设备设置的统一 Apply 按钮。本文只展开本机功能。

## 产品内的两套真实页面

| 应用与导航 | 左列 | 右列 |
| --- | --- | --- |
| Synapse → SYNAPSE | 自动启动、通知 | 重置教程、配置迁移、Windows 动态灯光 |
| Synapse → GENERAL | 语言、本地 Release Notes 入口 | 本机版本信息 |
| Chroma → CHROMA_APP | 自动启动、通知 | 重置教程、配置迁移、Windows 动态灯光 |
| Chroma → GENERAL | 语言、本地 Release Notes 入口 | 本机版本信息 |

Synapse 的入口异步加载模块 4914；Chroma 加载 1743。各自的 Provider 包裹对应设置树，外层窗口焦点组件决定 body 是否显示。Synapse 使用 `windowName="synapse"`、`tabName="settings-synapse"`，Chroma 使用 `chroma-app`、`settings-chroma`。隐藏 body 与卸载组件是两件事。

设置根组件挂载时初始化共享频道与存储，读取语言，调用 `RazerApp.setBrowserTabVisible(true)`，监听 `beforeunload` 和 `languageChange`。关闭处理先设置 `showSettings=false`；窗口关闭回调随后 `window.close()`。当前组件卸载只直接移除 `beforeunload`，不能据此声称它注销了所有外部监听。

导航将历史列表和当前位置写入 sessionStorage。点击另一项会把新项追加到历史；工具栏前进／后退按当前位置派发 `updateActiveView` 和 `updateTabNavigation`；刷新调用 `window.location.reload()`。文档中的页面表来自真实渲染调用，而非 CSS 中保留的旧设置类名。

## 独立宿主 `/settings/`

独立窗口的原件、导航和 About 值源已由当前部署补齐，完整词法所有者及值绑定保留在[独立设置静态审计](../../../.analysis/results/frontend/host-settings-current/semantic-audit.json)。它与上面的两个产品设置页分别挂载、分别取版本，不能用 `/synapse/settings/` 的组件替代其证据。

### 两条启动入口与窗口生命周期

原生托盘菜单检查 `winMap` 中的 `razer-settings`；已存在则 focus，不存在则走宿主 `launchRazerApps`。宿主默认 origin 是 `https://apps.razer.com`，`SETTINGS` 常量为 `/settings`，拼接得到 `/settings/`；`application-host` 参数仅按官方 `getAppHost` 的 localhost／Razer 域名规则覆写。原生启动请求 1280×720、最小 600×500、policy 5。policy 5 先加载本机窗口壳，再让 Tab 加载实际设置 URL。

前端托盘齿轮是另一条真实入口：先隐藏托盘，再调用 `launchSettings("",{minimum_height:768,minimum_width:1000})`。该 SDK 默认宽高为 1000×768，使用当前页面 origin 加 `settingsPath`，参数序列化为 query；已有窗口则 activate。通知和 Widgets 的实际调用方还传入 `section:"notifications"` 并发送 `settingsScrollToSection` 广播。当前独立设置的 32 份自身 JS 中未发现该广播名的精确字面量；此扫描只界定已保留原件，不能证明不存在间接消费者，也不能证明不同部署间已经成功滚动。

页面 bootstrap 先初始化 SDK，再加载模块 8821，语言准备后才呈现根。初始化会登记应用／窗口事件、Escape、语言 storage 监听和定时器，调用 centralize；100 ms 后查询 `getWindowStatus`，在非最大化且 bounds 不是 1280×720 时调用 `setBounds` 请求该尺寸，再显示窗口。另一个 100 ms timer 也请求显示。托盘覆写的最小高度与该尺寸请求能否同时满足，须以实际宿主窗口结果核对，不能仅凭请求参数宣称最终尺寸。

初始化 effect 的返回值只直接调用 `window-list-change` 的 disposer，未为其余匿名事件、keydown、interval 和 timeout 提供完整清理。根类卸载时只断开自己可能创建的头像 MutationObserver。原件中的账户、反馈和联网编排属于来源说明，不进入本地优先产品范围。

### 实际导航与 General 挂载树

| 页签身份 | 实际加载 |
| --- | --- |
| `TEXT_SOFTWARE`，默认选中 | chunks 439＋921，模块 9302，读取安装列表 |
| `TEXT_SYSTRAY_SETTING` | chunk 762，模块 9762，Quick Launcher、General、Widgets |
| `TEXT_GENERAL_SETTING` | chunk 414，模块 3414，语言与 About |

页签选择进入 Redux 的 `selectedTab`，异步 Router 根据当前名字选择组件；导航同时处理 Enter、窗口宽度变化及溢出菜单。Products 不在这份挂载描述中。根传给工具栏的 `currentTabNavigation` 固定为 0、`tabNavigations` 固定为空数组，也没有传入 back／forward 回调，因此两项始终禁用具有当前源码依据。Refresh 则沿工具栏调用 `window.location.reload()`。

General 沿 `BodyWrapper → body-widgets → 两个 widget-col` 挂载：左列 GENERAL 包含语言，右列 ABOUT 包含独立 About；此树没有 Release Notes 子项。语言顺序为 `en/de/es/fr/ja/kr/pt-br/ru/zh-cn/zh-tw`，点击先关闭菜单，派发 `updateLanguage`，再调用 `RazerApp.setLanguage`。组件另注册匿名 `languageChange` 监听，没有对应 `off` 返回。Systray 的双击行为读取 `systrayDblClickAction`，无值时默认 `{type:"showMenu"}`，可选菜单或本机非 website 应用；点击立即保存，没有页面级 Apply。安装、launcher、widget 的最终服务消费者仍需各自完整审计。

### 独立 About 的真实版本 producer

About 的显示数字初值为空。挂载后先 `caches.match(new URL("/systray/systrayv2/manifest.json",location.href),{ignoreSearch:true})`；命中时直接取缓存 JSON，不再 fetch。未命中才 fetch，并仅在 HTTP 200 时解析网络 JSON。正常显示表达式是 `(e.version || "").replace("1.","4.") + "." + e.buildVersion`，这里的替换作用于第一次出现的 `1.`。

本次通过相同获取收据独立保留的托盘 manifest 给出 `version="1.0.14"`、`buildVersion=2609150422`，按上述表达式静态求值得 `4.0.14.2609150422`。这是输入与算法核对，没有运行页面。独立设置自身的部署版本为 `0.0.41.2606221239`；宿主版本和两个产品 Dashboard 的版本同样不能代替这个 About 值源。

非 200 且没有缓存时，原 helper 返回仅带 `buildVersion:0` 的对象，显示表达式得到 `.0`。缓存读取、fetch 或 JSON 解析抛错才进入 catch：同步调用 `RazerApp.getAppData("chroma-app","version")`，再显示 `"4." + 返回值`。该 API 从 `apps` 中按应用名读取字段，没有应用或字段时可能产生 `4.undefined`，原件没有补充校验。About 的 async effect 没有取消或旧代次拒绝，不能把组件卸载视为已终止请求，也不能把这个版本元数据回退扩展为设备读取失败后的默认值回退。

后续对照需分别覆盖缓存命中且不联网、未命中／200、非 200、缓存／JSON／网络异常、应用字段缺失、切换页签期间延迟返回和关闭清理。外链、版权及社交组件仍按本地范围规则筛选；不能因为整份组件被保留为分析证据就把联网业务重新挂回产品。

## 自启和启动时最小化

两个复选框的组件初值均为 true。挂载后，自启值由 `RazerApp.getAppAutoStart(appName)` 异步返回；最小化值由 `getMinimizedOnStartUp(appName)` 从应用记录读取，只有非 undefined 值才覆盖组件初值。主自启关掉时，子复选框禁用。

点击自启先调用 `setAppAutoStart(appName,value)`，再更新页面；最小化同样先调用 `setMinimizedOnStartUp` 再更新页面。这两个 setter 更新应用项的 `isAutoStart`／`isMinimizedOnStartUp` 和 `lastUpdated`，然后重算整机启动参数。页面没有等待注册表写入确认，也没有通用失败回滚或写后 GET。

自启异步 GET 与编辑之间没有请求代次或修订比较，组件卸载也没有取消该 Promise；若返回晚于点击，原页面的 GET continuation 仍可再次设置 checkbox。两个点击回调不等待 setter：SDK setter 返回本机应用记录或未找到记录时的 undefined，`updateAppsAutoStart()` 的底层写入不作为其返回值。因此页面选中、本机记录更新和 OS 启动项更新是三个阶段，不能把 UI 的立即变化或返回记录当作系统写入确认。

聚合过程遍历本机应用列表：

1. 收集 `isAutoStart===true` 的应用，身份取 `name2 || name`，保留列表顺序。
2. 没有自启应用时调用低层 `setAppAutoStart(false)`。
3. 有自启应用时构造 `apps=<逗号分隔名称>&autoStart=1`。
4. 另收集所有 `isMinimizedOnStartUp===true` 的应用，追加 ` --launch-force-hidden=<名称列表>`。这份列表由最小化字段筛选，并未再与自启列表取交集。
5. Electron 分支去掉 `&autoStart=1` 再调用 `setApplicationAutoStart`；非正式 host 分支还附加 `--application-host`。

当前 Windows 原生实现打开 HKCU 的 `SOFTWARE\Microsoft\Windows\CurrentVersion\Run`，值名是 `RazerAppEngine`。开启时写入 REG_SZ：`"<启动程序>" --url-params=<参数> --autoStart=1`；关闭时删除该值。导出返回 void，写入失败只记录日志，随后关闭注册表句柄。

启动程序路径从 `GetModuleFileNameW` 得到。函数分离父目录与文件名，查找父目录中的 `app-`；找到时保留其前缀并拼接原文件名，从版本目录改指根启动程序。未命中才保留原路径。这一分支由父目录裁剪函数 RVA `0x87110` 和字符串拼接函数 `0x7a230` 的完整伪代码核实。

读取 `getApplicationAutoStartApps` 则从 Run 值提取 `--url-params=` 后、首个空格前的 token，转换为 UTF-8 返回。上层用 `String.includes(appName)` 判断指定应用是否自启；它不是对注册表参数做一套新的结构化解析。原生返回的分配内存由宿主字符串转换／`FreeMalloc` 链释放。宿主启动时另外把 `launch-force-hidden` 按逗号解析，因此自启与隐藏是独立条件。

Synapse 的两个点击回调每次都重新构造 `debounce(Xs,2000)` 并立即调用其返回函数。模块 6079 的默认值为 leading=false、trailing=true，因此每次点击各自启动一个尾沿 timer；回调没有保留同一个 debouncer，连续点击不会在这里合并成一个 2 秒后的 storeData。`Xs` 发送 `{action:"storeData",windowName:"settings-synapse"}`，返回函数和 timer 未被页面保留，卸载未取消。Chroma 当前分支只构造 `debounce(ks,2000)`，没有调用返回值，因此这两个表达式本身不启动 timer，也不触发 `settings-chroma` 的 storeData。两套页面不能归并为相同的延迟保存语义，底层自启 setter 也不依赖该后续表达式才能执行。

## 通知

通知键的实际名字为 `displayNotifications`。通知块读取它后调用与点击相同的 writer：先更新组件 state，再调用 `RazerApp.updateAppData(appName,{displayNotifications:value})`，最后写入 localStorage。因而首次 mount 和同键的外部 storage 事件也会重新请求更新本机应用记录，并非只有用户点击才产生写入；它没有设备 setter 或读回。storage 监听只对相同 key 重读，卸载移除原监听回调。用户点击立即执行本地记录与键写入，没有另一个 Save 按钮；底层记录不存在及部分写入失败的边界见下节。

两套页面的读取表达式不同，但通知键缺失时都会先产生 true，再经 JSON.parse 得到 true。Synapse 判断是否为字符串；Chroma 判断是否为 null／undefined。这里仅描述这一个本机通知键，不能把它套到设备读取。

Synapse 的 localStorage writer 依赖 setItem 的字符串转换，Chroma 显式 `JSON.stringify`；正常布尔值均写为 `true`／`false` 文本。两者均没有包住通知 JSON.parse 或本机写入的 catch，也没有把任意合法 JSON 限定为布尔值。损坏或非布尔存储的处理不能描述成“读取失败后仍默认成功启用”；该源码边界须在本项目解析契约和错误状态中单独处理。

## 公共设置保存、事件与宿主 flush

`updateAppData` 先剔除保留的身份／安装／窗口描述字段，再将其余扩展字段交给 `updateAppDataItem`；`displayNotifications` 属于保留的扩展字段。实际存储是 localStorage 中 `apps` 键的一整份 JSON 数组：按应用 `name` 找到记录，合并变更、更新 `lastUpdated`，再序列化写回。未找到记录时返回 undefined，不新增应用；通知组件忽略该返回值，仍继续写独立的 `displayNotifications` 键。两个键没有共同事务，任一同步写入抛错也没有调用方补偿，不能将两者描述成原子保存成功。

SDK 的全局 storage 消费者只在相应订阅存在时处理 `apps` 事件：比较旧／新数组的具名字段，向 `appsDataChange` 发送当前应用列表和差异；若当前窗口有 appName 且对应字段确实变化，再过滤本应用并发送 `appDataChange`。这些事件表示本机元数据发生变化，没有设备应答、OS 启动项确认或磁盘 flush 完成语义。通知组件还独立观察 `displayNotifications`，收到同键变化后重走自己的 writer；这两层事件消费者分别保留。

Synapse 自启回调的延迟 `storeData` 已追到实际宿主消费者：

| 阶段 | 当前调用及实际结果 |
| --- | --- |
| 设置页面 | `Xs` 调用 `apiElectron.doElectronAction({action:"storeData",windowName:"settings-synapse"})`，没有返回或等待该 Promise |
| preload | `doElectronAction` 通过 `ipcRenderer.invoke("electronAction",request)` 送入宿主 |
| 宿主 dispatch | `storeData` 分支调用 `TabManager.storeData(windowName)` 后 break，没有返回保存结果 |
| 窗口所有者 | `TabManager.storeData` 调用 `TabStore.writeData`；优先按名字找 tab，否则查独立窗口 |
| 存储所有者 | tab 分支直接调用该 webContents 的 `session.flushStorageData()`；独立窗口分支先检查 BrowserWindow 与 webContents 存在且未销毁，再调用同一方法；没有目标时不执行 flush |

这条链刷新目标 Chromium session 的存储，不是另一份设备配置写入，也没有自行把页面数据转成 JSON 文件。`writeData` 与上层方法都没有返回／等待 flush，页面没有收到成功、失败或读回；IPC 已完成只能说明 dispatch 已返回。失败不能被本项目升级为保存成功。

延迟任务只保存窗口名，没有页面实例或连接代次；若原窗口在 timer 到期前关闭、同名窗口随后建立，宿主按调用时的同名目标解析。Synapse 页面的各 timer 未在卸载取消，也没有对应恢复动作；Chroma 自启表达式未调用返回的 debouncer，因而没有这条 timer flush。通知监听的配对清理、自启任务的取消缺口和窗口存储的真实持久化应分别核对。

## 通知开关的实际消费者

通知应用的当前 main 依据 URL 中一次读取的 `notificationName` 选择真实根：值为 restart 时挂载 `SystemRestartRequiredWindow`，其余挂载普通 `Root`。两套入口收据分别保留，实际 JS 字节相同；两套收据的入口、asset-manifest 和 manifest 本轮前后采样均一致，已取得文件、19 项获取失败和 23 项 chunk 枚举未决仍独立记录。

普通通知根在 `notifications` 数组变化的 effect 中调用模块 2864 的 `Ru` 与 `Wy`。前者在通知为空时隐藏窗口，并负责按实际通知 DOM 测量定位；后者仅在通知非空时读取共享 localStorage 的 `displayNotifications`，键为 null 才写入 true，再调用 `setBrowserVisible(value)`。该根的 storage 监听只观察语言键，没有直接观察通知开关；因此切换设置不会在这条源码链中立即重新执行显示判定，也不会删除队列或取消通知 timer。宿主最终严格比较 true／false 才 show／hide，其他类型不能当作已经生效的显示状态。

模块 2864 的宽高回报函数 `aU`／`Y6` 调用同一个 `Wy`，并把其返回值赋回局部队列后写入 `notifications` 键；`Wy` 没有 return，因此这次存储收到 undefined，经当前序列化／setItem 路径成为 `undefined` 文本。该键不能被本项目当作有效队列或通知实际显示的证明；实际渲染来源仍是 Redux 的 `notification.notifications`。这里记录原件完整函数的实际数据流，没有据函数名称补造队列排序或正常持久化行为。

restart 根使用自己的 `resizeWindow`：宽高为 0 时隐藏，其他情况取得屏幕、居中后显示，没有调用 `Wy`。其关闭动画后的 250 ms timer 没有在 effect 中取消，ResizeObserver 则有 disconnect，语言监听也有同身份移除。普通根在 mount 注册 window-storage 观察并启动初始化；卸载只调用宿主 unregister，没有对 `J` 注册到共享 EventEmitter 的 callback 执行 off，也没有遍历清除通知 timer 表。初始化异步结果没有页面代次判定。逐条新增／更新／移除会按 id＋type 清理对应 timer，这与整页卸载是不同路径。实际通知条目另在 mount／load 和 ResizeObserver 中分别调用 `aU`／`Y6`，该观察没有保留用于 disconnect 的句柄。

`rz_native_notification` 又是独立分支：普通根组织原生通知参数后调用 `doNativeNotificationAction`；宿主 `handleNativeNotification(showNotification)` 先查询 dark mode，再调用 `launchRzNotification`。该 JS 分支未读取 `displayNotifications`，与上面的普通窗口显示开关分别建模。实际原生 ABI、系统提交结果、签名及退出链如下；本文没有执行原生通知。

## Windows 原生通知的真实边界

当前包的 `main` 直接指向 `build/Release/rzNotification.node`，没有另一层 JavaScript wrapper。PE 是 AMD64，`node_api_module_get_api_version_v1` 返回 8；`napi_register_module_v1` 经初始化函数注册以下三个 JavaScript 方法。它们是 N-API callback，不能当成同名 C ABI 导出用 `ctypes` 猜签名调用。

| JavaScript 方法 | 当前入参检查与结果 | 原生入口 RVA |
| --- | --- | --- |
| `showToastNotification(dataJson,hmacInfoJson)` | 至少两个参数且前两个均为字符串；否则同步返回 false。两个 UTF-8 JSON 字符串交给原生工作线程，同步等待其布尔结果，不返回 Promise，也不接收 JavaScript callback | `0x3050 → 0x9550 → 0x1e960` |
| `getNotificationSetting()` | 原生工作线程取得三个固定 AUMI 的 OS 设置，序列化后转成 JavaScript 对象；没有 app 参数 | `0x3240 → 0x9500 → 0x20e50` |
| `checkNotificationEnabled(aumi)` | 至少一个字符串，否则 false；空 AUMI、创建 notifier 失败、读取失败或设置值非 0 均为 false | `0x3930 → 0x94b0 → 0x218f0` |

三个方法分别拥有惰性建立的原生队列／线程单例；调用把 packaged task 入队、唤醒并等待 future。工作线程执行 `CoInitializeEx(NULL,COINIT_MULTITHREADED)`，退出时仅在该初始化成功的情况下配对 `CoUninitialize`。它们不是 N-API 异步工作完成回调。错误参数返回 false 这一分支已经确认，但 JSON 解析及字段类型错误会进入 C++ 异常路径，不能据此把全部异常都描述成 false 或保证能安全转为 JavaScript 异常。

通知 JSON 的实际字段来自当前原生完整函数：

| 字段 | 读取／分支语义 |
| --- | --- |
| `id`、`templateType` | id 缺失为空文本；templateType 缺失为 0。构造函数按索引读取固定文本行数 `[1,2,2,3,1,2,2,3]`，没有在入口校验索引边界；不能把任意整数当作合法模板 |
| `scenario`、`expiration`、`duration` | 缺失整数为 0。scenario 的 1／2／3 对应 Alarm／IncomingCall／Reminder，其余 Default；expiration 正值按毫秒加到系统 FILETIME；duration 为 1 时 short，其他非零值 long，0 不写该属性 |
| `text1/2/3` | 缺失为空文本，转 UTF-16 后仅填入模板实际具有的文本槽位；文本通过 DOM text node 写入，不直接拼接未经转义的 XML |
| `iconPath`、`iconName`、`isDarkMode`、`cropHint` | iconPath 优先；其为空且 iconName 非空时，按 isDarkMode 选择本机通知图片。cropHint 为 1 走圆形／appLogoOverride 分支，其他值不能自行推导成新能力 |
| `heroImagePath`、`inlineHeroImage`、`audioPath`、`audioOption` | 独立图片和音频分支。hero 图片受模板与 OS build 条件约束；audioOption 为 0 且路径为空时不修改模板音频 |
| `appName`、`appUserModelID`、`app` | 前两者可显式提供；缺失项仅在 app 命中固定映射时补齐。synapse → Razer Synapse 4／Razer.Synapse.4；chroma-app → Razer Chroma 4／Razer.Chroma.4；virtual-ring-light → Razer Virtual Ring Light／Razer.VirtualRingLight。未命中记录空字段并让初始化失败，没有通用成功默认应用 |
| `shortcutPolicy`、`actions` | shortcutPolicy 缺失为 0，非零才走 ShellLink 校验／创建链。actions 缺失直接通过；每项消费 index／id／name／arg，按 index 放入预先按集合长度分配的 action 槽位，越界不会插入该项。重复或稀疏 index 不能改写成简单遍历追加语义 |

数字 helper 对缺字段返回 0；已存在值仍要走 JSON 类型转换，字符串 helper 对非字符串抛类型异常。当前页面只发送 app、id、scenario、templateType、text1/2/3、iconName、cropHint 和 actions；过期时间、音频、hero、shortcutPolicy 等属于 addon 支持但当前 producer 没有送入的分支，不能宣称页面已接通。

宿主把系统查询结果写到 `payload.darkMode`，原生图片选择却读取 `isDarkMode`。当前页面与宿主均未补后者，因此这一调用链读取到缺失整数 0；不能写成深色查询结果已经控制原生选图。选图顺序为 isDarkMode=false 时 `<name>-black.png → <name>.png`，true 时反向，实际文件必须存在；两者均不存在返回空路径。当前函数使用可执行文件位置与 `CommonDLL\\NotificationImages` 构造路径，属于原件行为，本项目资源访问须归统一安装与缓存所有者。

显式 iconPath／heroImagePath 若为小写 http(s) 前缀，会走 WinHTTP GET、临时文件及句柄释放；本机路径不走下载。当前 helper 没有以完整 HTTP 状态、每次 WriteFile 的返回及写入长度确认下载成功，也没有在本条成功路径证明临时图片最终删除。该联网分支不直接进入本产品范围；授权静态媒体必须使用统一资源服务，不能照搬 addon 的任意 URL 下载和临时文件生命周期。

## Toast XML、系统提交与回调

实际 OS 链为取得 `ToastNotificationManager` activation factory → `CreateToastNotifierWithId(AUMI)` → `GetTemplateContent(templateType)` → 修改 DOM → `ToastNotification` factory 的 `CreateToastNotification(document)` → 设置可选过期时间 → 注册事件 → `ToastNotifier.Show(notification)`。最后一次调用的完整参数已由汇编核对，不能沿 Hex-Rays 省略的显示参数误写成零参数调用。

正文通过 `CreateTextNode`／`AppendChild` 写入。actions 以 `CreateElement("action")` 与 `SetAttribute(content,arguments,activationType)` 构造；需要时创建 actions 节点，设置 ToastGeneric 和 long duration。arg 含 `://` 时 activationType=protocol；不含协议分隔且非空时使用 background＋索引，空 arg 使用 system＋dismiss。进入这些 DOM 分支前，业务 helper 会检查所有非空 arg：逐字节转小写后必须以 `razerappengine` 开头，否则拒绝整条通知；该检查只是前缀检查，不等于宿主的完整 scheme 正则校验。正文点击没有从 payload 另取一个 launch URI，不能将动作按钮的 protocol 参数泛化成整张通知的点击链接。

Windows major ≤6 不执行现代 actions／audio／duration／scenario 分支；圆形图片和 hero 又包含 build ≥14393 的条件。模板索引、对应文本槽位、icon／hero 的选择及现代 OS 条件分别保留，不能因某个 ToastGeneric 用例通过就覆盖全部模板与系统分支。

原生显示函数以新 GUID 的 Data1 作为内部通知身份，保存 Toast 对象与 Activated／Dismissed／Failed 的 event token；wrapper 将返回身份是否非零转成 bool。false 可以来自初始化、DOM、事件注册或 Show 失败；true 只证明该路径产生非零身份并且 Show 返回非失败 HRESULT，没有等待 Activated、Dismissed 或用户实际看到通知。GUID 生成失败与部分注册失败的中途资源状态须独立测试，不能由正常路径的 Release 推断异常清理完整。

事件注册次序是 Activated → Dismissed → Failed。Activated 读取事件 arguments，有内容时按十进制索引进入 activatedWithIndex，否则进入 activated；Dismissed 读取系统 reason，并在相关过期时间已到时调整本地 reason；Failed 进入 failed。它们先调用共同清理函数，将内部通知标为结束、按保存的 token 移除三个 OS 事件并从集合释放。内部 handler 具有 toastChanged 的 activated／activatedWithIndex／dismissed／failed JSON 构造，但只有回调槽非空时才发出；当前三个 N-API 导出没有设置该槽，也没有 JavaScript 订阅导出。不能据这些保留函数声称宿主收到了每次失败或点击回调。

`getNotificationSetting` 逐项读取 Razer.Synapse.4、Razer.Chroma.4 和 Razer.VirtualRingLight 的 `ToastNotifier.Setting`。原生把 manager／notifier／Setting 调用失败映射为该 AUMI 的 5；宿主再映射 0 Enabled、1 DisabledForApplication、2 DisabledForUser、3 DisabledByGroupPolicy、4 DisabledByManifest、5 NotSupported，enabled 仅严格为 0。5 同时包含调用失败，不能从它反推系统确实不支持；宿主整体异常另返回空对象。该查询与本机 `displayNotifications`、页面显隐、Show 的 HRESULT 四者互不替代。

## Worker、签名和点击恢复

Windows producer 每次生成 8 字节随机 keyId 的十六进制文本和 32 字节随机 hmacKey 的十六进制文本，分别 JSON.stringify 通知与密钥信息，再开 Electron Worker 加载 addon 并同步调用显示函数。message 只返回 `{ok}`；worker catch 的 error 文本没有被 message 消费者记录为结构化失败。20 秒 timer 调用 terminate 并 resolve(false)，error／exit 也 resolve(false)；message 清除 timer 并 resolve(ok)。成功消息后没有显式 terminate／等待退出，也没有让页面保存这一任务的所有权。

原生对非空动作 URI 先规范化一处结尾斜杠：无 query／fragment 时移除协议之后的末尾 `/`；有 query／fragment 时移除最早分隔符前一位的 `/`。HMAC 为 BCrypt 的 SHA256＋HMAC 标志，**密钥使用 hmacKey 的十六进制字符串本身的 UTF-8 字节，不是解码成 32 字节**；数据使用规范化 URI 字节，输出小写 hex。随后在 fragment 前用 `?key=<id>&sig=<hex>` 或 `&key=...` 追加签名。BCrypt 中途失败返回空签名，当前函数保留原动作参数，不是拒绝该条的通用分支。

同一通知的动作共用 keyId。宿主只有收到 truthy ok 后才将其放入 pending keys 并尝试持久化；生成并提交 Toast 早于这一保存时点。密钥以 Electron safeStorage 加密写到 `State/pendingNotificationKeys.enc`，加密不可用时跳过写盘，写失败仅记录日志，没有原子临时文件或写后确认。10 天期限在加载及 prune 时过滤；verify 函数自身没有再次比较 createdAt，不能把它描述成每次点击都严格检查期限。

宿主恢复／激活按真实入口分别核对：

| 入口 | 当前行为与确认边界 |
| --- | --- |
| 初次启动 | ready 时先 loadPersistedKeys；在 Windows 找到匹配 URI 后，按 background-manager 已存在／加载中／尚未建立分别立即 fire、等 did-finish-load，或暂存并监听窗口创建。窗口监听在命中及 before-quit 移除 |
| second-instance | 保存本次 argv 并调用 fire；它使用宿主完整 scheme 正则，只处理首个匹配项 |
| fireNotificationURI | 提取 key／sig，移除签名参数并做同一斜杠规范化；URL 解码后查 key，以 Node crypto HMAC-SHA256 和 timingSafeEqual 比较严格小写 64 位 hex。有效后先删除该 key 并持久化，再向未销毁／未崩溃／未 disposed 的 webContents 广播 notification-uri。同通知第二个动作或重复激活会因同 key 已消费被拒绝；广播本身不确认业务动作已执行 |
| pending URI map | 按协议保存最后一条 URI；getPendingNotificationURIs 返回全部值并清空。store／drain 自身不校验 HMAC，也不消费密钥，不能把 drain 结果当作已通过 fire 校验的输入 |
| getNotificationURI／open-url | 前者只从当前 argv 取原始 URI；open-url 另广播 handle-notification-uri。两者不经过上述 fire 验签函数，必须按实际调用者继续追踪 |

当前 background-manager 的实际 HTML 通过 module script 启动 `index-b4c3eb99.js`，另以 deferred classic script 加载共享 manifest；import map 将入口真实使用的 `razer-config` 指向 `/synapse/assets/js/razer-config@latest/index.mjs`。启动时还以文档 URL 为基址登记 `./sw.js`，后者的唯一 `importScripts` 是 `./service-worker-message.js`。这些原件及其字面 ESM／动态 import 已补齐：[通知消费者静态审计](../../../.analysis/results/frontend/background-manager-current/notification-uri-consumers.json)保留 56 份 JS 的完整 AST 核对、515 条依赖边和实际启动根；该字面图没有缺失目标，但不涵盖任意运行时构造的模块 URL。

入口把 otfm 模块的导出 `R` 导入为 `e`；该导出真实绑定到 `Me=new Se`。`Se` 完整类的 constructor 将 preload 的 `response("notification-uri")` 转为这个实例上的 `emit("notification-uri")`。对上述 56 份原件枚举的 367 个事件注册调用中，没有发现同名业务注册；也未发现调用 SDK `getNotificationURI()` 的消费者。初筛中的 39 个动态事件参数及 46 个计算成员调用已逐项保存起点、完整所有者、参数域和当前语法调用者，不只保留计数：8 项事件参数来自 BLE／Monitor／IoT 的设备常量，16 项来自另一个 SDK 的具名事件常量，另有 1 项浏览器可见性事件的四种前缀分支，均核对到源码赋值。其余 14 项属于 EventEmitter 包装、SAX、analytics、RemoteStorage、账户 pubsub 或 Observable；其中 subscribe 的若干参数实际是账户协议／观察者回调，不是事件名字。EventEmitter helper 的两个当前词法调用方均在静态 once 实现内；全部原件的显式 `.once()` 调用也只有该 helper，`_delegateEvent` 没有当前引用。

46 个计算调用分别落入硬件 parser 表、卸载存储清理表、迭代器／生成器、Lodash、XML、Promise、日志、analytics、同步与 worker 缓存钩子等实际域。硬件 parser 的 32 个直接调用均传入具体 parser 身份；Loupedeck 的 URI 形式路由只有一个当前词法调用方，来自 `ws_message` 的数据，不能把它接到通知广播。RemoteStorage 的延迟方法六次入队只使用 get／put／delete；模块迭代的五个当前调用分别使用 `acceptAllLocal/acceptAllRemote/onUserLogout/clearUserData/onUserLogin`。公开通用包装仍接受调用者数据；99 个事件方法非调用引用、可能的别名逃逸、外部调用者与依赖流程的值流不由这些语法清单自动证明。因此审计没有将“当前所核调用域未发现该消费者”升级为所有动态消费者都不存在。另对 707 份含相关词的已保留前端 JS 扩大精确文本搜索，也未找到 pending／open-url 通道的字面调用者；这部分只是有界初筛，不代表这些其他应用已经逐份完成语义审计。

Service Worker 的 58 个 precache URL 已逐项保留原件，带 hash 的 JS 使用 `revision:null`，其中明确包含 `assets/otfm-d56d4f50.js`；这只是 URL 缓存条目，两个 worker 原件没有宏事件注册或 producer 改写。其 precache 优先缓存命中、缺失时可联网，导航读取 precached index；安装文件为 cache-first，安装 manifest 为 network-first，设备目录 JSON 为 cache-first。后两类使用忽略 query 的匹配／键规则，因此抓取到当前网络字节不能证明已有 renderer 一定执行同一字节。启动向 controller 发送 `updateURL(DeviceDataJson)` 与 `checkAssetsVersion`；前者逐 URL 更新，并仅在任一项目成功的响应下触发设备列表重查，不表示全部成功。后者按 `/synapse/assets/index.json` 更新 `cacheAssets`，单条失败继续，通用回复只投递给 `clients.matchAll()` 的第一项；当前入口没有消费该回复的分支。active／waiting worker、实际缓存身份、部分失败及客户端顺序仍未运行验证，这些原版联网缓存行为不进入本项目离线产品路径。

清单另包含 `sw.html`；它只引入共享 `/synapse/assets/js/sw.js`。这份额外 JS 也已保留和解析，但不能因为被 precache 就当作主页面已执行的模块。其 `window.onmessage` 从父窗口接收 `mfetch/start`，`start` 使用消息中的 url／sw 参数登记 worker，未给 sw 时才取 `service-worker.js`；未将猜测出的默认 URL 当成实际启动依赖。该缓存页额外产生的 1 个动态“response”其实是按相关 ID 完成 Promise 的消息总线，两个计算调用分别访问相关 ID 回调表及固定 `mfetch/start` 表；它们也逐项保留，不能与 preload 事件桥混为一谈。

pending drain 的真实入口是 `apiElectron.doNativeNotificationAction({action:"getPendingNotificationURIs"}) → nativeNotification IPC → handleNativeNotification → drainPendingNotificationURIs`，不是普通 `electronAction`。SDK `getNotificationURI()` 则走 `electronAction/getNotificationURI`，只取启动 argv；`open-url` 又使用另一个 `handle-notification-uri` 广播。当前字面依赖图中未发现 pending drain 请求者和上述两种 URI 的最终业务动作；不能将三个入口合并、替它们推定打开页面，或把未经校验的 drain 结果当作验签结果。点击的“OS 启动协议 → 宿主验签与广播”已有静态证据，最终打开页面／执行动作仍未闭合，也没有运行时对照。

原生队列的 atexit 清理设置 stop、通知条件变量、等待线程结束并释放队列；WinToast 单例析构则逐条撤销事件、释放通知与回调并配对其 COM 初始化。当前 N-API 注册未设置 env cleanup hook，atexit 也不能自动等同于 Electron Worker 正常退出／20 秒 terminate 时已完成全部清理。尤其线程归属、回调在途、部分注册失败、成功消息后的 worker 退出和进程级析构须保留为独立验证项。

这条链的最小测试集合须分别覆盖：参数数量／类型和损坏 JSON；8 个模板与文本／XML 特殊字符；三个 AUMI 与各系统设置值／失败；动作缺失、空 arg、外部前缀、稀疏／重复／越界 index；URI 的 query／fragment／斜杠规范化、文本密钥、签名失败、单次消费和密钥恢复；darkMode 字段差异；DOM／Show／部分事件注册失败；超时、退出、取消与全部 token／COM／线程释放。纯逻辑与模拟测试先使用独立原件期望；实际 addon／OS 对照必须通过已核实的显式测试入口，不能猜 N-API ABI、执行厂商 worker 充当普通测试或将显示与点击共同失败视为一致通过。当前文档只完成上述静态链核对，没有新增实际 DLL／OS 对照收据。

## 重置教程

这不是设备恢复，也没有设备写入。Synapse 当前重置动作的本机字段与操作如下；同一次点击按完整函数顺序执行，没有统一设备 Apply 或文件事务。

| 操作 | 当前实际键 |
| --- | --- |
| 标记本轮已经重置 | `disableSynapseSettingReset=true`、`resetTutorials=true` |
| 置 true | `showCalibrationMessage`、`isShowTutorialMacro`、`showWelcomeMessage`、`isShowIntroductionBanner`、`isShowArmoryIntroductionBanner`、`showTHXNews`、`isShowChromaStudioTutorial`、`showOnboardingTutorialCarol`、`isShowSensitivityTutorial`、`showAnalogTutorial`、`tutorialSeen` |
| 删除，让相应页面再次判断首次引导 | `showGamerRoomTutorial`、`showDashboardTutorial`、`showIotTutorial` |
| 修改已有产品文档 | 仅键匹配 `^synapse_\d+$`、JSON 可解析且两个教程字段至少一个严格为 false 的文档；对其中实际具有的 `showAIPromptMasterTutorial`／`showAppProfileAITutorial` 字段置 true，再写回完整文档 |

产品文档扫描逐项捕获 JSON.parse 失败并忽略该条，不给其余产品补字段。页面 mount 先读取 `disableSynapseSettingReset`，再检查固定教程键；初次 effect 没有立即调用产品文档扫描。产品键的 storage 事件才调用扫描并可重新允许重置；非产品键事件仅检查固定键。上述三个“删除”键只要存在布尔值即使为 false，也会重新允许重置，其余被观察的键用 falsy 判定。当前 Synapse effect 注册和移除 storage 监听时分别创建了新箭头函数，移除表达式不是同一个回调身份，不能写成已成功注销。

Chroma 的 mount 只以 `isShowTutorialChromaApp`、`isShowChromaStudioTutorial`、`isShowIntroductionBanner` 是否全部严格等于 true 决定按钮状态，之后仅在这三个键的 storage 新值为字符串 `"false"` 时重新启用。点击将三者与 `isShowArmoryIntroductionBanner`、`showTHXNews`、`showOnboardingTutorialCarol` 一并置 true；后面三个不在该监听的重新启用集合中。它使用同身份监听清理。两套页面的初始判定、被观察键集合、重置集合和卸载行为不能合并。

## 迁移入口和动态灯光

迁移按钮先查询窗口服务客户列表。Synapse 查找 `syn3-profile-migration`，Chroma 查找 `chroma-app-syn3-profile-migration`；已存在则 activate，不存在则打开 `/profile-migration/`，Chroma 附带 `?app=chroma-app`，窗口参数含 `policy=3,tab_visible=1,shouldFocus=1`。按钮本身不读取和转换配置文件。

动态灯光块只在 Windows build ≥22631，或 build 22621 且 UBR ≥2506 时显示。它在本机设置中切换 Chroma／Windows 控制权，并观察 `wdl-devices.isSwitching` 显示加载与禁用状态；系统设置链接调用 `msSettings("personalization-lighting")`。控制权的生产者、设备列表及恢复链见[动态灯光](../shared/dynamic-lighting.md)。

## 语言下拉框与翻译

显示顺序是 English、Deutsch、Español、Français、日本語、한국어、Português (Brasileiro)、Русский、中文（简体）、中文（繁體）。它由固定数组决定，不做语言名称排序。选中下标用 `code === lang.toLowerCase()` 找到；点击取对应 code，先 `updateLanguage(code,translationsImporter)`，再以原始字符串写入语言键。storage 事件同步其他窗口的语言变化。

语言转换函数按区域别名映射到 `en/de/es/fr/ja/kr/pt-BR/ru/zh-CN/zh-TW`。读取语言键不是字符串时才取 `navigator.language` 并转小写。翻译导入器先取默认英文包，再按需要取选中语言包；Promise.all 完成后才派发语言 action。首次字典尚未建立时派发 `unknown`。当前 thunk 没有完整的导入失败 catch，不能描述为加载失败仍成功切换。

单项文案的查找规则则是选中语言字典 → 英文字典 → 文案 key，插值替换 `{{参数名}}`。这是具体翻译函数的规则，不是设备数据的读取规则。

当前 Synapse 下拉组件在打开时计算剩余空间与菜单高度，最多按 180 px 选择向上／向下展开；打开后将已选项滚入视野。点击外部或窗口失焦关闭，卸载移除原 click／blur 监听。没有选中项、下标为 -1 或越界时直接不渲染。该版本的值更新分支包含原拼写 `disabeld`；文档不把它改写成已更新 `disabled`。

## 关于及工具栏附属功能

Synapse 从 dashboard manifest 获取 version 和 buildVersion，失败分支读取本机应用项的 version；Chroma 请求自己的 manifest，并将显示版本第一段改为 4。Chroma 额外读取 `sensa-devices` 中的设备类别及 dualSenseControllerSupport 配置，二者同时成立才显示相关版权行。

版权年份取当前年份。本机工具栏附属功能包含通知、固件状态和迁移提示，其内部行为由对应功能文档说明。

## 页面状态、尺寸与当前 Rust 接线

表中的本机范围不能代替实际挂载树。Synapse 的 `ho` 左列实际为 `Ks/bn/Ia`，右列 `Un/Ta/ia`；`Ia` 是推荐、产品发布与促销偏好，属于排除的联网业务，不在本机页面保留禁用占位。`uo` 的左列 `Te` 同时挂载语言和 Release Notes，不只是下拉框；右列为 `Fs` 关于。Chroma 的 `mi` 左列 `Hs/ye`、右列 `An/Zs/de`，`vi` 左列 `Ae`、右列 `Ps`。Release Notes 的原调用先置 loading，检查 engineVersion ≥4.0.633，经 background-manager 频道请求窗口；收到 settings 频道 `releaseNotePatch/turnOffLoading` 才清 loading，engineVersion 缺失或过旧则立即清除。该原流程不能等同于 Rust 已打开本地缓存弹窗，也不能因它是联网上游就为本地窗口伪造成功。

原设置主体沿 `BodyWrapper → body-widgets → widget-col → widget → setting-block` 布局，列宽 600 px，正文 inset 为 `10px 20px 20px`；当前样式在窄窗将设置列 margin 归零，不能继续叠加通用列的 30 px。widget 标题 18 px，普通说明 14 px／17 px；语言下拉框的 Synapse 专用规则为 188 px，Chroma 挂载的 plain dropdown 为 230 px。通知帮助锚点为 left 226／top 4，提示框 left 242／top 4／width 300 px，包含说明与三个列表项。自启子项树状线、禁用透明度、重置按钮 disabled、灯光 switching 的 spinner／禁用、语言向上／向下展开、菜单 blur／外部关闭及窗口焦点隐藏必须分别保留。CSS 中的旧 side-navigation、device-setting 和 delete 样式没有被当前这两页根直接挂载，不能据保留 selector 额外制造页面或弹窗。

当前代码有三个独立设置表面，应按实际入口审查，而非将任一 `Settings` 类型计为全部已还原：

| 页面／动作 | 当前 Rust、UI 与后端所有者 | 已确认边界与剩余工作 |
| --- | --- | --- |
| Synapse 设置 root | [SettingsPage](../../../crates/razer-settings/src/preferences/settings_page.rs) 持有 Synapse／General／用户要求的本地 Products 三个页签、SelectState、订阅、草稿和保存快照；[product_settings_navigation](../../../crates/razer-shell/src/shell/devices/product_settings_navigation.rs) 将 Synapse 产品设置路由到同一 entity | Products 是本项目的本地浏览入口，不属于厂商的两个原始设置页签；当前 root 的页签切换与关闭／重开不能据共享 host navigation 自动算作原 sessionStorage 历史、刷新和 `showSettings` 链全部完成 |
| 独立主机设置窗口 | [SettingsWindow](../../../crates/razer-settings/src/window/settings_window.rs) 为另一个 `/settings/` 表面，包含 Software、Systray、General 和本地 Products；shell [settings_window](../../../crates/razer-shell/src/shell/windows/settings_window.rs) open-or-focus，普通最小 600×500、托盘齿轮覆写 1000×768，默认 1280×720 | 这个窗口不是 Synapse／Chroma 产品设置的替代物；固定禁用 back／forward 与当前独立原件一致，但 Rust refresh 只切回首项并卸载 tray selector，不等同原页面 reload；安装列表与快捷 launcher 没有真实 host catalog，仍未完整实现 |
| Synapse 自启 | `StartupRequested` 经 shell [startup owner](../../../crates/razer-shell/src/shell/host/startup.rs) 串行提交到 [razer-platform::startup](../../../crates/razer-platform/src/startup.rs)，由 Windows Run key 适配器真实读写本程序注册项；初次读取不产生保存，旧 revision 不能覆盖新编辑，退出排空已接受的写入 | 这是独立 Rust 的本机 OS 链，非调用官方业务 DLL；仅自动启动有 OS 初读，最小化来自本地草稿。setter 成功不增加原件没有的 GET／回滚；当前未有与官方实际函数和真实启动行为的对照收据 |
| Synapse 通知开关 | checkbox 修改 `AppPreferences.notifications`，`SettingsEvent::Changed` 经 shell `save_auxiliary_preferences` 写 workspace 文件 | 当前普通 profile 通知的 [实际消费者](../../../crates/razer-shell/src/shell/windows/profile_switch_notification.rs) 从 `razer_storage::source_local` 读 `displayNotifications`；本次未见设置点击将该 workspace 字段同步至此键的桥接。两个持久化成功不能相互替代，原生 Toast 的独立链也未由该 checkbox 接通 |
| Synapse 教程重置 | `ResetTutorials` 经 shell 更新设备 intro、Gamer Room、Dashboard 教程并保存；按钮本地 state 控制 disabled | 与上文官方固定键集合、产品文档扫描／删除字段及所有产品消费者尚未逐项对应；只重置这几个 entity 不代表完整教程恢复链 |
| Synapse 动态照明 | [settings_lighting](../../../crates/razer-settings/src/preferences/settings_lighting.rs) 呈现二选一、帮助、switching 和系统设置入口；点击发出 `DynamicLightingRequested` | 当前 shell 仅记录“本地选择”，没有提交控制权服务；初值来自 `dynamic_lighting_draft.unwrap_or(false)`，render 固定传 `Some`，尚未使用控件已有的未知状态禁用路径。不得将它显示为已经观测或确认的设备控制权 |
| Chroma 设置 | [ChromaSettings](../../../crates/razer-app-pages/src/chroma/chroma_settings.rs) 保留独立 page/history/focus、语言、说明、迁移、关于和弹窗；[state owner](../../../crates/razer-state/src/chroma_settings.rs) 串行读写，拒绝旧 generation／revision，页面观察该 owner | AutoStart、StartMinimized、Notifications、DynamicLighting 当前通过 `Change` 写 [Chroma 本地存储](../../../crates/razer-storage/src/chroma_settings.rs)；此链没有 Chroma OS 自启 setter 或灯光控制权服务写入。现有 retry／discard／未保存阻止关闭是 Rust 附加行为，尚无上文官方设置页通用确认／失败语义依据；不能将本地持久化等同功能闭合 |
| 语言、关于与 Release Notes | Synapse／Chroma Select 的确认进入共享语言 owner；SelectState 与订阅由各页面保留；版本／词表由本地目录读取，Release Notes 使用本地存储入口 | 独立 About 的托盘 manifest producer、两个产品的 Dashboard producer 与本地目录版本须分别对应；共享 locale 传播、原 dropdown 的失焦／外部关闭／无效选择以及 Chroma DualSense 条件仍需核对；本地文件／弹窗打开不覆盖原频道 loading 和关闭链 |
| 迁移、原生通知与清理 | 迁移进入独立 owning 页面；原生通知的实际 source→worker→addon→OS→URI 广播链见前文 | 迁移全流程、通知最终业务消费者、线程／事件 token 与窗口取消清理分别验收。没有实际 DLL 或真实设备对照时继续标未验证，不因入口存在、两边无设备或文档补全而升级 |

现有 `preferences/settings_tests.rs` 含自启观察与命令、控件尺寸、语言及持久化相关 UI 测试，Chroma 的状态／存储模块另有纯测试。本轮只读取用例与代码，没有执行窗口测试。后续必须独立覆盖读取值与本地值不同、未观测／失败、点击期间延迟读取、文件失败、取消、关闭后重开、语言与焦点、单个控件实际消费者和 OS／服务结果；相关设备／原生功能仍须与当前官方实际 DLL 结果逐项对照。

## 原件位置与边界

所有位置为 UTF-8 半开字节区间，身份对应各 source set 的 acquisition.json；此处不重复维护哈希。

| 所有者 | 原件／位置 |
| --- | --- |
| 独立设置的部署与完整静态收据 | [host-settings acquisition](../../../.analysis/inputs/frontend/host-settings-current/acquisition.json)、[analysis.json](../../../.analysis/results/frontend/host-settings-current/analysis.json)、[semantic-audit.json](../../../.analysis/results/frontend/host-settings-current/semantic-audit.json)；45 个完整词法所有者，12 条选定语义链 |
| 独立设置的原生窗口 producer | [宿主 main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js) 的 `launchRazerApps` `[13415,19476)`、托盘回调 `[25734,25816)`；[getAppHost.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/lib/getAppHost.js) `[0,447)`；[constants.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/constants.js) 的路径 `[0,313)`、最小尺寸 `[2227,2337)` |
| 前端托盘的独立设置消费者 | [554.2573b048.chunk.js](../../../.analysis/inputs/frontend/systray-left-current/raw/apps.razer.com/systray/systrayv2/static/js/554.2573b048.chunk.js) 的 gear `[31606,31705)`、通知 `[35397,35567)`、Widgets `[38672,38836)`；其实际 SDK 714 的 launch、origin、settingsPath 及五项常量逐项保留于审计 |
| 独立设置 bootstrap、根与 Router | [main.eda30dd1.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/main.eda30dd1.js) 的 bootstrap `[74785,75697)`；[97.d87e27ff.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/97.d87e27ff.chunk.js) 的根类 `[190482,193232)`、初始化 `[9706,11878)`、工具栏 `[186275,190034)`、Router `[132884,133128)` |
| 独立 General、语言与 About | [414.717cc452.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/414.717cc452.chunk.js) 的根 `[38575,38880)`、语言 `[11189,12059)`、About `[35640,38520)`、版本 producer `[36187,36762)`；[414 CSS](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/css/414.2e30559c.chunk.css) 的 `.section.about` 与 dropdown 样式单独保留 |
| 独立 About 的实际输入与错误分支 helper | [同次获取的 systray manifest](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/systray/systrayv2/manifest.json)；[714.42204095.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/714.42204095.chunk.js) 的 `getAppData` `[223264,223588)`、本机项查找 `[93587,93691)`、`apps` 读取 `[93329,93403)` |
| Synapse 设置树、导航与生命周期 | [720.1e5d1c8f.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/720.1e5d1c8f.chunk.js)，导航 `[213847,213878)`、页面 `[215189,216046)`、根 `[255096,256325)` |
| Synapse 自启、通知、教程 | 同上，`[80204,81361)`、`[48466,49912)`、`[50171,52516)` |
| Synapse 本机键身份 | [main.31a43758.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/main.31a43758.js)，模块 9937 `[5754,11536)`；通知、语言及教程键由实际导出别名解析 |
| Synapse 语言、关于 | 上述 720 原件，`[25795,28714)`、`[77041,80062)` |
| 翻译完整模块 | 上述 720 原件，模块 1778，`[256431,258548)` |
| 自启聚合／低层 SDK | [714.d742db5c.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/714.d742db5c.chunk.js)，模块 7071 `[84352,88321)`，底层方法 `[333519,334434)`；RazerApp 自启读取 `[219174,219737)`、自启设置 `[219964,220373)`、最小化读取 `[220374,220658)`、最小化设置 `[220659,221098)` |
| 本机应用记录与自启聚合的完整函数 | 同一 714 原件，`updateAppDataItem` `[87388,87572)`、`updateAppsAutoStart` `[87676,88205)`；页面 storeData 回调在上述 720 原件 `[80095,80194)` |
| 应用扩展字段、存储与事件消费者 | 同一 714 原件，`updateAppData` 注册调用 `[217578,218833)`、LocalStorage 完整模块 9790 `[202483,203227)`、`onStorageChange` `[105932,108308)`；模块 7071 的 `getAppsChanges` 比较具名应用字段 |
| Synapse debounce 的实际 timer 所有者 | [661.2b706756.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/661.2b706756.chunk.js)，完整模块 6079 `[16475,17424)`；每次构造各自拥有 timer、cancel 与 flush |
| Chroma 设置树与根 | [75.496bba91.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/75.496bba91.chunk.js)，导航 `[196785,196845)`、页面 `[198155,198729)`、根组件 `[235505,236424)`、Provider `[236534,236732)` |
| Chroma 通知、教程、自启、迁移 | 同上，`[29823,30889)`、`[47363,48422)`、`[76096,76984)`（含绑定）、`[76994,78037)` |
| Chroma 本机键身份 | [main.7c706211.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/main.7c706211.js)，模块 9937 `[5754,11369)`；与 Synapse 的导出别名分别解析 |
| Chroma debounce 的实际 timer 所有者 | [602.5664d830.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/602.5664d830.chunk.js)，完整模块 6079 `[13248,14197)`；75 原件的 storeData callback `[75976,76073)` |
| Chroma 独立 SDK 的应用记录／存储消费者 | [714.9923a812.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/714.9923a812.chunk.js)，模块 7071 `[97849,101818)`、LocalStorage 模块 9790 `[215980,216724)`、`updateAppData` 注册调用 `[231075,232330)`、`onStorageChange` `[119429,121805)`；已独立读取，与 Synapse 对应存储语义一致，页面 timer 分支仍分别判断 |
| Chroma 语言、关于、动态灯光 | 上述 75 原件，`[26061,28296)`、`[72416,75943)`、`[14039,15662)` |
| Synapse 设置样式 | [720.dbc9cca5.chunk.css](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/css/720.dbc9cca5.chunk.css)，设置主体 `[259805,266271)`、树状复选框 `[271270,271826)` |
| Windows 自启原生函数 | [chain-audit.json](../../../.analysis/results/native/chain-audit.json) 的 startup-settings；SysUtilsNative，Image Base `0x180000000`，RVA `0x6e2d0/0x6e3c0/0x6e950/0x6b9d0`；路径 helper `0x87110/0x7a230` |
| 宿主 storeData IPC | [preload.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/preload.js) 的 `doElectronAction` `[4469,4523)`；[main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js) 的 dispatch 分支 `[42758,42807)` |
| 宿主窗口与 session 存储所有者 | [TabManager.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/components/Tab/TabManager.js) 的 `storeData` `[4119,4147)`；[TabStore.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/components/Tab/TabStore.js) 的 `writeData` `[4364,4613)`；四份宿主原件均与当前输入索引逐项核对长度和 SHA-256 |
| 普通通知的开关、测量与局部存储 | [959.f2637d09.chunk.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/959.f2637d09.chunk.js)，`aU`／`Y6` 完整函数 `[942,1426)`、`Wy` `[2221,2419)`；LocalStorage 模块 8531 byte 26453 起 |
| 通知的实际根、观察与清理 | [main.0e07a27f.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/main.0e07a27f.js)，lazy 入口 byte 35736 起；[Root.7b6b2b9a.chunk.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/Root.7b6b2b9a.chunk.js)，普通根 `[21916,23285)`、window-storage 初始化／事件 `[17199,21368)`、restart 窗口复用 `[21394,21915)`；实际通知条目在 byte 6383／6896 调用 `aU`／`Y6` |
| restart 根的独立显示 | [SystemRestartRequiredWindow.16fcd6e0.chunk.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/SystemRestartRequiredWindow.16fcd6e0.chunk.js)，`resizeWindow` `[706,1148)`；同文件完整根保留关闭、观察与语言 effect |
| 通知窗口的最终显示消费者 | 同一宿主 preload 的 `setBrowserVisible` byte 6269 起；[common.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/components/Tab/common.js) 的 `BrowserUtil.setBrowserVisible` `[24814,25388)` |
| 独立原生通知 producer 与结果边界 | [nativeNotificationHandler.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/nativeNotificationHandler.js)，handler `[1157,1806)`、worker producer `[4428,5739)`；原生 addon 静态调用不等于实际 toast 已验证 |
| 通知业务消费者的启动域、订阅与 IPC 边界 | [background-manager acquisition](../../../.analysis/inputs/frontend/background-manager-current/acquisition.json)、[notification-uri-consumers.json](../../../.analysis/results/frontend/background-manager-current/notification-uri-consumers.json)；实际 HTML、import map、主字面图 56 份 JS、515 条边及条件缓存页的额外 1 份 JS；逐项参数／计算调用、138 个完整词法所有者及当前语法调用者均在审计。otfm 的 `Se` 完整类 `[4865,45300)`、实例 `[45306,45315)`；宿主 `nativeNotification` IPC `[60074,60125)`，preload 配对入口 `[4925,4993)`；pending drain 与独立 getter／open-url 不混用 |
| 后台 worker 缓存与更新的实际消费者 | [sw.js](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/sw.js) 路由／precache `[14381,20767)`、OTFM 条目 `[14698,14743)`；[service-worker-message.js](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/service-worker-message.js) actions `[310,4113)`、消息 handler `[4155,5382)`；[背景入口](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/assets/index-b4c3eb99.js) 的 worker 初始化 `[516927,519393)`、更新响应消费者 `[519025,519298)`；完整策略类与全部 precache URL 也保留于通知审计 |
| 原生通知输入与完整静态捕获 | [rzNotification.node](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/node_modules/node-rz-notification/build/Release/rzNotification.node) 与 [capture.json](../../../.analysis/results/native/5bdea80bf1fb8ef93003f56ebd624e07c25942bfa8b7b2105317c16ccf059e5e/capture.json)；Image Base `0x180000000`，IDA／Hex-Rays 9.1，3024 个函数均保留汇编，`0x83d48` 反编译失败。宿主提取、解包与本机安装副本的实际字节一致；捕获收据的 9 项输出长度和 SHA-256 已复核。捕获完整不等于全部语义闭合 |
| N-API、工作线程与释放 | 上述捕获的 RVA `0x3f00/0x3f10/0x3ab0`、`0x3050/0x3240/0x3930`；队列 `0x2640/0x2810/0x2a80/0x2d30/0x2750`，atexit `0x9c950/0x9c960/0x9c970`，WinToast 析构 `0x56c90` |
| 通知输入、图片、签名与系统设置 | 同捕获 RVA `0x1e960/0x26240/0x26420/0x5dd40`、`0x1c410/0x1c980/0x54d90`、`0x1d730/0x1e2e0/0x1cac0/0x1cdf0/0x514a0`、`0x20e50/0x218f0`；本机 URI 验签与密钥保存读取同一 nativeNotificationHandler 完整原件 |
| Toast DOM、提交与事件消费者 | 同捕获 RVA `0x56f00/0x57810/0x59de0/0x540b0/0x5b290/0x54560`；`Show` 实参额外核对 `0x58c63–0x58c70` 汇编；事件 `0x55d00/0x55bd0/0x55b10`，撤销 `0x5d070/0x58fa0/0x5c5e0/0x5c760/0x5c940`；实际 handler `0x5ef00/0x5f4c0/0x5f040/0x5fa90/0x60050` |

上述内容为完整源函数和实际调用方的静态阅读，新增的状态／timer 所有者用项目管理的 JavaScript AST 解析器核对，未执行厂商 JavaScript。Synapse 设置 acquisition 分别保留 20 个 map 获取失败和 26 个 chunk 枚举不可用项；Chroma 设置分别为 19 和 25，这两类缺项不是同一计数，也不能由已恢复工厂数量判定动态依赖已闭合。各设置页面以自己的部署收据为准，不用宿主版本替代页面版本。

独立设置本次 444 个 URL 中 406 项保留了字节，共 15,194,348 字节；入口、两个设置 manifest 及 About 真正读取的托盘 manifest 四项稳定性核对均通过，38 个 source map 返回 404。40 份 JS 的自动 chunk 枚举未提供结果；维护入口另外完整恢复 32 个注册中的 301 个词法工厂，缺失工厂和扫描错误均为零，40 份原 JS 的 AST 解析通过。原 Wakaru validate 仍以 41 项 `unresolved_reference` 返回 1，该失败保留在收据，不能以词法恢复取代验证通过。选定语义审计核对了输入索引中的 413 个原件身份，并未声称 Software、全部 Systray 消费者或任意运行时动态依赖已经审计完毕。

background-manager 的 117 个 URL 中 64 条记录保留了字节，获取收据的部署稳定性通过；53 条 source map 获取失败和 8 项 chunk 枚举不可用仍未解决。通知审计的字面依赖目标全部存在，58 项 precache 输入也均保留；这只证明该搜索域能够完整读取，不能取代 source map、动态值流分析、最终业务消费者或实际 DLL／OS 对照。

没有执行页面、注册表写入、登录启动、官方 DLL 实际功能对照或窗口运行验收。共享工具栏、配置迁移内部和后台窗口管理继续由各自功能文档展开；本文不把设置页入口确认计为这些模块全部完成。
