# 产品界面目录

| 目录 | 内容 |
| --- | --- |
| `mouse/` | 鼠标产品工作区、DPI、轮询率、自定义、校准、电源、灯光、滚轮和对应数据/测试 |
| `keyboard/` | 键盘产品工作区、按键映射、触发、Snap Tap、OLED、校准、电源、灯光和对应数据/测试 |
| `audio/` | 音频设备、麦克风、混音、OLED、Control Pod 和对应数据/测试 |
| `gamepad/` | 手柄、Kitsune、摇杆/扳机校准和对应数据/测试 |
| `system/` | 系统产品页面和源配置 |
| `accessories/` | 底座、灯带、ARGB、Hue、机箱风扇、Armory 和对应数据/测试 |
| `chroma/` | Chroma 产品界面、Studio 编辑器和对应数据 |

跨产品的工作区、配置文件、共享映射、帮助及服务入口留在本层。已有独立能力子目录保持其内部边界。

产品工作区入口使用各目录的 `mod.rs`。本层 `mod.rs` 保留已有 Rust 模块名称与可见性，例如 `features::mouse_products`、`features::keyboard_products`，避免为了文件分类改变外部调用和状态所有权。目录归类不代表该产品全部页面或设备写回已完成。

产品文件写入对应目录。源码证据、实现和完整页面验收按 [完整实现路线](../../../../docs/re/ui-readonly-first-roadmap.md) 核对。
