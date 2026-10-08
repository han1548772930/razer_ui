# 托盘当前界面、会话与关闭契约

来源为当前 `.ref/host-4.0.827/` 与 systrayv2。公共呈现、翻译和语义命令在 `src/shell/tray.rs`，Windows 注册/原生菜单/窗口位置在 `tray/windows.rs`，账户分支在 `tray/account.rs`。这一区分不表示 macOS/Linux 注册已经完成；官方 host 本身在 macOS 跳过 LeftSystray。

## 关闭、图标与点击

主窗口 X/原生关闭按当前 `closeWindowAction=HIDE` 隐藏，托盘 Synapse 恢复保留的 GPUI 实体。Exit 排空已请求写入，不自动保存未提交草稿；固件升级保护保留，退出写失败恢复窗口并显示错误。托盘创建失败时关闭退出，避免留下不可恢复的隐藏窗口。没有实现 Electron renderer hibernation 或 host force-close-window。

当前宿主的右键菜单实际经过 Electron 41.2.0 `NotifyIcon::PopUpContextMenu` → Chromium Views `MenuRunner`，并非 Win32 HMENU；宿主还固定 `nativeTheme.themeSource="dark"`。此前把两者视为同一种原生菜单是不准确的。

本地保留 `tray-icon` / `muda` 的 HMENU 适配器，单应用顺序为 Synapse、分隔、Settings、Log In、分隔、Exit；tooltip 为 Razer。用户允许少量托盘框架视觉差异，并要求不修改本地第三方依赖。菜单绘制、系统字体、背景、间距和圆角差异没有消除；缺少的 GPUI 菜单样式接口已整理为 [待提交 issue](../gpui-popup-menu-issue.md)，未提交到外部平台。Windows 图标选择改为读取实际 HMENU 的 `COLOR_MENU`，不再错误地以 `AppsUseLightTheme` 推断菜单背景。此处保留原始深浅图标并确保原生适配器内的可读性，是明确的框架差异，不能称为官方固定深色呈现。

未构造其他应用安装状态或账户会话。通知区图标按系统小图标尺寸选择当前 ICO 原始16/20/24/32/40/48/64帧，当前应用菜单图标用20帧；不缩放512帧、不改色或透明度。齿轮和用户图按当前 host `getCachedThemeIcon` 保留原始 PNG 像素和尺寸：齿轮14×14、浅色用户12×16、深色用户14×14。`muda 0.21.0` Windows `to_hbitmap` 仍把菜单图像绘入16×16位图；应用图标的安装目录来源也未接入。

注册在主循环开始后进行，以 set_tooltip 的 NIM_MODIFY 结果确认，失败最多尝试四次。诊断先清 LastError，记录窗口归属、完整性级别、提权和作业 UI 限制；NIM_ADD/set_visible 的库返回值不足以证明 Windows 注册成功。已有环境取证显示工作区可继承 Low Mandatory Level 会使 exe 以 Low 运行而无法注册 Medium Explorer 通知区；本地输出目录约束见 [开发说明](../development.md)。本次没有运行托盘或更改 ACL。

单击等待200ms后显示/定位/聚焦面板；再次单击不切换为隐藏。双击取消单击等待，并读取本地 Settings 的 ShowMenu 选择。失焦300ms隐藏，隐藏取消未完成显示任务；后台原生调整尺寸完成后才显示，避免 WM_SIZE 重入。系统 launch 分支及多应用目录仍未接入。

## 面板与账户分支

未登录 DOM 挂载60px登录行与60px单应用区，宽360、Roboto16/1.22、源背景/边框；实际可见范围取决于网页请求的视口高度，不能把 DOM 子节点总高直接当窗口高。应用区单独持有1px上边框，内部启动按钮为59px，悬停不改变分隔线。登录、启动和账户名字按钮显式覆盖 Base Button 默认1倍行高；启动标题按源 `.launcher .title` 大写，并保留省略处理。按压只降低启动图标透明度，外框保留源700px最大高度和默认箭头。根容器修正为源 `min-height:100%`，允许内容撑高；此前 `height:100%` 会改变外框下边线与内容溢出的布局关系。

网页请求高度按当前源码单数 `.app.list-unstyled` 查询；当前渲染实际为 `.apps`，未登录期只计算60px标题高度。已补查 preload → host `SET_BOUNDS` → Electron `BaseWindow::SetBounds` → `NativeWindowViews::SetBounds`：`resizable:false` 时设置新的最小/最大尺寸为请求尺寸。因此 host 初始 minimum_height=200 不能作为最终高度下限，之前的360×200判断已纠正。Windows 未登录分支按360×60请求、`trayY-60`定位，保留源码右侧10px间距并去掉仅账户/body分支拥有的纵向夹取；再次点击已显示面板只聚焦，不重新对齐。源 `body overflow:hidden` 对应本地视口裁切，启动图标保留32px不缩小。

独立复核补齐宿主实际创建链：`convertFeatureToWindowOptions` 在 policy6 时给出 frameless/transparent 并保留布尔 `resizable:false`，`Tab/common.js` 的 `new BrowserWindow({...t,...})` 继续传递它；Windows `CanResize()` 在 frameless 分支直接读取 `resizable_`。源码 `597/a` 的第一轮横向限制使用主屏 **WorkRect** 宽度（字段 width，或左右端点绝对值之差），不是 MonitorRect 宽；本地第一轮已改成同样的工作区范围，保留后续独立的左右端点判断。源码使用 Electron DIP 坐标，部分 monitor 字段只除 dpiScaleX、其他字段不除；本地仍使用物理坐标和窗口 scale_factor 适配，尚未证明混合DPI或所有任务栏位置像素等价。

阴影也不能只读创建参数：宿主 `b` 对布尔值原样返回，`hasShadow:0!==b(r.hasShadow,1)` 会把传入的 false 转成 true。当前 Electron Windows 构造仍在 translucent 且 frameless 时将 `params.shadow_type` 设为 `kNone`；这一条件链已保留原文证据，没有通过运行观察确认最终窗口阴影。

60px仅对应当前无账户分支，不能据此认定账户面板也是60px；真实账户驱动、body内容测量、400–700px动态请求和通知页复用Widgets高度仍未接通。源码 DOM 中只有 `user.item.id` 成立才挂载 navbar / `systrayBody`。本地默认 SignedOut 且没有真实账户发布者，因此与已登录官方面板的整体结构存在功能性差异，不能把它归为允许的轻微托盘样式差异。字体资产已注册Roboto，但实际回退、混合DPI/多屏/非底部任务栏尚未运行验收。

`TraySession` 明确区分 SignedOut、Guest、Authenticated。账户头部、访客/用户头像、Guest或razerId、登录/View Online 按钮、Widgets/Notifications导航、空内容及通知加载呈现都已有实际组件。默认仍 SignedOut；没有真实账户发布者驱动这些分支，不能把本地访客标签当作 host session。头像/名字、通知已加载为空与尚未加载分别保留状态。

设置齿轮提示立即挂载，100ms后开始100ms linear淡入；离开/窗口失焦淡出，100ms后卸载，重入取消旧任务。提示宽300、右对齐、位于按钮下、padding7/8、边框灰、14px/1.22、层级1060。空Widgets/Notifications设置按钮保留100ms颜色与按压透明度；访客按钮发 account-guest-logout，未连接服务时明确报错，不伪造登出/登录成功。

Settings、quick-panel、Widgets、Notifications 四种命令都打开当前独立 Settings 窗口，默认 Software；当前包没有旧 section listener，不能据命令名造新标签。仅 quick-panel 首次创建传1000×768最小尺寸，其余保留host默认。命名窗口已存在则聚焦并保留页面；详见 [Settings 实现](settings-current-audit.md)。

## 剩余项

真实账户登录/登出/在线档案、通知和Widgets数据、安装目录发现、多应用菜单/启动/退出、Exit All、系统launch双击动作及动态内容高度仍未接入。当前头部/空态组件的存在不代表账户数据读取成功。按住鼠标移动时 GPUI hover 与 DOM hover 存在边缘差异；原生菜单、焦点、透明窗口、字号、DPI和动画均未运行验收。

## 保留证据

- [当前线上源校验](tray-live-source-check.json)、[菜单与窗口来源](tray-source-receipts.json)、[关闭链](save-close-current-source.json)。
- [UI/CSS](tray-ui-current-evidence.json)、[账户组件/提示](tray-account-current-evidence.json)、[ICO原始帧](tray-icon-validation.json)。
- [当前呈现修正](tray-presentation-current-evidence.json)保留本轮JS AST、CSS、host图像加载和PNG尺寸收据；维护检查 `node tools/audit-tray-presentation.cjs` 与 `.work/resource-env/Scripts/python.exe tools/validate-tray-assets.py`。
- [窗口与框架链](tray-window-current-evidence.json)保留静态PE版本串/摘要、Electron版本固定URL/摘要、host/preload至尺寸约束的源片段和明确的平台差异；维护工具 `python tools/audit-tray-window-current.py`，可用 `--fetch` 仅下载惰性文本来源，不执行它。
- 维护工具 `prepare-tray-account.cjs`、`audit-tray-account.cjs`、`prepare-tray-account-assets.py`、`tray_assets.py`；主资源清单保留图像输入输出摘要。

这些收据记录静态来源和取证时文件，不能据编译/摘要匹配声称真实窗口或登录服务已经验收。应用、测试、安装器、厂商JavaScript和DLL均不在开发验证执行范围。
