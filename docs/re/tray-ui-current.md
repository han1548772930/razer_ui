# 托盘当前界面、会话与关闭契约

来源为当前 `.ref/host-4.0.827/` 与 systrayv2。公共呈现、翻译和语义命令在 `src/shell/tray.rs`，Windows 注册/原生菜单/窗口位置在 `tray/windows.rs`，账户分支在 `tray/account.rs`。这一区分不表示 macOS/Linux 注册已经完成；官方 host 本身在 macOS 跳过 LeftSystray。

## 关闭、图标与点击

主窗口 X/原生关闭按当前 `closeWindowAction=HIDE` 隐藏，托盘 Synapse 恢复保留的 GPUI 实体。Exit 排空已请求写入，不自动保存未提交草稿；固件升级保护保留，退出写失败恢复窗口并显示错误。托盘创建失败时关闭退出，避免留下不可恢复的隐藏窗口。没有实现 Electron renderer hibernation 或 host force-close-window。

`tray-icon` 与 `muda` 提供 Windows 原生菜单，单应用顺序为 Synapse、分隔、Settings、Log In、分隔、Exit；tooltip 为 Razer。未构造其他应用安装状态或账户会话。通知区图标按系统小图标尺寸选择当前 ICO 原始16/20/24/32/40/48/64帧，右键图标用20帧；不缩放512帧、不改色或透明度。

注册在主循环开始后进行，以 set_tooltip 的 NIM_MODIFY 结果确认，失败最多尝试四次。诊断先清 LastError，记录窗口归属、完整性级别、提权和作业 UI 限制；NIM_ADD/set_visible 的库返回值不足以证明 Windows 注册成功。已有环境取证显示工作区可继承 Low Mandatory Level 会使 exe 以 Low 运行而无法注册 Medium Explorer 通知区；本地输出目录约束见 [开发说明](../development.md)。本次没有运行托盘或更改 ACL。

单击等待200ms后显示/定位/聚焦面板；再次单击不切换为隐藏。双击取消单击等待，并读取本地 Settings 的 ShowMenu 选择。失焦300ms隐藏，隐藏取消未完成显示任务；后台原生调整尺寸完成后才显示，避免 WM_SIZE 重入。系统 launch 分支及多应用目录仍未接入。

## 面板与账户分支

未登录默认显示60px登录行与60px单应用启动行，宽360、Roboto16/1.22、源背景/边框；按压只降低启动图标透明度。网页请求高度按当前源码单数 `.app` 查询得到60，host minimum_height 为200；本地保留最小200、网页 trayY-60 和主屏 WorkRect 横向约束，外框填满整个窗口。未声称混合DPI/多屏/非底部任务栏像素完全一致。

`TraySession` 明确区分 SignedOut、Guest、Authenticated。账户头部、访客/用户头像、Guest或razerId、登录/View Online 按钮、Widgets/Notifications导航、空内容及通知加载呈现都已有实际组件。默认仍 SignedOut；没有真实账户发布者驱动这些分支，不能把本地访客标签当作 host session。头像/名字、通知已加载为空与尚未加载分别保留状态。

设置齿轮提示立即挂载，100ms后开始100ms linear淡入；离开/窗口失焦淡出，100ms后卸载，重入取消旧任务。提示宽300、右对齐、位于按钮下、padding7/8、边框灰、14px/1.22、层级1060。空Widgets/Notifications设置按钮保留100ms颜色与按压透明度；访客按钮发 account-guest-logout，未连接服务时明确报错，不伪造登出/登录成功。

Settings、quick-panel、Widgets、Notifications 四种命令都打开当前独立 Settings 窗口，默认 Software；当前包没有旧 section listener，不能据命令名造新标签。仅 quick-panel 首次创建传1000×768最小尺寸，其余保留host默认。命名窗口已存在则聚焦并保留页面；详见 [Settings 实现](settings-current-audit.md)。

## 剩余项

真实账户登录/登出/在线档案、通知和Widgets数据、安装目录发现、多应用菜单/启动/退出、Exit All、系统launch双击动作及动态内容高度仍未接入。当前头部/空态组件的存在不代表账户数据读取成功。按住鼠标移动时 GPUI hover 与 DOM hover 存在边缘差异；原生菜单、焦点、透明窗口、字号、DPI和动画均未运行验收。

## 保留证据

- [当前线上源校验](tray-live-source-check.json)、[菜单与窗口来源](tray-source-receipts.json)、[关闭链](save-close-current-source.json)。
- [UI/CSS](tray-ui-current-evidence.json)、[账户组件/提示](tray-account-current-evidence.json)、[ICO原始帧](tray-icon-validation.json)。
- 维护工具 `prepare-tray-account.cjs`、`audit-tray-account.cjs`、`prepare-tray-account-assets.py`；主资源清单保留图像输入输出摘要。

这些收据记录静态来源和取证时文件，不能据编译/摘要匹配声称真实窗口或登录服务已经验收。应用、测试、安装器、厂商JavaScript和DLL均不在开发验证执行范围。
