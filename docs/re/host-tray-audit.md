# 托盘：宿主源码契约与本实现

2026-10-03。机器可读结果见 [host-tray-audit.json](host-tray-audit.json)，抽取脚本 [tools/audit-host-tray.cjs](../../tools/audit-host-tray.cjs)（`--check` 失败即报错）。

## 左键：`LeftSystray`（Windows 才创建）

出处：`.ref/host-4.0.827/electron/components/Tab/LeftSystray.js`（建窗与事件转发）+ 弹窗应用自己
`.ref/applications/systray/systrayv2/static/js/main.9579c403.js` 的 `c()`（真正的尺寸与位置）。

先修正上一版本文档的一处判断：宿主的 `calculateWindowPosition` 只把 `{x,y,trayX,trayY}` 通过
`align` 事件**发给页面**，真正 `resizeTo`/`moveTo` 的是弹窗应用自己。两边合起来才是完整契约：

| 项 | 源码 | 本地 |
| --- | --- | --- |
| 初建窗口 | `app_name:"systray-left"`, `width:300`, `minimum_width:300`, `height:200`, `minimum_height:200`, `policy:6`, `browser_visible:0`, `always_on_top:!0`, `skipTaskbar:!0`, `useContentSize:!0`, `hasShadow:!1`, `resizable:!1` | `src/shell/tray/windows.rs`：360×200，最小 300×200，无边框透明弹窗、不可缩放 |
| 应用自调整 | `let l=700; let u=360, p=l;` → 有 `.header-2` 时 `u=360`、`p=header-2.clientHeight + .app.list-unstyled.clientHeight`，再 `resizeTo(u,p)` | 面板宽 360；高度=标题行 60 + 应用行 60（本地单应用），被最小高 200 夹住 ⇒ 实际 360×200 |
| 位置 | `h = x - u/2`、`f = y - p`，并按工作区与 `p = min(S/dpiScaleY*0.7, l)` 收缩 | `x = trayX - width/2`、`y = trayY - height`，按 `rcWork` 夹取 |
| 内容区 | `#systrayBody.style.maxHeight = p - 153 + "px"` | 未单独设高度上限（本地内容仅两行），记录在案 |
| 左键 | `tray.on("click")`：窗口不存在则重建；不可见则 `alignWindow()`；200ms 后转发 `{action:"click"}` 并聚焦；**从不因再次点击而隐藏** | 200ms 后显示并聚焦，不再 toggle |
| 双击 | 转发 `{action:"double-click"}` 并聚焦 | 同 |
| 失焦 | `handleBlur` 300ms 后 `hide()` | 300ms 后隐藏 |
| macOS | `LeftSystray can not create due to MacOS`（不创建） | 不创建，走原生菜单 |
| 登录 | 未登录时 `createLoginPage()` 打开 `razer-id` 窗口（`width:800,height:600,policy:6`） | 未实现（账户服务待统一接入） |

## 弹窗面板的样式（`.ref/applications/systray/systrayv2/static/css/554.7cdbd936.chunk.css`）

| 选择器 | 值 | 本地 |
| --- | --- | --- |
| `.systray` | `background-color:#222;border:1px solid #000;width:360px;max-height:700px;min-height:100%;cursor:default` | 面板 `#222`、1px `#000` |
| `.systray>.header-2` | `height:60px;background-color:#222;justify-content:center;padding-bottom:1px;transition:background-color .1s ease-in-out,color .1s ease-in-out`；`:hover{background-color:#000;color:#eee}` | 登录行 60px、`#222`，hover `#000`/`#eee` |
| `.systray>.apps` | `background-color:#111;border-top:1px solid #222;display:flex;flex-wrap:wrap;height:60px;justify-content:space-around` | 启动行 60px、`#111`、上边框 `#222` |
| `.systray>.apps>li` | `height:59px;padding:0 10px;flex:1 1;position:relative`；`:hover{background-color:#000}`、`:hover>.title{color:#eee}` | 同 |
| `.systray>.apps>li>.icon` | `width:32px;height:32px;background-size:100%`；`:active{opacity:.3}` | 图标 32px |
| `.systray>.apps>li>.title` | `color:#999;font-size:12px;display:none`；`.apps.apps-1>li>.title{display:block}`、`.apps.apps-1>li>.icon{margin-right:10px}` | 单应用时显示标题、10px 间距 |

## 右键：`createSystrayMenuRight()` 的原生菜单

出处：`.ref/host-4.0.827/electron/main.js` 的 `zn()`，最后 `rn.setContextMenu(...)` + `rn.setToolTip("Razer")`。

1. Apps 目录里每个已安装应用一项（本地化名 + `icon.ico` 缩放到 20×20）
2. 分隔线（仅当上面有项目）
3. 设置（齿轮图标）→ 打开 `razer-settings` 窗口
4. 登录 / 注销（用户图标，`id:"user-status"`，标签随登录态切换）
5. 分隔线
6. 多于一个应用在运行时：逐个「退出 <应用>」；最后 `exit_all_apps` 或 `exit`
7. `Debug Window`（仅未打包时）

本地 `menu_entries()` 与之对应（Synapse 一项 → 分隔线 → 设置 → 登录 → 分隔线 → 退出），工具提示 `Razer` ✓，菜单项 id 为 `synapse` / `settings` / `login` / `exit`。差异：本进程只承载一个应用，因此没有「逐应用退出」，也无 `Debug Window`；登录态切换需要账户服务，暂固定为「登录」。

`main.js` 里**没有** `tray.on("click")`——点击处理在 `LeftSystray.js` 中，因此不能把左键当成空操作。

## 仍待完成

弹窗内容目前是本地的登录/启动两行，尺寸与位置已按源码校正；弹窗**内部**的样式应继续从 `.ref/applications/systray/systrayv2/` 提取（`static/css/main.1665f0a2.css` 与各 chunk，例如 `.gold-and-silver`、widgets、notifications 等分组），当前尚未按该应用逐条对齐。
