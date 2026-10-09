# Chroma Settings 当前源码复查与实现（2026-10-07）

本轮补上 `/chroma-app/settings/` 的实际根及两个分组，并修复 Chroma Dashboard Settings 按钮错误进入 Synapse Settings 的路由。页面和本地编辑可用的代码链已接通；真实宿主查询、完整工具栏服务状态和所有教程消费者仍未完成，不能将本轮称为整个 Chroma 应用验收完成。

## 当前证据

- `tools/prepare-chroma-settings.cjs` 通过当前 manifest 范围内的 Acorn AST 和 CSS 静态解析生成 `chroma-settings-current-evidence.json`：33 条 AST 收据、331 条 CSS 规则、10 套语言和 25 张 SVG，另记原生文件哈希。
- 设置根模块为当前 `75.496bba91.chunk.js` 的 1743；文件 SHA-256 `bb16ef68a06348c2cbebb545514026a11caa5cf3564c1dcfd301c66ad2342159`。`Zo` 235505–236424 初始化 `settings-chroma`；`Ei` 选择 `mi/vi`，分别挂载 Chroma 与 General。
- `tools/audit-chroma-settings-live.py` 于 2026-10-07 对 8 张官方设置图和发行说明 HTML、manifest、JS、CSS 共 12 个 URL 重新做 HTTP 字节比较，全部与本地当前文件一致。完整时间、URL、ETag、哈希见 `chroma-settings-live-evidence.json`。仅下载和读取文件，没有执行下载代码。
- 发行说明当前 `main.b0db08bb.js` 的 `Ie` 496034–498383 按 app、OS、beta 生成 `rzr.to/{synapse|chroma}-{win|mac}-{prod|beta}-patch-notes`。此次 Chroma 入口使用 Chroma 产品参数，原 Synapse 调用默认保持 Synapse。
- 没有读取或重建用户禁止的四个历史源码目录。

## 路由与生命周期

当前 Dashboard `23322/Fs` 118948–122730 选择 `{name:"settings-chroma",url:"/chroma-app/settings/"}`，调用 84058 的 `_P`，flags 为 `sameWindow="policy=3"`、`autoFocus="shouldFocus=1"`。`_P/i` 1349402–1349775 复用命名目标，否则 `window.open`；这里没有 `attachToWindow`。

当前 `.ref/host-4.0.827/electron/components/Tab/Tab.js` 的 `setWindowOpenHandler` 4100–5133 持有发起页面的 windowId；普通 policy=3 分支通过 `getWindowById` 得到该窗口，再调用 `sendCreateNewTabAction`。所以原生 `ChromaWindow` 新增独立 Settings 页签，不能改为主 Synapse 窗口的 Settings。

`ChromaWindow` 保持 Dashboard、Studio、Settings、Migration 四种实际页签状态；关闭非当前页签不切换当前内容。关闭 Settings 返回先前仍存在的 Studio/Migration，关闭 Migration 在 Settings 仍存在时返回 Settings。Settings 本地保存失败时显示重试和放弃未保存更改两条路径；放弃恢复内存中的最近成功快照，不伪称修复磁盘失败。

全 Chroma 窗口关闭调用 `remove_window`，但 `AppShell.chroma_host` 保留该 Entity。`AppShell::open_chroma_window` 946 起复用 existing 并调用 `bind_window`，所以这一路不会丢失尚未保存的 Settings 草稿。重新绑定后语言同步使用当前窗口 handle；发行说明弹层销毁，避免复用旧窗口的焦点。语言订阅使用 `cx.subscribe` 和显式当前 handle，未沿用可能指向首次窗口的 `subscribe_in`。

## 实际页面与本地行为

| 当前根 | 已实现的控件/行为 | 仍待接入或复核 |
| --- | --- | --- |
| 1743/Hs 启动 | 两个 Checkbox、子项树线和说明；关闭自动启动仅禁用最小化，不清除其选择 | `getAppAutoStart("chroma-app")`、`getMinimizedOnStartUp` 真实查询尚未连接；当前值明确为本地意图 |
| 1743/ye 通知 | 本地开关、原文与三项帮助说明 | `displayNotifications` 的宿主 `updateAppData` 与实际通知消费者未连接 |
| 1743/An 重置教程 | 按当前源码记录六个本地 flag；重置会使已挂载的 Chroma Dashboard introduction/onboard 再次显示；观察 Dashboard 的 introduction 状态，关闭横幅后允许再次重置 | `isShowChromaStudioTutorial`、Armory 等全部消费者未接齐；六 flag 的本地草稿不能当作六套实际教程都已重置 |
| 1743/Zs 配置迁移 | 创建/激活 `chroma-app-syn3-profile-migration` 命名页签；独立 MigrationPage Entity 和显式 MigrationApp::Chroma | Scanner、worker、文件转换及服务结果仍未读取；不共享 Synapse 的迁移结果 |
| 1743/de 灯光归属 | 使用当前 Windows build/revision 条件显示；两段选择、说明、警告图、44px WDL 图和系统设置链接 | `isDynamicLighting`、`wdl-devices.isSwitching` 等真实观察未连接；本地切换不会改变设备归属 |
| 1743/Ae 语言 | 当前 10 种语言，复用全局 locale owner；与 Synapse 启动偏好分开存储 | 浏览器 storage 事件转换为原生 locale 订阅；没有读取真实宿主语言配置 |
| 1743/Ae 发行说明 | 正确 Chroma 官方链接、明确未读取的内容弹层及现有示例预览；返回页签保持弹层焦点 | 来源 engine≥4.0.633、background-manager `releaseNotePatch`、10秒 timeout/回执未连接；当前没有声称发出宿主请求 |
| 1743/Ps 关于 | 当前生产 logo、版本格式、版权、法律链接及竖线/隐私换行、Insider 和七个社交图标 | 双重 DualSense 条件只接受实际观察，当前无观察所以不显示商标；服务更新/真实安装版本未读取 |
| 1743/Vo 工具栏、ui 导航 | 后退/前进/刷新、标题、48px 两项导航和底边；600px 内容列与当前 CSS tokens | 工具栏账号、在线/更新等服务 widgets 尚未挂载；hover timing/tooltip 和全部窗口尺寸还需逐项静态复核 |

关于版本按照当前 Chroma Dashboard manifest 构造为 `4.0.63.2609150212`，页面明确标识来源版本，不把它当作已安装宿主的查询结果。源码安装主机版本亦未升级。

设置保存至 `store_path().with_file_name("chroma-settings-local.json")`，与 Synapse 本地偏好独立；可选字段区分未编辑与本地意图。普通 `fs::write` 失败可见、可重试；有未保存更改时刷新不会覆盖草稿。这里不是原子写入，也不是 DLL 保存成功。教程 reset 是独立的即时本地操作，不与配置文件写入构成事务。

Migration 当前 `main.512f18b6.js` 的 `ly` 2933060–2933127 读取 `?app=`，`sy` 持有窗口身份，`xD → OD` 将 app 传给工具栏 `currentApp`，主体为共同的 scanner UI。原生显式保留此身份，同时每次创建独立实体。未读取 scanner 的状态继续显示未读取，不自动构造空扫描/成功结果。

## 资源与静态验证

25 张图包括官方原图复制、SVG 元素静态提取和基于当前 Checkbox CSS 的勾选图。原页面内联 SVG 的 Facebook style 给全部七个社交图提供 `.ellipse/.social` 样式；独立资源必须显式带上这些共享规则，不能只提取各自 path。当前生成器还验证被展平的 g 元素仅有 id，不丢失变换或继承样式。

`assets/synapse/chroma-settings-embedded.rs` 已进入 `SynapseAssets::load/list`。`ChromaSettingsColors` 位于统一 `crates/razer-widgets/src/theme.rs`。实际 controls 使用 Base Button/Checkbox/Link；实体订阅随页面持有和释放。

已执行并通过：

- `node tools/prepare-chroma-settings.cjs --check`。
- `python tools/validate-chroma-settings.py`：25 张 SVG 的 XML、哈希、共享 CSS 与全局 load/list 注册。
- 相关修改文件 rustfmt。完整 `cargo check --locked --all-targets` 由父任务统一执行，结果由统一检查记录补充。

没有运行应用、构建、测试、安装器、下载 JavaScript 或 DLL。以上是源码/资源和类型层验证，不构成运行截图或真实服务验证。
