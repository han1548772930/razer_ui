# 2026-10-05 逐页现行源码复核

用户要求：已经实现的界面也要逐页与原代码核对；不以历史说明或改名后的旧符号代替复核。本报告只记录本次直接读取当前源码得出的结论。修复由主任务及对应实施子任务处理，报告中的“已确认”不等于“已修复”。

## 范围与方法

- 唯一 Synapse Dashboard 证据目录：`.ref/applications/synapse/dashboard/`。未读取四个已废弃目录。
- 先定位当前入口模块、lazy route、组件 JSX 和条件，再查看实际挂载类名适用的 CSS；CSS 中未挂载的后代选择器不算有效行为。
- 工具：`node tools/audit-ui-source-review.cjs`，调用维护中的 `webpack-source.cjs`/`css-source.cjs`，只执行本地静态提取程序，不执行下载的 JS。
- 机器收据：[ui-source-review-2026-10-05-evidence.json](ui-source-review-2026-10-05-evidence.json)，含当前 JS/CSS 原文、SHA-256、本地文件 SHA-256。偏移单位为 JS UTF-16 code units，结束位置不包含在内。
- 本次没有运行应用、构建或测试。静态检查不能声明像素截图已经一致。

## 页级检查进度

| 顺序 | 页面/分支 | 状态 |
| --- | --- | --- |
| 1 | Synapse 主导航 Dashboard | 已走完当前顶层组件树与本地挂载对照；下列偏差已提交修复负责人。复杂服务分支另列待深入项目 |
| 2 | Devices & Modules | 已完成主路由、目录、详情展开与行状态静态对照；确认主路由仍是自定义目录而非源分组树 |
| 3 | Gamer Room | 当前19388正文、marketing、空组、分组与tutorial已对照；热点原SVG补取受审批基础设施错误阻塞 |
| 4 | Global Shortcuts | 已核对当前94608主列表、录制、菜单、映射容器与删除；源各映射类型内部还需逐项深入 |
| 5 | 独立 Chroma Dashboard | 已完成导航、介绍横幅、预设效果、分组与device card主树对照；Apps与各模块内部仍待深入 |
| 6 | 产品页及 displayMode、弹层 | 待逐项核对，其他子任务正在实施特定产品分支 |

## 1. Dashboard

### 当前挂载链

`App.72827d47.chunk.js / module 35378 / x @15591` 的导航顺序是 Dashboard、Gamer Room、Devices & Modules、Global Shortcuts。Dashboard 分支加载 chunk 8302、2973、7861，再取 module 22534。当前 Dashboard 正文在 `7861.1b0e99a4.chunk.js`：`Ti → Li(connect) → Pi`，不是本地注释写的历史 chunk 4130。

`Pi.render @136979–145923` 挂载：首次使用提示 `Ai`（条件）、介绍横幅 `ji`（内部按 isBannerOpen 隐藏）、`.dashboard.flex[.reflow]`、`renderGroup()`。分组顺序由 module 1285 的 `a @916` 明确列出 devices、recommendation、module、onlineService、partnerDeals。每组只在 `items.length > 0` 时挂载，`xi → bi(connect)` 负责分组及卡片分发。

### 已确认偏差

| ID / 优先级 | 现行源码证据 | 本地消费者与偏差 | 修复依据 |
| --- | --- | --- | --- |
| D01 / P0 | `Pi.render`、`getIntroductionBannerStatus`；`ji @133178–136928`；7861 CSS `.introduction-banner-container @32987` | `src/shell/main_pages/dashboard_cards.rs::dashboard` 只有设备、模块、在线服务三块，没有介绍横幅、显隐状态、关闭动作及两个导览按钮 | 横幅初始状态来自存储：无值或真时显示；高至少 531、宽至少 1220、最大 2500、顶部 margin 10、圆角 5。标题 RazerF5 42/24，主体 14，两个 100×100 应用标志与中间箭头。用户要求已本地实现入口直开，因此按钮使用导览分支 |
| D02 / P0 | `Pi.updateGroupModuleItems`；module 22431 `AVAILABLE_MODULES → N`；`X @39430–41831` | `dashboard_module_group` 把模块写死为 addWifiDevice、tour；`service_pages.rs::MODULES` 已有 Alexa、Macro、Profiles、Feedback、Armory、ProfileMigration 等本地页，但 Dashboard 缺少相应入口 | 源表的 8 项顺序、name/icon/window 映射有现行 AST 可循；按本地已实现能力提供入口，不能依赖不存在的 installer 回执把它们全部隐藏。Armory 另有 feature flag/maintenance 分支，需明确映射 |
| D03 / P1 | `z.getDeviceStatusText`、`z.render @19894–39109`；`K @18844`；55 CSS `.name @190868`、`.ed-name @191110`、`.status @191955` | `dashboard_device_group` 把设备卡第二行替换成固定“预览设备/本地快照”，丢失 edition 与当前 profile 状态 | 原树 name、editionName、status 是三行；edition 12/14 #707070，status 12/14 #44d62c、宽 230、ellipsis、white-space:pre。状态应精确查找 activeProfile GUID；本地 `active_profile_obj()` 会回退首项，不能直接等同源逻辑。设备无 editionName 时留空，不能用 edition_id 猜名称 |
| D04 / P1 | `z.generateBatteryData`、`renderBattValue`、`render`；55 CSS `.box-item .batt @193159` 起 | 同一设备卡没有任何电池/断电/待机角标，虽然 `Device` 已有 has_battery、power_status | 基础 badge 右上 right/top 10，字 14，图标 26×26 / background 20；源 Dashboard 的电量分档与设备页通用 badge 并不完全相同，不能直接复用并宣称一致。需要依源判定 low-batt、charging、paused、未知/断电、显示数值与图标开关 |
| D05 / P1 | `ee @42209–42712` 只挂 `.box.box-no-device`；55 CSS `.box @187502` 与 `.box-no-device @199770` 没有背景；`.box-item @187758` 才有 #111 | `dashboard_grid.rs` 给所有卡片的共享 style 设 `bg(group_box=#111)`，包括不可拖动的空设备卡，因此空卡背景变黑 | 空设备卡应透出 body #222；290×220、2px dashed #5d5d5d、圆角 5、padding 10/20/9 保留 |
| D06 / P1 | `ee` 标题 `f.kEL → NO_DEVICE_FOUND`（由 module 54693 literal 解析） | `main_pages.rs::dashboard_empty_devices` 写“没有可显示的设备”，未使用源文案 | 使用当前语言包 NO_DEVICE_FOUND；已有两个外链、margin 57/67、间隔 10 与当前树相符 |
| D07 / P1 | 55 CSS 最后的 `.main-container .body-wrapper .dashboard @338625`，`margin-bottom:50px` | `dashboard()` 无底部 margin；shell 共用容器只有 padding-bottom 20 | 原 Dashboard 在 body padding-bottom 外还保留 50px，需在 Dashboard 自身恢复，不能给所有主导航共用增加 50 |
| D08 / P2 | `G @18527`、`V @18616`：zh-cn 选中文，其余选英文，fallback `baseProductName → name.en` | `dashboard_device_group → Device::display_name()` 永远优先中文，切换英文等语言时设备名仍可能显示中文，并可落到人工 `productId N` | 调用当前语言选择，不把所有语言都当中文；模型缺少 baseProductName 时应补可信字段或明确空值边界，不发明产品名 |
| D09 / P2 | 55 CSS `.collapse .icon @184790` 指向 `icon_expand.55a47b0c.svg`（fill #999），hover @185150 白色版本 | `CollapseIcon` 用 `gpui-component::Icon`，未显式色；父级默认 #ccc。依赖实际 registry `gpui-component-0.7.0/src/icon.rs::into_svg` 将 SVG 作为 text_color mask 渲染，源文件自身 #999 不会自动保留 | 正常应 #999、悬停 #fff。须按状态显式着色或使用原色图像；单纯维持相同 path 不能证明图标颜色一致 |
| D10 / P2 | 55 CSS `.dashboard @182467` min-width 620，max-width 1220；`.reflow` 2460；media @186110/186232；`Pi.getMaxColumns` 以实际容器宽计算 | `MainLayout` 提前把容器宽量化为列数×310−20，并在 601–679px 内允许退成单列；源此范围仍有 min-width 620。拖动上界也由源 `.content` 实际宽减 290 决定，本地只到最后整列 | 应分开“容器实际宽”与“卡片布局宽”；保留现行最小宽及断点。<=600 分支需结合 body min-width 和滚动容器继续解析，不能直接把源码的疑似边界行为修成想当然 |

### 已核对相符的基础部分

- 基础卡 290×220；列/行步长 310/240、间隔 20；设备图片 250×140；模块图片 100×100、margin-top 15/bottom 25；设备/模块 padding 10/20/9：与当前 source JSX/CSS 一致。
- 在线服务当前仍为 store、gold、community、support 四项及现有 URL；`de @48033` 挂 `.box-img` 的背景图，不含 `div[data-type=loaded]` 子节点。因此 CSS 中针对该后代的 scale(1.1) 并不在此在线服务树生效，不能仅看到 CSS 就给本地强加放大动画。
- 卡片 hover border #44d62c4d、active #44d62c、2px/5px；拖动短按阈值 10px；归位 300ms 默认 ease：本地基础常量与当前 `xi`、`gi`、`.movableBox` 相符。
- 组收展的 max-height 0/2000、300ms ease-in，translation 300ms linear，图标 -90/0° 300ms linear，以及展开 overflow keyframes 均存在当前 CSS。更复杂的中断/逆转语义仍不能仅凭基础常量宣称完全一致。
- 首次使用提示当前是 `Ai @130775–132464`，100ms 延迟定位第二导航，left−92/indicator left+50，panel top108、marker top70、宽290、padding20、#fd8611 边框、标题16/正文14。当前本地按该几何挂在 Gamer Room 导航下，基础 geometry/文案吻合；没有把独立 Tour 与该 seen flag 合并。

### 已定位、仍需深入的条件分支

这些项是覆盖缺口，不把缺少服务数据误判为必须制造数据展示。

- `Pi.updateGroupRecommendationItems` 与 `updatePartnerDealsGroup` 及后续 `ge` / `Ce`：本地无对应数据模型和条件渲染；source 存在 eligible_products、有效期、feature flag、ignore 状态。
- `xi.getProdItems`：Synapse 2 / Windows 10 / in-development / Xbox / Playstation 特殊设备盒、翻转、Duallink 条件卡及 popup。
- `z`：firmware icon、WDL、mixer 初始化失败、presetLoading、subDevice/count、restart-required、设备 shutdown/standby、tooltip 边界定位；应根据真实字段逐个接入，不能仅因普通设备卡存在就标记完成。
- 组 hover 提升 z-index20 与局部卡片/tooltip层次；本地当前只检查了卡片内部排序。

## 修复交接

2026-10-05 已将 D01–D10 的现行符号、CSS位置和本地消费者同步给 `dashboard_integration`。本审计子任务不修改 Rust，以避免与实施任务冲突。下一页从当前 Devices & Modules 模块 44442 开始。

## 2. Devices & Modules

### 当前挂载链

同一 `35378/x` 的第三导航 lazy 加载 `6505.84205103.chunk.js / module 44442 / ae @35240`。`ae` 汇总数据后只挂以下四个 `H @29116–29851`：

1. `FIRMWARE_UPDATES` → `L @23655–29092`，items 由真实 needsUpgrade 与 firmwareUpdateInfo 去重合并。
2. `NEW_DEVICES` → `w @12624–17620`，来自连接设备中尚无 installedDevices 记录的设备。
3. `AVAILABLE_MODULES` → `w`，源静态目录 `ne @34632–35118` 减 installedModules。
4. `UPDATED_RECENTLY` → `O @20291–23250`，卸载中的设备/模块和已安装条目（已连接设备排前）。

`H` 的第一句是 `if(!items.length)return null`，所以空组不存在。与源公共样式组合后，当前 `6505.9782778c.chunk.css` 的 `.items @194` 为 width1220、margin `0 auto 40px`，每行80、margin-bottom1、padding左20右30、背景#111。

### 已确认偏差

| ID / 优先级 | 现行源码证据 | 本地消费者与偏差 | 修复依据 |
| --- | --- | --- | --- |
| M01 / P0 | `44442/ae → H → L/w/O` 完整树 | `main_pages.rs::modules_page` 自建“设备”组，后接 `ModuleCatalog::render` 的“模块目录”，夹入安装状态说明；已实现的源式状态集中在 `module_preview.rs`，通过独立预览窗而非主导航挂载 | 主页面应按上述源组与状态组件组合；本地能力直开是用户授权的入口策略，不要求伪造 installedDate/firmware 状态。无数据的组保持缺席，具有本地实现的模块入口要能直接访问 |
| M02 / P1 | `w`、`O`、`L` 均调用 `v.U`，即 `96689/c` 的 category/subCategory 图标；模块则用模块logo | `main_pages.rs::modules_page` 以 `dashboard_image(...40,40)` 给设备row放产品实物缩略图 | 源 `.item-icon` 的40×40不是产品Dashboard图缩小；应按当前 `96689/c` 返回的分类SVG匹配 |
| M03 / P1 | `ne` 只有 Alexa、Macro、linkedGames、Feedback、Armory 五项；`ie @35122` 按 isExchangeEnabled 切换 Armory title/icon | 本地同一 `MODULES` 数组把 Tour 和 ProfileMigration 也放入 Devices & Modules；Armory 标题及图标恒为 Workshop | 保留用户可达性，可放在源 Dashboard 的八项模块入口；Devices & Modules 的目录成员必须来自此页自己的源 `ne`，不能把不同页面的注册表互换 |
| M04 / P1 | `te @31248–34628` 含 Macro/Alexa 10语言完整介绍；`ee @30034–31244` 含标题 | `service_pages.rs::MODULES` 的 Macro.description 是 “Independent Macro application window; local editor chrome...” 工程说明，title与其他目录名称也写死中文 | 使用 `te.macro[language]`，例如 zh-cn 是“通过宏模块为你喜爱的游戏引入强大的宏功能。轻松创建一组复杂的按键敲击操作，然后只需轻轻一按，即可准确地执行致胜的按键组合”；同样按 `ee` 和feature条件显示标题 |
| M05 / P1 | 6505 CSS `.item-description-info @4281` 为 #ccc/Roboto/14px/592px；`w` always renders fileSize；`ne.detail.size=0`，`u @12168` 将0格式化为 `< 1 MB` | 本地 module_row 展开描述未设14px，继承全局16px；下方文件大小整项缺席，源码以横排将 learnMore与fileSize布局，本地用v_flex/gap20 | 保留已正确的288×162图、20间距/内边距和min-height202，补14px、原文与静态file size字段。此处 `< 1 MB` 是源静态返回值，无需安装服务 |
| M06 / P1 | `O` 已安装行显示 LAST_UPDATED 本地化日期，REMOVE 链接，disconnected降低name/icon opacity；删除确认是同一row旁的 `A.A`，6505 CSS `.profile-del @3323` right30/top51；Macro另有窄确认内容 `z` | 主路径设备“详情”打开自造 `DeviceDetails` 400px居中快照检查器，其中序列号/快照固件/配置数量及按钮不存在于这个源页面；源更新/删除/详情状态在主路径缺席 | 用户要求源码UI复刻，应把源行内详情/firmware信息及相应确认挂回此页；工具用途的快照检查器不可用来代表源详情或删除弹层 |
| M07 / P2 | 6505 CSS `.item-name @1317` flex `0 0 500px`；`.info-text @1572` #707070；每组margin-bottom40 | 主设备row使用 flex_basis500+flex_shrink1；“本地快照/预览设备”色#999；模块目录额外margin-top30和一段note改变组间距 | 名字固定500、状态按源类别显示并用#707070，组间距40；自造文字影响布局不能当作原版状态 |

源 `w` 支持维护、下载、安装、保存中、取消、重试和离线tooltip；`L` 按SDK/USB/Dongle/Bluetooth条件禁用固件入口并展示warning，展开release版本/日期/说明；`O` 支持卸载进度、可移除条件、Macro依赖设备清单。这些源分支已提取到本报告收据，但 `module_preview.rs` 的每一条细节仍需另行逐条核验。当前结论仅是“主路径未采用这些树”，不把预览中存在状态当成主页面已完成。

## 3. Gamer Room

### 当前挂载链

`35378/x` 第二导航加载 `9388.974b5d43.chunk.js / module 19388 / Oe → He @338496`。He 按 `isBannerVisible` 挂 marketing banner `o @3180–5183`，常驻设备区域 `oe(connect) → je @334915–338167`。je 将设备子项按 isSynapseOverride 分入两组，过滤产品780/3880/3858；每组 `ue(connect) → ze @332118` 常驻 `content/content-inner`，有真实条目时显示 `fe`，无条目时显示 `re`，第一组无设备占位可打开IoT加入窗口。`Pe @330315` 是只在第一组前挂载的两步提示。

### 已确认偏差

| ID / 优先级 | 现行源码证据 | 本地消费者与偏差 | 修复依据 |
| --- | --- | --- | --- |
| G01 / P0 | `je.updateIotGroupDeviceItems/renderIotGroup`；`ze.getBoxItems`；`fe @326111` 与 `S @319598` | `GamerRoomPage` 没有设备集合/在线/开关/override状态，只能显示两个 placeholder；`render` 无条件展示banner | 源已有设备卡、power按钮、online/offline popup、Synapse Override toggle、Settings直达；应接入可信本地IoT数据与产品页，不因空状态已画出就把此页视为完成。banner显隐也要按状态条件建模 |
| G02 / P1 | `ze.render` 常驻 content/content-inner，55 CSS `.dashboard .box-group .content` 等对max-height/translation/rotation做300ms过渡 | `GamerRoomPage::group` 使用 `.when(!collapsed)` 直接卸载正文，箭头也直接切transform，完全没有收展动画 | 可复用已核过的组动画原语，但Gamer Room内容高度是176px，不是Dashboard220px；不要复制错误卡高 |
| G03 / P1 | `ze` 的 `.help` 与内层 `.tip`；`ie @334399` 明确 help position:relative/top0/right0、tip top20 | 本地标题行没有help图标，整个toggle用通用Toolkit tooltip | 显示源help图标并让它独立控制tooltip；标题点击仍只负责展开。源title/icon normal/hover颜色问题同 D09 |
| G04 / P1 | 55 CSS `.gr-display:after @317555` backdrop-filter blur30；`.iot-btn @320315` backdrop-filter blur5 | `product_panel` 只有半透明黑底，hotspot按钮也只有黑底与阴影，均缺背景模糊 | 需要保留源背景模糊行为；若渲染框架受限应明确记录具体受限属性，不把无blur近似标作相符 |
| G05 / P1 | `.gr-marketing__name @318170` 明确#fff；`o.render` Learn More末尾内联svg width21 height20；hover CSS @320075 给path绿色 | `product_info` 名称继承父#ccc；banner Learn More只使用普通文字link，源外链图标完全缺席 | 标题显式#fff、18px RazerF5/Bold；按当前JS内联path提取21×20图标，normal#ccc/hover#44d62c |
| G06 / P1 | `o` 只有span onMouseEnter，浮层容器 onMouseLeave 清空hoveredItem；无点击锁定状态 | 本地 `open_product/active_product` 增加点击后锁定Popover，离开panel后仍可常驻，改变源交互和层次 | 按原hoveredItem状态显示；产品panel自身点击才打开产品URL。键盘可达性可保留，但不能让鼠标点击按钮产生源中不存在的锁定行为 |
| G07 / P2 | `.gr-display @317335` min-width400，`.gr-marketing` 无固定width，仅灯双卡的description min-width284；JS b 的transformX为−23.5/−86.5 | 本地product_info所有卡固定284，panel固定400/652。单卡description也被限制284，且popup基础组件会按视窗重新定位 | 双卡284/36 gap与padding可对应；单卡须按当前实际CSS收缩与min-width而非套双卡宽。需静态核清本地Popup定位约束后再认定边界时的精确偏移 |

Gamer Room banner基本531高、2560断点930×2500、渐变四节点、文案24/16/14、文本gap16、三个hotspot坐标、空卡186×176/padding15/gap8、两步提示尺寸与色值均能从当前源码找到对应。本次也核实 `style:(ie.boxGroup,{zIndex:d})` 的逗号表达式确实丢弃marginTop30等对象，所以本地不应用那30px是正确的。

### 热点动画源缺失及补取结果

当前 JS `19388/l @3070` 与当前 asset-manifest 都指定 `static/media/gamer_room_hotspot_animation.53dd5566.svg`。本地 `assets/synapse/gr-hotspot.svg` 是无animation节点的静态环，现调用强制40×40。当前 `.ref/applications/synapse/dashboard/static/media/` 缺少该原SVG，因此不能把历史资源文档里的关键帧描述当作本次新复核结果。

已使用维护工具 `fetch-application-assets.py --route synapse/dashboard --assets static/media/gamer_room_hotspot_animation.svg` 补取，普通环境返回 network_error。随后 require_escalated 动作被自动审批基础设施503拒绝，消息明确为“automatic approval review could not be completed / not a determination that the action is unsafe”；动作未执行，也未绕过审批。其他静态核对继续完成。此项仍需补得当前原SVG后解析动画时序与固有尺寸。

**后续修复状态（2026-10-05）：** 上述网络结果仍是原尝试的真实记录。主任务后来通过当前manifest的MD4内容指纹及166个当前SVG校准，离线恢复了热点原SVG；这不是独立live下载比对。现已从恢复后的当前文件静态提取完整SMIL并实现36×36热点动画，替换静态40px环，详见 [gamer-room-current-review-2026-10-05.md](gamer-room-current-review-2026-10-05.md) 和 [current-media-offline-recovery.json](current-media-offline-recovery.json)。本节“资源缺失”是修复前的检查状态。

## 4. Global Shortcuts

### 当前挂载链

`35378/x` 第四导航加载 chunk2973/4608，正文为 `4608.e973916f.chunk.js / module 94608 / Ve → Fe @133597–135627`。Fe 挂 `custom-global-shortcuts` widget列和共享映射编辑器 `se(connect) → ie @55661–104922`。列表 `Be(connect) → Oe @122466–133049` 通过 `Te @114312–121787` 渲染每条，输入捕捉框 `Me(connect) → be @106835–114069` 在每个已映射row的右侧。源并不是“先打开表单，再选择输入+输出”这一棵UI树。

### 已确认偏差

| ID / 优先级 | 现行源码证据 | 本地消费者与偏差 | 修复依据 |
| --- | --- | --- | --- |
| S01 / P0 | `Te.render/renderShortcutDetails`；55 CSS `.shortcut_item @212487`、`.shortcut_title @213858`、`.shortcut_content_special @214045` | `features/shortcuts.rs::render` 把组合键和输出做成一个ghost Button标签“组合键 · 动作”，常驻两个“复制/删除”按钮；缺少源分配种类/值/子值结构 | 左侧宽58%，type为10px #6a6a6a大写、value独立行、子值#999带1px绿色左线；右侧是录制框与三点菜单；无映射时还有59×9/128×15骨架块 |
| S02 / P0 | `Fe` 同时挂列表与 `ie`；`ie.customStyleTop` 在global-shortcuts模式返回132px；55 CSS `.custom-global-shortcuts .key-config.right @210301` left `calc(50% + 294px)!important`，并有窄屏覆盖 | `editor()` 是自定义292px侧边表单，含“录制快捷键”、修饰键checkbox、Hypershift、鼠标下拉、操作下拉；通过flex_wrap作为列表兄弟布局。真正的录制交互 `be` 应直接在row右侧，不在该表单内 | 必须复用或按当前ie分支重建源共享映射组件，不能以当前表单近似替代源key-mapping UI。源宏/Chroma已实现能力也应解除错误安装门控，但不据此改变整个编辑树 |
| S03 / P1 | `Te.renderActions` 三点按钮，点击时显示menu；55 CSS三点26×26/background20/margin-left20、menu150×85/right−124/top27、27px每行 | 本地没有三点按钮与菜单，复制/删除常驻并占据row宽度，编辑时整组动作统一disabled | 使用源三点normal/hover/active状态及Edit/Duplicate/Delete菜单；菜单displayActions由clickAway清除，边界<=850另有right0覆盖 |
| S04 / P1 | `.shortcut_item` width600、margin-left−10、padding15px20px、1px透明全边框；hover#ffffff1a、选中border#44d62c | 本地row min-height70、padding8px20px、border-bottom#5d5d5d、gap12；没有源选中外框和整行hover表面 | 去掉无源依据的分隔线并恢复外框/row盒模型；不能因为整体widget600宽就认定内部row相符 |
| S05 / P1 | `Oe.render` 只含 SHORTCUTS_MESSAGE、顶部add、shortcut列表、底部add_box；keys由54693 literal核实 | 本地标题“全局快捷键”、说明“为快捷键分配操作，并在不同应用程序中使用。”写死中文；页尾额外加入保存说明、检查配置、应用到引擎、引擎状态和丢弃全部按钮 | 用源 SHORTCUTS_HEADER/SHORTCUTS_MESSAGE/ADD_SHORTCUT，源中无工程诊断区；应独立安排开发诊断，不把它们算入原版正文 |
| S06 / P1 | `Te.hasShortcutDuplicates/renderWarning` row warning20×20；`.shortcut_item_warning_content @213413` width300/height49/right2/top22；`be` 用keyboard_listen_warning类 | 本地重复输入只在自定义editor中显示文字错误，列表没有警告icon及源tooltip，录制框warning样式也缺失 | 按既有重复检测结果接回源警告位置与状态；源冲突提示不会重排为列表底部错误区 |
| S07 / P2 | 源全局shortcut `.titleRow @210442` 左右各margin10；add_box相对top17/margin-bottom25，并有hover绿色边框、active70%绿色 | 本地panel标题未补这10px，add_box使用真实margin-top17占布局，未见源hover/active边框样式；源add图标normal/hover切两份资源，本地通用asset_button需再次核对实际图标及状态 | 逐项恢复对应用于该节点的级联，尤其relative top不会增加布局高度，不能等同margin-top |

本地删除弹层当前已采用源row left274/top53锚点、300px宽与红色确认外观，基本外框与55 CSS相符；源当前 `Te` 的delete通过 `oe.A`（共享profile-del）呈现，内部实际字体/动作transition仍需继续追共享组件，不据此声明整个删除弹层完全相符。输入编码引擎审计也不能替代本页UI审计。

## 5. 独立 Chroma Dashboard

本节直接读取 `.ref/applications/chroma-app/dashboard/` 当前 manifest 指向的文件。此应用的 manifest 使用 `/chroma-app/dashboard/static/...` 绝对应用路径，基础 `Source` 工具只默认接受 `./static/...`；本次工具在严格核对前缀后为这个实例正规化文件列表，不把“工具找不到模块”误判为源版本已更换，也不依赖旧审计JSON。

当前主导航是 `6883.9a0692ae.chunk.js / 23322 / Ks @123179–130136`，Dashboard lazy加载 `9700.0a3ca923.chunk.js / 62296 / Wn @214771–221386`。Wn先挂intro/priority状态，再挂独立的介绍横幅 `Pn @207251`，最后是 `.chroma-app-dashboard-wrapper`，内部为效果组 `Ss @100364` 和 `12093/B` 的groups。模块导航lazy加载6570/module40554/K，而不是一个空div。

### 已确认偏差

| ID / 优先级 | 现行源码证据 | 本地消费者与偏差 | 修复依据 |
| --- | --- | --- | --- |
| C01 / P0 | `23322/Ks → nt` 主导航；332 CSS `div.nav-tabs @20276` min-height48/border-bottom2 #000；navs font12；`.nav @22923` border-radius14/padding7 10/margin-right20；`.nav.active @23228` 绿底黑字 | `chroma_page.rs::navigation` 为46高、14px、gap36、只有选中绿色文字，没有pill背景，底边#111 | 恢复48px、12/14文字及28px高pill状态，active#44d62c/#111、hover#2d2d2d/#ccc、pressed#3cbf27/#111；当前三个标题文字本身正确，布局和选中态错误 |
| C02 / P0 | `Ss → bs`；332 CSS `.apply-effect @377848` width120/height130/padding20 10 0；effect-icon @379345 为48×48；p @378359 可见12px居中文案 | `presets()` 把每个效果做80×80/icon54，没有可见label，仅tooltip。这个差异无需任何服务数据即可复现 | 恢复120×130、48图标、可见label，外层gap10；Advanced Effects 按当前profile列表非空条件加第8项，Chroma Apps是最后一项。源9项表 `Cs @95813`，本地当前永远只有7个quick+Apps |
| C03 / P1 | `Pn` 双应用CTA和split-arrow；9700 CSS intro @34225 min-width1220/max-width2500，heading1 @34678 bold700，body max-width1020，arrow140/top−30，body first#fff | `introduction()` min-width1000、主标题42但无bold、第一行标签继承#ccc，没有两个Start Tour按钮、没有箭头；横幅还被包在Dashboard max-width1220内 | 源横幅是wrapper之前的兄弟节点，不受卡片wrapper1220限制。按钮按用户授权打开已实现的Synapse/Chroma导览；关闭图标按原24px资源而非host20px glyph |
| C04 / P1 | `Kt` 与 `Ss` 按collapsed保留正文树；332 CSS @386787 起content/translation300ms linear、show-overflow延时；Ss顶部margin20+title17+独立help | `group()` 直接卸载内容、直接切箭头，缺少Ss专属marginTop20/title17/独立help。复核纠正：通用pb20及uppercase与原CSS一致，不列为错误 | 按Chroma自己的级联实现，不能套Synapse旧55的ease-in值：此处effective max-height transition为linear。保留collapsed模型与动态body高度 |
| C05 / P1 | `bs @96603–100360` 配合332 CSS normal/gray/hover动画图、is-sync-active边框、pending spinner与不支持设备tooltip | 本地预设只有固定普通SVG+统一disabled状态，hover只改绿边；没有当前源的同步选中态、等待图、gray资源和源hover动画 | 七种效果值仍以Cs为准；动画应静态解析current animated SVG，不能自行造时间曲线。源unsupported tooltip显示具体原因/设备，而本地仅重复效果名 |
| C06 / P1 | `12093/B` eight groups由Wn.getDashboardGroups按immersive能力筛选，Kt根据type显示设备、第三方、Chroma Connect、Sensa HD、modules、services；ue设备有edition/brightness/battery/effects等 | 本地Dashboard只effect+is_chroma_device组+services，设备brightness下方是简单“百分比 + effect字符串”，edition/battery/effect图标及色块缺席；Modules导航直接返回空div，结构上无法响应任何inventory | 暂无真实inventory不要求凭空显示条目，但必须具备对应源分支，并将已实现模块连接成真实入口。先复刻可从本地状态可靠提供的基础设备字段；服务依赖部分明确列缺口 |
| C07 / P2 | `Wn` validDevices>4时wrapper max-width2460，332 CSS <=1279强制max-width910；intro独立；`.box-item-customize .name @409353` max-height33/min-height17 | 本地wrapper固定max1220、没有reflow宽2460；设备name改成single-line ellipsis，源允许最多约两行 | 恢复分组容器断点、reflow及设备名高度条件；基准卡290×245、padding10 20 20、name-tag高50本地已对上，不应因此掩盖文本行数与动态状态差异 |

Chroma Apps 的当前77778内部、Chroma各模块独立UI以及产品dialog内部本批未逐一审完；不能由本节推导为全部Chroma页面已核完。当前CSS合约与17个Chroma AST收据已追加进同一JSON的 `chroma` 字段。

## 本批交接与验证结果

- 目前覆盖5个主页面/入口树，共列38条偏差或需要继续确认的边界事项（D10、G07等有明确限定，不能当作已完成像素验证）。
- `node tools/audit-ui-source-review.cjs` 最后执行成功：Synapse 60个AST合约、535条CSS；Chroma 17个AST合约、425条CSS；13个本地消费者文件哈希。
- 未修改Rust实现；已向主任务和Dashboard实施负责人提交逐项发现。后续修复后应按这些符号/选择器回查实际消费者，不能仅把报告状态改为完成。
- 本次没有运行应用、构建、测试、下载JS或DLL。唯一未完成的资源补取是上节说明的Gamer Room热点SVG审批503；后续继续从现行产品UI/独立displayMode逐个核对。
