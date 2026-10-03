# 鼠标与键盘原生工作区复核（2026-10-03）

本轮只依据 `.ref/devices/<productId>/` 的当前静态源码。没有读取停用目录，没有执行厂商 JavaScript、应用、构建、测试、安装器或 DLL。数据准备程序只解析 AST、JSON、SVG 和图片。复核不代表全产品界面已完成。

## 已补齐的具体内容

| 范围 | 本轮接入 |
| --- | --- |
| 鼠标输入 | 74 个通用工作区产品的实际 `groupList`，共 854 个有 `inputID` 的输入；源顺序、视图分组、禁用状态、鼠标映射类别限制与默认恢复。无 `inputID` 的滚轮模式说明不计为输入。182 仍使用原有专用工作区。 |
| 鼠标图片 | 75 个规格的默认产品图全部准备完成，其中182供原有实现使用。补上同步 context、SVG 图片和231直接导入的 `topView` 分支。非默认版本、附件侧板和非顶部视图插图仍需逐分支准备。 |
| 鼠标联动 | 8 个 `smart_preset` 产品的非对称距离滑块按实际三组值切换：着陆/抬升为1/2、1/3、2/3；关闭独立 XY 时同步 Y=X；切换/恢复配置清除按键选择并补入缺失的源码默认字段。 |
| 键盘几何 | 71 个键盘/小键盘的默认布局，6,494 个原始路径/圆角/圆形区域及71张产品图；复用原生 `KeyRegion` 绘制与精确命中。717 两个实体 FN 区域共享逻辑输入，使用几何坐标给原生子树独立标识。 |
| 键盘控件 | 52 个实际追到游戏模式组件的产品，接入开关、仅在游戏中启用、Windows 键禁用和条件 Alt+Tab/Alt+F4；15 个电源页接入实际的调暗与睡眠控件。 |
| 键盘电源范围 | 7个旧型为15–60分钟；574/2595的睡眠为1–15分钟；6个新型使用配置中1、3、5、10、15分钟离散值。15个产品的默认值来自实际挂载 reducer，而非根据产品名或导航名推断。 |
| 键盘映射 | 当前有限的键盘键映射补上源 `KeyInput`/`AnalogInput` 输入描述，按目标输入 ID 保持控件身份；恢复配置时同步保留控件。 |

所有更改仍是本地配置草稿；保存到本机不代表设备已接收参数。

## 可复现的维护入口

`tools/extract-keyboard-products.cjs` 支持 `--mouse`，从当前 bundle 的配置和实际默认 `groupList` 导出输入。鼠标常见默认分组模块为1368，新型为21368；工具按存在的已知模块选择。105的源码明确复用104的 `DeviceInfo`，仍保留105自身目录和文件哈希。包含非字面量 context 的配置只提取可验证字段，不执行这些函数。

键盘页面扫描修正了 `connect(...)(withWrapper(Component))` 的嵌套组件追踪；`extract-keyboard-lazy-evidence.cjs` 另从导航 `bind(require,moduleId)` 解析691的5个懒加载页面默认导出。整包 webpack 对象不算组件，静态校验明确拒绝这类节点。尚未解决的跨模块组件继续保持缺口，不根据 bundle 中存在词汇就展示控件。

可依次运行：

```text
node tools/extract-keyboard-products.cjs
node tools/extract-keyboard-products.cjs --mouse
node tools/extract-keyboard-page-evidence.cjs
node tools/extract-keyboard-lazy-evidence.cjs
python tools/prepare-keyboard-artwork.py
node tools/extract-keyboard-svg-map.cjs
python tools/prepare-keyboard-products.py
python tools/prepare-mouse-assets.py
python tools/prepare-mouse-products.py
python tools/validate-mouse-keyboard-products.py
```

图片/几何准备需要 Pillow 和 fontTools。本次使用已有 `.work/resource-env/Scripts/python.exe`。全局资源嵌入由主任务统一处理，不能根据 `.png` 猜测后缀；鼠标以 `mouse-product-assets.json` 的 `output`、键盘以 `keyboard-product-manifest.json` 为准。源按键/config审计JSON不应作为图片嵌入。

231没有共享图片 context：当前 module560 的 `jm` 导出索引0，经过配置 `topView` 选择 `topview.da4ca3f5.avif`。该分支在准备工具中保留明确断言，源码改变时需要重新核对。

## 仍需完成

鼠标的完整映射抽屉/映射类别、表面鼠标垫管理、动态灵敏度曲线、配对、完整灯效参数及高级 Chroma、连接方式/固件条件和附件分支仍未完整实现。鼠标配对导航已不再静默停留于上一页，但真实配对界面仍待接入。

键盘默认布局之外的地区布局与版本、完整映射编辑器、其余 Customize 控件（包括回报率、Snap Tap、Command Dial）、Actuation、Calibration、OLED、Pairing，以及灯效参数/高级 Chroma 仍需继续。文件中存在页面标题或已解析到源组件，均不计为这些界面已完成。

`cargo check --locked --all-targets` 已通过；另完成 JSON、源文件 SHA-256、资源存在、几何有效性、物理区域身份、范围/默认值和整包节点排除检查。没有运行应用，所以没有像素、字体、滚动、焦点或硬件往返验收。机器记录见 [mouse-keyboard-native-coverage.json](mouse-keyboard-native-coverage.json)。
