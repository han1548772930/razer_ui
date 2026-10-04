# Armory 窗口（`/synapse/armory/`）依据与实现

> 2026-10-04 更正：以下四页签常显和正文说明是旧实现，已撤销。当前已重新读取模块29770根、20540导航reducer、77989功能hook、60094开关取值及3026/95889介绍横幅。没有功能数据时各能力缺省false；加载结束后根切到Browse，guest=true时只显示Browse与MyDownloads。本地现按这个已稳定下来的默认分支呈现，补内部历史并接入壳工具栏。横幅使用原AVIF/关闭SVG、24px RazerF5标题及14/17 Roboto正文，去掉源码键和技术说明。收据见[armory-default-source.json](armory-default-source.json)，维护工具为[audit-armory-default.cjs](../../tools/audit-armory-default.cjs)。
>
> 仍缺搜索、筛选、已启用能力分支、详情/分享弹层和服务数据。横幅关闭仅保留在当前页面实体；原localStorage持久化未接入。未复现启动时的短暂加载状态，也未运行实窗像素验收，不能将本次默认分支修正认定为完整Armory。

2026-10-03。机器可读依据见 [armory-app-current-audit.json](armory-app-current-audit.json)，抽取脚本 [tools/audit-armory-app.cjs](../../tools/audit-armory-app.cjs)（`--check` 失败即报错）。

## 窗口契约（全部取自当前源码）

| 项 | 值 | 出处 |
| --- | --- | --- |
| 窗口名 | `armory`（共享常量模块里与 `macro` 相邻定义） | 设备包 `static/js/main.*.js` |
| 创建参数 | `policy=5,tab_visible=0,app_name=synapse,width=1280,height=720,minimum_width=600,minimum_height=500,app_icon_path=Synapse\window.ico` | 同上 |
| 打开 URL | `/synapse/armory/?view=my-upload&sharePopup=true&pid=<pid>`、`/synapse/armory/?source=<...>` | 打开方（设备窗口） |
| 查询参数 | `action`、`displayMode`、`guid`、`pid`、`sharePopup`、`source`、`type`、`view` | armory 应用入口 |
| 根分支 | `"armory" === displayMode` → `body-wrapper` 追加 ` custom-scrollable`（macro 用 `customMacro`） | armory 应用 |
| 标题 | `isExchangeEnabled ? DASHBOARD_EXCHANGE : DASHBOARD_WORKSHOP`（zh-CN 均为「互换」） | armory 应用 |

## 导航（源码数组顺序）

```js
navs: [{id: i.ftd, name: i.ftd},   // SPOTLIGHT_HEADER   精选推荐
       {id: i.Hzf, name: i.Hzf},   // BROWSE_HEADER      浏览
       {id: i.Vb_, name: i.Vb_},   // MY_DOWNLOADS_HEADER 我的下载
       {id: i.C9E, name: i.C9E}]   // MY_UPLOADS_HEADER  我的上传
```

显示条件：`navs.filter(e => (!re || e.id !== C9E) && !!(Te || (e.id !== C9E && e.id !== ftd)))`。
`Te = isPhase1FeaturesEnabled`（hook），`re` 是初值 `true` 的本地 state ⇒ **「我的上传」默认隐藏**；未启用 phase-1 时「精选推荐」也隐藏。本地没有这两项运行时状态，因此四项都绘制，并在正文里标注该条件（不假装已登录/已启用）。

## 本地实现

- 页面：[src/shell/armory_page.rs](../../src/shell/armory_page.rs)：顶栏四个导航标签（key 与顺序同源码）+ 每页的诚实说明（Armory 资料分享服务未接入，不显示任何资料数据）。
- 窗口接线：`Location::Armory`（标题「互换」）、`HostTab::Armory`（窗口 id `armory`、图标 `synapse/module-armory.svg`）。
- 模块目录：`armory` 盒从「安装门控」改为**直接打开**本地窗口（`ModulePage::Armory`）。当前 7 行里已有 5 行直接打开：Alexa、宏、Armory、配置文件迁移、介绍导览；`linked-games`、`feedback` 仍显示来源盒/窗口/URL 门控文本。

## 图标

armory 应用引用的两个图标都已定位：`logo_armory_workshop.9cfc256a.svg`（已打包为 `synapse/module-armory.svg`）与 `logo_armory_exchange.svg`（`.ref/applications/synapse/dashboard/shared-apps/`，已按字节打包为 `synapse/logo-armory-exchange.svg`）。

## 仍未接入

Armory 的资料分享服务与内容：浏览（含 300ms 防抖搜索）、精选推荐、我的下载、我的上传、分享弹层（`sharePopup=true`）、账户（Razer ID）与 `armoryProfiles` 数据。这些都属于「服务后端统一接入」的范围；界面不显示任何伪造的资料条目。
