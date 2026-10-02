# 鼠标垫灯光与帮助

2026-10-02。范围为已下载原始 bundle 中确认的七款鼠标垫，正常导航都是 Lighting，Help 为工具栏独立入口。当前实现为本地设备工作区，不代表硬件写入或 Chroma 服务已连接。

| PID | 产品 | 默认亮度 | 快速效果（原顺序） | Wave 方向 |
| --- | --- | ---: | --- | --- |
| 3072 | Firefly Hard Edition | 100 | Audio Meter / Breathing / Reactive / Spectrum / Static / Tidal / Wave | 顺时针 11、逆时针 12，默认 12 |
| 3073 | Goliathus Chroma | 100 | Audio Meter / Breathing / Reactive / Spectrum / Static | 无 |
| 3074 | Goliathus Extended Chroma | 100 | 同 3073 | 无 |
| 3076 | Firefly V2 | 66 | 同 3072 | 同 3072 |
| 3077 | Strider Chroma | 66 | 同 3072 | 同 3072 |
| 3078 | Goliathus Chroma 3XL | 100 | 同 3073 | 无 |
| 3080 | Firefly V2 Pro | 66 | 同 3072 | 向左 1、向右 2，默认 2 |

## 原始依据和实现

各产品 `.ref/devices/<pid>/static/js/main.*.js` 的 DeviceInfo（module 8193）和 QUICK_EFFECTS 定义确认上述差异；module 9228 的方向缺省表是 `{CWCCW:12, UPDOWN:4, LEFTRIGHT:2}`。数据集中在 [product.rs](../../src/product.rs)，效果列表和新建配置缺省值在 [settings.rs](../../src/features/settings.rs)。新建、复制、切换及本地保存配置继续使用每台 Device 的 Profile，方向 11/12 不再被通用 1/2 校验截断。

实际页面复用 [device_pages.rs](../../src/features/device_pages.rs) 的两列灯光布局和受约束的内容滚动区。顶部先显示产品图和点阵背景；下方左列为亮度及关闭灯光，右列为快速/高级效果。

七款原入口都在控件列前实际挂载 ProductImage：3072/3074/3076/3077 的 Lighting 位于 module 3288，3078 位于 6665，3080 位于 5937，均引用 module 7855；3073 的 `xd` 入口挂载同等的内联 `_l`。原图区域 `.widget-prod` 高 250px、宽 1024–1220px、上下各 10px 外距，使用 22px 周期的点阵及径向淡出。图片只固定高度，按原始宽高比计算宽度；325px 宽仅属于这些入口未启用的 `customDotPattern`，不能套在 Goliathus Extended、Strider 或 Firefly V2 Pro 上。本地按产品与 edition 解析嵌入图，缺少 edition 时遵循原 edition 0 回退；图与控件归属同一个滚动区。

- 原 SwitchOffLighting 只渲染 `checkDisplay`，`disabled:!brightnessOn`。不显示键盘闲置复选框与滑条；原状态仍保留 `isIdleEnabled:false,idleMinutes:1`。3072/3074/3076/3077 的该组件在 module 3288，3078 在 6665，3080 在 5937，3073 为内联 `xd`。
- 亮度关闭不禁用快速效果的选择；页面不添加源码没有的这类限制。
- Reactive 检查 `validDevices.filter(category === MOUSE)`。没有真实兼容鼠标时，`reactiveWarning` 替代颜色和时长控件，使用原 `REACTIVE_WARNING` 文案。当前没有该服务数据，不把本地鼠标快照或预览设备当成兼容鼠标。
- Reactive 的原 `.warning:before` 使用 20px `warning.ad3f47f8.svg`，正文前留 30px。本地复用已嵌入的同一图标，以 20px 图标和 10px 间距恢复这一区域，正文可换行。
- Static 的原控件 `hideNoColor:true`，不提供无颜色选项。Wave 的两类方向使用现有原版 CW/CCW、Left/Right 图标；颜色等参数保存在各效果的缓存中，切换不丢弃。
- Tidal 的方向区按原 `direction mt10` 紧接双色/随机颜色行；Wave/Wheel 参数区保留 `mt20`。快速/高级效果和方向按钮的圆角与其宽高一起使用相对缩放，避免改变字体缩放后内外圆角比例失配。
- 帮助入口使用每款 DeviceInfo 的 supportPage/masterGuideURL，语言后缀按原函数转小写。保留支持、恢复出厂、序列号、注册和条件固件区域；恢复出厂仍等待设备服务。

## 产品入口与资源

设置的“本地工作区”提供七个明确预览按钮，命令行也接受 `--preview-product <pid> --tab lighting`。构造使用真实产品 ID 和 `PREVIEW-<pid>` 本地标记，不继承合成键盘的输入点、布局、固件或窗口句柄。顶部标签使用原 `MOUSEMAT.svg`。

[采集工具](../../tools/fetch-razer-product-images.py) 静态解析 Webpack 请求映射，下载收据记录 URL、HTTP 状态、长度和 SHA-256。34 个原产品图请求、17 个 PluginImages Dashboard 图及 1 个品类图标均已取得；[准备工具](../../tools/prepare-resources.py) 沿现有转换/嵌入路径生成。17 个确认有产品图映射的 edition 为 3072：0/1；3073：0；3074：0/128/129/130/132/133；3076/3077/3078：0；3080：0/128/129/130/131。目录中更广的 edition 列表不作为图片存在证据。

## 验证与剩余范围

[回归用例](../../src/features/mouse_mat_tests.rs)覆盖新建配置默认值、编辑配置保存往返、产品实例能力、实际亮度/关闭灯光控件、两类方向和 Help 往返、Reactive 提示及参数保留；新增七款产品图区域的挂载、250px 高度、20px 图文区域间距、缩放和窄窗口水平滚动检查。数据层另覆盖效果归一化和旧配置迁移。只编译测试，不运行测试、应用或服务；新增用例并不构成实际图片解码或窗口验收结果。

本次静态复核逐一读取七款 module 8193，检查 QUICK_EFFECTS 顺序、DEFAULTPROFILE、DeviceInfo、Wave/Tidal 初始方向、Reactive 判断和 SwitchOffLighting；并对照共享 `main.93c574c8.css` 的 `.warning`、`.toggle-btn`、`.twoway-lighting` 和配置工具栏规则。新发现并修正的是上述提示图标、Tidal 间距与缩放圆角；这些检查不替代实际窗口验收。

资源来源校验与全目标编译通过。尚未动态验收窗口、滚动、拾色器与动画；设备亮度/灯效写入、Chroma 接管与同步、兼容鼠标实时列表、WDL 及特殊模式仍须接通和核对。其余产品和独立应用继续按总清单推进，不能用本页代表全部界面完成。
