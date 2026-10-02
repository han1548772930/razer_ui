# Razer Synapse 4 UI 与行为规格

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](re/20-current-source-version.md)为准。
> 文档审计后已接入新的 Rust 设备工作区，当前实施进度见[重构状态](re/03-implementation-gap.md)。本文中“本轮仅审计”指前一轮文档任务。

## 1. 依据与范围

本规格汇总宿主、Dashboard 和182、653、777产品应用的历次审计，包含历史结论。当前官方宿主源码为 `.ref/host-4.0.827`，Dashboard 为 `.ref/applications/synapse/dashboard`；本轮重新审计的明确范围见[当前版本与用户问题复核](re/20-current-source-version.md)。以实际入口、render 树、事件处理函数、产品配置和 CSS 为证据，不能从 `TAB_*` 翻译词汇表推导产品路由。

早期宿主审计使用4.0.563；本机安装为4.0.821；当前正式更新流为 **4.0.827**。三个产品模块另有独立构建号，详见[源码审计](re/00-source-audit.md)。本文不代表全部 Synapse 产品或所有固件版本。

当前项目使用 [Cargo.toml](../Cargo.toml) 中的 **gpui-kit 0.7.0**。实现规范依据项目内 [GPUI Kit skill](../skills/gpui-kit/SKILL.md) 和 [Design Guides skill](../skills/gpui-kit-design-guides/SKILL.md)。网页 CSS 数值是复现基线；Rust 中的控件行为、焦点和状态应遵循 GPUI Kit，而不是模拟 DOM。

## 2. 真正的应用与产品导航

| 层级 / 产品 | 原版入口 | 普通页面顺序 | 特殊入口 |
|---|---|---|---|
| 主应用 | frontend `HomePage` | Dashboard → Gamer Room → Devices & Modules → Global Shortcuts | Settings 独立应用路径 |
| 182 DeathAdder V3 Pro | `GM.navs` | Customize → Performance → Power → Calibration | HELP；`multiDevicePairing` 独立模式 |
| 653 BlackWidow V4 Pro | `zh.navs` | Customize → Lighting | HELP |
| 777 Kraken BT Sanrio | `Ov.navs` | Sound → Mic → Lighting → Power | HELP |

HELP 虽在产品根 `navs` 中声明，但导航组件会将其过滤到帮助入口。不能把它当作普通居中胶囊 Tab；也不能把“正在配对”从翻译表拿来作为 182 常驻 Tab。

以下是本轮明确纠正的旧假设：

- 三个产品都没有独立 Scrolling 页面。滚轮映射与键盘 Command Dial 分别属于 Customize 的功能。
- 653 没有独立 Performance / Power 页；回报率、游戏模式、Snap Tap、Command Dial 均在 Customize 中。
- 777 没有 Customize / Calibration 页。
- 主应用第三项是 Devices & Modules，固件更新只是其中一组；第四项已定位为 Global Shortcuts。
- Settings 不是每个产品末尾固定附加的 Tab。

## 3. 壳层与视觉基线

实际层级为：Electron 外层应用标签 / 系统窗口按钮 → 前端工具栏 → 应用或产品导航 → 内容。外层标签切换应用窗口；内层导航切换当前应用页面。浏览器标签关闭、整个窗口关闭、页面内弹窗关闭是三种不同动作。

| 部位 | 原版 CSS / DOM 基线 | 复现约束 |
|---|---|---|
| Electron 标签栏 | 高 42，黑底；`.visible` 后显示 | 拖动区与按钮 `no-drag` 分开 |
| 窗口按钮 | 每个宽 48，占完整栏高 | 最小化、最大化/还原、关闭调用原生窗口行为 |
| 前端工具栏 | 高 38；左侧按钮 40×38；右侧 46×38 | 后退/前进依赖历史；刷新检查未保存内容 |
| 产品导航 | 最小高 48，底边 2；active 绿色胶囊 | 两侧 profile / 状态区域与中部导航共同布局 |
| 页面背景 | `#222` | 不能用所有卡片通用背景替代 |
| body | padding `10px 20px 20px`，最小宽 600 | 页面自行决定滚动方式 |
| widget | 常见列宽 600，内边距 `30px 40px`，圆角 5 | `.widget-col` 内纵向堆叠；不可摊成一行 |
| 大内容区 | 常见 max-width 1240 | 不等于所有页面或窗口强制宽 1240 |
| 图案背景 | `.dot-bg` 的 CSS gradient，格距 22 | 不需要下载一张“点阵背景图” |

字体以原包 Roboto 为准。大小写来自原版 locale / CSS；业务标识不能使用翻译文本。原版条件样式、特殊版本背景和媒体查询必须按所在页面使用，不能全局套用。

完整布局、窗口图标和交互见 [应用壳层](screens/00-app-shell.md)。

## 4. 页面功能边界

| 页面 | 已确认结构与行为 | 不能据此推导的能力 |
|---|---|---|
| 182 Customize | 鼠标图、两侧按键标注、映射抽屉、Standard/Hypershift、未保存提示 | 任意其它鼠标的布局和按键数量 |
| 182 Performance | DPI 阶段 / XY、条件回报率、Windows 鼠标属性 | 将 Lift-off 放进性能页；无条件显示 8K |
| 182 Calibration | Smart Tracking；对称 / 非对称距离；首次提示 | 表面列表、手动移动扫描、完成动画 |
| 182 Power | 闲置 1–15 分钟；低功耗阈值 5–100，步长 5 | 统一电源开关；与 777 共用范围 |
| 182 Pairing | 独立 iframe 容器、扫描/绑定/解绑、双设备分支 | 点按钮立即产生一个虚构已配对设备 |
| 653 Customize | 键盘 SVG 命中区和图片、游戏模式、Snap Tap、回报率、Command Dial | 仅一张键盘图片即可完成按键配置 |
| 653 Lighting | 亮度、关灯条件、快速/高级灯效、WDL 条件提示 | 用设备支持 Chroma 代替资源安装状态 |
| 777 Sound | 产品图、音量、Windows 声音属性、独立 audioEq | 仅显示十条水平滑条即可对应原版 EQ |
| 777 Mic | 独立 micEq、EQ 容器 | 根据共有字段添加增益/侧音；与 Sound 共享 EQ |
| 777 Lighting | 亮度、Stream Reactive Lighting、快速/高级灯效 | 复制 653 的关灯条件卡 |
| 777 Power | 可开关闲置计时，实际 5–60 分钟、步长 1 | 根据子组件默认 min=15 忽略父组件 min=5 |

数值、预设数组、资源与验收条件见 [页面索引](screens/README.md)。

## 5. 能力与状态模型

`[建议]` 实现时至少区分以下来源：

1. **产品静态能力**：productId、editionId、layoutId、DPI 范围、灯效列表、支持的控件。
2. **连接和运行时能力**：USB / wireless / BLE、dongleId、固件、是否多设备配对、是否支持 HyperPolling、当前系统动态灯光控制。
3. **设备实例和 profile 数据**：序列号/容器 ID、selectedProfile、映射、各项参数、独立音频/麦克风 EQ。
4. **应用交互状态**：当前应用/页面、映射草稿、待保存确认、选中按键、展开菜单、焦点、异步请求状态。
5. **模块安装和外部应用状态**：Chroma 安装是否完整、正在接管灯效的应用、Streamer Companion 是否存在。

静态配置里有某个字段，不意味着页面必定渲染它。共享组件的默认值也不能覆盖父组件实际传入值。请求已经发送、UI 已更新、本地文件已保存、设备已经回读确认，是四个不同结果。

当前 [AppShell](../src/shell.rs) 负责主应用、导航与异步本地保存，每个设备由 [DeviceWorkspace](../src/features/workspace.rs) 保留 Profile 设置、映射草稿和控件实体；全局快捷键与服务连接各有独立实体。设置中的后台服务查询只显示接口/版本/音频信息，尚未接通设备参数提交与回读。[实现状态](re/03-implementation-gap.md) 记录具体范围。

## 6. 资源契约

每个实际使用的资源应记录：所在页面/状态 → 原始 import 或动态 URL → 构建文件 / 内嵌模块 → 本地路径 → 使用尺寸与变体条件。

- 产品上下文中的 `*.png` 经构建可能映射为 `*.avif`。不能按原始扩展名查找后断言缺失。
- Dashboard 的 `PluginImages/*_dashboard1x.png` 和 Customize 的 `img_prods/prd-1x.png` 用途不同。
- 653 的按键 SVG 包含 input ID、分组和选择几何，另有布局 DEVICECONFIG；把它转成整张 PNG 会丢失交互信息。
- 1x / 3x 图是清晰度变体，不意味着界面应放大三倍。
- CSS 图案、内联 SVG、字体和状态图标同样是资源；不能只列产品大图。
- 本地主前端 manifest 存在资源缺口；同名文件在其它产品目录出现时，只能列为候选，不能当作原路径已经恢复。

完整文件映射与缺失清单见 [资源索引](re/04-resource-index.md)。

## 7. GPUI Kit 落地要求

`[建议]` 先修正路由和功能归属，再重建设备图/几何与卡片布局，再接状态和后端。组件使用可调整外观的库控件；领域值由页面/feature owner 持有，需要连续交互的控件保留 `Entity<State>` 和订阅。Slider 不得在每次 render 时重建；重复控件的 ID 应由设备、profile、按键或频率派生。

壳层、页面、资源管理和后端适配应有明确边界。保存失败、设备断开、安装缺失、查询未完成必须能表达；禁止由 UI 定时器伪造硬件成功。详细组件/API 核对见 [GPUI Kit 映射](re/05-gpui-kit-mapping.md)。

## 8. 验证范围

原版审计采用静态逆向；Rust 重构本轮仅做 `cargo check`，不运行 build、测试、应用、worker 或 DLL，没有验证原版 Electron 和真实硬件。原版压缩代码的符号只在对应构建中有效；定位时使用“文件 + 组件/函数 + 字段或 CSS selector”，不要依赖格式化文件行号。

后续验收应使用同产品、同 edition/layout、同连接方式、同窗口大小和缩放比例的画面；同时验证焦点、键盘、草稿保存、切页、断连及失败回滚。单靠截图或数组常量测试不能证明功能一致。
