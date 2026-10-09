# Armory 窗口（`/synapse/armory/`）依据与实现

> 2026-10-05 正文续审：3026/77989/54693 重新核对后，未收到 feature 响应时横幅使用 hook 初值 false 对应的 `WORKSHOP_GET_STARTED`，已修正原生硬写 Exchange 的偏差。无响应与有效的全 false feature 响应不同，后者会计算 `isExchangeEnabled=true`。详见[独立应用正文覆盖表](shell-workspace-current.md)。

> 2026-10-04 更正：以下四页签常显和正文说明是旧实现，已撤销。当前已重新读取模块29770根、20540导航reducer、77989功能hook、60094开关取值及3026/95889介绍横幅。没有功能数据时各能力缺省false；加载结束后根切到Browse，guest=true时只显示Browse与MyDownloads。本地现按这个已稳定下来的默认分支呈现，补内部历史并接入壳工具栏。横幅使用原AVIF/关闭SVG、24px RazerF5标题及14/17 Roboto正文，去掉源码键和技术说明。收据见[armory-default-source.json](armory-default-source.json)，维护工具为[audit-armory-default.cjs](../../tools/audit-armory-default.cjs)。
>
> 筛选/排序控件已按源码 `54408`、`49496`、`38198`、`52259` 挂载到 Browse 与 My Downloads 顶栏：Browse 使用 24px filter SVG、230px 多选面板、设备类型/下载状态复选框和“查看所有项目”，My Downloads 使用 26px sort SVG 与六项排序；这些选择只改变本地 UI 状态，不伪造服务过滤或排序结果。搜索入口已按源码 `68142` 接入左侧导航：点击搜索图标展开 200px 输入框，使用原 20px 搜索/清除图标，输入变更保留 300ms 防抖边界；查询仍不会伪造 Armory 服务结果。服务筛选/排序、已启用能力分支、详情/分享弹层和服务数据仍未接入。横幅关闭按 `isShowArmoryIntroductionBanner` 写入本地 APPDATA 偏好。未复现启动时的短暂加载状态，也未运行实窗像素验收，不能将本次默认分支修正认定为完整Armory。

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
`Te = isPhase1FeaturesEnabled`（hook），`re` 是初值 `true` 的本地 state ⇒ **「我的上传」默认隐藏**；未启用 phase-1 时「精选推荐」也隐藏。本地按稳定后的 `phase1=false, guest=true` 默认分支只绘制 Browse 与 My Downloads；字段仍保留条件分支，未假装已登录/已启用。

## 本地实现

- 页面：[crates/razer-app-pages/src/armory_page.rs](../../crates/razer-app-pages/src/armory_page.rs)：源码顺序的导航标签、左侧搜索入口、Browse/My Downloads 的筛选/排序静态控件、介绍横幅和内部历史（Armory 资料分享服务未接入，不显示任何资料数据）。搜索输入只维护本地查询壳与防抖边界，不把本地文本当作远端结果。
- 窗口接线：`Location::Armory`（标题「互换」）、`HostTab::Armory`（窗口 id `armory`、图标 `synapse/module-armory.svg`）。
- 模块目录：`armory` 盒从「安装门控」改为**直接打开**本地窗口（`ModulePage::Armory`）。当前 7 行的具名窗口均已接入本地页面：Alexa、宏、Profiles、Armory、反馈、配置文件迁移、介绍导览。顶部 App Picker 的 Macro、linkedGames、Armory、Feedback 也直接调用具名窗口打开器；当前源码的 `feedback-synapse` 契约见 [反馈应用审计](feedback-app-current-audit.md)。

## 图标

armory 应用引用的两个图标都已定位：`logo_armory_workshop.9cfc256a.svg`（已打包为 `synapse/module-armory.svg`）与 `logo_armory_exchange.svg`（`.ref/applications/synapse/dashboard/shared-apps/`，已按字节打包为 `synapse/logo-armory-exchange.svg`）。

筛选和排序分别静态还原内联模块 `70017`、`8679`，生成 `armory-filter.svg`、`armory-sort.svg`；收据工具比较输出 `d`、`transform`、`viewBox` 和尺寸属性与当前模块，并验证 10 种语言的 17 个控件文案。Browse 多选菜单采用 230px 宽、left 29px、4px 垂直 padding、20px 复选框及 1px 分隔线；排序面板采用 181px 宽、right -6px。下载状态互斥，选择“查看所有项目”清空其余选项，排序按页签分别保存；选项点击后按源码关闭菜单，本地点击菜单外或 Escape 也会关闭。复选框使用源码两段 tick 几何与 100ms/200ms 时序。

## 仍未接入

Armory 的资料分享服务与内容：浏览（含 300ms 防抖搜索的远端建议/查询）、精选推荐、我的下载、我的上传、内容卡片、详情弹层、分享弹层（`sharePopup=true`）、账户（Razer ID）与 `armoryProfiles` 数据。这些都属于「服务后端统一接入」的范围；界面不显示任何伪造的资料条目。

## Current content card and popup receipts (2026-10-04)

The current source renders contribution cards in lazy module `27588`, triggers paged loading through `86024`, provides the detail popup in `13476`, and mounts upload/share flow in `66517`. `tools/audit-armory-default.cjs` records these modules and the `profile-card`, `detail-footer`, `share-new-profile-modal`, `share-new-profile-form`, `profile-act`, and `.reshare` CSS receipts. The local shell keeps the service boundary explicit: it does not display fabricated items or synthesize `sharePopup=true` payloads.
