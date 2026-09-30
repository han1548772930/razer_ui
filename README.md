# razer_ui —— 雷云 4 的原生替代 UI

用 [gpui-kit](https://github.com/longbridge/gpui-kit)（Rust UI 库）重写雷云 Synapse 4 的界面，
替代它那套 Electron + 远程 Web 前端。

> ## ⚠️ 先读这个
>
> **本项目的 UI 目前与雷云不等价。** 缺口清单见
> **[`docs/FEATURES.md`](docs/FEATURES.md) §7**：
> 鼠标方向缺 12 类、键盘方向缺 14 类真实功能。
>
> 文案与导航已换成雷云原文（前端本体已抓取，见
> [`docs/SYNAPSE-UI.md`](docs/SYNAPSE-UI.md)），
> 但**各标签页的真实归属、控件形态、布局间距尚未确认**。

---

## 跑起来

```bash
cargo run
```

首次构建要下载 `gpui-pre` 依赖（约 550 个 crate），耗时较长；
之后增量编译约 10 秒。

### 命令行参数

| 参数 | 作用 |
|---|---|
| `--probe [引擎名]` | 加载雷云原生引擎 DLL 并统计符号后退出 |
| `--selftest` | 不开窗验证功能层与持久化往返 |
| `--demo-keyboard` | 注入一台**合成**键盘（本机没有键盘），让键盘功能可见可测 |
| `--tab <名字>` | 直接打开指定标签页，接受真实 key（如 `TAB_EFFECTS`）或中文名 |
| `--select <productId>` | 初始选中指定设备（键盘页需要，如 `--select 9001`） |

示例：

```bash
cargo run -- --demo-keyboard --select 9001 --tab TAB_GAMING
```

---

## 界面结构与文案：已换成雷云原文

前端本体已经抓下来（127 个分块 / 9.4 MB，含 10 种语言包），
因此导航与文案都基于**雷云自己的代码与文案**，不再是我编的：

- **25 个真实标签页**（`TAB_*`）；但某台设备显示哪几个由**该设备的模块**决定，见 `src/nav.rs`
- **4992 条真实中文文案**，见 `locales/zh-CN.json`，经 `rust-i18n` 使用
- **41 个真实动作名**，见 `src/model.rs::BUTTON_ACTIONS`
- **主前端 89 个命名模块**：34 个动作编辑器、30 个硬件协议解析器
- **设备模块 3 份已下载**（鼠标 182 / 键盘 653 / 耳机 777），各带 32 / 32 / 0 个动作编辑器
- **逐界面布局**，见 [`docs/screens/`](docs/screens/README.md)

> ⚠️ **更正**：此前 README 里那张「区块 → 标签页」映射表，
> 以及据此做出来的侧边栏分区，**是我从 key 名前缀推断的，没有依据**。
> 读到真实前端后可确认的外壳是 `Header + DeviceList + Content` + 弹窗根
> `IotPopupRoot`；各标签页的真实归属尚未确认。


---

## 目录结构

```
src/
  main.rs              入口；含 --probe / --selftest 两个命令行模式
  nav.rs               导航结构：区块 Section + 标签页 Tab（带真实 TAB_* key）
  app.rs               应用外壳：标题栏 + 侧边栏 + 标签页 + 内容区；所有设置写入口
  model.rs             数据模型，逐字对应雷云实测 JSON
  features.rs          鼠标/键盘/灯光/电源/宏 的功能模型
  store.rs             本地配置持久化（%APPDATA%\razer_ui\profiles.json）
  demo.rs              合成演示键盘（--demo-keyboard）
  fixtures/
    measured_devices.json   本机真实设备快照（从 dashboard.log 抓取）
  backend/             方案 2：直接调用雷云原生引擎 DLL
    mod.rs             引擎清单 + 路径解析（不加载）
    dll.rs             DLL 定位（两处安装路径、版本号排序）与加载
    lighting.rs        lighting_driver.dll 绑定（Configure(json) 接口）
  pages/               各标签页
    dashboard.rs performance.rs customize.rs keyboard.rs
    lighting.rs power.rs macros.rs engines.rs placeholder.rs widgets.rs
```

---

## 已验证

### 方案 2：5 个引擎里 4 个可被 Rust 直接复用

命令行逐进程实测（`cargo run -- --probe <引擎名>`）：

| 引擎 DLL | 结果 | 用途 |
|---|---|---|
| `lighting_driver_v1.9.14.0.dll` | ✅ 8/8 符号 | Chroma 灯光写出（JSON 接口） |
| `RzLightingEngineApi_v4.0.55.0.dll` | ✅ 12/12 符号 | Chroma 效果引擎 |
| `mapping_engine.dll` | ✅ 4/4 符号 | 按键映射 / 宏 |
| `simple_service.dll` | ✅ 7/7 符号 | 音频 / 进程 |
| `SysUtilsNative.dll` | ❌ **DllMain 永久阻塞** | 系统工具 |

`SysUtilsNative.dll` 会在加载时挂死进程，已列入黑名单（`backend::blocks_load`）。

### 功能层与持久化

`cargo run -- --selftest` 验证：3 台设备加载、DPI 夹取、宏与 Hypershift 序列化、
写盘后回读一致。

### 两个踩过的坑（都写进代码注释了）

1. **绝不在 UI 线程加载这些 DLL**。第一版把引擎探测放进 `render()`，整个界面卡死。
   现在探测只在命令行、独立进程里做。
2. **`include_str!` 的 JSON 可能带 UTF-8 BOM**。用 PowerShell 写文件会带 BOM，
   导致 JSON 从第 0 字节解析失败，而 `unwrap_or_default()` 把错误静默吞掉，
   表现为「界面莫名其妙没有设备」。现在会剥离 BOM 并显式报错。

---

## 已知限制

- **UI 不等价**：缺口见 `docs/FEATURES.md` §7。文案与导航已换成雷云原文，
  但**功能覆盖仍缺 12+ 类**，且各标签页的真实归属与控件形态尚未确认。
- **设置只落本地，未下发硬件**：写硬件需要 HID 协议（方案 3）或
  `lighting_driver.Configure` 的 JSON 结构，两者都还没拿到。
- **灯光效果名未验证**：效果名定义在远程前端；本机日志只实测到 `static` 与 `Reactive`。
- **本机无 Chroma 设备、无键盘**：灯光只能看接口，键盘只能用合成演示设备。
- **HID 层不可直接复用**：雷云用 Node 原生模块 `node-rz-hid`，Rust 需改用 `hidapi`。
  这是方案 2 → 方案 3 的天然接缝。

## 版权提示

雷云 Logo 与设备图片归 Razer 所有。作为替代实现，本项目应使用自有素材。

## UI 规范

本项目的界面遵循仓库内的两份 **gpui-kit 规范**（它们是要求，不是参考）：

| Skill | 内容 |
|---|---|
| [`skills/gpui-kit/`](skills/gpui-kit/SKILL.md) | Coding Guides：架构、状态归属、`ElementId`、测试、公共 API |
| [`skills/gpui-kit-design-guides/`](skills/gpui-kit-design-guides/SKILL.md) | Design Guides：颜色、层级、间距、圆角、密度、交互状态、文案 |

改界面之前先读它们。已经据此修正的问题记在
[`docs/screens/00-visual-system.md`](docs/screens/00-visual-system.md) §「依据规范修正过的问题」，
关键几条：

- **颜色不写在调用点**：一律 `cx.theme()`；雷云的原始色值只出现在 `src/main.rs` 的主题定义里。
- **圆角走命名档位**：雷云的两级圆角对应 `radius_tokens().md`（控件 3px）与 `.lg`（卡片 5px）。
- **不嵌套卡片**：一个 600px widget 就是一层表面。
- **状态要可见**：hover / focus / selected / disabled 各有区别。

## 相关文档

| 文档 | 内容 |
|---|---|
| [`docs/screens/`](docs/screens/README.md) | **逐界面文档**：每个界面的布局分区与功能项，一个界面一个文件 |
| [`docs/screens/00-app-shell.md`](docs/screens/00-app-shell.md) | **整体骨架**：应用外壳 / 顶栏三区 / Dashboard / 应用设置 / 设备页两级模型，**以及本项目当前偏差清单** |
| [`docs/screens/00-visual-system.md`](docs/screens/00-visual-system.md) | **视觉规范**：从模块 CSS 全量提取的颜色/圆角/字号/边框/过渡/各状态，**以及与 gpui-kit 主题令牌的对应** |
| [`docs/SYNAPSE-UI.md`](docs/SYNAPSE-UI.md) | **前端功能与布局**（技术栈、外壳、25 个标签页、每设备一份独立模块、动作编辑器、协议解析器） |
| [`docs/SYNAPSE-FEATURES-FULL.md`](docs/SYNAPSE-FEATURES-FULL.md) | **完整功能全表**：4992 条真实文案 / 1101 命名空间（自动生成） |
| [`docs/FEATURES.md`](docs/FEATURES.md) | 功能清单 + **实现状态与缺口**（§7 鼠标/键盘逐项） |
| [`docs/re/01-ipc-api-surface.md`](docs/re/01-ipc-api-surface.md) | IPC 全量契约 |

