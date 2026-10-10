# 设置入口与正式页面呈现

本项依据当前 Synapse Settings、Dashboard、Alexa、Profile Migration 和对应产品源码，修正“服务连接”中的旧测试入口。静态根组件、动作、精确字符区间、源码 SHA-256、255 段产品组件及实现文件哈希见 [证据](settings-entrypoints-current-evidence.json)，维护工具为 `node tools/audit-settings-entrypoints-current.cjs --check`。

当前官方 Synapse Settings 的 `4914/kr` 实际挂载 `po` 顶部导航和 `mo` 正文，`$i` 仅声明 Synapse/General 两个原版导航。`ho` 是左右两栏，左侧 `Ks/bn/Ia`、右侧 `Un/Ta/ia`；`uo` 左侧语言与 Release Notes `Te`、右侧 About `Fs`。样式表虽定义旧 `.side-navigation`/`.setting-content`，这些节点没有出现在当前挂载链，不能据此添加侧栏。原版没有本地“服务连接”诊断页。这个本地入口区不能作为原版页面覆盖的证据，也不能通过示例选择器推断真实服务或硬件状态。

本项同时修正正文几何：原 `.body-wrapper` 内距为上10、左右20、下20；`.body-widgets` 最大1240；两栏各600，不新增20px水平gap；每张卡片保留上下10px margin、30/40px内距、5px圆角和18px RazerF5标题。窄窗口的两条同特异性媒体规则按源码顺序核对，最终 `.body-widgets div.widget-col` 在1279px及以下使用左右30px margin；当前根不挂 `.setting-content`，其250px侧栏偏移和80px尾部内距不适用。推荐开关不再重复显示标题，保留独立可访问名称与原受控状态。颜色继续使用当前源对应的既有主题 token。

| 本地入口 | 当前原代码与实际呈现 |
| --- | --- |
| 配置迁移 | Settings `Ta` 激活已存在的 `syn3-profile-migration`，否则 `window.open(...,"policy=3,tab_visible=1,shouldFocus=1")`；迁移根 `OD` 挂 toolbar 和 `wg` 正文。复用正式 `Location::ProfileMigration` 和唯一 HostTab，不再开外层样例 Dialog |
| Alexa | 当前应用根 `ch` 挂独立 Provider、`QE` 正文与其他根级状态。复用现有 `open_module_tab(Alexa)`，不再把整个应用塞进样例 Dialog |
| 设备和模块 | 复用正式 `Tab::Modules` 和持久 `module_catalog`，不再开模块样例选择器或改写实时目录 |
| 更多应用 | Dashboard `96776/E` 是 toolbar 的 `app-explorer`，切换 `openPopUp`，使用实时目录。复用顶部已有 `AppPicker`、原锚点和真实目录；保留弹层，移除外层测试 Dialog。重复打开不关闭，Escape/失焦关闭仍由原有 PopoverState owner 管理，焦点返回顶部 trigger |
| 控制板 | 打开正式 `Tab::Home`。顶部状态属于原 Header `We`，不是独立测试页面；此入口不制造 offline/update/compatibility 观察 |
| 设备灯光设置 | Settings `ia` 是 Synapse 页右侧的 inline widget，包含 Chroma/WDL 切换、说明和 Windows 设置外链。返回当前 Settings 的 Synapse 页，不再创建带伪造控制权的测试 Dialog |
| Chroma 教程 | 保留已有正式 `Location::Tour`，没有新增外层测试 Dialog |
| 磁轴键盘 | 打开登记产品 740 的正式产品工作区；校准流程仍由产品页拥有，原有内部校准弹层不因本次路由修复被改成页面 |
| Philips Hue | 打开产品 769 的正式 Hue 工作区；不注入 Sample 网桥、灯具或娱乐区，未知通信状态保持原有不可用边界 |
| Aether 灯带 | 打开产品 784 的正式工作区；不注入示例 LED 数量、在线、电源、占用或其他设备 |
| 无线 ARGB | 打开产品 3884 的正式工作区；不注入 ready、端口、检测数量或供电观察。3886 仍通过全部产品选择器打开，保持该产品自身分支 |
| 主板与有线 ARGB | 打开产品 3871 的正式工作区；778 和其 edition 仍通过全部产品选择器打开，不用3871样例状态替代 |
| 自动化 | 打开产品 3946 的正式工作区；不注入 Sample 音频设备、游戏、宏、快捷键目录或 Chroma 安装状态 |
| 鼠标底座 | 打开产品 241 的正式工作区；配对及取消配对确认由产品本身拥有，不再以状态样例 Dialog 代替整个产品页面 |

产品入口沿用已登记产品的原页面 owner、导航、ProfileBar、状态和本地草稿生命周期。没有实体设备时仍明确是产品预览，不能把 PREVIEW 身份用于真实通信。产品内部原版提示、确认和校准弹层保持各自语义；不能以“消除所有弹窗”为修复标准。

本项闭合的是入口与页面归属，未宣称全部页面像素等价或全部服务链完成。Hue/IoT/ARGB/自动化/迁移/Alexa 等页面的真实发布者、写回和细节缺口仍见对应详细审计与全程序矩阵。本地预览草稿也不等于硬件保存。当前只完成原源码静态核对和编译检查；没有运行应用、测试、DLL 或设备操作，字体、DPI、遮挡、焦点和实际画面尚未运行验收。
