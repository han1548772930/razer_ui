# 全局设置：Synapse 与 Chroma

设置页管理应用的启动、通知、教程、迁移入口、灯光控制权、语言和版本信息。这里的大部分选项属于本机应用状态；它们不等同于某个设备配置档，也不经过设备设置的统一 Apply 按钮。

## 两套真实页面

| 应用与导航 | 左列 | 右列 |
| --- | --- | --- |
| Synapse → SYNAPSE | 自动启动、通知、推荐 | 重置教程、配置迁移、Windows 动态灯光 |
| Synapse → GENERAL | 语言、发行说明 | 关于 |
| Chroma → CHROMA_APP | 自动启动、通知 | 重置教程、配置迁移、Windows 动态灯光 |
| Chroma → GENERAL | 语言、发行说明 | 关于 |

Synapse 的入口异步加载模块 4914；Chroma 加载 1743。各自的 Provider 包裹对应设置树，外层窗口焦点组件决定 body 是否显示。Synapse 使用 `windowName="synapse"`、`tabName="settings-synapse"`，Chroma 使用 `chroma-app`、`settings-chroma`。隐藏 body 与卸载组件是两件事。

设置根组件挂载时初始化共享频道与存储，读取语言，调用 `RazerApp.setBrowserTabVisible(true)`，监听 `beforeunload` 和 `languageChange`。关闭处理先设置 `showSettings=false`；窗口关闭回调随后 `window.close()`。当前组件卸载只直接移除 `beforeunload`，不能据此声称它注销了所有外部监听。

导航将历史列表和当前位置写入 sessionStorage。点击另一项会把新项追加到历史；工具栏前进／后退按当前位置派发 `updateActiveView` 和 `updateTabNavigation`；刷新调用 `window.location.reload()`。文档中的页面表来自真实渲染调用，而非 CSS 中保留的旧设置类名。

## 自启和启动时最小化

两个复选框的组件初值均为 true。挂载后，自启值由 `RazerApp.getAppAutoStart(appName)` 异步返回；最小化值由 `getMinimizedOnStartUp(appName)` 从应用记录读取，只有非 undefined 值才覆盖组件初值。主自启关掉时，子复选框禁用。

点击自启先调用 `setAppAutoStart(appName,value)`，再更新页面；最小化同样先调用 `setMinimizedOnStartUp` 再更新页面。这两个 setter 更新应用项的 `isAutoStart`／`isMinimizedOnStartUp` 和 `lastUpdated`，然后重算整机启动参数。页面没有等待注册表写入确认，也没有通用失败回滚或写后 GET。

聚合过程遍历本机应用列表：

1. 收集 `isAutoStart===true` 的应用，身份取 `name2 || name`，保留列表顺序。
2. 没有自启应用时调用低层 `setAppAutoStart(false)`。
3. 有自启应用时构造 `apps=<逗号分隔名称>&autoStart=1`。
4. 另收集所有 `isMinimizedOnStartUp===true` 的应用，追加 ` --launch-force-hidden=<名称列表>`。这份列表由最小化字段筛选，并未再与自启列表取交集。
5. Electron 分支去掉 `&autoStart=1` 再调用 `setApplicationAutoStart`；非正式 host 分支还附加 `--application-host`。

当前 Windows 原生实现打开 HKCU 的 `SOFTWARE\Microsoft\Windows\CurrentVersion\Run`，值名是 `RazerAppEngine`。开启时写入 REG_SZ：`"<启动程序>" --url-params=<参数> --autoStart=1`；关闭时删除该值。导出返回 void，写入失败只记录日志，随后关闭注册表句柄。

启动程序路径从 `GetModuleFileNameW` 得到。函数分离父目录与文件名，查找父目录中的 `app-`；找到时保留其前缀并拼接原文件名，从版本目录改指根启动程序。未命中才保留原路径。这一分支由父目录裁剪函数 RVA `0x87110` 和字符串拼接函数 `0x7a230` 的完整伪代码核实。

读取 `getApplicationAutoStartApps` 则从 Run 值提取 `--url-params=` 后、首个空格前的 token，转换为 UTF-8 返回。上层用 `String.includes(appName)` 判断指定应用是否自启；它不是对注册表参数做一套新的结构化解析。原生返回的分配内存由宿主字符串转换／`FreeMalloc` 链释放。宿主启动时另外把 `launch-force-hidden` 按逗号解析，因此自启与隐藏是独立条件。

Synapse 的点击回调调用 debounce 返回函数；Chroma 当前分支只构造 `Us()(ks,2000)` 而未在该表达式中调用返回值，不能把二者写成相同的延迟后续动作。

## 通知

通知块读取本机通知键，更新组件 state，同时调用 `RazerApp.updateAppData(appName,{[key]:value})` 和 localStorage 写入。storage 事件只对相同 key 重读，卸载移除原监听回调。用户点击后立即持久化，没有另一个 Save 按钮。

两套页面的读取表达式不同，但通知键缺失时都会先产生 true，再经 JSON.parse 得到 true。Synapse 判断是否为字符串；Chroma 判断是否为 null／undefined。这里仅描述这一个本机通知键，不能把它套到设备读取。

## 推荐，仅 Synapse 挂载

推荐块管理 `show`、`showNewRelease`、`showNewDeals`、`ignoredList`、`ignoredPid` 和 `ownedPid`。类别依次包含 Accessories、Audio、ChromaHDK、Keyboards、Keypads、Laptops、Mats、Mice、Monitors、Streaming。类别勾选通过 ignoredList 取反表示；总开关关闭时，内部区域通过 disabled 状态阻止交互。

每个操作立即合并 patch 到同一个 localStorage 项。首次缺项创建的对象只有 `show:true`、`showNewRelease:true`、两个空忽略数组；不能凭页面初始 state 为它补出 `showNewDeals` 字段。重置按钮只清 `ignoredPid`／`ownedPid`，不重置类别或总开关。它服务于原版在线推荐，属于本项目范围排除项，但与本地设置的实际区别仍需记录。

## 重置教程

这不是设备恢复。Synapse 重置一组产品／模块教程键，并扫描 `^synapse_\d+$` 的本地文档。仅当文档本来具有 `showAIPromptMasterTutorial` 或 `showAppProfileAITutorial` 字段时才把该字段改为 true；它不凭空给所有产品补字段。

界面是否允许再次重置由教程键及已有产品文档共同决定。当前 Synapse effect 注册和移除 storage 监听时分别创建了新箭头函数，移除表达式不是同一个回调身份。Chroma 的教程集合较小，使用另外的初始判定和同身份监听清理，不能直接复用 Synapse 的语义说明。

## 迁移入口和动态灯光

迁移按钮先查询窗口服务客户列表。Synapse 查找 `syn3-profile-migration`，Chroma 查找 `chroma-app-syn3-profile-migration`；已存在则 activate，不存在则打开 `/profile-migration/`，Chroma 附带 `?app=chroma-app`，窗口参数含 `policy=3,tab_visible=1,shouldFocus=1`。按钮本身不读取和转换配置文件。

动态灯光块只在 Windows build ≥22631，或 build 22621 且 UBR ≥2506 时显示。它在本机设置中切换 Chroma／Windows 控制权，并观察 `wdl-devices.isSwitching` 显示加载与禁用状态；系统设置链接调用 `msSettings("personalization-lighting")`。控制权的生产者、设备列表及恢复链见[动态灯光](../shared/dynamic-lighting.md)。

## 语言下拉框与翻译

显示顺序是 English、Deutsch、Español、Français、日本語、한국어、Português (Brasileiro)、Русский、中文（简体）、中文（繁體）。它由固定数组决定，不做语言名称排序。选中下标用 `code === lang.toLowerCase()` 找到；点击取对应 code，先 `updateLanguage(code,translationsImporter)`，再以原始字符串写入语言键。storage 事件同步其他窗口的语言变化。

语言转换函数按区域别名映射到 `en/de/es/fr/ja/kr/pt-BR/ru/zh-CN/zh-TW`。读取语言键不是字符串时才取 `navigator.language` 并转小写。翻译导入器先取默认英文包，再按需要取选中语言包；Promise.all 完成后才派发语言 action。首次字典尚未建立时派发 `unknown`。当前 thunk 没有完整的导入失败 catch，不能描述为加载失败仍成功切换。

单项文案的查找规则则是选中语言字典 → 英文字典 → 文案 key，插值替换 `{{参数名}}`。这是具体翻译函数的规则，不是设备数据的读取规则。

当前 Synapse 下拉组件在打开时计算剩余空间与菜单高度，最多按 180 px 选择向上／向下展开；打开后将已选项滚入视野。点击外部或窗口失焦关闭，卸载移除原 click／blur 监听。没有选中项、下标为 -1 或越界时直接不渲染。该版本的值更新分支包含原拼写 `disabeld`；文档不把它改写成已更新 `disabled`。

## 发行说明

两套页面均要求 engineVersion 至少为 `4.0.633`，使用分段数字比较。点击先进入 loading，再向 `background-manager` 频道发 `releaseNotePatch`，携带 `open:true` 与对应 app 名；发送后关闭该临时频道。

Synapse 等待 `settings` 频道的 `releaseNotePatch/turnOffLoading` 清除 loading，当前组件没有额外超时。Chroma 监听 `chroma-settings`，并持有一个 10 秒计时器：回执到达、发送异常或卸载时清理；到时仅清页面 loading。这个计时器不证明发行说明窗口已加载完成。

## 关于及工具栏附属功能

Synapse 从 dashboard manifest 获取 version 和 buildVersion，失败分支读取本机应用项的 version；Chroma 请求自己的 manifest，并将显示版本第一段改为 4。Chroma 额外读取 `sensa-devices` 中的设备类别及 dualSenseControllerSupport 配置，二者同时成立才显示相关版权行。

版权年份取当前年份，社交／许可证／隐私链接调用默认浏览器。语言改变后重算社交 tooltip 的偏移。工具栏还挂载通知、固件提示、迁移提示、账户、评分、离线提示等共享组件；账号、在线支持、推荐和发行说明网络内容与本地能力分别标识，不能把它们误当设备配置写入。

## 原件位置与边界

所有位置为 UTF-8 半开字节区间，身份对应各 source set 的 acquisition.json；此处不重复维护哈希。

| 所有者 | 原件／位置 |
| --- | --- |
| Synapse 设置树、导航与生命周期 | [720.1e5d1c8f.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/720.1e5d1c8f.chunk.js)，导航 `[213847,213878)`、页面 `[215189,216046)`、根 `[255096,256325)` |
| Synapse 自启、通知、教程 | 同上，`[80204,81361)`、`[48466,49912)`、`[50171,52516)` |
| Synapse 推荐与存储 | 同上，`[87814,88057)`、`[88269,90987)` |
| Synapse 语言和发行说明、关于 | 同上，`[25795,28714)`、`[77041,80062)` |
| 翻译完整模块 | 同上，模块 1778，`[256431,258548)` |
| 自启聚合／低层 SDK | [714.d742db5c.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/714.d742db5c.chunk.js)，模块 7071 `[84352,88321)`，底层方法 `[333519,334434)`；RazerApp 自启读取 `[219174,219737)`、自启设置 `[219964,220373)`、最小化读取 `[220374,220658)`、最小化设置 `[220659,221098)` |
| Chroma 设置树与根 | [75.496bba91.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/75.496bba91.chunk.js)，导航 `[196785,196845)`、页面 `[198155,198729)`、根组件 `[235505,236424)`、Provider `[236534,236732)` |
| Chroma 通知、教程、自启、迁移 | 同上，`[29823,30889)`、`[47363,48422)`、`[76096,76984)`（含绑定）、`[76994,78037)` |
| Chroma 语言／发行说明、关于、动态灯光 | 同上，`[26061,28296)`、`[72416,75943)`、`[14039,15662)` |
| Synapse 设置样式 | [720.dbc9cca5.chunk.css](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/css/720.dbc9cca5.chunk.css)，设置主体 `[259805,266271)`、树状复选框 `[271270,271826)` |
| Windows 自启原生函数 | [chain-audit.json](../../../.analysis/results/native/chain-audit.json) 的 startup-settings；SysUtilsNative，Image Base `0x180000000`，RVA `0x6e2d0/0x6e3c0/0x6e950/0x6b9d0`；路径 helper `0x87110/0x7a230` |

上述内容为静态源码阅读。没有执行页面、注册表写入、登录启动或窗口运行验收。共享工具栏、配置迁移内部和后台窗口管理继续由各自功能文档展开；本文不把设置页入口确认计为这些模块全部完成。
