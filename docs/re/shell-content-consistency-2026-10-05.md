# 已有 Dashboard / 服务内容一致性专项（2026-10-05）

本次从当前 Dashboard 的真实挂载树重新核对内容，不将路由存在、图标文件存在或组件复用视为视觉验收。证据来自 `.ref/applications/synapse/dashboard/` 的 22534、44442、19388 与其 55/6505 CSS；字体补充核对当前宿主 4.0.827 和当前 Alexa 的 `@font-face`。未读旧源、未运行应用、构建、测试、下载脚本或 DLL。

可重生成收据：[shell-content-consistency-current-evidence.json](shell-content-consistency-current-evidence.json)，维护工具 `tools/audit-shell-content-consistency.cjs`。它记录 10 个挂载合约、140 条 CSS、7 个真实字体面、字体原/输出哈希、原始及修正后的字重信息。

## 已修正的差异

| 对象 | 当前源码 | 原生原先差异 | 本次处理 |
| --- | --- | --- | --- |
| Gamer Room 横幅标题、产品悬浮面板名称，以及双应用介绍横幅加粗标题 | RazerF5 / 700，CSS 指向独立 RazerF5-Bold | 仅注册 Regular，缺实际 Bold 字形 | 加入真实 Bold 字体；按 CSS 归一为 RazerF5 / 700 |
| 宿主标签文字 | 当前宿主 Roboto.css 注册 Light / 300；HostTab 指定 LIGHT | 本地未加载 Light，只有 400/500/700 | 加入真实 Roboto-Light / 300 |
| Alexa Skills 标题及 Settings 标题的字重匹配 | 当前 CSS 注册 RazerF5 Thin / 100；100 与没有对应 200 字体面的 200 请求使用 Thin | 缺 Thin，只有 Regular | 加入真实 Thin / 100；Alexa 专项代理负责调用点 CSS 匹配修正 |
| Roboto Medium 字族 | CSS 把 Roboto-Medium 声明为 `Roboto` / 500 | 原 TTF 的 legacy family 为 `Roboto Medium`，未等效 CSS 别名 | 转换时统一 name 表族名，保持真实 500 字形 |
| Devices & Modules 安装失败文字 | 44442/w 的 `.second-from-left.info-text` 提供 14px | 漏设字号，继承 body 16px | 补 14px |
| 已安装行“正在移除”文字 | 44442/O 的 `.item-action.info-text` 提供 14px | 漏设字号，继承 body 16px | 补 14px |
| 新设备安装 / 重试的离线提示 | `.item-tooltip .tip`：黑底、#5d5d5d 1px 边、#ccc Roboto 14/16、padding 8/10、right -160/top 32；opacity 300ms linear、visibility 即时 | 通用 GPUI Tooltip 的位置、等待方式、字体、外框均不是此挂载树 | 改为本行相对定位源样式控件 |
| 离线提示状态条件 | w 只在默认和 error 分支挂载可 hover 的 `.no-internet`；installing/downloading/canceled-offline 不显示 | 所有离线阶段都添加通用 Tooltip | 按源码分支收窄 |

字体不只是增加文件：原始 RazerF5-Bold / Thin 的 OS/2 `usWeightClass` 都是 400，legacy family 又分别为 `RazerF5 Bold` / `RazerF5 Thin`。浏览器依赖 `@font-face` 声明把它们映射到统一族名及 700/100；GPUI Windows 的 DirectWrite 路径从字体文件选取族名、GetWeight，不能只复制原始内部标签。维护工具只在必要时调整 `name`、OS/2 字重/标志和 `head.macStyle`；逐表断言其余字体数据与当前原始 WOFF2 解压后完全一致，未重画字形、修改字宽或重新设计字体。

`tools/font_assets.py` 同时供完整 `prepare-resources.py` 和增量 `prepare-font-assets.py` 使用，后者只替换连续字体资源区，不改变其他资源顺序。本次用已存在的 `.work/resource-env/Scripts/python.exe` 运行字体准备，不导入不可用的 Pillow，不安装依赖。

## 逐页及公共结构核对范围

| 页面 / 部位 | 本次明确核到的源规则 | 核对结果与尚未完成部分 |
| --- | --- | --- |
| 主导航 | 48px 最小高、底部 2px 黑边；12px Roboto / 14px 行高、7/10 padding、20 间距；普通 #999、选中 #111/#44d62c、hover #ccc/#2d2d2d | 现有静态尺寸/颜色调用对应这些规则。没有因此判定 overflow、键盘焦点、全部 hover 动画和所有宿主尺寸均已验收；未改通用 surface |
| Dashboard 分组 / 卡片 | 290×220 卡、310×240 间距计算；分组 10px 上下 margin；名称 14/16、edition 12/14、状态 12/14；#111 卡、透明空卡、#5d5d5d 虚线 | 后续专项已修正独立透明度，补 spinner/retry/console/WDL/firmware/preset/init 树、主机图与提示、卡内短拖拽操作；详见 [Dashboard 状态树核对](dashboard-card-state-current-audit.md)。服务桥接、固件行平滑滚动和原生像素一致性仍未验收 |
| Dashboard 在线服务 | 22534/de 的四项来源、URL、原图 `background-size:contain`、140px 图、10px 图下间隔、名称与说明的大写和字号 | 已核到对应内容与静态布局。源 hover zoom 选择器针对子 `div[data-type=loaded]`，de 此树并无该节点，不能套用推荐卡的缩放动画；复杂拖动与层叠未验收 |
| Dashboard 空设备 | 22534/ee 两条真实 URL、标题和链接大写；标题 57/67 margin；链接 14/16 与首项 10px 下间距 | 静态结构对应。标题的 CSS normal 行高与 GPUI 当前固定值尚未证明一致 |
| Devices & Modules 四组 | 44442/ae→H 的组顺序、空组返回 null；1220px 宽、40px 组间隔、RazerF5 24px 绿标题；80px 行、padding left20/right30、40px 图标、500px 名字区 | 已核到以上主体规则及普通行 16px 名字。CSS normal 标题/描述行距、完整服务数据变更、所有展开/卸载状态仍未完全验收 |
| 模块 / 设备说明 | w 的 20px padding、#2d2d2d、min-height202；图片288×162/mr20，文本592px/14px，说明mb20 | 已对照调用点。原设备 `detail.srcImage` 没有观测到可验证映射时仍留源尺寸区域，未用 Dashboard 图冒充；HTML 描述的完整内联样式未验收 |
| 安装进度 / 固件说明 | 200×8 进度、#2c5824/#44d62c、300ms；progress wrapper pt20/info12px mt5；固件左列200、gap10、外链20px right-25/top-2 | 已核到主要静态规则及已存在的进度实现。固件长 warning 的滚动、release-notes 标记/嵌套列表、未知 service 状态不计完成 |
| Gamer Room 营销主体 | 19388/He、u 的真实挂载；531px 高、>=2560时2500×930；标题24/700、副标题16/400、描述14/17；产品浮层min400×312、36/24 padding、36 gap、名称18/700 | 本次解决真实 700 字体缺失。源 backdrop blur 30px/5px 当前未实现，pre-wrap、normal 行高、响应式 containing block、所有设备/教程子树未全核 |
| Settings 内服务预览 | 入口复用清单参考既有 service-preview-current-audit；它是开发工具外框，不是官方产品页 | 不能把服务链接可打开、与正式页共用实体或 Tab 图标正确当作页面整体一致；产品主体由其他并行专项继续核对 |

## 字体及布局未验收边界

- GPUI `TextStyle::default()` 的行高为 `phi()`，约 1.618；当前 CSS 很多标题及说明使用 `normal`。Roboto 的 hhea 是 2400/2048，Win 指标是 2458/2048；RazerF5 Regular 的 hhea 为 1366/1000。不能把固定 1.2 或 17px 宣称为这些不同字体的精确 normal。本次补实际字重，没有掩盖这项行高差异。
- 当前页面 CSS 常用 `Roboto,sans-serif` / `RazerF5,sans-serif`，Roboto 某些 font face 还优先 `local(...)`；GPUI 使用内嵌优先与操作系统回退。中文缺字的具体 fallback 字体、回退字符尺寸/行距与 Chromium 是否一致尚未证明。没有擅自替换为微软雅黑或其他中文字体。
- 主内容 wrapper 在 600/1279/2560 断点及外层 `.widget` / `.body-widgets` 是否匹配，需要完整 route 级 cascade 核对；本次没有把局部测到的 1220px 推广为所有页面的公共宽度。
- 新离线 Tooltip 已修正已知字体/颜色/锚点/分支和时间，但 CSS `pre-wrap`、绝对定位 shrink-to-fit 宽度、原生祖先裁剪/层叠仍需继续核。未声称像素验收通过。

本批执行字体资源准备及其逐表验证、Rust 格式化、源码/CSS/哈希静态审计。父线程已在并行编辑收敛后通过 `cargo check --locked --all-targets`、资源/语言键/JSON与专项静态校验；未运行应用或测试。汇总见[本轮索引](ui-consistency-round-2026-10-05.md)。
