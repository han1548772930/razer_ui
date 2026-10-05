# Profiles 正文与现有弹层复核

本轮仅使用当前 `/synapse/profiles/` 的 manifest、main CSS、`9449` CSS，以及实际挂载模块。没有运行应用、构建、测试或下载 JavaScript。

入口 `43/Ba` 的两个视图为 `oe` Games 与 `Ga` Devices。Devices 经 `$n` 打开 `Ua`；`Ua` 经 `7693/I` 挂载关联游戏卡片，并把目标 Profile 传给卡片。两个添加入口均经 `3137/f → 5529/r`。当前组件和 CSS 条件、偏移、SHA-256 见 [profiles-content-current-evidence.json](profiles-content-current-evidence.json)，维护工具为 [audit-profiles-content.cjs](../../tools/audit-profiles-content.cjs)。

## 修正的差异

| 页面 / 控件 | 当前源码证据与改动 |
| --- | --- |
| 全局字体 | `body,html` 为 Roboto 16px、`#ccc`；原生明确指定，局部标题/卡片/输入仍保留各自字号。源码未显式指定的 `normal` 行高没有被宣称为精确字体度量。 |
| Games / Devices 导航与列表 | 28px 导航 + 上下各11px padding + 2px 边框，实际外高52px；Games 的绝对正文仍从50px开始，Devices 正文跟随52px导航。900px min-width 仅用于 Games `.drag-area`，Devices 不再错误强制900px。 |
| Games 搜索 | `8844/m.removeSearchResult` 清空并收起输入；原生补相同行为。点击 Views/Order 区域不会提前关闭空搜索，符合 `navRef` 排除逻辑。Add 弹层的 `3137/f.removeSearch` 只清空、不收起，与主页面分开。 |
| Games 工具栏提示 | Add/Search 以前只有无障碍标签，现补 `.main-nav li:hover .tooltip`：即时显隐、触发器左15px/上30px、内容宽度、8×10px padding、黑底/灰边。未连接的 Scan/Refresh/Browse 保留服务提示，不假装执行。 |
| Add 弹层内容 | `f.render` 只挂 `.fixed-header` 和 `.content > renderAppRows()`；移除本地额外添加的两行空目录说明。无服务响应时没有目录行，不解释成扫描已完成。 |
| Add 弹层返回按钮 | `Ua` 虽把 goBack 传给 `f`，但 `f.render` **没有转发 goBack 给 r**；原生之前多画返回按钮，现移除。来自设备页时关闭仍调用返回设备弹层的链路，保留 no-popup 无过渡切换。 |
| 弹层标题与关闭 | 保留36px标题框和晚段20px/10px padding；改为源块级文本布局，避免 flex 居中把文本向上移动。Add 恢复1px底部阴影；关闭按钮200ms背景过渡；现有16px RazerF5、19px行高、36px按钮和20px原图保持。 |
| DeviceGames 弹层下限 | 该根不匹配 `.profiles-link-games` 的 min-width:0，保留800px最低宽；≥1600px分支仍为1300px。中间宽度的 CSS auto/shrink-to-fit 尚不能当作完成，见下表。 |
| Profile 下拉的行为 | `Ua/Y` 只改 `activeProfileIndex`，作为关联目标；原生不再调用 `workspace.select_profile` 改设备活动 Profile。选择与真实硬件激活不再混同。 |
| Profile 重命名 | 32个UTF-16单位限制；按源 ECMAScript trim 和 `1867/p` 的大小写敏感同名检查过滤空名/重复名，保留 Enter/Blur 提交、Escape取消。本地最终元数据层仍有独立 trim，见限制。 |
| 更多菜单 | 20px原图在26px框内居中；打开/按下绿边、悬停灰边、200ms边框；可用行按300ms变化背景。服务命令保持不可用，仅已实现的本地重命名可用。 |
| 关联游戏卡片 | 去掉字体字符“✓”，按 `.check-box` 两段3px几何、100/200ms绘制；修正2.4px圆角、默认 `#737373`、checked hover `#7ce26c`、pressed `#2f941e`、busy黑底短横线。busy条的文字按left40/top12/max-width80%放置。 |
| 关联游戏添加卡 | 240×190、body120/footer70、14px/16px、`#ccc`；页脚按普通块从顶部开始，修正此前垂直居中和灰字。边框按200ms变化。普通Games添加卡仍是另一套290×220/body150尺寸。 |

14个使用中的图标均与当前 CSS 指定文件按字节核对：Add/hover、Scan/hover、Search/hover、Refresh/hover、灰色搜索、Clear/hover、Close、40px Plus、Profile More。没有用新绘制图标替换已有原图。

## 审查范围与仍未完成的内容

| 范围 | 已核对 | 尚未完成 |
| --- | --- | --- |
| Games 顶栏 / 空目录 | 当前挂载、两个Tab、图标、搜索处理器、过滤/排序数据、52/50几何、添加入口/条件 | 顶栏三列在窄宽度下的 CSS intrinsic flex shrink、溢出导航、完整键盘/焦点和所有字号的实际度量；真实扫描状态未知。 |
| Games 有数据与详情 | `oe` 的筛选/排序/删除/详情入口已有AST证据 | 本地没有全局gameList；正常/removed/missing游戏卡、设备详情、封面编辑等未实现，不能因默认只有添加卡就认定完整。 |
| Devices 卡片 | `Ga → $n` 的290×220、图片250×140、标题14px、edition12px、hover/active边框；取消错误900px下限 | IOT/subDevices、editionName和游戏小图标缺实际数据；各类别图片特例和长标题、完整scroll/normal行高仍待核对。 |
| DeviceGames 卡片 | 本地已知关联、active/busy状态、目标选择、CSS边框/勾选与添加卡 | 真实封面/图标/missing标志、源设备图标列表、Last Played/Most Played没有输入数据；当前窗口打开时采样的已知游戏目录仍需外部新增关联刷新策略。 |
| DeviceGames Profile 菜单 | 源菜单数组、默认General/Factory隐藏规则、230×27框、32长度、重复名、重命名、点击目标不激活设备 | add/import/duplicate/export/delete服务命令未接入；window blur关闭、DOM事件冒泡/焦点完整对等、正常菜单与GAMEPAD异常条件仍待专项。 |
| Add Game（两个打开者） | 实际挂载和不转发goBack、空rows、源工具栏、标题/关闭、no-popup返回链 | 已安装程序行/筛选/排序、Browse多选exe/url、刷新服务、拖放未接；数据存在后的内层scroll高度仍需逐项复核。 |
| 弹层几何 / 公共控件 | 源固定backdrop、89/110/20与100分支、min-height650、CSS媒体查询保留、已修最低宽 | DeviceGames 900–1399px范围的auto/shrink-to-fit宽仍为本地适配，未完成静态布局求解；公共SynapseSelect hover border仍瞬时，不是源300ms；公共组件本批没有修改。 |
| 原生适配差异 | 本地目标/草稿持久数据与服务状态分离 | 32单位校验当前拒绝超长编辑，浏览器maxlength可能截断粘贴；元数据层另做Rust trim；源normal行高、浏览器表单默认字体、DPI下字形/栅格、tooltip viewport裁剪、按键动画与真实命中均未运行验证。 |

新增审计包含14个局部绑定、400条保留media条件的CSS、14个图标，`--check` 检查源码契约及本地文件哈希。旧 `audit-profiles-app.cjs` 继续保留两Tab/语言包审计。格式化和静态检查通过；最终 `cargo check --locked --all-targets` 由父任务集中汇总。以上不是整页视觉或服务完成声明。
