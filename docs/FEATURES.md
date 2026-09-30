# 雷云 4 功能清单

> **本文已被重写。** 旧版大量条目是从日志关键词与本机实测「推断」出来的，
> 其中不少与实际不符。现在前端本体已经拿到，功能清单以**雷云自己的代码与文案**为准。
>
> - 完整功能全表（4992 条）→ [`SYNAPSE-FEATURES-FULL.md`](SYNAPSE-FEATURES-FULL.md)
> - 前端技术栈与布局 → [`SYNAPSE-UI.md`](SYNAPSE-UI.md)

---

## 0. 证据分级

| 标记 | 含义 | 可信度 |
|---|---|---|
| `[前端]` | 抓自 `apps.razer.com` 的前端本体（HTML/JS/CSS） | 最高 |
| `[文案]` | 前端语言包里的原文（4992 条） | 最高 |
| `[清单]` | `asset-manifest.json` 里的模块名 | 最高 |
| `[IPC]` | 雷云 Electron 的 IPC 契约（本地 asar） | 高 |
| `[DLL]` | 命令行逐个加载引擎 DLL 并统计符号 | 高 |
| `[实测]` | 本机日志 / 设备上报的真实数据 | 高 |

**不再使用**「推断」类条目作为功能依据。

---

## 1. 功能来源：前端本体

| 项 | 值 | 证据 |
|---|---|---|
| 入口 | `https://apps.razer.com/synapse/dashboard/` | `[前端]` |
| 应用版本 | `0.0.86` | `[前端]` |
| git 提交 | `4073c224f2f452aa18e3128b9c7ee82b94e6ffc3` | `[前端]` |
| 技术栈 | React 18.2.0 + react-redux 8.0.5 + Sentry + pkijs | `[前端]` |
| 内部代号 | `project_anne_dashboard` | `[前端]` |
| 代码分块 | 127 个（9.4 MB），其中 89 个命名模块 | `[清单]` |
| 文案条目 | 4992 条中文 / 5014 条英文，1101 个命名空间 | `[文案]` |
| 语言 | 10 种（zh-CN / zh-TW / en / ja / kr / de / es / fr / ru / pt-BR） | `[前端]` |

---

## 2. 界面分区：标签页

### 2.1 标签页词汇表（25 个）

来源两处，已对齐 `[文案]` `[前端]`：

- 语言包 **23 个** `TAB_*`（都有中文原文）；
- 设备模块额外导出 `TAB_KEY_BINDS`、`TAB_MY_MACROS`
  （文案键 `TEXT_NAV_TAB_KEY_BINDS`=**按键绑定**、`TEXT_NAV_TAB_MY_MACROS`=**我的宏**）。

| key | 中文 | key | 中文 |
|---|---|---|---|
| `TAB_HOME` | 首页 | `TAB_POWER` | 电源 |
| `TAB_CUSTOMIZE` | 自定义 | `TAB_BATTERY` | 电池 |
| `TAB_KEY_BINDS` | 按键绑定 | `TAB_AUDIO` | 音频 |
| `TAB_MY_MACROS` | 我的宏 | `TAB_SOUND` | 声音 |
| `TAB_GAMING` | 游戏 | `TAB_MIC` | 麦克风 |
| `TAB_LIGHTING` | 灯光 | `TAB_MIXER` | 混音器 |
| `TAB_EFFECTS` | 效果 | `TAB_EQ` | 均衡器 |
| `TAB_COLOR` | 颜色 | `TAB_ENHANCEMENT` | 增强 |
| `TAB_CALIBRATION` | 校准 | `TAB_HAPTICS` | 触觉 |
| `TAB_PERFORMANCE` | 性能 | `TAB_OLED` | OLED |
| `TAB_DISPLAY` | 显示 | `TAB_PAIRING` | 正在配对 |
| `TAB_SCROLLING` | 滚动 | `TAB_SETTING` | 设置 |
| `TAB_DEMO` | 演示 | | |

### 2.2 设备实际显示哪些标签页 `[前端]`

**关键**：以上只是词汇表。某台设备显示哪几个标签页，**由该设备自己的模块决定**
（每个产品一份独立的 React 应用，见 [`SYNAPSE-UI.md`](SYNAPSE-UI.md) §2.5）。
已下载三份模块实测：

| 设备 | productId | 标签页（按模块内声明顺序） |
|---|---|---|
| DeathAdder V3 Pro（鼠标） | 182 | 自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
| BlackWidow V4 Pro（键盘） | 653 | 自定义 · 性能 · 灯光 · 电源 · 滚动 |
| KRAKEN BT SANRIO（耳机） | 777 | 自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

界面外壳是 `Header + DeviceList + Content`，外加独立的弹窗根 `IotPopupRoot` `[前端]`。

### 2.3 逐界面文档

每个界面的**布局**（UI 分区 + 布局类名）与**功能项**写在单独文件里：
**[`docs/screens/`](screens/README.md)**

| 界面 | key | 出现于 | 功能项 |
|---|---|---|---|
| [自定义](screens/01-customize.md) | `TAB_CUSTOMIZE` | 鼠标 · 键盘 · 耳机 | 192 |
| [性能](screens/02-performance.md) | `TAB_PERFORMANCE` | 鼠标 · 键盘 | 52 |
| [正在配对](screens/03-pairing.md) | `TAB_PAIRING` | 鼠标 | 68 |
| [校准](screens/04-calibration.md) | `TAB_CALIBRATION` | 鼠标 · 耳机 | 74 |
| [电源](screens/05-power.md) | `TAB_POWER` | 鼠标 · 键盘 · 耳机 | 90 |
| [滚动](screens/06-scrolling.md) | `TAB_SCROLLING` | 鼠标 · 键盘 | 55 |
| [灯光](screens/07-lighting.md) | `TAB_LIGHTING` | 键盘 · 耳机 | 131 |
| [声音](screens/08-sound.md) | `TAB_SOUND` | 耳机 | 264 |
| [麦克风](screens/09-mic.md) | `TAB_MIC` | 耳机 | 106 |

---

## 3. 按键动作全集

### 3.1 动作名（主包常量块）`[前端]`

雷云主包里有一串动作常量，顺序即菜单顺序：

```
LEFT_CLICK  RIGHT_CLICK  SCROLL_CLICK  SCROLL_UP  SCROLL_DOWN
SENSITIVITY_STAGE_UP  SENSITIVITY_STAGE_DOWN  MOUSE_BUTTON_4  MOUSE_BUTTON_5
SCROLL_MODE_SWITCH  LEFT_SENSITIVITY_CLUTCH  RIGHT_SENSITIVITY_CLUTCH
GLOBAL_SENSITIVITY_CLUTCH  DEFAULT  KEYBOARD_FUNCTION  DYNAMIC_KEY_STROKE
MOUSE_FUNCTION  JOYSTICK  ROLLER_FUNCTION  SWITCH_KEYMAP  SENSITIVITY
DEVICE_BRIGHTNESS  SCROLLING  MACRO  INTERDEVICE  SWITCH_LIGHTING_PROFILE
SWITCH_DEVICE_PROFILE  SWITCH_DEVICE_SENSITIVITY  CONTROLLER  CONTROLLER_V2
CONTROLLER_PLAYSTATION  SWITCH_PROFILE  SWITCH_LIGHTING  RAZER_HYPERSHIFT
LAUNCH_PROGRAM  MULTIMEDIA  DIAL_MULTI_FUNCTION  WINDOWS_SHORTCUT
TEXT_FUNCTION  DISABLE  NONE
```

真实中文见 [`SYNAPSE-FEATURES-FULL.md`](SYNAPSE-FEATURES-FULL.md)
（如 `LEFT_CLICK`=左键单击、`SCROLL_MODE_SWITCH`=滚动模式切换开关）。

已落到代码：`src/model.rs::BUTTON_ACTIONS`（41 项）。

### 3.2 动作编辑器（34 个）`[清单]`

每个动作有一份独立的配置界面模块：

| 模块 | 含义 | 模块 | 含义 |
|---|---|---|---|
| `MapDefault` | 默认 | `MapSwitchKeymap` | 切换按键映射 |
| `MapDisable` | 禁用 | `MapSwitchProfile` | 切换配置文件 |
| `MapMouse` | 鼠标功能 | `MapGlobalSwitchProfile` | 全局切换配置文件 |
| `MapKeyboard` | 键盘功能 | `MapHyper` | Hypershift |
| `MapKeyboardCombineMouse` | 键盘+鼠标 | `MapDynamicKeyStroke` | 动态键程 |
| `MapMacro` | 宏 | `MapPerfect180` | Perfect 180 |
| `MapMacroKey` | 宏按键 | `MapPerfect180GameList` | Perfect 180 游戏列表 |
| `MapMedia` | 多媒体 | `MapAppSpecific` | 按应用 |
| `MapLaunch` | 启动程序 | `MapInterDevice` | 设备交互 |
| `MapWindows` | Windows 快捷键 | `MapJoyStick` | 摇杆 |
| `MapText` | 文本功能 | `MapAudio` | 音频 |
| `MapSensitivity` | 灵敏度 | `MapAILauncher` | AI 启动器 |
| `MapGlobalSensitivity` | 全局灵敏度 | `MapControllerV2` | 手柄 V2 |
| `MapBrightness` | 亮度 | `MapControllerPlaystation` | PS 手柄 |
| `MapBrightnessGlobal` | 全局亮度 | `MapDialFunction` | 多功能旋钮 |
| `MapLighting` | 灯光 | `MapControlKnobFunction` | 控制旋钮 |
| `MapScrolling` | 滚动 | `MapRoller` | 滚轮功能 |

### 3.3 键位名常量 `[前端]`

雷云**前端代码**里定义了 **92 个** `KEY_*` 键位常量（从 V8 代码缓存提取），
例如 `KEY_BACKSPACE`、`KEY_CAPS_LOCK`、`KEY_APOSTROPHE`、`KEY_CLOSE_BRACKET`、
`KEY_APPLICATION`、`KEY_BEFORE_SPACE` 等。

> **注意**：这些常量只存在于**代码**中；**语言包里并没有它们的译文**
> （语言包的 `KEY_` 命名空间只有 7 条，且都是别的含义：`KEY_LIGHTS`=补光灯、
> `KEY_MENU`=菜单、`KEY_SHIFTER`=变调、`KEY_SELECTED`=已选按键…）。
> 因此 `KEY_A`、`KEY_F1`、`KEY_HENKAN` 这类名字**不在语言包里**，不能当作既有译文使用。
> 键位的显示名应由操作系统键名映射得到，这一点尚未确认。

---

## 4. 领域功能

完整清单（按命名空间，逐条含中英原文）见
[`SYNAPSE-FEATURES-FULL.md`](SYNAPSE-FEATURES-FULL.md)。各领域规模：

| 命名空间 | 条数 | 覆盖的功能 |
|---|---|---|
| `TEXT` | 153 | 文本功能（输入文本、快捷键串） |
| `AUDIO` | 91 | 音频模式、输出、省电、提示音 |
| `OLED` | 84 | 带 OLED 屏设备（动画、系统信息显示） |
| `PERFORMANCE` | 73 | 性能模式、屏幕刷新率、超频 |
| `MIC` | 55 | 麦克风、监听、降噪 |
| `LIGHTING` | 49 | Chroma 效果、亮度、键盘灯 |
| `CUSTOMIZE` | 48 | 按键映射、Hypershift、Snap Tap |
| `THX` | 48 | THX 空间音效 |
| `POWER` | 41 | 电源、省电、休眠 |
| `SCROLL` | 40 | 滚轮档位、自由滚动、触觉 |
| `DYNAMIC` | 38 | 动态键程（模拟光轴） |
| `CALIBRATION` | 35 | 表面校准、灵敏度匹配 |
| `BATTERY` | 30 | 电池、健康优化 |
| `MACRO` | 14 | 宏、录制、队列 |
| `CHROMA` | 25 | 幻彩控制室 / 互联 / SDK |
| `KEY` | 7 | 补光灯、菜单、变调、已选按键等 |
| `DEBOUNCE` | 4 | 回弹模式 |
| `BOSS` | 4 | 老板键 |

> 数字与 [`SYNAPSE-FEATURES-FULL.md`](SYNAPSE-FEATURES-FULL.md) 的命名空间总览一致，可复核。

---

## 5. 后端能力

### 5.1 IPC 契约 `[IPC]`

17 个通道，全量见 [`re/01-ipc-api-surface.md`](re/01-ipc-api-surface.md)。

### 5.2 原生引擎 DLL（方案 2 的依据）`[DLL]`

命令行逐进程加载并统计符号（`--probe <引擎名>`）：

| 引擎 | 结果 | 用途 |
|---|---|---|
| `lighting_driver_v1.9.14.0.dll` | ✅ 8/8 符号 | Chroma 灯光写出（`Configure(json)`） |
| `RzLightingEngineApi_v4.0.55.0.dll` | ✅ 12/12 符号 | Chroma 效果引擎 |
| `mapping_engine.dll` | ✅ 4/4 符号 | 按键映射 / 宏 |
| `simple_service.dll` | ✅ 7/7 符号 | 音频 / 进程 |
| `SysUtilsNative.dll` | ❌ **DllMain 永久阻塞** | 已列入黑名单 |

### 5.3 硬件协议 `[清单]`

30 个 `rzHardwareEvents*Parser` 模块，覆盖通用协议（Protocol25/30/40）、
键区与 OLED、手柄、以及各音频芯片（Avnera / NXP / Mxic / Valkyrie …）。

---

## 6. 设备数据模型 `[实测]`

本机实测两台真实设备（`src/fixtures/measured_devices.json`）：

| productId | 名称 | 类别 | 关键能力 |
|---|---|---|---|
| 182 | Razer 炼狱蝰蛇 V3 专业版 | Mouse | 电池 47%、DPI 100–30000 步进 50、支持 XY 独立、3 个可映射按键、Chroma=false |
| 179 | RAZER HYPERPOLLING 无线接收器 | Accessory | 单配置文件、Chroma=false |

DPI 档位模型（逐字对应上报 JSON）：

```json
"dpiStages": {
  "stages": [{ "x": 400, "y": 400, "independent": false, "visible": true }],
  "active": 3, "enable": false, "count": 5
}
```

子模块是**运行期下载**的：`products/{productId}/ui/{productId}_{edition}/`，
状态机 `waiting → downloading → ready`。

---

## 7. 实现状态与缺口

> **「已实现」的口径**：界面（布局按 `docs/screens/` 的实测布局）＋ 数据模型 ＋ 本地持久化。
> **硬件下发尚未接入**——方案 2 的引擎调用层已就绪（见 §7.1 最后三行），
> 但还没有把界面上的改动转成 `Configure(json)` 发出去。

### 7.1 已实现

| 项 | 位置 |
|---|---|
| 25 个真实标签页词表 + **按设备**挑选标签页（带 SVG 图标） | `src/nav.rs`、`src/app.rs` |
| **24 / 25 个标签页有专属页面**（仅 `TAB_DEMO` 仍是占位） | `src/pages/` |
| 设备页真实布局（`.body-widgets` 600px 卡片两列 + 250px 产品图区） | `src/pages/widgets.rs` |
| 视觉规范落地（雷云配色映射到 `cx.theme()` 令牌） | `src/main.rs`、`docs/screens/00-visual-system.md` |
| 4992 条雷云原文（rust-i18n） | `src/i18n.rs`、`locales/` |
| 41 个真实动作名 | `src/model.rs::BUTTON_ACTIONS` |
| DPI 档位编辑 + 柱状图可视化 | 性能页 |
| 轮询率 / 抬升距离 / 加速度 / 传感器旋转 | 性能页 |
| Hypershift 第二层 | 自定义页 |
| 灯光区域效果 / 颜色 / 亮度 / 速度 | 灯光页 |
| 宏编辑（步骤 / 循环 / 延时） | 宏页 |
| 电源 / 电池 | 电源页 |
| **滚动**：模式 / 触觉等级（含「最多禁用 2 个」） / 每转级数 / 阻力 / 高精度 / 横向 / 加速 | 滚动页 |
| **校准**：表面配置文件增删选 / 校准状态机 | 校准页 |
| **配对**：接收器类型 / 固件状态 / 已配对列表 / 配对与取消 | 配对页 |
| **声音**：音量 / 混音 / 均衡器 / THX·杜比 / 音频计 / 镜像 | 声音页 |
| **麦克风**：增益 / 增强档 / 侧音 / 监听 / AI 降噪 / 高通 / 限制器 / 采样率 | 麦克风页 |
| **Snap Tap 快速敲击**（5 种录入模式 / 上限 4 组 / 模式说明直接引用雷云原文） | 键盘页 |
| **动态按键敲击**（四阶段绑定 + 按下开始/结束灵敏度） | 键盘页 |
| **可调触发**（主/第二触发 0.1–4.0mm、快速触发、触发反馈、误触与非法组合提示） | 键盘页 |
| **老板键**（按 `BOSS_KEY_CONFIGURATION_TIP` 归属**鼠标**，落在自定义页） | 自定义页 |
| **变调**（按 `KEY_SHIFTER_TOOLTIP` 归属**线路输入**，落在声音页） | 声音页 |
| **按应用切换配置**（自动/手动两式说明引用原文、游戏→配置关联、删除走 `AlertDialog` 并写明对象） | 设置页 |
| **全局亮度**（应用级状态，非单设备；少于一台 LED 设备时说明原因） | 设置页 |
| **键盘渐暗**（电池供电无活动后调暗，非键盘设备不显示无效控件） | 灯光页 |
| 混音器 / 均衡器 / 增强 / 设置（配置文件切换）/ 按键绑定 | 对应页面 |
| 单元测试 15 项（协议 ID、引擎命令、Snap Tap 上限、触发点约束、功能归属） | `cargo test` |
| 引擎 DLL 探测（命令行 `--probe`） | `src/backend/` |
| **灯光引擎动作协议**（36 个动作 ID + 12 个效果 ID，已固化为枚举） | `src/backend/protocol.rs`、`docs/re/02-lighting-actions.md` |
| **`lighting_driver` 命令层**（`device.register` / `device.unregister` / `mode.set`） | `src/backend/lighting.rs` |
| **引擎调用入口**（`--lighting version` / `--lighting handshake`） | `src/main.rs` |

### 7.2 鼠标方向缺口

| 真实 key `[文案]` | 含义 | 现状 |
|---|---|---|
| `DPI_STAGE` `DPI_STAGES` `NEW_DPI` `CURRENT_DPI` | DPI 档位 | ✅ 已实现 |
| `SENSITIVITY_STAGES` `SENSITIVITY_STAGE_UP/DOWN` | 档位切换 | ✅ 已实现 |
| `CONFIGURE_POLLING_RATE` `HYPER_POLLING_RATE` `HYPER_POLLING_RATE_SUPPORT_8K` | 轮询率 / 8K | ✅ 已实现（125–8000Hz） |
| `AUTO_SWITCH_POLLING_RATE_WHEN_INGAME` `INGAME_POLLING_RATE_HEADER` | 轮询率智能切换 | ✅ 已实现 |
| `HYPERPOLLING_WIRELESS*` | 接收器配对 / 解绑 | ✅ 已实现（配对页） |
| 抬升距离 | Liftoff | ✅ 已实现（低 / 中 / 高） |
| `CALIBRATION_INFORMATION` `CREATE_OWN_SURFACE_PROFILE` `ADD_MAT` | 表面校准 | ✅ 已实现（表面增删选 + 状态机） |
| `GLOBAL_SENSITIVITY_CLUTCH` `LEFT/RIGHT_SENSITIVITY_CLUTCH` | 灵敏度滑块（含左右联动） | ✅ 已实现 |
| `SENSITIVITY_MATCHER` `DELETE_ALL_SENSITIVITY_MATCHER_*` | 灵敏度匹配（配置增删 + 匹配状态） | ✅ 已实现 |
| `DEBOUNCE_MODE` `DEBOUNCE_MODE_DESC_1/2` | 回弹模式（原文引用雷云说明） | ✅ 已实现 |
| `CONFIGURE_SCROLL_WHEEL_STAGES_SETTINGS` `CYCLE_UP/DOWN_SCROLL_WHEEL_STAGES` | 滚轮档位 | ✅ 已实现（含「最多禁用 2 个」约束） |
| `FREE_SPIN_SCROLLING_MODE` `HIGH_RESOLUTION_SCROLLING` `HORIZONTAL_SCROLLING` | 自由滚动 / 高精度 / 横向 | ✅ 已实现 |
| `ACTIVE_SCROLL_WHEEL_HAPTICS` | 滚轮触觉反馈 | ✅ 已实现 |
| `BATTERY_HEALTH_OPTIMIZER` `*_MOUSEMAT_*` | 电池健康优化器（含停止充电阈值） | ✅ 已实现 |
| `POWER_SAVING_*`（V1–V5 按设备分档） | 省电 | ⚠️ 部分 |
| `LIGHTING_ON_BATTERY` `BRIGHTNESS_WHEN_INACTIVE` | 电池时灯光 / 启用时亮度 | ✅ 已实现 |
| `CHROMA_STUDIO` `CHROMA_APPS` `CHROMA_SDK` | Chroma 生态 | ⚠️ 仅区域效果 |

### 7.3 键盘方向缺口

| 真实 key `[文案]` | 含义 | 现状 |
|---|---|---|
| 92 个 `KEY_*` 键位常量 `[前端]` | 逐键映射 | ❌ 只有设备上报的 3–7 个输入点驱动 |
| `KEYMAP` `TEXT_KEYMAP` `ASSIGN_NOW` | 按键映射 / 立即分配 | ⚠️ 部分（步进切换，无下拉选择） |
| `HYPERSHIFT` | Hypershift | ✅ 已实现 |
| `SNAP_TAP` `CREATE_SNAP_TAP_*` `SNAP_TAP_TOOLTIP_*` | Snap Tap 快速敲击（**5 种录入模式**，上限 4 组） | ✅ 已实现 |
| `KEY_SHIFTER` `KEY_SHIFTER_TOOLTIP` | 变调（线路输入音高/速度） | ✅ 已实现（**在「声音」页**——原文归属音频设备，非键盘） |
| `DYNAMIC_KEY_STROKE` `PRESS_START/END` `*_SENSITIVITY` | 动态按键敲击（四阶段 + 灵敏度） | ✅ 已实现 |
| `BOSS_KEY` `BOSS_KEY_CONFIGURATION` | 老板键 | ✅ 已实现（**在「自定义」页**——`BOSS_KEY_CONFIGURATION_TIP` 原文归属鼠标） |
| `KEY_LIGHTS` | **补光灯**（此前误读为「按键级灯光」） | ❌ 缺 |
| `DIM_KEYBOARD_LIGHTING_DESC` `_TIPS` | 电池供电、无活动后调暗灯光（0=关闭，仅无线且未充电时生效） | ✅ 已实现（灯光页） |
| `IDLE_EFFECT` `IDLE_EFFECT_DESC` `IDLE_EFFECT_TOOLTIP` | 闲置效果（`当充电板空闲或未为设备充电时激活`） | ❌ 缺（充电板/鼠标垫方向） |
| `AUDIO_MIX` `AUDIO_MODE` `AUDIO_MONITORING` `AUDIO_EQ` | 键盘上的音频功能（旋钮 / 混音 / EQ） | ❌ 缺 |
| `MACRO_QUEUE` | 宏队列 | ⚠️ 可编辑宏，无录制与队列 |
| `APP_PROFILE` `PROFILE_SWITCHING` `LINKED_GAMES` `ADD_GAME_TITLE` `REMOVE_GAME_MESS` | 按应用切换配置（**自动/手动两式**、游戏→配置关联、破坏性删除走 AlertDialog） | ✅ 已实现（设置页） |
| `PROFILE_MIGRATION` | 从雷云 3 迁移配置 | ❌ 缺（仅呈现文案与入口说明） |
| `BRIGHTNESS_GLOBAL` `_DESC` `_DESC_AT_LEAST_ONE_LED_DEVICE` | 全局亮度（一次性调整所有设备，少于一台 LED 设备时说明原因） | ✅ 已实现（设置页） |
| `MACRO_KEY_COVER_*` | 可换键帽识别 | ❌ 缺 |
| `CUSTOMIZE_ANIMATION_*` `CUSTOMIZE_SYSTEM_INFO_*` | 动画 / 系统信息显示（带屏设备） | ❌ 缺 |
| `ACTUATION_POINT` `PRIMARY/SECONDARY_ACTUATION` `RAPID_TRIGGER` `ACTUATION_FEEDBACK` | 可调触发（模拟光轴 / 霍尔磁性轴，0.1–4.0mm） | ✅ 已实现 |
| `TAB_GAMING` | 游戏模式 | ✅ 已实现（Win 键 / Alt+Tab / Alt+F4） |

### 7.4 硬件下发

`lighting_driver.dll` 的 `Configure(json)` 已绑定，但 **JSON 结构定义在远程前端里**，
尚未取得，因此目前**设置只写本地，没有下发到硬件**。

---

## 8. 外壳与整体布局

**已核出并对照修正**。完整说明、证据与复现命令见
[`docs/screens/00-app-shell.md`](screens/00-app-shell.md)。摘要：

### 8.1 已修正（原偏差 → 现在的做法）

| # | 原偏差 | 现在的做法 |
|---|---|---|
| 1 | 左侧设备侧栏 | **已删除**。改为 `.main-container` 纵向：标题栏 + `.nav-tabs` + `#body-wrapper` |
| 2 | 标签页单独一条 | 顶栏改用标准 `TabBar`（`.pill()`）；三区用 `prefix` / 标签项 / `suffix` |
| 3 | 顶栏无左右区 | 新增 `profile_region`（设备名 + 配置文件）与 `right_region` |
| 4 | 电池只在电源页 | 电量移到**顶栏右区**，`≤10%` 用 `theme.danger` |
| 6 | 设备卡片尺寸自定 | 改为真实 `DeviceCard` 几何 **290×220**，并**可点击选设备** |
| 7 | `Tab::Setting` 当设备页 | 改为**应用级**标签页（三台设备都没有 `TAB_SETTING`） |
| 8 | `Tab::Home` 当设备页 | 改为**应用级**首页，恒定在顶栏最前 |
| 5 | Dashboard 是设备卡片网格 | 改为 `.dashboard` 容器 + **可折叠分段 `.box-group`**（含 `.backdrop-box` 背板、折叠箭头、拖拽把手外观） |

### 8.2 共用部件已逐条对齐

详见 [`docs/screens/00-app-shell.md`](screens/00-app-shell.md) §6。已对齐的：

| 部件 | 关键规格 | 落点 |
|---|---|---|
| 卡片 `.widget` | `padding:30px 40px`、`margin:10px auto`、`border-radius:5px`、`background:#111`、`flex:0 0 auto`、`position:relative` | `widgets::card` / `widget_card` / `widget_slot` |
| 按钮 `.thx-btn` | 27px 高、`padding:7px 5px 6px`、`border-radius:3px`、字号 12px、`text-transform:uppercase`、hover/active 由 `opacity:.8`/`.6` 混色 | `widgets::btn`（**47 处调用点已统一**） |
| 顶栏标签 `.nav` | 圆角 14px、内距 `7px 10px`、右距 20px、`#999`/`#2d2d2d`/`#44d62c`/`#3cbf27` | `AppShell::tab_strip` |
| 顶栏 `.nav-tabs` | `#222` 底、`border-bottom:2px solid #000`、`min-height:48px`、三区 `25% / max-content / 25%` | `AppShell::top_bar`（**只有一条栏**） |
| 开关 `.switch` | `32×18` + `padding:2px` + 胶囊；关态 **`#707070`**、开态 `#44d62c`；滑块 **`14×14` 且为 `#111`**（深色，不是白）；`left:1px → 15px`；hover `opacity:.7` | `widgets::toggle_button`（**44 处调用点**，含主题令牌修正） |
| 下拉 `.s3-dropdown` / `.s3-options` | 面 `27px`、透明底、`1px solid #515151`、**悬停/展开变绿 `#44d62c`**；列表 `#000` 底、`1px solid #515151`、`max-height:180px`；选项 `25px`、hover `#ffffff1a`、选中字色 `#44d62c` | `widgets::select_row`（基于 `Popover`） |

> **修正了一个我自己引入的回归**：`.nav:active` 的 `#3cbf27` 与
> `.thx-btn:hover` 混色得到的 `#3db22a` 是两个不同的值，曾被我合并成一个令牌。
> 现已分开（主题 `button_primary_hover=#3db22a`，`app.rs` 的 `NAV_ACTIVE_BG=#3cbf27`）。
>
> **修正了两个方向就错的开关令牌**：`switch` 原为 `#333`（真实 **`#707070``**）、
> `switch_thumb` 原为 `#ccc`（真实 **`#111`**——滑块是深色，不是白色）。

### 8.3 仍待修正

| 现状 | 真实布局 |
|---|---|
| 提示气泡走库默认（**有圆角**） | `.tip` 是**直角**、`#000` 底、`padding:8px 10px`、`max-width:300px` |
| 开关 / 下拉的 `disabled` 态未接；滑块的 `.thumb:active`（`#383838`）未接 | `opacity:.3` / `#383838` + 绿边 |
| `stepper_row`（「◀ 值 ▶」）**39 处**待换成滑块 | 雷云里不存在这个控件 |
| 滑块已实现（`widgets::slider_row`，真实 `.slider-container` 规格），但只接了 2 处 | 真实界面用 `.s3-dropdown` 直选或滑块。**枚举型已全部换成真下拉**（10 个站点，见 `00-app-shell.md` §6.7）；剩下 41 处是连续型数值，应换滑块 |
| 顶栏右区只有电量、保存提示、帮助 | 原版还有重启/立体声警告、过滤栏（本实现无安装器与立体声路由，不做假警告） |
| 分段折叠/重排无动画；重排用上/下移按钮 | `max-height` + `transition .3s`；重排是拖拽 |
| `.widget` 的 `disabled` 子元素 `opacity:.3` | 未做成统一状态 |

### 8.3 无法复现的差异

原版把「仪表盘 / 应用设置」与「设备页」做成**两个独立窗口**
（主前端 `openNewTab` 打开 `products/{id}/ui/index.html`）。本实现只有一个窗口，
因此把设备选择放进顶栏左区，并用 `Tab::Home` / `Tab::Setting` 承载那两个页面。

## 9. 尚未验证的部分

| 项 | 为什么还没有 |
|---|---|
| 硬件写入 | 需要 HID 协议，或 `lighting_driver.Configure` 的 JSON 结构 |
| Chroma 效果参数表 | 目标动作 ID 已知，但各效果的参数字段未逆向 |
| 带屏设备的界面 | 未下载到声明 `TAB_OLED` / `TAB_DISPLAY` / `TAB_HAPTICS` 的设备模块 |
| 键盘上的音频旋钮 | 未下载到声明 `TAB_MIXER` / `TAB_EQ` 的键盘模块 |





