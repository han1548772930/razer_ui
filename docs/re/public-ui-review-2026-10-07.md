# 已实现公共页面与 Macro 全页复查队列（2026-10-07）

用户本轮要求覆盖此前做过的所有页面与产品；本表是公共应用组的逐页进度，不是全项目完成表。产品 331/1419 的总队列由父任务单独维护。这里的“已回读”均只表示注明的源码/挂载/状态范围实际读过，运行/视觉未验收，设备或服务写回未接；不能以某组共享组件存在替代该页复查。

本组当前修复：Macro 阶段录制及取消生命周期、Profiles 设备关联弹层实时观察/过滤和本地配置增删复制、Settings 自动启动本地草稿、Chroma 本地偏好保存错误与重试。其余项保留独立缺口或待审。精确来源文件、SHA-256 和 AST 范围见 [公共组当前证据](public-ui-review-current-evidence.json) 与 [Macro 录制证据](macro-recording-current-evidence.json)。

## 已完成的实际复查批次

### Macro 录制

详见 [宏录制独立报告](macro-recording-review-2026-10-07.md)。`Ka` 的阶段录制按钮原为已知占位，现在先选阶段再进入共有 recorder/countdown；取消在 stopped/清理之后晚到也不提交成功结果；actor 非正常结束不再永久锁住 UI。临时行、原草稿、Undo/Save、全局快捷键与前台快捷键编辑器的边界分别核对。

### Profiles 设备关联游戏

当前 `/synapse/profiles/` 的 43/`Ua` 位于 `static/js/9449.8f17b519.chunk.js`，SHA-256 `823e7a30acddaa3650be99cf430055fcbd505d39ebfb679685c9878aa7ce70a3`，AST 186302–195556。实际 source 订阅 `gameList/linkedGames/devices`，按 `[P.sortType,P.filterType,c]` 更新；`z` 的 Linked Games 分支查找任一 `isVisible` 关联设备，Profile 下拉改变关联目标而非硬件活动配置。

独立回读 `profiles_page.rs → devices.rs → DeviceGamesDialog → game_tiles/game_tile` 后修复：

- 弹层原来仅在 `new` 采样已知游戏，父页面 `set_devices` 不更新已打开弹层。现在订阅所有传入 workspace，更新设备集合并吸收后续已知关联；原先看过的游戏在解除关联后仍留在本次弹层 All 列表，避免把“没有关联”误作“已卸载”。这依然不是安装程序扫描目录。
- Linked Games 原来与 All 一样直接克隆缓存。现在仅保留在任一当前 workspace 的 Profile 中实际存在的关联；Removed 仍需真实 missing/removed 状态，不能从本地解除关联推断。
- `selected_profile` 现在校验目标仍存在于当前设备配置；`observe_in` 同步外部新增/删除/重命名后的下拉选项和标题，保留仍有效的独立关联目标，否则回退实际活跃/首个可见配置，并取消旧目标的重命名和延迟删除。
- 实际弹层根补 `TestSupportExt` 与标题标签；更新当前 Profiles native 哈希证据。

Profile 菜单 Add/Duplicate/Delete 已补齐本地草稿路径。当前 `Ua` 的 `q/Z/X` 分别发出 `ON_ADD_PROFILE`、`ON_DUPLICATE_PROFILE`、`ON_DELETE_PROFILE`，本地只映射其 UI 意图，不发送服务请求。Add 按主机名生成默认名；Duplicate 保留全部产品设置及 opaque `source_settings`，采用当前 `1867/d9` 的去编号/唯一名称规则，游戏关联保持独立 GUID 域。ID 同时避开工作集合和已保存基线，避免删除再新增复用身份。新增/复制改变弹层关联目标，不激活硬件配置。

Delete 按 `Ua` 等待 100ms，再挂载 `43/ia` 式红色确认层，外部点击/Escape 可取消，只有确认才更改本地集合；至少保留一项，删除活动项才恢复下一项。删除前分别使用既有鼠标 `mapping_dirty()` 和 1382 `AudioEditor.changed` 标准拒绝未提交编辑，避免 `restore_active` 清掉 Playback/EQ 草稿。确认根、确认按钮、错误和本地状态有稳定测试/访问标识。

`ia` 位于同一 9449 chunk，AST 143749–146200；菜单数组 `xa` 185614–185743。命名函数 `1867/AM` 389087–389254、`1867/d9` 389257–389885 位于 `main.776df2b1.js`，SHA-256 `5c765e57ea009fa8f66ce9f973a99cf8a1b2e7c994c99a776a0ad8feeba9a6c2`。本地增加名查找超过源码 AM 的 100 次上限时继续找唯一名称，最少一项保护也属于本地完整性约束；未声称字节级行为相同。

后续 [Import/Export 弹层](profiles-transfer-native.md)已接本地选择和文件预览，但完整导入应用／官方文件输出仍未实现。Games 正常卡片/详情、安装程序浏览及扫描列表仍未实现；确认层的浏览器溢出滚动/额外高度和 window-blur 行为未验收，不能因服务写回后置而从 UI 任务中删除。这里没有调用 DLL 写入或报告已写设备。

### Settings / Synapse 自动启动

当前 `/synapse/settings/` 的 720/`Ks` 位于 `720.1e5d1c8f.chunk.js`，SHA-256 `09885484dcda19f25b9b216d575da5d2cbed2abe5678e6280c0011387be4ce64`，AST 80204–81361；父布局 `ho` 215189–215412、General `uo` 215416–215565。源码初始化两项 true，随后分别查询 host；`minimizedOnStartup` 的禁用条件只有 `!autoStart`，切换 autoStart 不清除 minimized。

原生正式页两个复选框此前恒 false 且禁用，将可实现的 UI 也一并后置。现在 `AppPreferences.startup_draft: Option<LocalStartupDraft>` 保存明确的本地意图，None 表示尚无本地选择，绝不解释成系统已关闭。编辑初值按源 true/true；自动启动恢复可编辑，最小化按主项禁用，事件处理器也重复检查条件；关闭主项保留子项。UI 明确标注“本地设置草稿；系统状态未读取，尚未应用到宿主”。本地保存沿既有 Preferences/SettingsEvent 路径，未增加系统注册表、宿主或 DLL 写操作。

此前 [Settings 审计](settings-current-audit.md) 中“正式控件禁用”由本批替代；真实读取/回执、宿主应用和跨窗口同步仍为独立未完成项。

### Chroma 本地偏好保存

回读 `chroma_page.rs` 的全部六个本地 `Preferences::save` 调用后发现写入错误均被忽略。现在目录创建、序列化和文件写入返回的错误通过统一 `save_preferences` 保留，并在实际根显示 `chroma-preferences-error` 和重试按钮；会话编辑不因失败回退，成功重试清除错误。此项修复是本地文件可靠性补丁，不代表官方服务写回。仍使用普通文件写入，未实现原子替换；加载损坏偏好仍回退默认值，这些边界未升级成完成状态。

### Chroma 设置入口缺口

本次沿当前 `23322/Ks → Bs → Fs.clickSetting` 回读：`currentApp="chroma-app"` 时必须打开 `settings-chroma`、`/chroma-app/settings/`，目标保持 `sameWindow/autoFocus`。源码为 `chroma-app/dashboard/static/js/6883.9a0692ae.chunk.js`，SHA-256 `cb2dccd827d7de81d3b7d774f267ca6e4ec57f057f40e51c8f3ac1e116039567`，`Ks` 123179–130136、`Bs` 122852–123126、`Fs` 118948–122730。

本批初审时 `ChromaPage::toolbar` 发出 `OpenSettings`，`ChromaWindow::bind_window` 却进入主窗口 Synapse Settings。后续已改为 Chroma 窗口内的命名 `settings-chroma` 页签，并实现 Chroma/General 两页及单独迁移身份；具体状态见 [后续记录](ui-review-followup-2026-10-07.md)。宿主状态读取和剩余视觉细节仍不能计为完成。

### Feedback 访客表单与日志确认

已实际重读当前 `4496/ps` 的分类、输入、邮件 blur、提交启用、Privacy、日志复选框/确认和提交处理，共 14 个方法，并沿 `shell.rs` 的 retained `FeedbackPage` 路由读到实际 `Render → form/log_request`。源码 `feedback/static/js/496.003ef6c6.chunk.js` 的 SHA-256 为 `95ac5ba42de8699251bc2dea7c99d993306a7ae7d9ee188a5f29b97f82ce7a5f`，关键范围：`categoryOnClick` 104985–105244、`enableSubmit` 105973–106202、`handleBeforeSubmit` 107536–107753、`renderEmail` 111389–112178。

当前源分类只有 1（功能反馈）、3（客服/问题）、4（Privacy），未复活源码中仍被引用但未提供的 REPORT_BUG 独立分类。原生按访客模式实现：客服邮件必填、普通反馈空邮件可用；UTF-16 标题 50/邮件 255/详情 32000；空白判定保留 ECMAScript 集合；Privacy 隐藏输入但保留草稿；客服未勾日志先确认，两种选择均保留草稿并明确报告服务不可用。局部逻辑回读一致，本次无此文件行为修改。

账号/安装应用/实际设备目录尚无观察接口，非访客分支不可由访客页面替代；草稿目前仅 retained Entity 会话内保存，不是文件持久化。日志归档与真实提交回执、成功/失败服务界面、全部 CSS/焦点的运行验收仍待完成。本次没有收集日志或发送反馈。

## 逐页面矩阵

`已回读/partial` 是所列范围的本次实际复查，未宣称整页完工。`挂载已回读` 仅确认真实渲染树与主要入口，仍需逐控件审查。`待审` 不因历史审计或哈希验证而提升状态。

| 页面键 | 本轮范围/状态 | 当前源与实现 | 剩余差异或下一步 |
| --- | --- | --- | --- |
| `/synapse/macro/#my-macro/recording` | 已回读/partial，已修 | `_r/Nr/Rr`、58190 `xa/Gr/le`；`macro_page/recording*.rs` | monitor/virtual screen 查询；全局录制快捷键；原生事件及视觉实际验收 |
| `/synapse/macro/#my-macro/phased` | 阶段录制已回读/已修；其它操作待继续 | 58190 `Ka/Fr`；`macro_page/phased.rs` | 阶段拖动/展开/选择逐操作复查；来源有能力门控，不凭新增 root 开启 Phased 创建 |
| `/synapse/macro/#my-macro/record-options` | 已回读/partial，已修 | 58190 `Za/vr/Rr`；`record_options.rs/record_shortcut.rs` | 轨迹与系统快捷键真实能力；全部焦点/浮层边界 |
| `/synapse/macro/#my-macro/library` | 挂载、rename/duplicate/delete/save 路径回读 | Macro main `Mn/Bt/an`、25572；`state.rs/tree.rs/unsaved.rs` | 导入导出、保存失败后继续导航、文件夹搜索/拖动逐项复核 |
| `/synapse/macro/#my-macro/keyboard` | 待审 | 58190；`keyboard.rs/keyboard_windows.rs` | 布局能力、按键录入/取消/焦点；不可从录制 decoder 推定单键编辑器已审 |
| `/synapse/macro/#my-macro/mouse-delay-loop` | 待审 | 58190；`editors.rs/row_controls.rs/row_actions.rs` | 每种编辑器值、校验、Add/Delete/Undo/Save |
| `/synapse/macro/#my-macro/text` | 待审 | 58190；`text*.rs` | emoji/文本浮层/提交/取消/选区 |
| `/synapse/macro/#my-macro/launch-command` | 待审 | 58190；`launch.rs/body.rs` | 程序/网页/命令各分支、本地保存与取消 |
| `/synapse/macro/#my-macro/nested` | 待审 | 58190；`nested*.rs` | 宏引用、循环依赖、子菜单几何、失效引用 |
| `/synapse/macro/#key-binds` | 挂载与会话数据边界回读 | 21700 `U/M/I`；`bindings.rs/bindings/` | 本地关联仅会话保存；各设备物理输入/播放配置与删除；不能把会话关联写成设备映射成功 |
| `/synapse/macro/#help` | 挂载回读 | 1519；`body.rs::help` | 支持入口/资源与点击目标逐项验证 |
| `/synapse/dashboard/#dashboard` | 根与组入口挂载回读 | 22534 `Pi`；`main_pages/dashboard_cards.rs` | 卡片 state 证据已由父任务复核更新；服务/推荐条件继续待审，不能把其它负责人修复替代本组逐控件复查 |
| `/synapse/dashboard/#devices-and-modules` | 已定位实际 Entity，待审 | 44442；`module_catalog` 实体及 service rows | 安装/更新/删除/确认与本地打开不能由 Dashboard 卡片代替 |
| `/synapse/dashboard/#gamer-room` | 已定位实际 Entity，待审 | 19388；`gamer_room*.rs` | 房间/设备/热点/教程及只读服务，每项单列 |
| `/synapse/dashboard/#shortcuts` | 已定位实际 Entity，待审 | 94608；`features/shortcuts*.rs` | 快捷键映射、编辑/新增/删除、保存与未保存保护 |
| `/synapse/settings/#synapse/auto-launch` | 已回读/partial，已修 | 720 `Ks/ho`；`settings_page.rs/preferences.rs` | 真正 host 查询、应用意图的后续写回与回执 |
| `/synapse/settings/#synapse/other-widgets` | 挂载和实际 handler 回读 | 720 `bn/Ia/Un/Ta/ia`；`settings_page.rs/settings_lighting.rs` | 完整教程集合、通知与推荐实际服务、WDL 观察；逐控件 CSS/状态复核 |
| `/synapse/settings/#general` | 挂载回读 | 720 `Te/Fs/uo`；`settings_page.rs` | 语言/版本真实刷新及回退、release notes 回执 |
| `local-settings/#connection` | 用户保留本地页，待审 | `settings_page.rs::connection/runtime_page.rs` | 父任务持有 runtime 页面，不伪造官方同名根 |
| `/settings/#software` | 根和空目录分支已回读/partial | 8821/9302；`settings_window.rs::software` | 非空已安装目录、自动更新控件及状态 |
| `/settings/#systray` | 根/本地 showMenu 入口回读 | 9762/6584；`settings_window.rs/settings_systray_action.rs` | 5 个 launcher 槽位/Widgets 非空目录、Launch 分支，不得以空目录布局当完成 |
| `/settings/#general` | 挂载回读 | 3414；`settings_window.rs::general` | locale 切换与标题/版本/所有链接状态细审 |
| `/synapse/profiles/#games` | 空态/添加入口挂载回读 | 43 `Ba/oe`；`profiles_page.rs` | 全局 gameList、普通/removed/missing 卡片及详情缺 UI |
| `/synapse/profiles/#devices` | 卡片→弹层入口已回读/partial | 43 `Ga/$n/Ua`；`profiles_page/devices.rs` | IoT/subDevices、edition、图片类别、非空数据条件 |
| `/synapse/profiles/#device-games` | 已回读/partial，已修 | 43 `Ua/ia/xa`、1867 `AM/d9`、7693；`DeviceGamesDialog` | 同步与本地 Add/Duplicate/Delete 已补；Import/Export、真实目录/封面/时长及确认层溢出滚动仍缺 |
| `/synapse/profiles/#add-game` | 空目录真实入口回读 | 3137 `f`→5529 `r`；`AddGameDialog` | 安装程序/多选浏览/刷新/拖放及取消流程 |
| `/chroma-app/#dashboard` | 挂载和本地保存错误路径回读，保存错误已修 | 23322 `Ks`、62296 `Wn`；`chroma_page.rs` | 非空服务组、设备模态、安装/固件/能力；普通写入非原子，损坏偏好加载仍回退 |
| `/chroma-app/#modules` | 挂载与 Studio 打开入口已回读 | 65596 `P5`；`chroma_page.rs::modules` | 其它模块非空/已安装/更新/错误分支 |
| `/chroma-app/#apps` | 挂载回读并确认缺口 | 77778 `Se`；`chroma_page.rs::apps` | SDK 总开关仍 disabled；应用列表、优先级拖动及启停 UI 待接；自动排序/显示禁用目前仅本地偏好 |
| `/synapse/chroma-studio/#layers` | 已回读/partial | EditorCanvas 4264；`chroma_studio.rs/chroma_studio_layers.rs` | 分组内容、拖排、整组复制，配置选择/导入导出/关联 |
| `/synapse/chroma-studio/#canvas-toolbar` | 已回读/partial | 4264、1638；`chroma_studio.rs/chroma_studio_canvas.rs` | populated device/LED、涂抹、panning/rotation/fit、独立预览/帧传输 |
| `/synapse/chroma-studio/#ambient` | 实际属性挂载/工作缓存已回读 | 1958；`chroma_studio_properties.rs` | 屏幕选择/高亮 service；slider/preset 完整输入边界 |
| `/synapse/chroma-studio/#static` | 实际属性挂载已回读 | 3181/1698；`chroma_studio_color.rs` | 取色 service；HEX/RGB/brightness/自定义色每项输入回读 |
| `/synapse/chroma-studio/#spectrum` | 实际属性挂载已回读 | 8552；`chroma_studio_gradient.rs/chroma_studio_duration.rs` | gradient 弹层、光标、停止点边界与每项键盘操作 |
| `/synapse/chroma-studio/#breathing` | 实际属性挂载已回读 | 2777；`chroma_studio_properties.rs/chroma_studio_playback.rs` | 随机/颜色 sentinel、duration/playback 每项交互复查 |
| `/synapse/chroma-studio/#fire` | 实际属性挂载已回读 | 5591；`chroma_studio_properties.rs/chroma_studio_color_dropdown.rs` | 热/冷色弹层与 shared color 每项复查 |
| `/synapse/chroma-studio/#reactive` | 缺口确认：属性未挂载 | 7518 | 本地 render 没有分支；不能以 source data 声称完成 |
| `/synapse/chroma-studio/#ripple` | 缺口确认：属性未挂载 | 5305 | 同页独立 root/设备条件/gradient/speed/width/playback 待实现 |
| `/synapse/chroma-studio/#starlight` | 缺口确认：属性未挂载 | 6548 | 独立 gradient/random/density/duration 待实现 |
| `/synapse/chroma-studio/#wave` | 缺口确认：属性未挂载 | 1591 | 独立 gradient/speed/pause/width/direction/angle/playback 待实现 |
| `/synapse/chroma-studio/#wheel` | 缺口确认：属性未挂载 | 8379 | 独立 gradient/speed/center/direction/playback 待实现 |
| `/synapse/chroma-studio/#tidal` | 缺口确认：属性未挂载 | 8154 | 独立两色/random/speed/center/direction/playback 待实现 |
| `/synapse/chroma-studio/#audio` | 缺口确认：属性未挂载 | 9120 | 独立 gradient/boost/auto boost/decay 待实现 |
| `/synapse/chroma-studio/#chroma-generate` | 缺口确认：属性未挂载 | 2474 | 媒体/配置增删/导入及生成服务边界待实现 |

Studio 未挂载结论来自重新解析全部 13 当前根，并回读实际 `StudioProperties::render`：只有 breathing/fire/ambient/static/spectrum 分支，其余落入空 `div`。记录器/描述符/默认参数对象不改变这一结论。已挂载 5 根亦不因此获得全部控件完成状态。

## 24 个公共应用端点的完整队列

下面覆盖 `application-catalog.json` 的全部 24 项。端点 HTTP 状态沿用目录中 2026-10-02 的抓取记录；本次只读取当前本地 HTML/manifest 并重新记录哈希，没有声称再次联网。表中的“已定位”只是实际代码入口候选，须结合前面的逐控件矩阵，不能算整页复查完成。

| 当前端点 | 具体范围/原生入口 | 本轮状态及下一步 |
| --- | --- | --- |
| `/alisha/` | Host constants 明确 Streamer Companion；AppPicker 有入口 | 未找到对应 Rust 主页面；独立应用控制页待审，不能以安装入口代替 |
| `/background-manager/` | Appengine Background Manager | 后台基础设施端点，当前 manifest 已记录；不是已完成可见 UI，查询/观察包装仍独立待审 |
| `/chroma-app/dashboard/` | `ChromaWindow → ChromaPage` | Dashboard/Modules/Apps 见矩阵；本地保存失败已修，工具栏 Settings 目标偏差已确认 |
| `/chroma-app/settings/` | 当前独立 Chroma Settings 资源存在 | 缺口确认：原生按钮进入了 Synapse Settings；须补独立根及其控件 |
| `/cortex/` | Host constants 的 Cortex 目标 | 抓取记录 404，未获得当前页面证据；不能依据不存在的源声明 UI 等价 |
| `/feedback/` | `Location::Feedback → FeedbackPage` | 当前访客表单/日志确认实际回读 partial；账号/目录/回执及其它状态待审 |
| `/natalie/` | Host constants 的 Virtual Ring Light；AppPicker 有入口 | 未找到对应 Rust 主页面；光效/屏幕/本地设置页面待审 |
| `/profile-migration/` | `Location::ProfileMigration → MigrationPage::new` | 正式入口与 preview 构造分开；仅定位，选择/分组/取消/准备/结果根待逐项当前源复查 |
| `/rz-app-menu/` | `AppPicker`、`app_picker_host.rs` | 实体已定位；安装事实/推荐/模块/本地打开/错误分支待逐项回读 |
| `/rz-user-profile-menu/` | `account_menu.rs` | 实体已定位；访客/登录态/锁定/退出确认与观察接口待审 |
| `/settings/` | `SettingsWindow` | Software/Systray/General 见矩阵，不能以空目录状态完成非空目录 UI |
| `/sophie-lite/` | Host constants 的 7.1 Surround Sound | 未找到该独立 Rust 页面；不能用产品音频 Family 代替应用根 |
| `/sophie/` | Host constants 的 THX Spatial Audio | 未找到该独立 Rust 页面；不能用产品 THX 控件代替应用根 |
| `/synapse/` | Host 顶级命名入口 | 抓取记录 404；实际主界面来源 `/synapse/dashboard/` 单列 |
| `/synapse/alexa/` | `Location::Alexa → AlexaPage` | 实体已定位；installer/登录/地区/语言/服务与每个弹层仍待本轮逐项复查 |
| `/synapse/armory/` | `Location::Armory → ArmoryPage` | 实体已定位；Browse/Share/贡献表单及内嵌 product displayMode=armory 必须逐根审查 |
| `/synapse/chroma-studio/` | `StudioSession → ChromaStudio` | 13 属性根已回读挂载；8 根未挂载已明确，层/画布/保存仍 partial |
| `/synapse/dashboard/` | `Main(Tab::Dashboard)` 及附属主标签 | Dashboard/Devices and Modules/Gamer Room/Shortcuts 见矩阵 |
| `/synapse/introduction-tour/` | `IntroductionTour` 的 Synapse/Chroma 两套 steps | 两套原生数据已定位；每步媒体/文字/Next/Previous/Close/滚动须分别回读 |
| `/synapse/macro/` | `MacroPage`、独立 Macro 产品根 | 本次录制/取消/阶段录制已修，其余编辑器/弹层按矩阵逐项审查 |
| `/synapse/profiles/` | `ProfilesPage → DeviceGamesDialog/AddGameDialog` | 同步/关联过滤/本地 CRUD 已修；Games 正常内容与导入导出等仍缺 |
| `/synapse/settings/` | `SettingsPage` | 本地 startup 草稿已修；所有其它控件/查询/异常回执继续待审 |
| `/synapse/update-fw/` | `FirmwareUpdate` | 实体已定位；各阶段、阻断弹层和 preview/真实观察边界待本轮复查；不执行安装器 |
| `/systray/systrayv2/` | `tray.rs`、Windows/portable 菜单 | 实体已定位；主题/用户/应用/退出/宿主观察与原生窗口生命周期待审 |

Pairing、Gamer Room 独立弹层、Chroma/Host 窗口、发行说明及设备独立 displayMode 另有总队列项目；上面 24 项并不是这些附属根的替代清单。

## 检查记录

- `tools/audit-public-ui-review.cjs`：当前 manifest 范围静态 Acorn，3 Settings 声明、9 Profiles 根/辅助作用域、Dashboard Pi、13 Studio 属性根、3 Chroma 工具栏根和 14 Feedback 方法；同时记录全部 24 端点当前 HTML/manifest 哈希。
- `tools/audit-profiles-content.cjs --check`：14 AST、400 CSS、14 原图；此次修复后 native 哈希已更新。
- `tools/prepare-settings-window.cjs --check`：33 AST、556 CSS、10 locale，通过；只证明独立 `/settings/` 证据不漂移。
- Studio 原有检查：45 源收据/资源/145 SVG 矩形、8 host 收据/62 CSS、13 effect roots/40 receipts/19 CSS 文件通过；没有运行 Canvas 或动画。
- `audit-dashboard-card-state.cjs --check` 曾报告 native 哈希漂移；已定位 `dashboard_device_card.rs` 与 `dashboard_device.rs`，父任务已完成对应复核及证据更新。
- 修改文件执行 rustfmt；完整 `cargo check --locked --all-targets` 由父任务统一执行。本组没有应用、构建、测试、下载 JS 或 DLL 的运行记录。
