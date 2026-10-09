# 182 Smart Tracking 首次介绍：当前来源与修正

审计日期：2026-10-02。本次沿当前正式站点的真实 render 和 CSS 重新核对首次校准介绍，不沿用覆盖表中“外观仍需细化”的历史结论。

## 当前来源

重新 GET `https://apps.razer.com/synapse/products/182/ui/` 下的 HTML、UI manifest、asset manifest、入口 JS、入口 CSS 和两个关闭 SVG，全部 HTTP 200，且与 `.ref/devices/182/` 逐字节一致。下载内容只用于静态读取，没有执行 JavaScript。

UI manifest 和 HTML 的版本信息一致：`version=1.0.0`、`buildVersion=2607020654`、`gitMetadata.currentBranch=master`、commit `87521bb44f19f46da616dd9caca045672cf6f3be`。manifest 中另有设备驱动 `resourceManifest.version=1.0.1`，它不是此 UI 的版本。

| 文件 | SHA-256 |
| --- | --- |
| `index.html` | `7a5b75c10ce99520f6c83c36fc2a3b6c57511507f22ab49f648029a8b2d4112a` |
| `manifest.json` | `1c5b68c358d6ebc8c6755c72abdf821c5ffbfdc7f457f97f1a2b268582425527` |
| `asset-manifest.json` | `3b10a13f94ab33fd41b0918cedd818ee2871dd9a23a35776cc5b3642f0248424` |
| `static/js/main.db20a7c4.js` | `7626cc9c0a20cf491a481c701829429ac8f23c9c8c5b907736505f70d3b9bcc7` |
| `static/css/main.48c20423.css` | `dec1b71e91cc5e261b9c84bfcdfe501d5c6aa7f16491e103561a5fd69e916a1e` |

请求、响应头、文件哈希和当前源码摘录保存在 [`.work/smart-tracking-current/`](../../.work/smart-tracking-current/)，集中摘录为 [render-and-style-evidence.json](../../.work/smart-tracking-current/render-and-style-evidence.json)。

## 实际渲染路径

当前入口脚本中的链为 `GM.navs[id=4] → mD → PD → pD → LD`。`LD` 的第一个子节点按状态渲染 `DD`，后面直接渲染 Smart Tracking widget。`DD` 的结构为：

```text
div.body-widgets.flex                    (Ls/Ds)
  div.welcome                           (DD)
    div.calibration-welcome.mouse-mat-calibration
      div.close                         onClick=dismissWelcome
      div.title                         MOUSE_MAT_CALIBRATION_HEADER
      div.body-text                     MOUSE_MAT_CALIBRATION
  div.widget                            Smart Tracking
```

它是随页面流动的介绍卡片。当前 `DD` 没有 modal、遮罩、确定/下一步按钮、设备示意图、焦点锁定或 Escape 处理。右上角关闭是唯一操作。此前未编译的 `crates/razer-pages/src/features/calibration.rs` 不属于这条实现链；实际原生实现是 `features/mod.rs → device_pages.rs::DeviceWorkspace::calibration_page`。

## 尺寸、位置及状态

| 部位 | 当前官方 CSS / 行为 | 原生实现核对 |
| --- | --- | --- |
| 页面容器 | `.body-widgets`: 横向 flex、wrap、居中、最大 1240px；`div.flex>div{flex:auto}` | 修正为可换行的 1240px 容器，不再将介绍一起限制在 600px |
| welcome 外层 | `display:flex;justify-content:center;margin-bottom:20px`，作为直接 flex 子项伸缩 | 恢复独立外层，内部卡片按内容宽度居中 |
| 介绍卡片 | 背景和 1px 边框均 `#2d2d2d`；圆角 5px；padding `20px 30px`；relative | 既有参数正确，保留 |
| 标题 | RazerF5、26px、300、行高 30px、下距 10px、`#44d62c`、居中、大写 | 既有参数正确，保留 |
| 正文 | 14px、行高 16px、`#ccc`、居中、下距 0 | 保留并明确绑定正文前景色；英/简中文案均在当前 182 JS 中确认 |
| 关闭命中区 | 36×36px，top/right 为 0；透明底色；背景图居中 20×20px | 既有命中区和资源尺寸正确 |
| 关闭默认/hover/active | 默认灰白 SVG；hover 为绿色 SVG；active 使用绿色并整体 opacity `.7` | 两个原始 SVG 均已复验，既有叠层和透明度保留；背景始终透明，因此 CSS 的 `.2s background-color` 没有可见颜色过渡 |
| Smart Tracking widget | 固定 600px，`flex:0 0 auto`，margin `10px auto` | 增加自身固定宽度和上下 10px margin；介绍之后的间距为 welcome 下距 20px 加 widget 上距 10px |
| 窄窗口 | 这一路没有 `.widget-col` 的 30px 双侧边距；600px widget 加 body 双侧 20px padding | `workspace.rs` 对 Calibration 单独使用 `WIDGET_WIDTH`，避免通用 stacked 分支额外增加 60px 最小宽度 |

关闭资源的本地运行副本与刚获取的官方文件也字节一致：

- `icon_close_white.8ab462b8.svg → assets/synapse/calibration-close.svg`：`0d37e919b0165d5dcff4a6336b3af20fd06b29dbb8dfa0035581f6ae008ce4cd`。
- `icon_close_green.45f61360.svg → assets/synapse/calibration-close-active.svg`：`d9a92ca5978e1d640e76bec9d2b777112eac9238e2aefacb64218c7168f6a03e`。

## 关闭、持久化和焦点

当前 `LD` 从 module 2478 的 storage helper 读取全局 `showCalibrationMessage`（module 9937 导出 `jU0`）。该 helper 对值做 JSON 读写。缺省时写入 `true` 并显示；关闭回调设为 `false` 并立即隐藏。此标志没有拼接 profile 或设备 ID，不随配置文件切换重置。

原生实现使用语义相反的 `tracking_intro_seen`：默认 `false` 表示显示；点击关闭设置 `intro_seen=true` 并发送一次 `WorkspaceEvent::IntroDismissed`。shell 同步所有设备工作区，并通过串行辅助偏好 writer 保存全局标志。这条保存路径采用设备的已保存快照，不提交映射草稿或 Settings 表单。保存失败时继续保持未保存状态，重启后的表现取决于最后成功保存的文件，不能将 UI 已隐藏等同于磁盘写入成功。

首次介绍不主动获取焦点，也不锁定页面。原生关闭按钮保留键盘可操作性；这次补充键盘激活后先移向现有的下一个校准控件，避免焦点停留在将被移除的按钮。鼠标激活仍保持先前焦点，gpui-component 的 Button 本身会在鼠标按下时 `prevent_default`，不会抢焦点。未增加 Escape 关闭或点击外部关闭。

## 校验边界与滚动条

完成了当前来源 HTTP/哈希复核、源码/CSS/资源静态核对及该文件格式化。未启动应用、构建或测试，因此不将静态布局映射宣称为运行截图验证。

本次介绍结构没有新增 z-index、遮罩或跨页面的绝对定位节点。唯一绝对定位元素是卡片内的关闭按钮；正常窗口下 body 保留 20px 侧 padding。滚动条属于 `DeviceWorkspace::render` 的共享滚动容器，此修正不改变其绘制顺序或层级。控制面板覆盖滚动条的问题应在共享 scrollbar 容器及其遮挡规则中另外核实。
