# Dashboard 设备卡片状态树核对（2026-10-05）

本次延续已有页面的全面一致性核对，使用当前 `.ref/applications/synapse/dashboard/` 的 22534/z、E、Z、xi、xe 和最终 `55.4e8559cb.chunk.css`。没有读取历史前端，也没有运行应用、构建、测试或下载的 JavaScript。

静态凭据：[dashboard-card-state-current-evidence.json](dashboard-card-state-current-evidence.json)。维护工具：[audit-dashboard-card-state.cjs](../../tools/audit-dashboard-card-state.cjs)。本轮提取 12 个模块内 AST 合同、144 条相关 CSS，以及 17 个原版状态资源的来源与输出哈希。旧 [Dashboard 设备凭据](dashboard-device-current-evidence.json) 中的三项资源缺失已消除。

## 已修正的内容

| 范围 | 当前源码行为 | 本轮修正 |
| --- | --- | --- |
| 普通设备图像 | 非 ready 的图片本身 `.blurred` 为 0.3；图像容器 disabled 与名称组 disabled 是不同条件 | 不再用同一个 disabled 值统一变淡；显式 false 的 supportsStandbyMode 不应用缺省 standby/off 分支 |
| 主机模式 | Xbox Controller/Headset/Audio-Earbuds、PS 普通/Arcade 采用不同源图；PS 图像不加 blurred；主机名称单独再乘 0.3 | 挂载真实分支、145/250×140 尺寸和主机快捷键行；移除缺图时的 Synapse logo 替代 |
| 加载与重试 | restart/canceled/ready/error、waiting-off 不显示 spinner；error/canceled/noAliveSign 显示重试，离线保留按钮和橙色提示 | 独立恢复 spinner 与 retry；重试顶部 25% margin 以 250px 容器宽计算为 62.5px |
| Spinner | 原 SVG 自带 2 秒 SMIL；角度 0/180/720、可见弧长 10%/50%/10%、方形端点 | 原生静态 SVG 不会播放 SMIL，改用相同圆弧、速度分段、线宽和端点几何绘制；尊重系统减少动画设置 |
| 预设加载 | 只读取 deviceState.presetLoading.value；黑色半透明全卡遮罩、250×140 底对齐文本与 5px 进度 | 使用观测值；宽度 100ms ease-in；三点 600ms ease-in 与 150/300ms 初始延迟、0/29/30/98/99/100% 关键帧 |
| 初始化失败 | 仅 mixer_system_check_failed 显示 #1119 全卡层、20px padding、12px 正文 | 补真实条件和提示；默认卡片操作按源码受阻 |
| 重启/固件/WDL | 重启警告、24px 固件提示、24px WDL 图标；WDL 在 firmwareUpdateInfo 存在时偏移到 left39 | 使用源 SVG、文字、尺寸和 hover；固件匹配原 firmwareUpdateDevices 的 PID+序列号+needsUpgrade，不使用合并列表推断 |
| 电池和状态图标 | standby-off 使用独立 24px 图；Xbox/PS 用白色原图 mask；所有分支先受 !showSpinner 门控 | 替换原先空图标槽；保留 2 秒电量显示延迟，修正 restart 状态的普通电池抑制；补主机提示分支和边缘翻转 |
| 卡内操作 | onMouseDown 设置 overrideAction，只有短拖拽释放才执行 | 嵌套重试、固件、WDL 沿用网格拖拽判断；键盘操作由原生按钮处理，并阻止再次激活外卡 |
| 默认卡片行为 | minRequiredVersion、PS/Xbox 是逻辑短路，不会 focusTab；preset/off/standby/init-fail/updating 阻止打开；restart 请求通知 | 修正默认打开条件；固件转到 Devices & Modules，WDL 转到 Settings |

17 个素材通过维护工具 [dashboard_card_assets.py](../../tools/dashboard_card_assets.py) 与 [prepare-dashboard-card-assets.py](../../tools/prepare-dashboard-card-assets.py) 准备，完整资源流水线也引用同一模块。16 个 SVG 逐字节复制当前源，PS 的原版 AVIF 仅由已安装 ImageMagick 转为 RGBA PNG；清单记录源/输出哈希及输出尺寸，没有重绘素材。

## 明确保留的边界

- 本轮仍是静态核对，不能声称像素或运行时验收。CSS normal 行高、系统中文字体回退、pre-wrap 与原生 shrink-to-fit、组合状态下的 tooltip 裁剪/覆盖仍需在允许的条件下进一步验证；参见[公共字体核对](shell-content-consistency-2026-10-05.md)。
- 新 metadata 是真实观测的投影，不会根据本地页面存在或计时器伪造 noAliveSign、WDL、固件、安装或预设进度；实时 reducer/storage 数据桥接尚未接通。
- retryInstall、resuscitate 与 restart notification 没有可用服务 adapter。点击时使用现有通知机制说明请求尚未发送，不改变 setupStatus、不启动安装器、不假装成功。
- 固件图标已导航到正确页面；源 sessionStorage 标志和进入后平滑滚动至首个固件行尚未实现。
- 缺少某个普通设备的可验证嵌入图时保留原尺寸图像槽。源码 onError 的产品 0/0 Dashboard 回退必须建立在实际图片加载失败上，不能拿“未准备本地资源”伪造源 onError。
- 鼠标交互受源禁用条件约束，离线重试按源 pointer-events:none 留给卡片拖拽；没有通过隐藏页面或无条件禁用入口缩小核对范围。

本轮完成 Rust 格式化、静态 AST/CSS 与资源哈希校验；父线程负责合并后的 `cargo check --locked --all-targets`，其结果以主线程最终汇总为准。
