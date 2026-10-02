# 按项目 skills 实现原版 UI：GPUI Kit 映射

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](20-current-source-version.md)为准。

> 文档审计后已接入新的 Rust 设备工作区，当前实施进度见[重构状态](03-implementation-gap.md)，本轮样式与资源依据见[UI 样式源码复核](06-style-source-audit.md)。以下区分已接入代码与后续扩展建议。

## 1. 依据和已确认的版本

项目依赖 [Cargo.toml](../../Cargo.toml) 中 `gpui-kit = "0.7.0"`，锁定版本见 [Cargo.lock](../../Cargo.lock)。本轮参考：

- [GPUI Kit skill](../../skills/gpui-kit/SKILL.md)
- [组件 family conventions](../../skills/gpui-kit/references/conventions.md)
- [Coding Guides](../../skills/gpui-kit/references/coding-guides.md)
- [Design Guides skill](../../skills/gpui-kit-design-guides/SKILL.md)
- [Design Guides 正文](../../skills/gpui-kit-design-guides/references/design-guides.md)

应用依赖 gpui-kit，使用 component（有样式组件）、base（可复用行为）、assets（库图标）。当前页面已接入这三层；原版尺寸由应用样式负责，导航、开关与 EQ 使用框架行为组合。

原版视觉是产品要求，skills 规定如何可靠实现控件行为、状态、焦点、主题和布局。二者可以同时满足：保留原版页面结构和比例，通过框架组件/行为原语完成交互。

## 2. 已有启动链

[main.rs](../../src/main.rs) 已调用 gpui_kit::application、init、配置主题和 open_window。保留这条 Root/overlay 管理链，避免额外嵌套 Root 或自制一套弹窗层。

当前 `with_assets(resources::SynapseAssets)` 使用应用的[资源入口](../../src/resources.rs)：先查生成的 `assets/synapse/embedded.rs`，未命中时回退到库的 `AllAssets`。启动时调用 `register_fonts`，把 Roboto Regular、Medium、Bold 和 RazerF5 Regular 四份 TTF 注册进 text system；主题字体名对应实际打包字节，无需依赖系统预装字体。

## 3. 组件与状态分工

| 原版需求 | GPUI Kit 方向 | 应用 owner 负责 |
|---|---|---|
| 页面导航/模式 | 当前设备页使用 Base Tabs 配合 Button 胶囊样式和 Tab/TabList 语义 | 路由可达性、设备能力、active ID |
| 保存/重试/外链 | Button | 领域操作和 enabled 条件 |
| 亮度/省电/游戏模式 | 当前 Switch 使用 `surface::SynapseSwitch` 包装 Base Switch；独立勾选项使用 Checkbox | bool 与 profile/设备的同步 |
| DPI、亮度、Smart Tracking | Slider + 必要数值输入；DPI 按稳定五槽保留各自控件，使用原生拖放 | 范围、步长、各槽启用/XY、排序身份、请求节流/提交 |
| Sound/Mic EQ | 当前组合 Base Slider/SliderTrack/SliderIndicator/SliderThumb，显式设置纵向几何 | 两个独立 EQ 域、频率、预设、Custom、Reset |
| Profile/映射/灯效 | Select / Combobox | 候选项、稳定领域 ID、安装/能力过滤 |
| 输入名称/按键文本 | Input；映射录制使用键盘事件捕获 | 验证、提交/取消、稳定物理键与修饰键编码 |
| 配对确认/未保存 | Dialog / AlertDialog，经 WindowExt | 保存/丢弃/继续回调、错误、焦点恢复 |
| 映射侧面板 | 当前为 Customize 容器内定位的 292px 分类编辑器，左侧 230px 输入抽屉独立滚动 | 活动 inputID、来源 anchor、能力过滤、草稿及继续流程 |
| 颜色编辑 | ColorPicker | 真实 effect 参数、支持的颜色数量 |
| Tooltip/加载/反馈 | Tooltip、Spinner、Notification | 状态文案与何时出现 |
| 产品图 | img / ImageSource / ObjectFit | 资源键、edition/layout、比例/清晰度 |
| 键盘命中图/鼠标连线 | 自定义绘制与 Base Button；16 布局共 1901 个精确命中形状 | SVG 几何、inputID、缩放变换；Base Button 管理焦点和键盘激活 |
| 设备分组 | flex/scroll，规模大时评估 List/VirtualList | 排序/折叠/设备身份；不为少量固定卡过度抽象 |

标注“当前”的条目对应已接入代码；其余条目列出组件族选择，运行时服务及条件分支的剩余项见重构状态。具体 API 必须查本地 0.7.0 源码或 skills 参考，不能把 React props 按名字翻译成 Rust builder。

`surface::SynapseSwitch` 保留 Base Switch 的受控值、disabled、焦点与键盘激活，应用绘制原版 32×18 轨道和 14px 圆形滑块。亮度与音量通过 `panel_with_control` 把开关放回卡片标题行。设备导航使用 28px 高、14px 圆角的胶囊；外层 `.tab_group()` 参与键盘焦点遍历。

## 4. 已核对的 Slider API

本机 registry 的 `gpui-component-0.7.0/src/slider.rs` 导出 Slider、SliderState、SliderEvent、SliderValue。可确认：

```rust
// component::slider
Slider::new(&entity_of_slider_state)
    .vertical(); // 也有 horizontal()

// base::slider 的状态构造器，由 component 重导出
SliderState::new()
    .min(-5.0)
    .max(5.0)
    .step(1.0)
    .default_value(0.0);
```

这是库的构造签名片段。当前 [EQ 实现](../../src/features/audio_page.rs) 没有直接使用带默认样式的 `component::Slider::vertical()`：该组件自身默认高 120px，仅将外层容器设为 300px 不能改变实际滑条。`eq_slider` 改用 Base Slider/Track/Indicator/Thumb，输入区高 300px、滑块 16px、中心有效行程 284px、轨道宽 6px；轨道两侧保持暗色，只绘制中央零点亮色标记。

EQ 继续引用 [controls.rs](../../src/features/controls.rs) 中 retained SliderState 和原有订阅，Sound/Mic 隔离、Custom 与 Reset 状态语义不变。Entity 在初始化或领域集合变化时创建，render 只引用已有 Entity。数值提示已接入原版 26×20 随动气泡，在悬停滑条时出现；详见[EQ 样式审计](06-style-source-audit.md#4-eq-的真实结构)。

Select::new 需要 `&Entity<SelectState<D>>`，其 state 依 delegate 类型；ColorPicker::new 同样引用 retained Entity。不得仿照旧版 API 编造 `.on_change` 或参数列表。当前事件订阅保存在 owner 中，程序同步与用户 Change 分开处理；后续扩展仍须核对实际签名，不能立即 drop 订阅。

## 5. 身份、状态和副作用

`[建议]` 按以下边界组织：

- App shell：应用窗口、主路由、设备选择、全局弹层入口。
- Device/profile owner：已确认值、profile 列表、连接能力、草稿/dirty。
- Feature owner：本页编辑状态、retained 控件 Entity、订阅、请求 task。
- Resource resolver：产品/edition/layout/用途 → 本地资源，缓存和失败状态。
- Backend adapter：设备请求、系统属性/外链、安装/引擎事件；不在 render 中执行。

重复控件 ID 由“设备实例 + profile + 功能 + inputID/频率/mode uid”派生，不用列表 index、翻译标签或随机 render ID。设备实例要考虑 serial/container；同 PID 多个设备不能相互覆盖。

render 不读取磁盘、枚举设备、启动 DLL、发送写入或泄漏字符串。异步工作通过框架 task/context 返回 owner；销毁/切 profile 后的旧响应应通过目标身份/请求代次过滤。

连续 slider 变化先更新草稿/预览；是否 mouseup 提交或节流按原事件契约决定。不要每次 render/鼠标移动都写磁盘，也不要用 notify 代替完成回调。

## 6. 主题、密度与缩放

CSS px 在逐页文档中用于建立原版基线，当前 `surface::css` 以 16px 根字号把这些尺寸转换为 rem；颜色、边框、disabled、hover、pressed 和 focus token 集中维护。

保持shell高度、导航密度、widget列、产品图比例和EQ纵向布局；使用统一缩放/相对尺寸处理字体及spacing。原图3x只是更高分辨率，不应让布局变成3倍。设备图和命中几何必须使用同一坐标变换。当前 `surface::page_columns` 保持原版600px卡片、20px间隔与1240px内容最大宽度；在原1279px媒体条件下按左右30px列余量转为纵向，窄窗口通过正文双轴滚动访问固定内容。Customize产品命中区仍保留770/730坐标契约；777 Mic保持原940px固定面板，不随窗口缩小挤压频段。详见[响应式规则](06-style-source-audit.md#10-原版响应式规则与正文滚动)。

普通工具控件可用库尺寸档位；原版特殊设备图可定义专用布局 token。不得为迁就一个默认组件尺寸改变原版信息层级，也不能为了像原图放弃键盘、焦点与 disabled 语义。

## 7. 资源格式和字体

[资源索引](04-resource-index.md) 记录原 PNG request 到实际 AVIF 的映射。当前 gpui-pre 的 image 依赖显式列出常见格式，Cargo.lock 的 image 0.25.10 中 `avif`（ravif）与 `avif-native`（dav1d）是不同 feature；锁文件出现 ravif 不等于 AVIF 解码链可用。

当前[资源生成器](../../tools/prepare-resources.py) 已把 `.ref/devices` 内所需 AVIF 转成 RGBA PNG，保留源路径、源/输出 SHA-256、尺寸和用途映射。产品图、图标和字体由同一清单生成嵌入表；运行时不读取 `.ref`。帮助 SVG 原为 URL fragment 选择状态的精灵，生成时按 default/hover/active 设置根 viewBox，保留源路径。

四份字体由 `.ref/host-4.0.827/electron/assets/fonts` 的 WOFF2 转成 TTF 并在启动时注册；设备资源目录未提供这些字体，因此仍沿用该已确认来源。转换保留原字体时间戳，确保重复生成不因执行时间改变字节。当前 160 条资源记录、98 个原 Webpack 请求、56 条动态资源解析项，以及 16 布局 / 1901 个命中形状和生成嵌入表，已由[资源校验器](../../tools/validate-resources.py) 只读核对。

653 的 Chroma SVG_PRODUCT 包含灯光选择分组；Customize 则读取 main 模块 21368（布局 1）及 30387（其余布局）的 groupList，在 730×340 内生成输入路径，不能把 Chroma 的 960×360 SVG 当作 Customize 命中图。两类几何与 DEVICECONFIG 均保留来源。

## 8. 验收策略

前序版本执行过 30 项测试，其中 7 项使用生产视图验证 GPUI 布局与交互。本轮保留并补充动态资源、响应式布局、映射、输入抽屉与五槽 DPI 用例；按用户要求不运行应用、build 或测试，不能把历史通过结果视为新增测试已执行。当前静态检查结果见[重构状态](03-implementation-gap.md#5-验证状态)。验收范围如下：

1. 路由与能力：三个产品正确页面、HELP/Pairing 特殊入口、条件卡显隐。
2. 业务不变量：Mic/Sound 隔离、Smart Tracking 关系、DPI 阶段/XY、正确 effect ID。
3. UI 交互：生产视图里的点击/拖动/键盘、焦点、草稿退出、profile 切换；使用 skills 指定的 GPUI Kit UI integration testing 方法。
4. 异步：设备断开、取消、超时/失败、旧请求响应、重复订阅。
5. 样式：以原版 JS/CSS 和实际资源核对布局、状态与命中几何；旧截图不作依据。当前已有不同窗口宽度与字号比例的布局检查，操作系统 DPI 与完整窗口表现另行验证。
6. 后端：有设备确认/回读才能标记硬件功能完成，本地保存和演示路径单独报告。
