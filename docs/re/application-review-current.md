# 公共应用当前核对范围

本页覆盖[应用目录](application-catalog.json)的24个端点；产品/独立displayMode另见[产品范围](product-review-current.md)和[逐页队列](ui-review-queue-2026-10-07.json)。端点、路由、AST或资源存在都不是完成页数；具体状态以当前契约和[修复登记](ui-fix-registry.json)为准。已登记且未变化的行为只在新问题或相关源码/实现变化时复查。

HTTP状态是原目录的抓取记录，当前收据仅保存本地HTML/manifest哈希，没有重新联网验证。404或没有资源时不从其他应用推定界面。所有可见UI编辑、增删、Apply/Save和本地保存仍在范围内；只读服务/设备观察独立接入，DLL写回后置。

| 当前端点 | 原生入口/当前契约 | 尚需覆盖的范围 |
| --- | --- | --- |
| /alisha/ | Streamer Companion登记入口 | 独立主界面未接，安装入口不能代替应用UI |
| /background-manager/ | 后台基础设施 | 查询/观察包装独立核实，不计可见UI完成 |
| /chroma-app/dashboard/ | [Chroma Dashboard](chroma-dashboard-current.md) | 非空服务组、SDK应用列表/优先级/启停、精确UI |
| /chroma-app/settings/ | [Chroma Settings](chroma-settings-current-audit.md) | 真实host查询、全部教程消费者、工具栏状态及视觉 |
| /cortex/ | Cortex登记；目录为404 | 缺当前页面证据与独立主界面 |
| /feedback/ | FeedbackPage；[访客表单](feedback-app-current-audit.md) | 非访客/真实账户与目录、日志归档、提交回执及结果页 |
| /natalie/ | [Virtual Ring Light原链](ring-light-ui-current.md) | pod/ring/license/setting/version及legacy FFI已部分语义读取；独立界面未接，全部分支/原生正文未完成 |
| /profile-migration/ | MigrationPage；正式/preview构造分开 | 选择、分组、取消、准备、结果逐项审查及真实scanner/转换 |
| /rz-app-menu/ | AppPicker/app_picker_host | 实际安装状态、推荐、模块和错误条件；本地可用入口不可被安装门控冒充阻断 |
| /rz-user-profile-menu/ | account_menu | 真实访客/登录/锁定/退出确认与观察发布者 |
| /settings/ | [独立Settings](settings-current-audit.md) | Software非空安装目录/更新、Systray五槽/Widgets/launch、host同步和输入细节 |
| /sophie-lite/ | 7.1 Surround Sound登记入口 | 独立主界面未接；产品音频页不能代替 |
| /sophie/ | THX Spatial Audio登记入口 | 独立主界面未接；产品THX控件不能代替 |
| /synapse/ | host顶级登记；目录为404 | 正式主界面来源单列/synapse/dashboard/ |
| /synapse/alexa/ | AlexaPage | installer/登录/地区/语言/真实服务及各弹层条件 |
| /synapse/armory/ | [Armory应用](armory-app-current-audit.md)、[产品根](armory-products-current.md) | Browse/Share/贡献表单与各displayMode条件、真实服务 |
| /synapse/chroma-studio/ | StudioSession→ChromaStudio；[当前属性](studio-properties-current.md) | 层/组/拖排、实际设备LED与预览、配置/导入导出/关联及当前属性缺口 |
| /synapse/dashboard/ | [Dashboard](dashboard-ui-current.md)、[Modules](module-service-ui-current.md)、[Gamer Room](gamer-room-ui-current.md)、[Shortcuts](shortcuts-ui-current.md) | 各契约的服务生产者、非空/异常条件及运行验收 |
| /synapse/introduction-tour/ | IntroductionTour的Synapse/Chroma两套steps | 每步媒体/文字/Next/Previous/Close/滚动分别验收 |
| /synapse/macro/ | [Macro](macro-ui-current.md) | 动作/录制/UI独有剩余项，真实屏幕查询、全局快捷键、全产品绑定根 |
| /synapse/profiles/ | [Profiles](profiles-ui-current.md) | Games非空/扫描、完整.synapse4本地应用/文件输出、产品规范化 |
| /synapse/settings/ | [Synapse Settings](settings-current-audit.md) | host启动查询、完整教程/通知/推荐/WDL、版本刷新与回执 |
| /synapse/update-fw/ | FirmwareUpdate；[固件契约](firmware-update-current-audit.md) | 各阶段、阻断/退出、preview与真实观察边界及安装器服务 |
| /systray/systrayv2/ | [Tray](tray-ui-current.md) | 真实账户/通知/Widgets/多应用、动态高度和原生生命周期 |

Pairing产品根另见[MultiPairing](multi-pairing-ui-current.md)与[Receiver](receiver-ui-current.md)。Gamer Room独立模态、Chroma/Host窗口、发行说明及其他displayMode都不能被24端点表替代。

## 需要保留的跨页面边界

Settings自动启动已是独立LocalStartupDraft：未选择为None，编辑初值按源true/true，主开关关掉只禁用最小化并保留其值；已接本地偏好保存，不表示读取或修改host。完整通知/推荐/storage同步、教程集合、WDL归属和release notes真实回执仍缺。用户保留的local connection/runtime页是本项目入口，不伪装同名官方根。

Feedback当前分类为1功能反馈、3客服、4Privacy，标题/邮件/详情限50/255/32000 UTF-16，ECMAScript空白语义；客服邮件必填且未勾日志先确认，Privacy保留隐藏草稿。已实现的表单/确认只是retained实体会话，非文件持久化；真实账户/安装软件/设备目录、日志和服务器成功/失败页未接，没有实际日志收集或发送。

Studio的13个当前属性根均有源证据，但render/共享控件/能力门控和真正设备数据分别核对；不沿用固定“8根未挂载”数量。Macro录制、Profiles本地CRUD及Chroma保存错误处理的实际状态已进入各当前契约，不从旧源码收据推导新的完成结论。

## 保留证据与验证

[public-ui-review-current-evidence.json](public-ui-review-current-evidence.json)保留Settings/Profiles/Dashboard/Studio/Chroma/Feedback作用域、源原文/SHA和24端点本地文件哈希；维护工具为audit-public-ui-review.cjs。其native文件指纹是取证快照，不是当前所有页面已重审的证明。各应用的更细证据由对应current文档链接。

验证仅限静态源码/资源/JSON、格式和`cargo check --locked --all-targets`。测试源码只编译，应用、构建、安装器、下载JavaScript/worker和DLL不运行；视觉、输入和真实读取不得报告为已通过。
