# 产品独立 Armory 页面接入（第一批）

本轮在 `src/features/armory_product.rs` 实现独立产品内容，并在 `SourceProductWorkspace::supports_mapping_page / mapping_page_element` 优先选择该内容。不是把 Customize 页面换个入口名称。

当前源的完整 root condition、词法作用域解析、render 函数、CSS、图片导入表和初始化值见 [静态证据](armory-product-roots-current-evidence.json)。由 `tools/audit-armory-product-roots.cjs` 读取每个产品自己的当前 manifest、JS 与 CSS，不执行下载代码。

| 产品 | 源调用关系 | 本地行为 |
| --- | --- | --- |
| 1303 Nommo Chroma | `jg → Xg → Ul → Ll` | 独立产品图及点阵，250高，margin10 auto，max1220，Armory取消原min1024 |
| 1304 Nommo Pro | `Qf → qf → eR → Zl` | 使用该产品自己的同类独立产品图根，无声音/灯光编辑器 |
| 1313 Kraken Kitty Edition | `IK → lK → Sy → ly` | 使用该产品自己的同类独立产品图根，无Customize表单 |
| 3893 Hanbo | `HH → fH → fg` | 产品图200高，下方CPU、冷却、GPU三组读数；C/F直接切换。没有把requestedFanMode/requestedPumpMode编辑器当成该root |
| 3894 PWM控制器 | `$P → AsyncRouter → lazy 7855/default g → m` | 仅实际category为ACCESSORY/MOUSEMAT时显示独立产品图；DEFAULT对应另一模块3288，未冒用同一个图页 |

上面五个产品现在经现有 `has_armory_device_page` 能力检查直接打开独立root，其中3894保留category条件。本批没有为其他产品推断同样的root。正常主导航的音频与配件内容没有改变。

## Hanbo 数据与布局

`fg` 读取 `temperatureReducer.isCelsius` 和 `elsaReducer.runtimeData`。源码 `BC.runtimeData` 明确以 cpuTemp/cpuSpeed/fanSpeed/pumpSpeed/liquidTemp/gpuTemp/gpuSpeed 全0、isHardwareOutdated=false 初始化。本地保存这一初始化界面；这些零不是硬件实测值，也不是驱动回应。尚未接入真实遥测更新和硬件过旧警告状态。

每个数字块padding20、label14大写；value为RazerF5 30，温度宽118、GHZ宽103、RPM宽141。三组为#111背景、#707070一像素边框、圆角5，组内分隔线1。C/F按原公式 `1.8 * value + 32`、华氏保留1位小数，选中底#707070/字#111。原CSS未为这里指定动画，未添加任意过渡。

宽屏读数顺序为CPU、风扇/泵/冷液、GPU；viewport<=1279时，`.CoolerInfo_info` max1000、wrap、gap10×20、center，第二组order1，所以本地显示顺序为CPU、GPU、冷却。这里是数字读数组件，没有根据“仪表”一词猜画圆盘。

当前实现使用产品实例的container id隔离本地摄氏/华氏状态。`specialAudio===true` 的390高分支、paired mouse附图及customDotPattern并未由本地未知能力值触发；这些条件不被硬编码为已支持。

## 产品图片缺口

图片只允许来自对应产品 `img_prods/prd` 的Webpack导入，绝不替换为PluginImages Dashboard缩略图。`resources::device_image` 可以在精确原图完成准备后提供产品图；原图未准备时保留空图区域及原点阵，不能把它称为完整视觉复刻。

通过维护工具 `fetch-razer-product-images.py --products 1303 1304 1313 3893` 静态解析了21个原URL（含关联Dashboard和favicon请求），本次普通权限获取均network_error；没有提权、没有替代图、没有新增猜测图。关键prd-3x源：

- 1303 edition0：`prd-3x.230dc93d.avif`。
- 1304 edition0：`prd-3x.530bfa78.avif`。
- 1313 edition0/128：两个当前导入分别指向`prd-3x.30e8b41c.avif`与`prd-3x.d478cac1.avif`；精确edition对应见证据的images记录。
- 3893 edition0/128：`prd-3x.e1b22559.avif`与`prd-3x.a3f7d8f1.avif`；精确edition对应同上。
- 3894：`prd-3x.5c35b579.avif`；其后单独获取3894的4个源URL也均network_error。

这些仍是资源缺口。没有新增可供注册的位图/SVG，所以不存在需要伪造的增量资源manifest。

## 继续接入的原始证据

3894 PWM控制器root `$P` 按类型通过 AsyncRouter 和 lazy module7855选择内容。本轮继续追到当前main.be5b5e43.js内7855的default g→connect(m)，确认是产品图，才按ACCESSORY/MOUSEMAT条件开通；DEFAULT的3288仍不在此映射范围。3907 Cooling Pad的`TU → rU → sU`包含专用配置图、CPU/GPU、风扇图表等31个追踪到的renderer；当前普通Cooling设置页不能代表该独立root，尚未开通，证据已提取供下一批使用。

本子任务只运行静态Acorn/CSS解析、资源获取及哈希核验；没有运行应用、构建、测试或cargo。Rust格式化和唯一允许的`cargo check --locked --all-targets`由主任务统一执行，结果由主任务记录。
