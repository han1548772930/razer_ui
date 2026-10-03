# 十个原有适配器的逐页复核

> 覆盖审计里原来把十个 `existing_partial_native_adapter` 产品的 27 个主页面整体记为 `existing_partial_not_reaudited_here`。本轮按页复核了这 27 页：每一页现在都带本地路由与当前源码依据，状态改为 `partial_native_reaudited`。复核结论由 [audit-native-product-coverage.py](../../tools/audit-native-product-coverage.py) 生成，落盘在 [native-product-coverage.json](native-product-coverage.json)（每页 `evidence` 字段）。

## 复核范围内涵

写进依据的只有三类已核对事实：

1. 本地渲染位置（文件与函数）；
2. 该页在该产品上的可达性（`src/nav.rs` 的 `Tab::for_product` 分支）；
3. 该产品存在哪些当前源码数据（规格文件、资源清单或产品包常量）。

**没有**写进依据的：页面内部逐字段的数据来源、视觉一致性、交互与硬件行为。`partial_native_reaudited` 仍然只表示「有部分原生内容」，不等于完成，也不改变「没有产品被标记为全部完成」。

## 逐页结果

| 产品 | 页面 | 本地路由 | 依据 |
| --- | --- | --- | --- |
| 182 DeathAdder V3 Pro | TAB_CUSTOMIZE | `device_customize` | `customize_page.rs`；`nav.rs:29` 可达；规格 `mouse_products.rs:128`（`mouse_products_data.json` 带 `source_sha256`） |
| 182 | TAB_PERFORMANCE | `device_performance` | `device_pages.rs:34`；`nav.rs:30` 可达；规格里有 DPI/回报率字段 |
| 182 | TAB_POWER | `device_power` | `device_pages.rs:405`；`nav.rs:32` 可达；规格里有 `power_slider`／`low_power_slider`／`low_battery_slider` |
| 182 | TAB_CALIBRATION | `device_calibration` | `device_pages.rs:189`；`nav.rs:33` 可达；规格里有 `calibration`／`smart_lift_max`／`smart_landing_max` |
| 182 | HELP | `device_help` | `help_page.rs:111`；`help_page.rs:21` 给出 182 自己的 `DeviceInfo` 链接 |
| 653 Blackwidow V4 Pro | TAB_CUSTOMIZE | `keyboard_customize` | `nav.rs:35` 可达；布局数据 `resources.rs:176` 的 `keyboard-653-layouts.json` 与 `keyboard-653-deviceconfig.json` |
| 653 | TAB_LIGHTING | `device_lighting` | `device_pages.rs:472`；`nav.rs:35` 可达；效果表 `settings.rs:377`／`:380` |
| 653 | HELP | `device_help` | 同 182 的自有链接 |
| 777 Kraken BT Sanrio | TAB_SOUND / TAB_MIC | `device_audio` | `audio_page.rs`；`nav.rs:36` 可达；`audio.rs` 记录三份已下载设备模块都没有声明 `TAB_AUDIO` |
| 777 | TAB_LIGHTING | `device_lighting` | `device_pages.rs:472`；效果表 `settings.rs:384`／`:387` |
| 777 | TAB_POWER | `device_power` | `device_pages.rs:405`；`nav.rs:36` 可达 |
| 777 | HELP | `device_help` | 同 182 的自有链接 |
| 3072 / 3073 / 3074 / 3076 / 3077 / 3078 / 3080 | TAB_LIGHTING | `mousemat_lighting` | `device_pages.rs:472`；`nav.rs:37` 只给出 Lighting；数据 `product.rs:10`／`:59`（模块 9228 方向常量）与 `settings.rs:374` 的逐产品效果表 |
| 上述七个 | HELP | `device_help` | `help_page.rs:111`；`support_links` 走 `audited_mouse_mat` 的 support/guide 前缀 |

## 仍然存在的缺口（不因本轮复核而消失）

- 十个产品都没有进入 `source_help_data.json` 描述符，Help 走的是本地 `help_page`，其逐页条件（序列号、注册、固件、重置、系统信息等）覆盖度不等同于其余 321 个产品的描述符路径。
- 182 的 `mouse_products_data.json` 条目的 `pages` 为空数组：规格数据齐全，但页面清单仍由适配器决定，没有迁到 family 描述符路径。
- 653 的布局/灯光、777 的音频与灯光同样依赖各自的手写页面，没有纳入 family 描述符的 `pages` 断言。
- 设备页之外的界面缺口（托盘、OLED、宿主服务、跨平台）见 [当前完成状态](21-ui-completion-status.md)。

重新生成：`python -X utf8 tools/audit-native-product-coverage.py`；该命令只解析本地数据与 Rust 源码，不运行应用、构建、测试或下载物。
