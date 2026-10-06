# 当前源码复刻继续记录（2026-10-06）

本轮仍以当前 Dashboard、host 4.0.827 及各产品 manifest 声明的源文件为依据。
执行范围限于源码解析、资源获取/转换/校验、格式化和
`cargo check --locked --all-targets`。没有运行应用、构建、测试、安装器或厂商代码。

## 已接入的界面与条件

- [Nommo 1303/1304](nommo-effects-native.md)：灯光页右列快速/高级效果、六种灯效
  参数、颜色弹层、方向切换及对应过渡。Studio 的真正目标仍未实现，按钮保持不可用，
  不把 Chroma Dashboard 误作 Studio，也不显示下载/安装门控。
- [四款相机](camera-presentation-current-audit.md)：首批 58 个分组、45 个折叠区域、
  31 项帮助和 3 项条件警告；修正 3594、3595 的质量控件挂载，补回 3592 PROCESSING
  的分辨率选择。[提示层级](camera-tooltip-layer-current-audit.md)按原始悬停状态在
  200/9999 之间即时切换，原淡入和定位过渡保持各自时序。
  后续预览区域在 3594/3595/3596 增加原本挂载的来源、Camo、眼睛切换和关闭提示，
  总计增至 64 个分组、51 个折叠区域、34 项帮助；3592 CAMERA 原本没有该组件，
  不补造。3594/3595 的 Camo 入口连接本产品 HELP，3596 使用原外部链接。
- [第一批 Armory 独立根](armory-audio-entrypoints-2026-10-06.md)与
  [第二批](armory-audio-next-roots-2026-10-06.md)：新增 1319、1325、1328、1330、
  1331、1332、1335。1335 使用其本包实际 `specialAudio` 条件的 390px 图框，不能
  沿用普通 250px 外框。包括原有根，共补齐 13 款产品、21 个产品/edition 身份的
  41 张精确原图/Dashboard 图；不通过相近产品图代替缺失资源。
- AppPicker 以本地可打开能力显示已实现入口，移除虚构的 Chroma 安装记录。
  有可显示内容时不追加安装状态提示；原本未实现的目标仍不冒充已实现。
- [设备入口](local-device-entrypoints-2026-10-06.md)：以实际持有的 renderer 和非帮助页
  判定本地能力，解除六种安装阶段对卡片/选择器的门控。保留 setupStatus 原值和最低
  固件、待机/断电、主机模式、Mixer 错误、预设加载、更新/重启等真实条件。
- [公共滑条](source-slider-current-audit.md)：为已核验的源 CSS 恢复 300ms
  透明度和滑柄背景过渡，保留即时禁用、即时边框及 Nommo 关闭亮度仍能拖动的特例。
- [步进箭头](stepper-visibility-current-audit.md)：Nommo 恢复 hover/focus 显示与
  100ms linear 透明度变化；通过 CSS 优先级核对保留四款相机的常显覆盖。
- [1383 Kraken V4 Pro OLED](audio-oled-home-native.md)：按自身 reducer 初值补回七张 Home Screen Display 卡片，
  支持本地模式选择、关闭遮罩和 Media 编辑草稿。其他编辑器未用相近页面替代。
  静态恢复 27 项素材，校验灰度像素与动画帧时长；预览中的系统数据来自源码初值，
  不是本机遥测。补入当前主机原版 RazerF5 SemiBold 600，避免使用合成字重。

## 先前批次验证（七卡和 Media 阶段）

- `cargo fmt --all -- --check` 与 `cargo check --locked --all-targets` 通过。
  后者仅保留 `layer_button`、`select_profile`、`set_monitor_runtime` 三项既有未使用方法警告。
- 统一资源校验通过：1257 项共享资源源/输出哈希、35 项服务 SVG、27 项 OLED 素材；
  同时检查 133 项 Webpack 请求、74 种基础产品图、59 种基础 Dashboard 图及
  16 个键盘布局的 1901 个输入形状。Armory 专用表另核验 41 张图、21 个产品/edition 身份。
- 原生产品数据校验通过；嵌入 JSON 校验为 41 份、0 失败、4 项既有结构推断跳过。
  这不表示跳过的结构也经过同一 schema 校验。
- 相机挂载/条件/提示层级、设备直达入口、Dashboard 状态、Nommo、Armory、OLED、
  滑条和步进箭头均有对应的当前源码静态收据。没有进行运行时像素或交互验收。

## 静态校验发现并修复的数据串接错误

`accessory_controls_data.json` 中产品 **126** 的 `TAB_LIGHTING` 首段曾错误包含
`179:hyperpolling-pairing`，源码也指向产品 **179**，绑定的
`/runtime/indicatorLedStatus` 在 126 配置中不存在。该错误使 Razer Mouse Dock 的灯光页出现
接收器配对入口，并导致原有数据校验失败。

当前 126 的 `main.766604be.js`（SHA-256
`c9043f49ba6f2dc94f46e6839e19c7246112e6177e8d3493ee6fd4904bb0a135`）
中真实灯光根 `Wa` 位于 UTF-16 区间 `[3913666,3913924)`：左列是 `Va/Ba`，
右列是 `H/fa`。它与 179 的接收器根是不同组件树；126 配置和原生成器均没有这条
179 配对 action。已移除误插的整段，179 仍由自己的专用 receiver 页实现配对。

`validate-native-product-data.py` 现在额外要求每条控件 key 的产品前缀和已声明
source 路径归属其自身产品。没有通过放宽控件种类或允许缺失状态路径掩盖错误。

## 完成边界

上述是明确的局部实现范围。原有 331 个产品入口、1419 个导航页的“部分接入”
统计不能据此改成全产品完成。剩余产品内部控件、独立根、服务观察态及交互细节
继续按源码逐项处理。运行窗口、像素、DPI、焦点和真实设备响应均未验收。

## OLED 编辑器后续并行批次

继续以1383自己的当前源码补齐五个编辑器，现有六个可编辑模式均可从EDIT打开。
补入固定预设槽、104个表情、本地裁剪预览、横幅四方向和字体选项、系统信息三张幻灯片，
并完成共享弹层、null状态恢复和“需要Synapse”提示8px边界处理。
资源增加到四套共176项；具体交互、来源和剩余边界见
[OLED编辑器续接](audio-oled-editors-2026-10-06.md)。上面的27项计数是先前批次的验证记录。

这一批的 `cargo check --locked --all-targets`、格式化检查、源码/资源只读校验均通过，
保留相同三项既有Rust警告。嵌入JSON增加到45份、0失败、4项原有跳过。横幅textarea默认
样式通过当前官方宿主的浏览器版本字符串与匹配的上游UA CSS恢复，不再随意设定其padding。
上述通过仍不表示应用已运行或全产品复刻已完成。
