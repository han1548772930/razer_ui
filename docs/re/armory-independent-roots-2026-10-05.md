# 产品独立 Armory 页面接入（含 2026-10-05 续接审计）

本轮在 `src/features/armory_product.rs` 实现独立产品内容，并在 `SourceProductWorkspace::supports_mapping_page / mapping_page_element` 优先选择该内容。不是把 Customize 页面换个入口名称。

当前源的完整 root condition、词法作用域解析、render 函数、CSS、图片导入表和初始化值见 [静态证据](armory-product-roots-current-evidence.json)。由 `tools/audit-armory-product-roots.cjs` 读取每个产品自己的当前 manifest、JS 与 CSS，不执行下载代码。

| 产品 | 源调用关系 | 本地行为 |
| --- | --- | --- |
| 1303 Nommo Chroma | `jg → Xg → Ul → Ll` | 独立产品图及点阵，250高，margin10 auto，max1220，Armory取消原min1024 |
| 1304 Nommo Pro | `Qf → qf → eR → Zl` | 使用该产品自己的同类独立产品图根，无声音/灯光编辑器 |
| 1313 Kraken Kitty Edition | `IK → lK → Sy → ly` | 使用该产品自己的同类独立产品图根，无Customize表单 |
| 3893 Hanbo | `HH → fH → fg` | 产品图200高，下方CPU、冷却、GPU三组读数；C/F直接切换。没有把requestedFanMode/requestedPumpMode编辑器当成该root |
| 3894 Head Cushion Chroma | `$P → AsyncRouter → lazy 7855/default g → m` | 当前 `8193.DeviceInfo.category` 固定为 ACCESSORY；直接选择产品图。DEFAULT 的 3288 为灯光编辑根，但在此产品当前源中不可达 |
| 3907 Laptop Cooling Pad | `TU → rU → sU/nU → um/cm` | 专用 CPU/GPU 信息、配置图区域/连线/三按钮、Smart 曲线；当前无遥测时显示 Not detected 和破折号，不制造零读数 |

上面六个产品现在经现有 `has_armory_device_page` 能力检查直接打开独立 root。3894 原先错误地依赖本地发现记录的 category；已改为尊重当前 bundle 的固定类别。这里曾将 3894 写成 PWM 控制器，现根据该产品 `DeviceInfo.masterGuideURL` 和 `DEVICE_NAME` 纠正；PWM 对应的其他产品不能用作此处证据。

## Hanbo 数据与布局

`fg` 读取 `temperatureReducer.isCelsius` 和 `elsaReducer.runtimeData`。源码 `BC.runtimeData` 明确以 cpuTemp/cpuSpeed/fanSpeed/pumpSpeed/liquidTemp/gpuTemp/gpuSpeed 全0、isHardwareOutdated=false 初始化。本地保存这一初始化界面；这些零不是硬件实测值，也不是驱动回应。尚未接入真实遥测更新和硬件过旧警告状态。

每个数字块padding20、label14大写；value为RazerF5 30，温度宽118、GHZ宽103、RPM宽141。三组为#111背景、#707070一像素边框、圆角5，组内分隔线1。C/F按原公式 `1.8 * value + 32`、华氏保留1位小数，选中底#707070/字#111。原CSS未为这里指定动画，未添加任意过渡。

宽屏读数顺序为CPU、风扇/泵/冷液、GPU；viewport<=1279时，`.CoolerInfo_info` max1000、wrap、gap10×20、center，第二组order1，所以本地显示顺序为CPU、GPU、冷却。这里是数字读数组件，没有根据“仪表”一词猜画圆盘。

当前实现使用产品实例的container id隔离本地摄氏/华氏状态。`specialAudio===true` 的390高分支、paired mouse附图及customDotPattern并未由本地未知能力值触发；这些条件不被硬编码为已支持。

## 产品图片缺口

图片只允许来自对应产品 `img_prods/prd` 的Webpack导入，绝不替换为PluginImages Dashboard缩略图。`resources::device_image` 可以在精确原图完成准备后提供产品图；原图未准备时保留空图区域及原点阵，不能把它称为完整视觉复刻。

第一批维护工具读取 21 个原 URL 时均遇到 network_error。续接时用 `fetch-razer-product-images.py --products 1303 1304 1313 3893 3894 3907 --timeout 5 --attempts 1 --retry-errors` 重试 29 个源 URL，结果为 28 个 network_error、1 个已有缓存，所需产品图仍缺失。关键 prd-3x 源：

- 1303 edition0：`prd-3x.230dc93d.avif`。
- 1304 edition0：`prd-3x.530bfa78.avif`。
- 1313 edition0/128：两个当前导入分别指向`prd-3x.30e8b41c.avif`与`prd-3x.d478cac1.avif`；精确edition对应见证据的images记录。
- 3893 edition0/128：`prd-3x.e1b22559.avif`与`prd-3x.a3f7d8f1.avif`；精确edition对应同上。
- 3894：`prd-3x.5c35b579.avif`；其后单独获取3894的4个源URL也均network_error。
- 3907：`prd-3x.3e1ae619.avif`，1x 为 `prd-1x.d1e9a95b.avif`。

这些仍是资源缺口。当前 `.ref/devices`、`.ref/applications`、`assets` 的同指纹文件检索没有找到对应原图。现有 AVIF 的文件名指纹不等于原始 AVIF 字节 MD4，不能冒用 SVG 的 MD4 恢复方法给 AVIF 证明身份。

本轮另提取了四个当前内联 SVG（CPU、GPU、复位、删除节点），并按当前 CSS 的 `warning.ad3f47f8.svg` 内容指纹从当前 Macro 缓存恢复一个警告图。五个 SVG 已追加资源 manifest 和 embedded 注册；原图仍没有替换为 Dashboard 缩略图。证据见 [小图标收据](armory-cooling-icons-current-evidence.json)，维护命令为 `node --openssl-legacy-provider tools/extract-armory-cooling-icons.cjs --check`。随后另准备了 10 个语言的轴标题与两个单位文字轮廓，见下节。

为下载缺失原图而请求网络提权时，自动审批系统因本地 API 不支持其审批模型 `gpt-5.6-luna` 返回 404，操作未执行。这是审批流程故障，不是不安全判定；没有绕过审批。

## 继续接入的原始证据

补充证据见 [剩余根审计](armory-remaining-roots-current-evidence.json)，由 `tools/audit-armory-remaining-roots.cjs` 静态读取两款产品各自当前 manifest、模块作用域、源组件、初始化值与 CSS。

3894 的 `3288.default ba → Fa` 会渲染 ProductImage、Brightness、Switch Off Lighting 和 Effects 三组控件，确实不是同一个图页。但此产品自身 `8193.DeviceInfo.category="ACCESSORY"`，`$P` 总是选择 7855；不为不可达的 DEFAULT 制造页面。

3907 已在 `src/features/armory_product/cooling_pad.rs` 独立接入。`Dn.runtimeData` 的 initStatus 和所有指标均 undefined；`am` 挂载后据此令 displayWarning=true，并把 systemMode 设为 none。因此 CPU/GPU 为 Not detected/破折号，性能模式组件 Rm 不出现。一级 Fixed/Smart 切换禁用；Smart 次级模式、单位、CPU/GPU、开关和复位仍可用。没有把普通 Cooling 页复制过来。

本地曲线按 Rn/An/Sn/ln 保存 CPU/GPU 各 Quiet/Balanced/Performance 三份草稿。默认 CPU Balanced 为 `(40,1000),(90,2200),(100,2200)`；图表温度跨度 60°C、500..3200 RPM、高 380，13 条温度网格和 5 条转速网格。节点可选择、在曲线上新增（最多 20）、垂直拖动（相邻节点单调约束）、删除（至少 2）；更改未发送硬件。百分比保持源 15.625% 下界及中间值一位小数规则。

速度单位轴现已恢复源 270° 旋转方向。GPUI 0.3.7 的 `Svg::with_transformation` 明确只旋转绘制、不旋转命中框，`Div` 和普通文字没有对应旋转 API。因此用当前 Roboto 字形轮廓静态准备旋转文字，再按变换后的真实矩形布置原生 %/RPM 按钮；无需错置的横排命中区或鼠标坐标补丁。按钮顺序、36 高源 pill、12/14 字号、长标题 lp 偏移与旋转中心均来自当前 CSS/JS。

`tools/prepare-armory-cooling-axis.py` 通过现有 resource-env 的 fontTools 读取真实字体轮廓和 GPOS kern，不执行应用；10 个 locale 的标题加两个单位文字共 12 个 SVG 已注册，文本与字体 SHA-256、字号、advance 和旋转收据见 [轴资源](armory-cooling-axis-resources.json)。Roboto 缺失的 CJK/俄文字形使用收据中明确命名的本机 Windows 字体；并未声称这些必然等于浏览器在所有机器上的 fallback，不分发整份系统字体。指针拖动另按已核对的 `Window::on_mouse_event` paint-phase API 捕获 move/up，所以移出图表仍按上下边界和相邻节点钳制。

CSS 审计还确认：Armory rU 没有挂载 `#coolingPerformance` 或 `.body-widgets`，相关普通页面的后代选择器不能用于这个根。实现保留通用 770×340 配置块，不擅自套普通 Cooling 的三列 flex。

## 仍需补齐

- 产品位图仍缺；原版加载后的图片自然尺寸与最终覆盖关系未能验证。
- 旋转轴已接入；缺失 Roboto 字形的系统 fallback 身份与浏览器的最终回退选择仍未做运行时对比。
- 统计表用原尺寸约束的原生行布局，尚未逐像素验证原 HTML table 的自动列宽；警告和节点 tooltip 的定位/过渡仍有差异。
- 实时遥测、固件过旧警告、已连接系统的性能/Hyperboost 状态、Fixed RPM 可达状态与远程映射尚未接入。不能把无遥测分支称为全部运行状态都已完成。

本子任务只运行静态 Acorn/CSS 解析、资源获取及哈希核验、Rust 格式化；没有运行应用、构建、测试或 cargo。唯一允许的 `cargo check --locked --all-targets` 由主任务统一执行，结果由主任务记录。
