# 独立应用正文逐页复核

本轮只采用当前 `.ref/applications/` 的实际挂载代码。下表把已经查证的修正和仍未完成的内容分开；已有页面、静态收据通过或 `cargo check` 均不代表整页视觉验收。应用、构建、测试和下载 JavaScript 均未执行。

> 后续批次：Profiles Games/Devices/DeviceGames及现有Add/Profile菜单已继续复核并修正，详见[Profiles正文专项](profiles-content-consistency-2026-10-05.md)。下表Profiles的“本轮未改内容”保留为初次批次记录，当前状态与剩余项以上述专项为准。

## 本轮已修正

Alexa 的当前入口为 `ch → QE → zE`，安装分支判断为 `update.installData`；已安装正文经 `Jr → Zr/Kr` 的 `.page > .main > .wrapper > .body-wrapper` 挂载四个页面。不是仅凭未使用的 CSS 类推断页面。

- **公共正文：** 末段 `body,html` 把早期 14px 重置覆盖成 Roboto 16px、`#ccc`，仍继承 `line-height:1.22`。原生改为该字体级联，控件自身的 12/13/14/16/18/24px 规则保留。
- **Home 已登录：** 两个 `<br>` 对应的段间空行改为 19.52px；`80vh` 以 Host 42px TabUI 下方 WebContents 高度计算。预览对话框仍是本地调试适配，不能作为原版窗口尺寸证据。
- **Skills：** 正文列表文字只缩进 20px，修正旧实现加圆点与 gap 后多缩进 14px 的问题；More/Close 文案按 header hover/active 做 100ms color ease-in-out、opacity linear，按下为 0.7；展开仍使用 200ms ease-out。五个分类 SVG 与当前 CSS 指向的文件逐一比较 SHA-256，相同。原生圆点仍是几何圆，未宣称复刻浏览器字体的 list-marker 栅格。
- **Settings：** 通用 Kit tooltip 替换为源 `sE → oE` 的专用帮助。保留文字 `?`、14px 圆形、13px 字/行高、`#5c5c5c` 底和 `#f4f4f4` 字；白色 hover 叠层 100ms。提示在右下方 5px，300px 外层、内容自适应、7×8px padding、14px/1.22、黑底及 `#5d5d5d` 边框。进入后等待 100ms 再做 100ms linear 淡入；离开淡出后移除；窗口失焦关闭。用户名样例仅在隔离的 ready 预览场景可见，生产未知账户不伪造登录。
- **RazerF5 字重：** Skills 源 100 使用 Thin；Settings 源指定 200，但该应用只有 100/400/600/700 face，CSS 匹配选择 Thin，因此原生明确用 Thin。Thin 实际字体注册由公共字体专项补齐。
- **Armory 横幅：** `3026` 的 `isExchangeEnabled ? $JL : HBy` 分别经 `54693` 解析为 Exchange/Workshop 标题。原生没有 feature 响应时，标题改用 hook 初值 false 对应的 `WORKSHOP_GET_STARTED`，与 Host 标题/图标分支一致。**无响应并不等于返回了全 false 的有效响应**：后者经 `77989` 计算会选 Exchange。

机器收据：[Alexa](alexa-content-current-evidence.json)，工具 [audit-alexa-content.cjs](../../tools/audit-alexa-content.cjs)；[Armory](armory-default-source.json)，工具 [audit-armory-default.cjs](../../tools/audit-armory-default.cjs)。

## 逐页覆盖和未完成项

| 应用 / 当前页面 | 本轮正文覆盖 | 尚待完成的核对或实现 |
| --- | --- | --- |
| Alexa Home | 当前根、三种账户挂载条件、字体级联和 ready 间距已核对；修正上列项 | Guest/Razer/code/error 分支按钮/区域完整排版、验证码链接/复制的真实数据输入、气泡截断 tooltip 与完整 hover 行为；实际字体度量/滚动 |
| Alexa Skills | 标题/开关、五组图标/文案、列表、单项展开与 More 状态核对 | source localStorage 设置持久化；最大高度测量在连续缩放/语言变化中的精确行为；键盘按下样式与浏览器默认 list-marker |
| Alexa Settings | 公共字体、表单组、checkbox 尺寸、禁用单次透明度、专用帮助 tooltip 核对 | 真实麦克风/用户名、源设置持久化与外部状态变化；快捷键捕获完整边界、logout 文本链接过渡、全部输入焦点/滚动行为 |
| Alexa Help | 当前 `OE → vE` 挂载和链接身份查证 | `.body-widgets/.widget-col` 在 ≤1279px 下 margin 与最小宽度、外链内联图标和 hover、字体/行高完整核对；不能因页面已有就标为完成 |
| Alexa Installer / Patch Notes / Logout | 本轮只保留既有实现；未重新验收 | 安装状态各分支、面板文字与按钮、尺寸/坐标/时序逐项重审；账户/安装服务后置 |
| Armory Browse / My Downloads | 重查 3026/95889/77989 与横幅默认标题；其余沿用既有收据 | 逐控件字号/行高、搜索展开/清除/防抖、filter/sort 面板及所有 hover/pressed 动画再次对照实际挂载 CSS；功能响应数据后置 |
| Armory Spotlight / My Uploads | 条件入口已有 source 依据；无能力数据时隐藏 | 启用分支、内容卡片、远端详情与完整分享 UI；本地已有 Share form/summary 必须继续核对，而非把空远端数据当作完工 |
| Profiles Games | 已有 43/Ba → oe、两项导航和添加磁贴审计；本轮未改内容 | game-tile 字体/间距、搜索与筛选控件全部状态、添加/扫描弹层和动效重新逐页核对；程序扫描/拖放/全局目录未接入 |
| Profiles Devices / Device Games | 已有 Ga 与设备关联弹层实现；本轮未改内容 | 290×220 device tile 与 240×190 linked-game tile 分别复核、Profile菜单/重命名、三级选择/返回链、子设备/editionName、媒体查询下尺寸和位置 |
| Feedback Form / Privacy / Log Confirmation | 当前 ps 实际只挂 renderMainPage/renderFooter 的既有审计保留；本轮未重新验证整个内容 | 表单控件字体度量、邮箱临近 textarea 高度、隐私遮挡与 tab 顺序、页脚内联链接、520×175 日志确认字体/按钮与交互；草稿以外的提交/日志后端未接入 |
| Chroma Dashboard Presets / Devices / Modules | 既有 C01–C07 修复保留；本轮未改内容 | 专属提示 top24/left8、随光标300px提示、原灰色/hover动画图、SDK未知状态、第三方/Sensa/immersive 分组、设备 battery/effect 色块、分组重排/吸附/reflow，见既有 Chroma 待办 |
| Chroma 独立模块 | 已确认 Dashboard 不等于 Studio | Studio/Visualizer/Sensa/Connect 完整编辑器未接入，不能以 Dashboard 壳替代 |
| Macro Home / Editor / Help / Bind Device | 父线程持续维护专项，本子任务本轮未编辑或宣称重审 | Sequence/Phased 全创作、原光标、配对连线/动画、行复制与多选拖动、嵌套菜单/字符面板生命周期、IME、完整字体/布局/状态审查；当前窗口捕获不等于硬件录制 |

禁止运行的约束仍有效；本轮采用静态 AST/CSS/资源比较及允许的格式化、`cargo check --locked --all-targets`。父线程最终检查已通过，见[本轮索引](ui-consistency-round-2026-10-05.md)；没有像素一致或服务已接通的结论。
