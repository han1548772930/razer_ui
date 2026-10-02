# 设备与模块页的 Razer 应用入口核对

2026-10-02 仅依据 `.ref/applications/synapse/dashboard/` 中当前官方构建核对。本文偏移是文件按 UTF-8 解码后的零基字符位置；下载来源和响应记录以相应目录中的 `source.json`、`.http.json` 为准。

## 结论与修正

[main_pages.rs](../../src/shell/main_pages.rs) 的 `modules_page` 原来在服务状态提示与模块目录之间添加 `surface::external("module-information", "Razer 应用", "https://www.razer.com/software")`。这是本地额外入口，当前官方 Devices & Modules 组件、HomePage 及内容容器均未渲染它，现已删除。

该入口调用 [surface.rs](../../src/ui/surface.rs) 的通用 Component Button：带 outline、外链图标，未指定模块行按钮的尺寸或状态规则。它位于 1220px 宽的纵向容器中，不能用 [service_pages.rs](../../src/shell/service_pages.rs) 的 `ModuleAction` 修正解释它的外观。删除入口同时消除了这处多余按钮；现有模块行操作继续采用源码的自然文字宽度、90px 最小宽度与 27px 高度。

“发现更多 Razer 应用”的官方入口在右上角“更多”所打开的独立 `rz-app-menu` 弹层中，其页脚链接目标是 `https://razer.com/pc/software`，实现与来源见[更多应用规格](../screens/19-app-picker.md)。它与设备与模块页的分组标题、安装按钮是不同元素。

工具栏保存入口另见[当前工具栏复核](toolbar-current-audit.md)：官方没有常驻普通“保存”按钮，但存在条件显示的 `header-unsaved` 菜单及“全部保存/全部丢弃”，不能把两个问题混为一谈。

## 当前官方组件树证据

| 文件与模块 | 位置及行为 |
| --- | --- |
| `static/js/App.72827d47.chunk.js`，35378 | HomePage 的 `render()` 从 18042 开始：兼容模式提示、禁用遮罩、工具栏、主导航、内容容器；19049 处异步载入 `6505` 并绑定 `44442`。页面外层没有额外应用链接或页脚。 |
| 同一 App 文件，23525 | 2483 处的 `main-container` 只包裹传入 children。 |
| 同一 App 文件，73358 | 83924 处的 `body-wrapper` 只包裹内容，并处理滚动、可选背景；没有应用入口。 |
| 同一 App 文件，13545 | 1630 处的 AsyncRouter / AsyncComponent 只选择、加载对应子组件，没有添加应用入口。 |
| `static/js/6505.84205103.chunk.js`，44442 | 模块从 11722 开始；29108 处 `H` 仅渲染分组标题、可选数量及项目行，不渲染 `header-link`。 |
| 同一 6505 文件，44442 | 40740 附近的最终 children 依次是 `H(D58)`、`H(jLf)`、`H(QgG)`、`null`、`H(hmK)`；最后一组传 `noProgressBar`。没有单独 Razer 应用按钮。 |
| `static/js/main.01550b17.js`，54693 | 导出 `D58`、`jLf`、`QgG`、`hmK` 分别对应 `FIRMWARE_UPDATES`、`NEW_DEVICES`、`AVAILABLE_MODULES`、`UPDATED_RECENTLY`。 |

当前 6505 中唯一 `razer.com` 字符串属于 Alexa 说明链接 `/chroma/alexa`，不属于应用发现入口。该文件没有 `/software` 路由。

当前 `static/css/6505.9782778c.chunk.css` 仍定义 `.items .item-header .header-link`，但上述实际组件树没有使用这个 class。单独存在的 CSS 规则不能作为挂载入口的证据。模块行 `.item-action.btn` 与页面中已删除的通用外链按钮也不应混用尺寸规则。

## 校验范围

本次仅静态读取和比对官方 JS/CSS，删除上述五行入口并记录证据；未执行下载的 JavaScript、应用、测试或原生服务。最终 Rust 编译检查由主任务统一进行。
