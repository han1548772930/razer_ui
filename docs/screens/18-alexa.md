# 18 — 独立 Alexa 应用

2026-10-02。依据 `.ref/applications/synapse/alexa` 中原始 JS、CSS、manifest 和语言字典进行静态复核；未运行原 JS、应用、设备 DLL 或测试用例。

## 源码及实际组件树

- `static/js/main.05f102d2.js`：`QE` 检查 `update.installData`，存在时进入 `RE → zE`，不存在时渲染安装器 `qE`。
- `zE` 的四个页面分别是 Home `Bp`、Skills `Yp`、Settings `yE`、Help `OE/vE`。Razer 与 Amazon 都已登录时才开放 Skills / Settings；其余账户状态只有 Home / Help。
- `Jr/qr` 实际生成 `.page → .main → .wrapper → .body-wrapper → .content`。`.page-home:not(.logged-in-amazon) > .main > .content` 等直系子选择器不命中这棵树，不能照 CSS 名称给游客页强行加全高居中。已登录 Home 的后代 `.content` 规则仍命中，最小高度为 80vh。
- `static/css/main.bbca16c4.css` 为尺寸、颜色和动效依据。正文基准为 14px / 1.22；`.body-wrapper` 最小宽 600，padding 为 10px 20px 20px。
- 原 `zE → Yr` 传入字面量 `home/skills/settings`，语言字典只定义 `TEXT_TAB_*`，所以导航保留 `HOME / SKILLS / SETTINGS`。Help 位于右侧，不是第四个文字标签。
- `.page-icon/.page-icon-2` 有 CSS，但 `Bp` 没有生成对应节点，因此 Home 不额外插入大号 Alexa 标识。顶部标识来自宿主的 38px 工具栏，页面组件不再生成一条重复工具栏。

## 当前入口与状态边界

宿主提供保留的 Alexa 页签和设置中的预览入口。`AlexaPage` 独立保存页面历史、账户预览、设置、展开项、安装预览和弹窗；前进/后退只操作此页的历史。刷新重新选择当前场景，不查询账户、安装器或硬件。

初始状态明确为“尚未读取安装与账户状态”，不把本地可见页签解释为已安装模块。上方的本地场景选择器用于查看原版依赖服务的分支；示例账户、验证码、麦克风、安装进度、大小、版本和更新内容均有可见说明。它不是原版账号、安装和录音服务的替代实现。

| 页面 / 状态 | 当前可查看及交互范围 |
| --- | --- |
| Home — 未登录 Razer | Razer ID 卡片；登录按钮显示服务未连接说明，不改变真实或示例账户 |
| Home — Razer 已登录 | 描述、蓝色 Amazon 登录按钮、支持地区；按钮进入显式本地请求中场景，不自动授予登录成功 |
| Home — 请求中 | 原按钮禁用；错误页 Retry 同样可进入待完成状态，Cancel 清除预览请求 |
| Home — 验证码 | 原说明、220×115 代码框和绿色 COPY；内容为 `SAMPLE`，地址是不可点击的示例占位；复制值明确不能登录 |
| Home — 错误 | `invalid_code_pair`、`unauthorized_client`、其他错误的原文；Retry / Cancel |
| Home — 就绪 | 原版两段使用说明及六个语音示例气泡；明确属于示例账户 |
| Skills | 五项技能、四项可展开；同一时刻只展开一项，关闭 Synapse Skill 不禁用技能列表 |
| Settings | Alexa 总开关、声音、卡片、唤醒词、快捷键、输入设备、语音语言及登出确认；全部只修改本地预览 |
| Help | 原 Master Guide 和 Alexa Guide & FAQ 外部链接 |
| 安装器 | 未读取、检测、可安装、等待、下载、保存、安装、错误；只切换本地场景 |
| 更新说明 | 加载、示例内容、空内容、更新等待、下载、保存及安装；原 footer 的 Update / Cancel 禁用逻辑 |

未连接的真实操作包括 Electron Razer 登录、Amazon codepair POST / 授权轮询、`AmazonUserInfo` / `AmazonLoginError` 回调、`amazonLogout`、Alexa 设置服务、输入设备枚举、全局快捷键注册、下载、保存、安装以及真实更新说明请求。不会据本地 UI 操作生成这些操作的成功状态。

## Home

- 游客卡片最大宽 480，背景 `#111`、圆角 5、padding 30px 40px；16px RazerF5 绿色标题，标题下距 20。
- 普通说明最大宽 550，顶部 20；验证码说明最大宽 560；就绪说明最大宽 400。普通描述的两段分别保留 20px 顶距。
- `.btn` 的最小宽为 90，字体 12、行高 1，padding 7px 16px 6px，边框黑色 30%、圆角 2。hover / pressed / disabled 的透明度为 .8 / .6 / .3，200ms ease-out。Amazon 按钮三态为 `#31c4f3/#6fd6f7/#3589aa`，文字 `#232f3e`。
- 代码框 220×115、背景 `#111`，代码 32px；COPY 是原绿色按钮，顶部 10px。Cancel 位于框外，顶部 10px，带下划线。
- 支持地区为两列，每列五行；第一列 English (US/AU/GB/IN 分别一行)、Deutsch (DE)，第二列 Français (CA)、Español (US/ES/MX 分别一行)、日本語 (JP)。列间距 50，列表 margin 10px 0 20px。
- 六气泡每项宽 300、padding 20、圆角 5、字体 20，左右及顶 margin 10、底 margin 40、最大高度 120；20px 三角尾巴位于左侧 20px。前 3 项蓝底白字，后 3 项绿底 `#212121` 文字。

## Skills

容器最大宽 980。标题绿色 24px RazerF5，开关 32×18、左距 10、顶偏移 -1。描述色 `#dadada`，下距 10。

| 行 | 原标题 key | 展开内容 |
| --- | --- | --- |
| basic-lighting | BASIC_LIGHTING_CONTROLS | BASIC_LIGHTING_DESC、BASIC_LIGHTING_CONTENT_2 |
| chroma-lighting | CHROMA_LIGHTING_CONTROLS | CHROMA_LIGHTING_CONTENT_1…5 |
| launch-application | LAUNCH_APPLICATION | LAUNCH_APPLICATION_CONTENT |
| multimedia | MULTI_MEDIA_CONTROLS | MULTI_MEDIA_CONTENT_1、MULTI_MEDIA_CONTENT_2 |
| power | POWERS_CONTROLS | 无展开命令 |

可展开项的整个标题行为按钮，保留键盘激活和展开语义。行间距 2，标题区 padding 20px 20px 19px、背景 `#111`，图标 41×41、右距 10；标题 16px / `#ccc`，描述 14px / `#999`、顶距 2。内容背景 `#2b2b2b`、padding 20，列表缩进 20、项间距 8。展开使用 Kit `Collapsible` 的实测内容高度与 200ms ease-out，切换方向时保留连续进度。

## Settings

面板最大宽 600、背景 `#111`、圆角 5、padding 30px 40px、底距 25；标题 18px RazerF5。右上角问号目标按原版为 14px 圆形，右 / 顶距 10。

开关轨道 32×18、圆角 9、黑色 30% 边框；14px 黑色滑块横移 14px，轨道与滑块均为 300ms 默认 ease。复选框 20×20，边框 `#737373`；勾选、hover、pressed 色分别为 `#44d62c/#7ce26b/#2f951e`，边框和背景 100ms ease-in-out；对勾两段分别按 200ms / 100ms ease 绘制。总开关关闭后各设置组 100ms linear 变为 .3 透明且拒绝操作，登出行仍可用。

快捷键字段宽 230、高 27、左距 30；默认 `Ctrl + Shift + A`。点击后捕获本地组合键，Escape 取消，忽略 Win，缺少支持的修饰键时补 Ctrl + Shift，不注册系统热键。关闭快捷键复选框会隐藏字段。

两个设置下拉框使用 `surface::select_alexa`，与普通选择器共享 Kit 的 SelectState / List / 焦点与键盘行为。原始尺寸、100ms 初始延迟、展开 / 退出规则见 [Alexa 下拉框源码复核](../re/alexa-dropdown-audit.md)。设备列表只有默认项和明确标注的示例麦克风。语言按原顺序保留 de-DE、en-AU、en-CA、en-IN、en-GB、en-US、es-MX、es-ES、es-US、fr-CA、fr-FR、ja-JP，默认 en-US。

## 弹窗、安装及更新

共享 `eh` 在挂载 100ms 后加 `.show`，移除 `.show` 后保留 300ms 再卸载。登出内容与背景淡入淡出 150ms linear，背景黑色 .7；关闭不因再次点击而重启截止时间。Kit Dialog 负责 Escape、焦点限制、恢复和层级，背景不接受点击关闭并阻止滚轮穿透。

登出面板最大宽 400，绿色边框、背景 `#111`、圆角 5、padding 20px 30px；按钮视觉顺序为左侧 Cancel / 右侧 Log Out。关闭目标 30×30，20px 灰 / 绿图标，100ms ease-in-out 交叉淡变。Cancel / 关闭不改变账户；确认只清除示例 Amazon 会话并重置 Home / 历史。

安装器 `qE` 宽 940，顶部 20。标题区背景 `#111`，左 / 顶 / 底 padding 20、右 30，图标 40；无 updateData 的检测分支仅在显式预览中出现。正文背景 `#2b2b2b`、padding 20；示例封面 288×162、右距 20。进度宽 200、高 8，标签在其下方 5px。原 `Install / Cancel / Waiting... / Downloading... / Saving... / Installing... / Learn More / Restart Synapse required` 字面量保留；等待和下载可取消，保存与安装禁用取消。示例操作不会自动进入安装成功。

更新说明 `ah` 仅在显式“已安装且有更新”示例中打开。面板最大宽 800、底部贴齐、背景 `#222`、仅顶角圆角 5；按实测面板高度从 100% 下方滑入，300ms ease-out，内容不额外淡入，背景仍是 150ms linear。header 为 padding 9px 10px 8px 和底边框；body 最大高 534、padding 17px 30px，footer padding 10px 30px。关闭目标 36×36，19px 灰 / 白图标，200ms linear。日期、版本、大小和 New / Improvements / Fixed 内容均明确标记为示例；空说明不生成分组。

`spinner.ef2d0235.svg` 使用 SMIL 动画，不能依赖 GPUI 的静态 SVG 播放。当前用框架 Animation 和绘制路径恢复原 viewBox 100、半径 30、线宽 10：2 秒周期，角度 0→180→720、弧长 10%→50%→10%，关键时间 0/.5/1。减少动态效果时显示静态圆弧；其他显式进度动画同样遵守框架的减少动态效果策略。

## 资源、文案及验证

实现位于 `src/shell/alexa_page.rs` 及其 `controls/sections/installer/dialogs` 子模块。原头标识、五项技能图标和关闭图标由资源准备工具嵌入；安装器封面是现有 Alexa 模块图的显式排版示例。原语言字典已静态提取到 `docs/re/alexa-source-locales.json`，10 个语种各 185 项，并合入现有 locale 的 `ALEXA_SOURCE` 命名空间。

`alexa_page/tests.rs` 新增真实组件交互回归源码，覆盖账户门禁、单项展开、禁用联动、快捷键、本地登出与焦点恢复、安装取消保护、提前关闭更新弹窗的卸载期限。下拉框另有共享组件回归源码。

本轮 rustfmt、静态差异核对和 `cargo check --locked --all-targets` 通过；统一资源校验通过421项源/输出记录。遵守当前约束，未运行应用或测试；测试源码可编译不代表交互、截图、动画或缩放已实测通过。真实服务连接及运行中的像素核对仍需独立验证。
