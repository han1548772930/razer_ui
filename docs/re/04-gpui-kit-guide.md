# 04 — gpui-kit 实现指南（Razer Synapse 4 替代 UI）

> 目标：用 `gpui-kit`（longbridge/gpui-kit）在 `D:\rust_test\razer_ui` 这个空 crate 里重建雷云 4 的 **UI 层**。
> 本文只讲 **怎么用 gpui-kit 把它搭出来**；功能逆向清单见同目录其它文档。
>
> 所有结论都来自实际读取的源码，引用格式为 `路径:行号`。**凡是本文没有亲眼在源码/文件系统里验证的，都会显式标注为"未验证/不确定"。**
> 参考仓库副本：`D:\rust_test\razer_ui\.ref\gpui-kit\`（HEAD = `caf830c02d08aaeaa0b15158dbe74cc7dcbac2b2`，分支 `main`，提交时间 2026-09-29，**仓库内无任何 git tag**）。

---

## 0. 结论摘要（TL;DR）

| 问题 | 结论 |
| --- | --- |
| 依赖方式 | **用 path 依赖指到 `.ref/gpui-kit/crates/kit`**，不要依赖 crates.io 的 `gpui-kit = "0.7"`（本机无法验证它在 crates.io 上存在，见 §1.3） |
| 能否立刻构建 | **不能。** 本机 `~/.cargo` 里 **一个 `gpui-*` crate 都没有**，且 cargo 出网失败（Clash fake-ip + schannel）。必须先修好网络/代理跑一次 `cargo fetch`（§1.4） |
| 精确 pin | `gpui-pre =0.3.7`（以及 `gpui-pre-platform` / `-macros` / `-reqwest-client` / `-sum-tree` / `-web`），共 **27 个 `gpui-pre*` 包**，全部 `=0.3.7`，由 `gpui-kit` 自己带进来，应用侧**不要**单独写 gpui 依赖（§1.2） |
| 最小启动 | `gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| { gpui_kit::init(cx); gpui_kit::open_window(opts, cx, |_, cx| cx.new(|_| View)) })`（§2） |
| 组件量 | 官方目录 67 项 + 目录漏掉的 10 个页面 + 若干基础模块，**合计 75+**（§3） |
| 关键缺失 | **没有系统托盘、没有按键/热键录制控件、没有原生文件对话框、没有渐变/灯效时间轴编辑器、没有设备热区画布（需自写 `canvas` Element）**（§3.4） |
| 主题 | `Theme::update(cx, \|theme\| { theme.colors.primary = rgb(0x44D62C).into(); })`，或加载 `ThemeSet` JSON + `Theme::apply_config`（§4） |
| i18n | 基于 `rust-i18n 4.2`；`zh-CN` 组件内置已有，只需补自己的 `locales/ui.yml` + `rust_i18n::extend!(gpui_component)`（§5） |
| Windows | MSVC + VS2022 C++ 工作负载 + CMake；**必须**把仓库 `.cargo/config.toml` 的 `/STACK:8000000` 复制到应用根目录（§6） |
| `runtime_shaders` | **不是 `gpui-kit` 的 feature**，它写在仓库 workspace 对 `gpui_platform` 的依赖上，会自动生效；应用侧不需要也不应该去开（§6.5） |

---

## 1. Cargo.toml 设置

### 1.1 仓库自身的依赖声明（事实）

`\.ref\gpui-kit\Cargo.toml:49-58`：

```toml
[workspace.dependencies]
gpui-kit = { path = "crates/kit", version = "0.7.0" }
gpui-component = { path = "crates/component", version = "0.7.0" }
gpui-base = { path = "crates/base", version = "0.7.0" }
gpui-component-macros = { path = "crates/component-macros", version = "0.7.0" }
gpui-kit-assets = { path = "crates/assets", version = "0.7.0" }
gpui-fps = { path = "crates/fps", version = "0.7.0" }
gpui-shell = { path = "crates/shell", version = "0.7.0" }
gpui-wry = { path = "crates/webview", version = "0.7.0" }
gpui-component-shell = { path = "crates/component-shell", version = "0.7.0" }
```

`\.ref\gpui-kit\Cargo.toml:61-73` —— **精确 pin 的原文与理由**：

```toml
# GPUI comes from the `gpui-pre-*` snapshots of Zed's crates, and any snapshot
# may change GPUI's API. These requirements are what the published gpui-kit
# crates carry to crates.io, so they pin the exact snapshot the release was
# built against: a caret requirement let the weekly release move applications
# onto a newer snapshot that gpui-component did not compile with (#3156). Bump
# every snapshot crate together with its Cargo.lock entry; `script/check-gpui-pin.ts`
# fails CI when a pin is not exact.
gpui = { package = "gpui-pre", version = "=0.3.7" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.7", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }
gpui_web = { package = "gpui-pre-web", version = "=0.3.7" }
gpui_macros = { package = "gpui-pre-macros", version = "=0.3.7" }
reqwest_client = { package = "gpui-pre-reqwest-client", version = "=0.3.7" }
sum-tree = { package = "gpui-pre-sum-tree", version = "=0.3.7" }
reqwest = { package = "gpui-pre-reqwest", version = "=0.12.15", default-features = false, features = [...] }
```

要点：
- `gpui` 这个 crate 名是 **rename**，真名是 `gpui-pre`；`use gpui_kit::*;` 拿到的是 GPUI 本体（`crates/kit/src/lib.rs:97` 的 `pub use ::gpui::*;`）。
- `gpui_platform` 打开 `runtime_shaders`（见 §6.5）。
- 这个 pin 是 **exact (`=0.3.7`)**，所以任何 `Cargo.lock` 里也必须是 `0.3.7`；不要在应用侧另写一个 `gpui-pre` 依赖。

### 1.2 仓库 Cargo.lock 是否含全部 `gpui-pre` 条目 —— **是**

`\.ref\gpui-kit\Cargo.lock` 中 `gpui-pre*` 包条目（全部 `version = "0.3.7"`，`source = "registry+https://github.com/rust-lang/crates.io-index"`），共 27 个：

```
gpui-pre, gpui-pre-apple, gpui-pre-bench-metrics, gpui-pre-collections,
gpui-pre-derive-refineable, gpui-pre-http-client, gpui-pre-http-client-tls,
gpui-pre-linux, gpui-pre-macos, gpui-pre-macros, gpui-pre-perf,
gpui-pre-platform, gpui-pre-refineable, gpui-pre-reqwest-client,
gpui-pre-scheduler, gpui-pre-shared-string, gpui-pre-sum-tree, gpui-pre-util,
gpui-pre-util-macros, gpui-pre-web, gpui-pre-wgpu, gpui-pre-windows,
gpui-pre-zlog, gpui-pre-ztracing, gpui-pre-ztracing-macro
```
外加 `gpui-pre-reqwest = 0.12.15`。
（统计命令：`Select-String -Path Cargo.lock -Pattern 'name = "gpui-pre' -Context 0,2`。）

**但是**：这个 lock 属于仓库自己的 workspace。我们的 `razer_ui` 是**另一个 workspace root**，会生成自己的 `Cargo.lock`。path 依赖**不会**让 cargo 复用被依赖仓库的 lock。

### 1.3 (a) crates.io 依赖 vs (b) path 依赖 —— 对比

| 维度 | (a) `gpui-kit = "0.7"` | (b) `gpui-kit = { path = ".ref/gpui-kit/crates/kit" }` |
| --- | --- | --- |
| 需要 `gpui-kit`/`gpui-base`/`gpui-kit-assets` 已在 crates.io 发布 | **需要**，本机**无法证实** 0.7.0 已发布（见下） | **不需要**，直接用本地源码 |
| 需要 `gpui-pre*=0.3.7` 在 crates.io | 需要 | **同样需要**（path 依赖不会 vendor 传递的 registry 依赖） |
| 源码可控性 | 与 lock 一致但不可读改 | 可直接读改（本任务需要读源码逆向） |
| 版本漂移风险 | `0.7` 是 caret，将来可能拿到不兼容的新版 | 锁死在 `caf830c0` 这个 commit，可复现 |
| `[patch.crates-io]` 生效 | 不适用 | **不生效**（patch 只在 workspace root 生效，仓库 root 的 `rquickjs` patch 对我们无效 —— 但我们不用 `gpui-shell`，无影响） |
| 结论 | 备选 | **推荐** |

**未验证项（重要）**：本机 crates.io 索引缓存 `~/.cargo/registry/index/index.crates.io-*/​.cache/gp/ui/` 下只有 `gpui`、`gpui-ce`、`gpui-component`（版本只到 `0.4.2`）、`gpui-component-macros`、`gpui-macros`、`gpui_collections` 等旧世代包，**完全没有 `gpui-kit` / `gpui-base` / `gpui-kit-assets` / `gpui-pre*` 的索引条目**。README 有 `docs.rs/gpui-kit` 徽章（`README.md:9`）、`crates/kit/Cargo.toml:12 publish = true`，说明它**意图**发布；但 0.7.0 是否真的在 crates.io 上，本文**无法确认**。所以走 path。

### 1.4 本机离线可行性 —— **目前完全不可构建**

实测证据：

1. `~\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\` 有 952 个已解包 crate，**没有一个名字含 `gpui`**。
2. `~\.cargo\registry\cache\...\` 有 1670 个 `.crate`，同样没有 `gpui-pre*`。
3. `cargo search gpui-kit` →
   ```
   error: failed to retrieve search results from the registry at https://crates.io
   Caused by: [35] SSL connect error (schannel: AcquireCredentialsHandle failed:
   SEC_E_NO_CREDENTIALS (0x8009030e))
   ```
4. `Invoke-WebRequest https://index.crates.io/config.json` → `基础连接已经关闭`（TLS 失败）。
5. `Resolve-DnsName index.crates.io` → `198.18.0.254`（`198.18.0.0/15` 是 Clash Verge 的 fake-ip 段），说明本机走代理工具分流，而 cargo / .NET 都拿不到这个代理。

**因此，"offline 能不能构建" 的答案是：现在两条路都不行。** 阻塞点不在 `gpui-kit` 的获取方式，而在于 **27 个 `gpui-pre-0.3.7` 包从未被下载过**，而 cargo 又没有可用出网路径。

**落地步骤（需要用户/父 agent 处理网络）**：

```powershell
# 1) 让 cargo 走 Clash 的混合端口（示例：7890），或让 index.crates.io 走直连
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
$env:HTTP_PROXY  = "http://127.0.0.1:7890"

# 2) 在仓库副本里做一次完整 fetch（会拉齐 gpui-pre 全家桶）
cargo fetch --manifest-path D:\rust_test\razer_ui\.ref\gpui-kit\Cargo.toml

# 3) 之后本项目可以用 --offline 构建（前提：Cargo.lock 已解析出 gpui-pre=0.3.7）
cargo build --offline
```

**可选加固（强烈建议，让 lock 一次对齐）**：把仓库的 lock 复制成我们的起点，cargo 会做最小重解析并保留 `gpui-pre*=0.3.7`：

```powershell
Copy-Item D:\rust_test\razer_ui\.ref\gpui-kit\Cargo.lock D:\rust_test\razer_ui\Cargo.lock
# 然后 cargo metadata / cargo build，让 cargo 把根包从 story 换成 razer_ui
```
（这一点是 cargo 的常规行为，本文**未实测**，因为无网络。）

### 1.5 推荐的 `Cargo.toml`（path 依赖）

`D:\rust_test\razer_ui\Cargo.toml`：

```toml
[package]
name = "razer_ui"
version = "0.1.0"
edition = "2024"

[dependencies]
# 单一入口：GPUI + gpui-base + gpui-component + 默认图标集全在里面
gpui-kit = { path = ".ref/gpui-kit/crates/kit", version = "0.7.0" }

anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rust-i18n = "4.2"

[profile.dev.package]
# 来自 website/docs/installation.md:100-113，Debug 下让框架依赖跑得动
gpui-pre = { opt-level = 3 }
gpui-pre-macros = { opt-level = 3 }
gpui-pre-platform = { opt-level = 3 }
gpui-component = { opt-level = 3 }
gpui-kit = { opt-level = 3 }
gpui-kit-assets = { opt-level = 3 }
rustybuzz = { opt-level = 3 }
taffy = { opt-level = 3 }
ttf-parser = { opt-level = 3 }
```

`D:\rust_test\razer_ui\.cargo\config.toml`（**照抄仓库** `\.ref\gpui-kit\.cargo\config.toml`，否则 Windows 上可能爆栈）：

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-arg=/STACK:8000000"]
```

### 1.6 `edition 2024` 与工具链

- 仓库 workspace 是 `edition = "2024"`（`Cargo.toml:47`），本机 `rustc 1.98.1 / cargo 1.98.1`，`rustup` 默认 host `x86_64-pc-windows-msvc` —— 满足。
- 官方文档要求 **Rust ≥ 1.92**（`website/docs/installation.md:42`，理由是 GPUI Linux 平台依赖 `oo7 0.6.0` 的 MSRV）。本机无压力。

---

## 2. 最小可用 `main.rs`

### 2.1 官方最小例子（原文，34 行）

`\.ref\gpui-kit\examples\hello_world\src\main.rs`：

```rust
use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

pub struct Example;
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}

fn main() {
    gpui_kit::application().run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_kit::init(cx);

        // Opens a window with a `Root` wrapping the view, so dialogs, sheets,
        // notifications and menus work in it.
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Example))
            .expect("Failed to open window");
    });
}
```

对应的 `examples/hello_world/Cargo.toml` 只有 `anyhow` + `gpui-kit`（`:8-10`）—— 这就是"应用只列一个依赖"的证据。

### 2.2 启动三段式的语义（来自 `website/docs/getting-started.md:72-78`）

1. `gpui_kit::application()` 创建桌面 Application；`.with_assets(...)` 注册图标源。
2. `gpui_kit::init(cx)` 初始化启用的层（含 `gpui-base` + `gpui-component`），**必须在开窗前调一次**。
   - `crates/kit/src/lib.rs:175-180`：
     ```rust
     pub fn init(cx: &mut App) {
         #[cfg(feature = "component")]
         gpui_component::init(cx);
         #[cfg(not(feature = "component"))]
         gpui_base::init(cx);
     }
     ```
   - `crates/component/src/lib.rs:127-147` 里 `theme::init` / `root::init` / `notification::init` / `dialog`(base) / `tooltip` / `table` / `menu` 等逐个初始化。
3. `gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| MyView))` 把内容包进 Base `Root`，返回 `(AnyWindowHandle, Entity<V>)`。
   - `crates/kit/src/lib.rs:149-164`：
     ```rust
     pub fn open_window<V: Render>(
         options: WindowOptions,
         cx: &mut App,
         build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
     ) -> Result<(AnyWindowHandle, Entity<V>)>
     ```
   - `closure` 必须返回**内容视图**，不是另一个 `Root`（`crates/kit/src/lib.rs:145-146` 的文档注释）。
   - `Root` 负责该窗口的 overlay 层（dialog / sheet / notification / tooltip / menu / 触摸选择 / 主题 / 窗口边框）。

### 2.3 注册图标资产（必须，否则 `Icon` 渲染不出来）

`examples/app_assets/README.md:7-28` 的原文要点：

| 需求 | 注册方式 | 结果 |
| --- | --- | --- |
| 组件内置图标 | `gpui_kit::assets::Assets` | 内嵌 **101** 个默认组件 SVG |
| 少量额外图标 | `icon_assets!` 产物 + `Assets` 组合 | 只内嵌选中的额外 SVG |
| 完整目录 | `gpui_kit::assets::AllAssets` | 内嵌全部 **1830** 个 SVG |
| 自己的 SVG | 自写 `AssetSource` | 加载自己的路径，可与 `Assets` 组合 |

`examples/sidebar/src/main.rs:192` 与 `examples/window_title/src/main.rs:43`：
```rust
let app = gpui_kit::application().with_assets(Assets); // use gpui_kit::assets::Assets;
```

自写 `AssetSource` 的完整实现见 `examples/app_assets/src/main.rs:8-29`（`#[derive(RustEmbed)] #[folder = "./assets"] #[include = "icons/**/*.svg"]` + `impl AssetSource`）。

**⚠️ 图标名的坑（对游戏外设配置器非常关键）**：
- `gpui_kit::component::IconName` 是**旧的兼容枚举，只有 `default-icons.txt` 那 101 个**。该文件里**没有** `gamepad` / `mouse` / `keyboard` / `joystick`（实测 grep 无匹配）。`default-icons.txt` 实际只有 `cpu`、`memory-stick`、`palette`、`play`、`star`、`settings`、`hard-drive`、`network` 等。
- `gpui_kit::assets::IconName` 是**完整目录**（1830 个 = 1818 Lucide 1.43.0 + 12 个自有），其中确实有：`gamepad`、`gamepad-2`、`gamepad-directional`、`joystick`、`mouse`、`mouse-left`、`mouse-right`、`keyboard`、`monitor`、`cpu`、`microchip`、`fan`、`thermometer`、`zap`、`lightbulb`、`palette`、`sliders-horizontal`、`gauge`、`crosshair`、`target`、`usb`、`bluetooth`、`headphones`、`speaker`、`volume`、`radio`、`wifi`、`battery*`、`power`、`settings`、`wand-sparkles`、`sparkles`、`flame`、`trophy`、`activity`、`rocket`、`shield`、`cable`。
  - 证据：`crates/assets/README.md:51-53`（"all 1,818 Lucide 1.43.0 icons plus 12 retained GPUI Kit icons"）+ 实测 `crates/assets/assets/icons/*.svg` 文件名。
- **要用这些图标，必须二选一**（`crates/assets/README.md:11-16`）：
  1. 注册 `gpui_kit::assets::AllAssets`（内嵌全部 1830，实测报告 **+1.02 MiB**）；
  2. 用 `icon_assets!(ExtraIcons, [Gamepad, Mouse, Keyboard, Fan, Zap, ...])` 只挑要的，再与 `Assets` 组合成自己的 `AssetSource`（+10 个图标约 +19 KiB）。`AllAssets`/`icon_assets!` 在 native 上都是把 payload 真正嵌进二进制。
- 自研图标（雷蛇设备线稿、热区底图）：`Icon::path("icons/xxx.svg")` 不会创建新的 `IconName` variant，只能走自己的 `AssetSource` key（`examples/app_assets/README.md:43-44`）。

### 2.4 自定义标题栏（雷云观感需要）

`examples/window_title/src/main.rs` 原文关键部分：

```rust
use gpui_kit::component::{TitleBar, button::{Button, ButtonVariants}, h_flex, v_flex};
use gpui_kit::*;

impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                // Render custom title bar on top of Root view.
                TitleBar::new().child(
                    h_flex().w_full().pr_2().justify_between()
                        .child("App with Custom title bar")
                        .child("Right Item"),
                ),
            )
            .child(div().id("window-body").p_5().size_full() /* ... */)
    }
}

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        // Setup GPUI to use custom title bar
        let window_options = TitleBar::window_options();
        gpui_kit::open_window(window_options, cx, |_, cx| cx.new(|_| Example))
            .expect("Failed to open window");
    });
}
```

`TitleBar::window_options()` 的来源（`crates/component/src/title_bar.rs:81-91`）：`titlebar: Some(title_bar_options())`（`appears_transparent: true`, `traffic_light_position`）+ `app_owns_titlebar_drag: true`；常量 `TITLE_BAR_HEIGHT = px(34.)`（`:15`）。文档明确要求把它作为 `WindowOptions` 的基座（`:67-80`）。

### 2.5 推荐给本项目的 `main.rs`（可直接落地）

以下代码的每一个 API 都在源码里出现过；`window_min_size` 字段名取自 `title_bar.rs:77` 的文档示例，类型未逐一核对 gpui-pre 0.3.7 的 RNA（在 §8 标为待验证）。

```rust
use gpui_kit::component::{
    ActiveTheme, Sizable, Theme, ThemeMode, TitleBar,
    h_flex, v_flex,
};
use gpui_kit::*;

struct AppShell;

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().child(
                    h_flex().w_full().pr_2().justify_between()
                        .child("雷云 4 (Rust)")
                        .child("device / profile"),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .gap_3()
                    .child("TODO: Sidebar + 设备页"),
            )
    }
}

fn main() {
    // 1) 注册图标源（否则 Icon 空白）
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx| {
        // 2) 初始化各层，必须在开窗前
        gpui_kit::init(cx);

        // 3) 暗色 + 雷蛇绿（#44D62C）
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |theme| {
            theme.colors.primary = rgb(0x44D6_2C).into();
            theme.primary_foreground = rgb(0x0A0A_0A).into();
            theme.ring = rgb(0x44D6_2C).into();
            theme.radius = px(6.);
        });

        // 4) 开窗（自定义标题栏 + 最小尺寸）
        let window_options = WindowOptions {
            window_min_size: Some(size(px(1080.), px(720.))),
            ..TitleBar::window_options()
        };

        gpui_kit::open_window(window_options, cx, |_, cx| cx.new(|_| AppShell))
            .expect("Failed to open window");
    });
}
```

居中开窗的写法（来自 `examples/sidebar/src/main.rs:197-202`，已实测存在）：

```rust
let window_options = WindowOptions {
    window_bounds: Some(WindowBounds::centered(size(px(900.), px(620.)), cx)),
    ..Default::default()
};
```

---

## 3. 组件清单

### 3.1 官方目录（`website/component/index.md` 原文，67 项）

**Basic Components**

| 组件 | 一句话用途 |
| --- | --- |
| Accordion | 可折叠内容面板 |
| Alert | 多种 variant 的告警条 |
| Attachment | 文件/媒体附件展示面 |
| Avatar | 用户头像（带 fallback 文本） |
| Badge | 计数徽标与状态点 |
| Bubble | 聊天消息气泡（对齐 + 反应） |
| Button | 多 variant 按钮 |
| Checkbox | 二元勾选 |
| Collapsible | 展开/收起内容 |
| DropdownButton | 带下拉菜单的按钮 |
| Icon | 图标显示 |
| Image | 图片显示（带 fallback） |
| Kbd | 键位显示（**只显示，不采集**） |
| Label | 表单标签 |
| Marker | 会话状态/分隔标记 |
| Message | 可组合的聊天消息结构 |
| MessageScroller | 尾部跟随的虚拟化消息列表 |
| Pagination | 分页导航 |
| Progress | 进度条 |
| Questionnaire | 可组合的多步问答 |
| Radio | 单选（`RadioGroup`） |
| Rating | 星级评分 |
| Skeleton | 加载占位 |
| Slider | 范围取值（**DPI/音量/亮度直接用它**） |
| Spinner | 加载/状态指示 |
| Stepper | 分步进度指示 |
| Switch | 开关 |
| Tag | 标签/分类 |
| TextView | Markdown / HTML 渲染 |
| Toggle | 切换按钮态（`ToggleGroup` 可选一组） |
| Tooltip | 悬停提示 |

**Form Components**

| 组件 | 一句话用途 |
| --- | --- |
| Input | 单行输入（`InputState` 实体持有状态） |
| Textarea | 多行文本（固定行数或自动增高） |
| Editor | 源码编辑器（高亮、行号、折叠、LSP） |
| Select | 下拉选择 |
| Combobox | 可搜索单选/多选下拉 |
| NumberInput | 数字输入（增减步进，**polling rate / DPI 数值框**） |
| DatePicker | 日期选择 + 日历 |
| TimeField | 分段式时间输入 |
| OtpInput | 一次性验证码输入 |
| ColorPicker | 颜色选择（调色板 / hex / alpha / RGB/HSL） |
| Form | 表单容器与布局（`Form` + `Field`） |

**Layout Components**

| 组件 | 一句话用途 |
| --- | --- |
| DescriptionList | 键值对展示 |
| GroupBox | 带边框的内容分组 |
| Root | 窗口级 provider（主题 / dialog / notification 宿主） |
| Theme | 颜色 / 字体 / 明暗外观定制 |
| Dialog | 对话框与模态 |
| Notification | Toast 通知（含系统通知） |
| Popover | 浮动内容 |
| Resizable | 可拖拽分栏（`h_resizable` / `v_resizable`） |
| Scrollable | 滚动容器 |
| Sheet | 从边缘滑出的面板 |
| Sidebar | 导航侧边栏（**雷云左侧设备/功能区导航就靠它**） |
| StatusBar | 底部状态栏（左/中/右分区） |
| Toolbar | 顶部工具栏（左/右分区 + 尺寸） |

**Advanced Components**

| 组件 | 一句话用途 |
| --- | --- |
| Calendar | 日历显示与导航 |
| Carousel | 相关项轮播 |
| Command | 命令面板（搜索 + 快捷动作） |
| Chart | 图表（Line / Bar / Area / Pie / Candlestick；实测还有 Radar / Sankey） |
| List | 列表（`ListDelegate` 虚拟化） |
| Menu | 菜单 / 上下文菜单 / 下拉菜单 |
| Settings | 设置 UI（`Settings` / `SettingPage` / `SettingGroup` / `SettingItem` / `SettingField`） |
| DataTable | 高性能数据表（虚拟滚动、列宽可调、排序、单元格选择） |
| Dock | 可停靠布局（分栏 / 拖拽标签 / 嵌套 / 边停靠，可序列化） |
| Tabs | 标签页（`TabBar` + `Tab`） |
| Tree | 层级树 |
| VirtualList | 大数据量虚拟列表（含不等高项） |

### 3.2 目录漏掉、但仓库里确实有的组件页

`website/component/*.md` 共 **78** 个文件（含 `index.md`，即 **77** 个组件页；77 − 67 = 10）。下面 10 个页面在 `index.md` 里没被列出：

| 组件 | 用途 | 证据 |
| --- | --- | --- |
| AlertDialog | 强打断确认框（`window.open_alert_dialog`） | `website/component/alert-dialog.md`；`crates/component/src/window_ext.rs:52-54` |
| Clipboard | 复制到剪贴板 | `website/component/clipboard.md`；`crates/component/src/lib.rs:37` |
| Empty | 空状态占位 | `website/component/empty.md`；`crates/component/src/lib.rs:45` |
| FocusTrap | 焦点陷阱 | `website/component/focus-trap.md`（`crates/base` 的 `FocusTrapElement`） |
| HoverCard | 悬停卡片 | `website/component/hover-card.md` |
| InputGroup | 输入框组合（前后缀、内嵌按钮） | `website/component/input-group.md`；`crates/component/src/input/group.rs` |
| Plot | 底层绘图原语（scale / grid / axis / label / Plot trait） | `website/component/plot.md`；`crates/component/src/plot/mod.rs:6` |
| Shimmer | 流光加载态 | `website/component/shimmer.md` |
| Table | 基础表格（Table/TableHeader/TableRow/TableCell） | `website/component/table.md`；`crates/base/src/lib.rs:177` |
| TitleBar | 自定义标题栏 | `website/component/title-bar.md`（**目录里也没有**） |

`examples/dock/src/main.rs:34-37` 的 story 列表进一步印证（`AccordionStory, ButtonStory, DataTableStory, ...`）。

### 3.3 组件 crate 里额外导出的模块/工具

来自 `crates/component/src/lib.rs:26-91`（`pub mod` 全表）与 `:93-120`（`pub use`）：

- `highlighter` —— Tree-sitter 语法高亮、诊断样式、语言注册表
- `history` —— 撤销/重做历史
- `native_menu` —— 原生菜单（macOS AppKit NSMenu / Windows Win32 popup menu）
- `global_state` —— `GlobalState`（组件级全局）
- `virtual_list` / `VirtualList` / `h_virtual_list` / `v_virtual_list`
- `resizable`（`ResizablePanel`、`h_resizable`、`v_resizable`、`resize_handle_appearance`）
- `window_border` / `window_paddings` / `WindowExt`
- `animation`（来自 base）、`styled`（`h_flex` / `v_flex` / `StyledExt` / `ThemeStyled` / `RoleOverride`）
- `sizing`（`Sizable` / `Size` / `StyleSized`：`xs` / `sm` / `md` / `lg`）
- `component_traits`（`Disableable` / `Selectable`）
- `index_path`（`IndexPath`，树/表路径）
- `element_ext`（`ElementExt` / `LengthExt` / `AxisExt` / `Edges` / `Measure` / `Placement` / `Side`）
- `text`（`markdown` / `html` / `TextView` / `TextViewState`）
- `time`（`calendar` / `date_picker` / `time_field`）
- `inspector`（debug 构建下自动开启，`:10`、`:130-131`）

**`SettingField` 的构造器**（`crates/component/src/setting/fields/mod.rs:154-270`）—— 搭雷云设置页最省事的工具：

```rust
SettingField::switch(value_fn, set_fn)                       // impl SettingField<bool>
SettingField::checkbox(value_fn, set_fn)                     // impl SettingField<bool>
SettingField::input(value_fn, set_fn)                        // impl SettingField<SharedString>
SettingField::dropdown(vec![(key, label)], value_fn, set_fn) // 不滚动
SettingField::scrollable_dropdown(..)                        // 选项多时用
SettingField::element(my_field)                              // 自定义 SettingFieldElement
SettingField::render(|options, window, cx| ...)              // 闭包自定义
SettingField::number_input(NumberFieldOptions, value_fn, set_fn) // impl SettingField<f64>
```
外围结构（`crates/component/src/setting/{settings,page,group,item}.rs`）：
```rust
Settings::new("app-settings")
    .page(
        SettingPage::new("Lighting")            // page.rs:34
            .icon(Icon::new(IconName::Lightbulb)) // page.rs:68（需 assets::IconName 或 icon_assets!）
            .default_open(true)
            .resettable(true)
            .groups(vec![
                SettingGroup::new().title("Effect").items(vec![   // group.rs:36,48,89
                    SettingItem::new("Chroma Effect", SettingField::dropdown(..)) // item.rs:46
                        .description("...")                        // item.rs:142
                        .keywords(["chroma", "rgb"])
                        .disabled(false),
                ]),
            ]),
    )
```
完整可用范例：`crates/story/src/stories/settings_story.rs:131-230`（含 `Global` 状态 + `Theme::change` 联动）。

### 3.4 游戏外设配置器**缺失**的东西（重点）

以下每一项都经过 grep/目录核对，确认 gpui-kit **没有**现成组件：

| 雷云需要的功能 | gpui-kit 现状 | 证据 / 替代方案 |
| --- | --- | --- |
| **系统托盘图标 + 托盘菜单**（雷云常驻托盘） | **没有** | `grep tray\|TrayIcon\|Shell_NotifyIcon` 在 `crates/` 下只有无关注释匹配；无 `tray` 依赖 | 必须自己用 `windows` crate（`Shell_NotifyIconW`）/ `tray-icon` crate |
| **按键/热键录制控件**（"按下要绑定的键"） | **没有** | `crates/component/src/kbd.rs:13,31,40` 只有 `stroke: Keystroke` + `Kbd::new(stroke)` + `Kbd::format()`，是**只读展示**；全仓库没有 "key capture" 组件 | 自写：`on_key_down` + `gpui::Keystroke`（`gpui::KeyDownEvent.keystroke`），配 `Kbd` 展示 |
| **宏/按键序列时间轴编辑器** | **没有** | 无对应模块；`mapping_engine.dll` 那类逻辑全要自研 | 自写 Element + `List`/`VirtualList` |
| **设备外观图 + 可点热区**（鼠标/键盘哪里点哪里） | **没有现成组件** | 无 device/hotzone 模块 | 用 GPUI 的 `canvas(...)` + `window.paint_path(...)` 自写 Element；可照抄 `examples/brush/src/main.rs:196-280` 的 prepaint/paint 模式 |
| **渐变 / Chroma 灯效时间轴、多层灯效叠加** | **没有** | `ColorPicker` 只能选**单个颜色**（`crates/component/src/color_picker.rs:55-129`，`ColorSelect`、`HslaSliders`）；无 gradient stop 编辑器 | 自写；`theme` 里的 `linear-gradient(...)` 只作用于主题 token，不是给用户编辑的控件 |
| **原生文件选择对话框**（导入/导出 profile、宏文件） | **没有** | `grep rfd\|FileDialog` 在所有 `*.toml` 里 0 匹配 | 加 `rfd` crate，或 Win32 `IFileOpenDialog` |
| **DPI / 灵敏度曲线拖点编辑** | 间接 | `plot`（`crates/component/src/plot/mod.rs`）+ `chart::LineChart/AreaChart` 能画，但**没有可拖拽控制点** | 自写拖点层，叠在 chart 上 |
| **风扇/水泵曲线编辑** | 间接，同上 | 同上 | 同上 |
| **多设备实时遥测**（温度/转速/电量曲线） | 可做 | `chart::AreaChart` / `LineChart` 已够；抄 `examples/system_monitor/src/main.rs`（`sysinfo` + `AreaChart` + `DataTable`） | 直接用 |
| **开机自启 / 需要管理员权限的操作** | **没有** | 无注册表/服务相关 API | 自己用 `windows` crate / `winreg`；雷云本机有 `Razer Elevation Service`（见 §7.1） |
| **USB/HID 设备枚举与 feature report 下发** | **不属于 UI 框架** | gpui-kit 只有 UI | 需要 `hidapi` / `windows` crate；或先做 UI 壳对接 `RazerAppEngine` |
| **拖拽重排列表项** | 弱 | `Dock` 有面板拖拽；`List` 没有 item 重排 | 用 `Dock` 的思路自写，或不做拖拽只做上下移动按钮 |
| **声音/音量电平表** | 没有 | 无 audio meter | 自写（`canvas` + 定时刷新） |
| **RTL / 界面镜像** | **明确不支持自动镜像** | `website/docs/i18n.md:146`："GPUI Kit does not expose an application-wide RTL layout switch" | 与雷云无关，记录备查 |

另有两点限制：
- i18n 只影响字符串，**不格式化**日期/数字/货币/复数（`website/docs/i18n.md:148-150`），本地化数字要自己做。
- `locale` 是 GPUI change tracking 之外的全局态，`set_locale` 后必须 `cx.notify()` 或 `cx.refresh_windows()`（`website/docs/i18n.md:118-138`）。

---

## 4. 主题：暗色 + 雷蛇绿 #44D62C

### 4.1 主题系统的四个事实

1. `Theme` 是 GPUI `Global` 单例（`crates/component/src/theme/mod.rs:234`：`impl Global for Theme {}`）。
2. 读取：`ActiveTheme` trait → `cx.theme()`（`theme/mod.rs:44-53`），`App` 已实现。
3. `Theme` `Deref`/`DerefMut` 到 `ThemeColor`（`theme/mod.rs:220-232`），所以 `theme.primary` 与 `theme.colors.primary` 是同一字段。
4. 默认是 **Light**：`theme::init` 里 `Theme::change(ThemeMode::Light, None, cx)`（`theme/mod.rs:36-42`）。

### 4.2 运行时改色（推荐 API：`Theme::update`）

`theme/mod.rs:288-290` 签名 + `:255-290` 的文档（原文非常关键）：

```rust
pub fn update<R>(cx: &mut App, edit: impl FnOnce(&mut Theme) -> R) -> R
```

文档在 `:261-277` 明确说：主题里颜色存了**两份**（`colors` 纯色 + `tokens` 可含渐变的可渲染背景），Base 层还留了一份投影（滚动条 / 分隔把手）。直接改 `global_mut` 会让三者"漂移"（侧边栏文字用新色、背景用旧色）。**所以必须用 `Theme::update`**，它会在闭包返回后：
- 把改过的 `colors` 同步到对应 token（丢掉该字段的渐变 —— 因为你要的就是纯色），
- 把单独改过的 token 的纯色写回 `colors`，
- 重建 Base 投影，
- 调 `cx.refresh_windows()`（**不需要你自己再刷新窗口**）。

雷蛇绿实践（写法取自 `theme/mod.rs:851-871` 的测试 `editing_colors_updates_the_tokens_and_the_base_projection`，其中就是 `theme.colors.primary = primary;` / `theme.sidebar = sidebar;`）：

```rust
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{px, rgb};

// 切暗色
Theme::change(ThemeMode::Dark, None, cx);

// 换主色为雷蛇绿 #44D62C
Theme::update(cx, |theme| {
    theme.colors.primary = rgb(0x44D6_2C).into();          // #44D62C
    theme.colors.primary_foreground = rgb(0x0A0A_0A).into(); // 绿底上要深色字
    theme.colors.ring = rgb(0x44D6_2C).into();             // 焦点环
    // 侧边栏强调色（雷云左侧导航选中态）
    theme.sidebar_primary = rgb(0x44D6_2C).into();
    theme.sidebar_primary_foreground = rgb(0x0A0A_0A).into();
    theme.radius = px(6.);
});
```

⚠️ 注意 `theme/mod.rs:282-287`：**在同一个 `update` 闭包里同时改 `mode` 和颜色是没用的**（切 mode 会重新加载该 mode 的主题、覆盖颜色）。正确顺序是两个 `update`：先 `Theme::change`，再改色。

其它可用的入口：
```rust
Theme::set_scrollbar_mode(ScrollbarMode::Always, cx); // theme/mod.rs:377
Theme::sync_system_appearance(window, cx);            // :354
```

### 4.3 用主题文件（更适合"雷蛇皮肤"长期维护）

**JSON 结构**（`website/component/theme.md` 配套 + `crates/component/src/theme/schema.rs:22-82` + 实例 `themes/aurora.json:1-12`）：

```json
{
  "$schema": "https://github.com/longbridge/gpui-kit/raw/refs/heads/main/.theme-schema.json",
  "name": "Razer",
  "author": "razer_ui",
  "url": "https://github.com/longbridge/gpui-kit",
  "themes": [
    {
      "name": "Razer Dark",
      "mode": "dark",
      "radius": 6,
      "radius.lg": 8,
      "shadow": true,
      "font.size": 15,
      "mono_font.family": "Consolas",
      "colors": {
        "background": "#0A0A0B",
        "foreground": "#E8E8EA",
        "border": "#1F1F23",
        "input.border": "#2A2A30",
        "selection.background": "#44D62C33",
        "primary.background": "#44D62C",
        "primary.foreground": "#0A0A0B",
        "primary.hover.background": "#5BE03F",
        "primary.active.background": "#37B023",
        "ring": "#44D62C",
        "accent.background": "#16161A",
        "accent.foreground": "#E8E8EA",
        "muted.background": "#131316",
        "muted.foreground": "#8A8A93",
        "popover.background": "#121216",
        "popover.foreground": "#E8E8EA",
        "overlay": "#00000080",
        "sidebar.background": "#0D0D0F",
        "sidebar.foreground": "#A8A8B0",
        "sidebar.border": "#1F1F23",
        "sidebar.primary.background": "#44D62C",
        "sidebar.primary.foreground": "#0A0A0B",
        "sidebar.accent.background": "#44D62C22",
        "sidebar.accent.foreground": "#44D62C",
        "button.background": "#17171B",
        "button.hover.background": "#1E1E24",
        "button.active.background": "#24242B",
        "button.foreground": "#E8E8EA",
        "button.primary.background": "#44D62C",
        "button.primary.foreground": "#0A0A0B",
        "switch.background": "#2A2A30",
        "switch.thumb.background": "#0A0A0B",
        "slider.background": "#44D62C",
        "slider.thumb.background": "#E8E8EA",
        "progress.bar.background": "#44D62C",
        "caret": "#44D62C",
        "scrollbar.thumb.background": "#2A2A30",
        "scrollbar.thumb.hover.background": "#3A3A42",
        "tab.background": "#0A0A0B",
        "tab.active.background": "#0A0A0B",
        "tab.active.foreground": "#E8E8EA",
        "tab_bar.background": "#0A0A0B",
        "table.background": "#0A0A0B",
        "table.head.background": "#131316",
        "table.head.foreground": "#8A8A93",
        "table.row.border": "#1F1F23",
        "table.hover.background": "#16161A",
        "table.active.background": "#44D62C22",
        "list.background": "#0A0A0B",
        "list.hover.background": "#16161A",
        "list.active.background": "#44D62C22",
        "list.active.border": "#44D62C",
        "group_box.background": "#0F0F12",
        "title_bar.background": "#0D0D0F",
        "title_bar.border": "#1F1F23",
        "status_bar.background": "#0D0D0F",
        "status_bar.border": "#1F1F23",
        "description_list.label.background": "#131316",
        "description_list.label.foreground": "#E8E8EA",
        "danger.background": "#F04A4A",
        "success.background": "#44D62C",
        "warning.background": "#F0B429",
        "info.background": "#3AA8F0",
        "drop_target.background": "#44D62C33",
        "drag.border": "#44D62C",
        "skeleton.background": "#17171B",
        "chart.1": "#44D62C",
        "chart.2": "#3AA8F0",
        "chart.3": "#F0B429",
        "chart.4": "#B06CF0",
        "chart.5": "#F04A4A",
        "chart.grid": "#1F1F23",
        "chart.bullish": "green-600",
        "chart.bearish": "red-600"
      }
    }
  ]
}
```

**取值格式**（`crates/component/src/theme/color.rs` 的解析器 + `themes/aurora.json` 实例）：
- `#RRGGBB` / `#RRGGBBAA`
- `linear-gradient(135deg, #4F46E5, #06B6D4)` 以及 `linear-gradient(to right, red-500 25%, blue-600 75%)`（`schema.rs:1230-1238` 的测试就是这两个字符串）
- 具名的 Tailwind 风格色阶，如 `green-600` / `red-600`（`themes/aurora.json:51-52`）
- `$schema` 指向仓库的 `.theme-schema.json`（编辑器补全用；local 路径 `\.ref\gpui-kit\.theme-schema.json`）

**加载主题文件的两条路**：

(a) 最稳、最可控 —— 直接解析 `ThemeSet` 并应用（全部签名已核实）：

```rust
use gpui_kit::component::{Theme, ThemeConfig, ThemeSet};
use std::rc::Rc;

let set: ThemeSet = serde_json::from_str(include_str!("../themes/razer.json"))
    .expect("invalid theme json");
let dark: Rc<ThemeConfig> = Rc::new(
    set.themes.into_iter().find(|t| t.mode.is_dark()).expect("no dark theme"),
);

Theme::update(cx, |theme| theme.apply_config(&dark)); // schema.rs:1064 pub fn apply_config(&mut self, config: &Rc<ThemeConfig>)
```
`apply_config` 会：写入 `theme.dark_theme` / `theme.light_theme`、装 highlight 主题、按 config 覆盖 font_size / font_family / mono_font_* / radius / radius_lg / shadow、用 `ThemeColor::dark()` 作 fallback 解析全部颜色、设置 `theme.mode`（`schema.rs:1064-1109`）。

(b) 运行时可热改 —— 监听 `./themes/*.json`（`crates/component/src/theme/registry.rs:98-118`）：

```rust
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry};
use std::path::PathBuf;

ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, |cx| {
    Theme::change(ThemeMode::Dark, None, cx);
})?;
```
`watch_dir` / `load_themes_from_str`（`registry.rs:151`）/ `sorted_themes()`（`:126`）都可用来做"换肤"下拉框。注意 `registry.rs:12` 的默认主题来自 `crates/component/src/theme/default-theme.json`，仓库 `themes/` 下还附带 21 套配色（`themes/*.json`，含 catppuccin / gruvbox / tokyonight / solarized / macos-classic 等），可直接拿来当备选皮肤列表。

### 4.4 与雷蛇品牌色相关的可用 token（`crates/component/src/theme/theme_color.rs`）

`ThemeColor` 有 **140+** 个 `pub … : Hsla` 字段（实测 grep `^    pub [a-z_0-9]+: Hsla,` 共 140 命中），与雷蛇最相关的分组：

- 通用：`background` `foreground` `border` `input` `muted` `muted_foreground` `accent` `accent_foreground` `popover` `popover_foreground` `overlay` `ring` `caret` `selection` `link` `link_hover` `link_active`
- 主色家族：`primary` `primary_hover` `primary_active` `primary_foreground`；`secondary*`；`danger*`；`warning*`；`success*`；`info*`
- 侧边栏（雷云左导航）：`sidebar` `sidebar_foreground` `sidebar_border` `sidebar_accent` `sidebar_accent_foreground` `sidebar_primary` `sidebar_primary_foreground`
- 控件：`switch` `switch_thumb` `slider_bar` `slider_thumb` `progress_bar` `skeleton` `scrollbar` `scrollbar_thumb` `scrollbar_thumb_hover`
- 数据展示：`table*`（`table` `table_head` `table_head_foreground` `table_even` `table_hover` `table_active` `table_active_border` `table_row_border` `table_foot` `table_foot_foreground`）、`list*`（`list` `list_head` `list_even` `list_hover` `list_active` `list_active_border`）
- 图表：`chart_1..chart_5` `chart_bullish` `chart_bearish` `chart_grid`
- 窗口：`title_bar` `title_bar_border` `status_bar` `status_bar_border` `window_border`（**`window.border` 只在 Linux 生效**，`schema.rs:637-641`）
- 基础色阶（用于 fallback 推导）：`base.red/red.light/green/…/cyan_light`
- 圆角：`Theme::radius` / `radius_lg` / `radius_full()` / `radius_2xl..4xl`（`theme/mod.rs:551-575`），`theme.radius = px(0.)` 会把所有胶囊/圆形一起方化。

---

## 5. i18n（rust-i18n）

### 5.1 事实

- 组件用 `rust-i18n`，在 `crates/component/src/lib.rs:122` 一句搞定：
  ```rust
  rust_i18n::i18n!("locales", fallback = "en");
  ```
- 组件内置词条：`crates/component/locales/ui.yml`（8419 字节，`_version: 2`），实测包含的 locale：**`en`, `zh-CN`, `zh-HK`, `zh-TW`, `it`**（`AGENTS.md` 说默认只加 `en`/`zh-CN`/`zh-HK`，实际文件里还有 `zh-TW` 和 `it`；`website/docs/i18n.md:9` 也这么说）。
- 命名空间：**组件词条必须放在 `gpui_component` 下**（crate 名 `gpui-component` 的 `-`→`_`，`website/docs/i18n.md:25-44`）。
- 优先级：**应用词条优先**，缺 key 回落到组件内置（深合并，`i18n.md:68-87`）。
- 切换：`gpui_kit::component::{locale, set_locale}`（`crates/component/src/lib.rs:150-157`，直接转调 `rust_i18n::locale/set_locale`）。
- 要求 `rust-i18n ≥ 4.2`（`i18n.md:11`；仓库 workspace 用 `"4.2.0"`，`Cargo.toml:95`）。

### 5.2 给本项目加 zh-CN 的做法

`Cargo.toml` 加 `rust-i18n = "4.2"`，然后：

`src/main.rs`（或 crate root）：
```rust
rust_i18n::i18n!("locales", fallback = "en");
```
`src/lib.rs` / `main.rs` 启动处，**顺序很重要**：
```rust
app.run(move |cx| {
    use gpui_kit::component as gpui_component;
    // 必须在 gpui_kit::init 之前
    rust_i18n::extend!(gpui_component);
    gpui_kit::init(cx);

    // 用中文
    gpui_kit::component::set_locale("zh-CN");
});
```
`locales/ui.yml`（应用自己的词条，`zh-CN` 直接可用，**不需要**复制组件那 8KB）：
```yaml
_version: 2
app:
  shell:
    title: { en: "Razer (Rust)", zh-CN: "雷云 4 (Rust)" }
  device:
    dpi: { en: "Sensitivity (DPI)", zh-CN: "灵敏度 (DPI)" }
    polling: { en: "Polling Rate", zh-CN: "回报率" }
    lighting: { en: "Lighting", zh-CN: "灯光" }
    keymap: { en: "Customize", zh-CN: "自定义按键" }
    macros: { en: "Macros", zh-CN: "宏" }
    performance: { en: "Performance", zh-CN: "性能" }
    profiles: { en: "Profiles", zh-CN: "配置文件" }
    cooling: { en: "Cooling", zh-CN: "散热" }
    power: { en: "Power", zh-CN: "电源" }
  status:
    connected: { en: "Connected", zh-CN: "已连接" }
    offline: { en: "Not connected", zh-CN: "未连接" }
  action:
    save: { en: "Save", zh-CN: "保存" }
    reset: { en: "Reset", zh-CN: "重置" }
# 需要覆盖组件的日期/月份等词条时，写在这里（key 必须与组件 ui.yml 完全一致）：
gpui_component:
  Calendar:
    month.January:
      zh-CN: "一月"
```
应用侧取值：`rust_i18n::t!("app.device.dpi")`。

### 5.3 三个坑（都出自官方文档）

1. `extend!` **只在启动时调用一次**（`i18n.md:66`）。
2. 组件内置词条**只对组件自己的 `t!` 生效**；应用代码写 `t!("gpui_component.Calendar.month.February")` 拿不到（`i18n.md:89-101`）。
3. `set_locale` 本身**不触发重绘**，必须 `cx.notify()`（视图内）或 `cx.refresh_windows()`（`&mut App` 里），并且缓存过的菜单/标签要重建（`i18n.md:118-140`）。

---

## 6. Windows 构建前置与坑

### 6.1 工具链

来自 `website/docs/installation.md:27`（Windows 段）原文要点：
- Windows 10 或更高；
- **Visual Studio 2022 Build Tools 或 Community，必须勾选 "Desktop development with C++" 工作负载（含 MSVC 与 Windows SDK）**；
- **必须用 MSVC Rust 工具链，不要 GNU**；
- **CMake 必须在 PATH 上**（`cmake --version` 能跑）；
- Rust **≥ 1.92**（`installation.md:42`）。

本机实测：`rustup show` → `Default host: x86_64-pc-windows-msvc`，已装 `stable-x86_64-pc-windows-msvc` 等 20+ 个 toolchain；`rustc 1.98.1`。**CMake 是否在 PATH 未验证**（本机 `D:\cmake` 存在，但是否入 PATH 未检查）。

常见报错与处置（`installation.md:77-84`）：`link.exe` 缺失 / 找不到 Windows SDK → 确认 C++ 工作负载 + SDK，必要时在 VS Developer PowerShell 里构建。

### 6.2 必须复制的 `.cargo/config.toml`（栈大小）

`\.ref\gpui-kit\.cargo\config.toml` 全文：

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-arg=/STACK:8000000"]
```

这个文件只对 `cwd` 在仓库内的构建生效。**我们的项目不在仓库里，所以必须在 `D:\rust_test\razer_ui\.cargo\config.toml` 重建一份。** GPUI 的布局/文本/递归渲染在 Windows 上默认 1MB 栈容易溢出；仓库专门加了 8MB。

### 6.3 Debug 性能

见 §1.5 的 `[profile.dev.package]`。文档原话（`installation.md:89-99`）：Rust Debug 构建下 GPUI / 组件库 / 布局 / 文本渲染基本未优化，`cargo run` 会明显卡；上面的 profile **不会让编译更快**（反而更慢），只是让运行时可用。**这些 override 只在你自己的 workspace root 的 Cargo.toml 里生效。**

### 6.4 首次构建量

仓库 workspace 的依赖非常重（wgpu、resvg、taffy、rustybuzz、ttf-parser、tree-sitter、quickjs（仅 shell）、markdown、html5ever、reqwest…）。`installation.md:71` 明确说第一次构建"may take several minutes"。**建议第一次直接 `cargo build`（不要 `cargo run`），并留足时间/磁盘。**

### 6.5 `runtime_shaders`

- **`gpui-kit` 自己的 `[features]` 里没有 `runtime_shaders`**（`crates/kit/Cargo.toml:18-67` 全表核对）。
- 它写在**仓库 workspace 对 `gpui_platform` 的依赖**上（`Cargo.toml:69`）：`features = ["font-kit", "x11", "wayland", "runtime_shaders"]`。
- 因此走 path 依赖时会**自动**带上；走 crates.io 时 `gpui-kit` 已把同样的 feature 声明写进了发布版 manifest（`Cargo.toml:62-64` 的注释说明了这一点："These requirements are what the published gpui-kit crates carry to crates.io"）。
- **应用侧不要**自己去加 `gpui-pre-platform` 依赖来改 feature —— `installation.md:84` 明确警告："do not override one GPUI package to a different version"。
- 语义：`runtime_shaders` 决定着色器是运行时编译还是内嵌预编译（属于 GPUI 平台层；本文未深入源码验证其具体行为，**标为未验证**）。

### 6.6 平台相关依赖（会自动拉，无需手动装系统库）

- `crates/component` 的 Windows 段（`crates/component/Cargo.toml:192-206`）：`resvg 0.45.1` + `windows` crate（`Win32_Foundation` / `Graphics_GdiPlus` / `System_Com*` / `UI_Input_KeyboardAndMouse` / `UI_WindowsAndMessaging` …），用于**原生菜单**（`native_menu/windows.rs`）。除 VS/SDK 外**不需要**额外系统库。
- `crates/base` 的 Windows 段（`crates/base/Cargo.toml:71-73`）：`windows`（`Win32_Foundation`、`Win32_UI_WindowsAndMessaging`）用于通过 `SystemParametersInfoW` 读"减少动态效果"偏好。
- 仓库 workspace 把 `windows` 锁在 `0.58.0`，并带 `Wdk` / `Wdk_System` / `Wdk_System_SystemServices`（`Cargo.toml:113-115`）。

### 6.7 其它确认过的限制

- 无托盘（§3.4）。
- 无文件对话框（§3.4）。
- `Dock` 状态的持久化要**自己写文件**（`examples/dock/src/main.rs:39-42,96-106` 里就是应用侧写 `docks.json` + `cx.on_app_quit` 保存）。仓库把 `/docks.json`、`/profile.json` 放进 `.gitignore`，说明这是应用职责。
- 退出/关闭动作、快捷键、确认流也归应用（`AGENTS.md` 的 "Root View System" 段 + `crates/kit/src/lib.rs:144`）。

---

## 7. 推荐模块布局

### 7.1 先固定几个"事实基线"（免得设计跑偏）

本机实测（`C:\Program Files\Razer\...`、`C:\ProgramData\Razer\...`、服务、卸载项）：

| 观测 | 值 |
| --- | --- |
| 已安装版本 | `Razer Synapse 4.0.662`（卸载注册表项） |
| 技术栈 | **Electron/Chromium**：`app-4.0.662\RazerAppEngine.exe` = **176 MB**，同目录有 `icudtl.dat`、`chrome_*.pak`、`libEGL.dll`、`libGLESv2.dll`、`v8_context_snapshot.bin`、`resources\app.asar`（**92 MB**） |
| 本机模块 | `CommonDLL\mapping_engine.dll`（按键映射/宏引擎）、`RzEngineMon.exe`、`rzNotification.exe`、`RzPowerTool.exe`、`RzSecurityTool.exe`、`simple_service.dll`、`SysUtilsNative.dll` |
| 服务 | `Razer Elevation Service`（**Running**，提权用）、`Razer Game Manager Service 3`（**Stopped**，按游戏切配置） |
| 通知图标类别 | `battery-warning`、`cable-error`、`charging-paused`、`download`、`fan`、`fan-error`、`led-error`、`legacy-devices`、`power-error`、`pump-error`、`temperature-error` |
| 其它 | `C:\Program Files (x86)\Razer\RzS3WizardPkg`（Synapse 3 → 4 迁移向导）、`C:\ProgramData\Razer\{GameManager3, Synapse3}`、`C:\Program Files\Razer\RazerAppEngine\Apps` |

> **说明**：上面是**文件系统/服务层面的观测事实**。Synapse 各页面的具体 UI 布局与交互细节属于另一份逆向文档的范围，本文不臆造；§7.3 的映射表以"能观测到的功能域 + 行业通识"给出，并在每行注明所依赖的 gpui-kit 组件。

### 7.2 文件树

```
D:\rust_test\razer_ui\
├─ Cargo.toml
├─ Cargo.lock
├─ .cargo\
│  └─ config.toml                 # [target.x86_64-pc-windows-msvc] rustflags /STACK:8000000
├─ build.rs                       # (可选) 版本号 / 图标资源
├─ locales\
│  └─ ui.yml                      # 应用词条 (en + zh-CN)，组件覆盖放 gpui_component: 下
├─ themes\
│  └─ razer.json                  # ThemeSet: "Razer Dark" "Razer Light"
├─ assets\
│  ├─ icons\                      # 自研图标（设备线稿、品牌、热区底图）
│  │  ├─ device-mouse.svg
│  │  ├─ device-keyboard.svg
│  │  └─ razer-logo.svg
│  └─ devices\                    # 设备外观位图/SVG（画热区用）
├─ docs\
│  └─ re\                         # ← 本目录：逆向笔记
│     ├─ 01-… 02-… 03-…
│     └─ 04-gpui-kit-guide.md
└─ src\
   ├─ main.rs                     # application()/init()/Theme/菜单/开窗 —— 只做启动
   ├─ app.rs                      # AppShell: TitleBar + Sidebar + 内容区 + StatusBar
   ├─ routes.rs                   # Route enum（设备页 / 功能区），Sidebar 选中态驱动
   ├─ state\
   │  ├─ mod.rs                   # AppState: Global（cx.global::<AppState>()）
   │  ├─ device.rs                # Device / DeviceKind / Capabilities（能力位图决定显示哪些页）
   │  ├─ profile.rs               # Profile / ProfileSet / 按程序联动（对应 GameManager）
   │  ├─ lighting.rs              # Effect / Zone / Chroma 参数模型
   │  ├─ keymap.rs                # Binding / Macro / 分层（Fn 层、宏层）
   │  ├─ audio.rs                 # EQ / 音量 / 麦克风
   │  ├─ performance.rs           # DPI 档位、回报率、风扇曲线、省电
   │  └─ persistence.rs           # serde_json ↔ %APPDATA%\razer_ui\profiles.json
   ├─ service\
   │  ├─ mod.rs
   │  ├─ device_registry.rs       # 设备枚举（HID/USB）
   │  ├─ hid.rs                   # feature report / control transfer 封装
   │  ├─ effects.rs               # 灯效帧循环（cx.background_executor + smol Timer）
   │  ├─ telemetry.rs             # 温度/转速/电量采样（sysinfo / HID）
   │  └─ autostart.rs             # 开机自启（注册表 Run 键）+ 提权判断
   └─ ui\
      ├─ mod.rs
      ├─ shell.rs                 # Sidebar + 内容 + StatusBar 组装
      ├─ theme.rs                 # Theme 加载/切换/换肤封装
      ├─ i18n.rs                  # set_locale + 通知重绘
      ├─ pages\
      │  ├─ mod.rs
      │  ├─ dashboard.rs          # 设备总览（电池/固件/连接状态）
      │  ├─ lighting\
      │  │  ├─ mod.rs
      │  │  ├─ zones.rs           # 分区选择（ToggleGroup / 设备热区）
      │  │  ├─ effects.rs         # 效果列表 + 参数
      │  │  ├─ color.rs           # ColorPicker 封装 + 预设色板
      │  │  └─ studio.rs          # Chroma 多层编辑器（自研）
      │  ├─ keymap\
      │  │  ├─ mod.rs
      │  │  ├─ mouse.rs           # 鼠标按键映射
      │  │  ├─ keyboard.rs        # 键盘按键映射
      │  │  ├─ macros.rs          # 宏列表 + 编辑
      │  │  └─ capture.rs         # 键位录制（自研，见 widgets/key_capture.rs）
      │  ├─ performance\
      │  │  ├─ mod.rs
      │  │  ├─ dpi.rs             # Slider + NumberInput + 档位列表
      │  │  ├─ polling.rs         # Select（125/500/1000/8000 Hz）
      │  │  └─ fan_curve.rs       # 风扇/水泵曲线（plot + 自研拖点）
      │  ├─ audio\
      │  │  ├─ mod.rs
      │  │  ├─ eq.rs              # 多段 EQ（Slider 数组）
      │  │  └─ mixer.rs           # 音量/麦克风/侧音
      │  ├─ power.rs              # 省电/休眠/低电量阈值
      │  ├─ profiles.rs           # 配置文件列表（List / DataTable）+ 导入导出
      │  └─ settings.rs           # Settings/SettingPage/SettingGroup/SettingItem
      └─ widgets\
         ├─ mod.rs
         ├─ device_canvas.rs      # ★ 自研 Element：设备外观 + 可点热区（canvas + paint_path）
         ├─ key_capture.rs        # ★ 自研：Keystroke 录制（on_key_down → Kbd 展示）
         ├─ dpi_curve.rs          # ★ 自研：可拖点曲线
         ├─ macro_timeline.rs     # ★ 自研：宏序列时间轴
         ├─ color_swatch.rs       # ColorPicker 预设色板
         └─ telemetry_chart.rs    # AreaChart 封装（温度/转速/电量）
```

### 7.3 功能 → gpui-kit 组件映射

| 雷云功能域 | gpui-kit 组件 | 备注 / 自研量 |
| --- | --- | --- |
| 左侧设备/功能区导航 | `component::sidebar::{Sidebar, SidebarHeader, SidebarGroup, SidebarMenu, SidebarMenuItem, SidebarFooter, SidebarToggleButton, SidebarCollapsible}` | 直接可用；范例 `examples/sidebar/src/main.rs:84-130`（含 icon/offcanvas/none 三种折叠） |
| 顶部标题栏 + 窗口拖拽 + 最小化/最大化/关闭 | `component::TitleBar` + `TitleBar::window_options()` | 范例 `examples/window_title/src/main.rs` |
| 底部状态栏（连接状态 / 同步 / 版本） | `component::status_bar::StatusBar` | 左/中/右分区 |
| 设备选择（多设备切换） | `Tabs` / `TabBar`，或 `Carousel` | 设备多时建议 `Tabs` + `Badge` 显示电量 |
| 设备总览卡片 | `GroupBox` + `DescriptionList` + `Badge` + `Progress` | 固件版本、电量用 `Progress`/`Badge` |
| 设置页（开关/下拉/数值/文本） | **`component::setting::*`**：`Settings`/`SettingPage`/`SettingGroup`/`SettingItem`/`SettingField::{switch, checkbox, dropdown, scrollable_dropdown, input, number_input, element, render}` | **几乎 1:1**，见 `crates/story/src/stories/settings_story.rs`；设置页带搜索框、sidebar、reset 按钮 |
| 灯效：颜色选择 | `ColorPicker` / `ColorSelect`（`ColorPickerState` + `ColorPickerEvent::Change`） | `theme/mod.rs` 里没有的渐变编辑要自研 |
| 灯效：开关 / 分区启用 | `Switch`（`.color()` 可指定强调色）/ `Checkbox` / `ToggleGroup` | |
| 灯效：分区选择网格（如 12 区键盘背光） | `ToggleGroup` 或自写 div 网格 | 键位形状不规则 → 走 `device_canvas.rs` |
| 灯效：亮度 / 速度 | `Slider`（`SliderState::new().min().max().step().default_value()`；也支持 `scale(SliderScale::Logarithmic)`） | 范例 `examples/brush/src/main.rs:36-50` |
| 灯效：效果选择（光谱/呼吸/波浪/涟漪/星光/静态/自定义） | `Select` 或 `Combobox`（效果名多/可搜时） | |
| 灯效：多层/Chroma Studio | **无** | ★ 全自研（`widgets/macro_timeline.rs` 那种时间轴 + `color_swatch.rs`） |
| 按键映射：当前绑定列表 | `List`（`ListDelegate`）或 `DataTable` | 表格化用 `DataTable`（列排序 + 虚拟滚动） |
| 按键映射：录制新键 | **无** | ★ 自研 `key_capture.rs`，展示用 `Kbd` |
| 按键映射：设备热区点击选键 | **无** | ★ 自研 `device_canvas.rs`（照抄 `examples/brush` 的 `canvas` + `prepaint`/`paint`） |
| 宏：宏列表 / 新建 / 重命名 / 删除 | `List` + `Button` + `Dialog`（重命名）/ `AlertDialog`（删除确认） | |
| 宏：步骤编辑（按键/延时/循环） | `DataTable` 或 `List` + `NumberInput`（延时 ms）+ `Kbd`（键） | 时间轴可视化要自研 |
| 宏：绑定到按键 | `SettingField::element` 或 `Combobox` | |
| DPI：档位与数值 | `Slider` + `NumberInput` + `RadioGroup`（选当前档） | |
| DPI：X/Y 独立、抬升距离、加速度 | `SettingField::number_input` + `NumberFieldOptions` | |
| 回报率 (Polling Rate) | `Select` / `SettingField::dropdown` | |
| 风扇/水泵曲线 | `chart::LineChart` / `AreaChart` + `plot`（`Grid`/`PlotAxis`/`PlotLabel`/`ScaleLinear`）+ ★ 自研拖点 | `crates/component/src/plot/mod.rs:6` 直接 re-export base 的 `PlotElement`/`Grid`/`ScaleLinear` 等 |
| 实时遥测（CPU/GPU 温度、风扇 RPM、电量） | `chart::AreaChart` + `DataTable` + `StatusBar` | **直接抄** `examples/system_monitor/src/main.rs`（`sysinfo` + `AreaChart` + `TableDelegate`） |
| 音频：均衡器多段 | `Slider` 数组（垂直 `Slider::vertical()`） | |
| 音频：混音/侧音/麦克风 | `Slider` + `Switch` + `Select` | |
| 电源/省电：低电量阈值、休眠、LED 亮度 | `Slider` + `NumberInput` + `Switch` | |
| 配置文件：列表 / 切换 / 复制 / 删除 | `List` 或 `DataTable` + `DropdownButton`（每行操作菜单） | `DropdownButton` 见 `website/component/dropdown_button.md` |
| 配置文件：按程序自动切换（GameManager） | `DataTable`（程序列表）+ `Button`（添加）+ ★ 需要进程监听 | |
| 手动切换提示 / 已切换 profile | `Notification`（`window.push_notification`，含 `.system()` 走系统通知） | `rzNotification.exe` 对应的能力 |
| 固件升级进度 | `Progress`（`Progress`/`ProgressCircle`）+ `Notification` | |
| 首次配置向导 / 多步问卷 | `Stepper` + `Questionnaire` | |
| 设备未连接 / 空状态 | `Empty` + `Skeleton`（加载中）+ `Spinner` | |
| 命令面板（快速跳转设置项） | `Command`（`CommandState`，内置快捷键匹配） | |
| 全局搜索设置项 | `Settings` 自带搜索框（`crates/component/src/setting/settings.rs:140-153`） | |
| 右键菜单（设备/配置文件） | `menu::context_menu` / `ContextMenu` | |
| 需要多面板/可停靠的高级布局 | `component::dock::*`（`DockArea`/`DockSkin`/`PanelRegistry`/`DockAreaState` + 序列化） | 范例 `examples/dock/src/main.rs`（含 `DockEvent::LayoutChanged` → 存盘、`cx.on_app_quit` 保存） |
| 长列表性能（宏列表、灯光预设、很多设备） | `VirtualList` / `List`（`ListDelegate`）/ `DataTable` | |
| 帮助/关于/许可 | `Dialog` + `TextView`（Markdown 内置渲染） | |

### 7.4 启动与退出的骨架（把 §2.5 扩成真实结构）

```rust
// src/main.rs
rust_i18n::i18n!("locales", fallback = "en");

mod app;
mod routes;
mod service;
mod state;
mod ui;

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx| {
        use gpui_kit::component as gpui_component;
        rust_i18n::extend!(gpui_component);   // 必须在 init 之前
        gpui_kit::init(cx);

        ui::theme::install(cx);               // Theme::change + Theme::update(雷蛇绿) 或 apply_config
        gpui_kit::component::set_locale("zh-CN");

        let options = WindowOptions {
            window_min_size: Some(size(px(1080.), px(720.))),
            ..gpui_kit::component::TitleBar::window_options()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| app::AppShell::new(window, cx))
        })
        .expect("Failed to open window");
    });
}
```
退出前保存（抄 `examples/dock/src/main.rs:96-106`）：
```rust
cx.on_app_quit(move |_, cx| {
    let state = /* dump */;
    cx.background_executor().spawn(async move { /* 写 profiles.json */ })
}).detach();
```

---

## 8. 明确的不确定项 / 未验证项

1. **`gpui-kit 0.7.0` 是否已发布到 crates.io** —— 无法确认（本机索引缓存里根本没有 `gpui-kit` 条目）。因此本文推荐 path 依赖。同理 `gpui-base` / `gpui-kit-assets`。
2. **本机完全没有 `gpui-pre*` 的索引缓存与 `.crate` 文件**，且 cargo 无法出网（Clash fake-ip + schannel `SEC_E_NO_CREDENTIALS`）。**任何形式的构建在当前环境下都会失败**，与选 (a) 还是 (b) 无关。必须先解决代理。
3. **跨 workspace 的 path 依赖 + workspace 继承**（`gpui.workspace = true` 这类）由 cargo 按被依赖包自己的 workspace root 解析 —— 这是 cargo 的既定行为，但本文**没有实机构建验证**（无网络）。
4. `Cargo.lock` 复用技巧（把仓库 lock 复制到项目根）是常规做法，**未实测**。
5. `WindowOptions` 的 `window_min_size` 字段名取自 `crates/component/src/title_bar.rs:77` 的文档示例；`Size<Pixels>` 的类型匹配**未逐字段核对** gpui-pre 0.3.7 的 RNA。若报错，退化为只用 `window_bounds: Some(WindowBounds::centered(...))`（该写法已在 `examples/sidebar/src/main.rs:198` 实测）。
6. `runtime_shaders` 的**语义**（运行时编译 vs 预编译）本文未深入 GPUI 源码确认，只确认了它写在 `gpui_platform` 的 feature 上。
7. CMake 是否在本机 PATH 上未检查；VS2022 C++ 工作负载是否安装未检查（`installation.md` 要求）。
8. 仓库 HEAD（`caf830c0`，2026-09-29）**没有任何 git tag**，所以"0.7.0"是 manifest 里的版本号，不代表已打 tag 发布。用它 path 依赖等于锁定这个 commit。
9. §7.1 的 Synapse 观测只到"文件/服务层面"；具体页面构成、控件行为、IPC 协议需要另一份逆向文档，本文不含。
10. `crates/component/locales/ui.yml` 的 locale 集合实测为 `en/it/zh-CN/zh-HK/zh-TW`，与 `AGENTS.md` 所述"默认只加 en、zh-CN、zh-HK"略有出入（多出 `it` 与 `zh-TW`）。

---

## 9. 证据索引（文件 → 结论）

| 文件 | 用到的结论 |
| --- | --- |
| `\.ref\gpui-kit\Cargo.toml:49-58,61-73,113-115` | path 依赖表、`gpui-pre =0.3.7` 精确 pin 与理由、`windows 0.58` + Wdk |
| `\.ref\gpui-kit\Cargo.lock` | 27 个 `gpui-pre*` = 0.3.7 条目齐全 |
| `\.ref\gpui-kit\.cargo\config.toml` | Windows `/STACK:8000000` |
| `crates\kit\src\lib.rs` | `pub use ::gpui::*`、`open_window`、`init`、`actions!` 宏、`test` 模块 |
| `crates\kit\Cargo.toml` | feature 全表（默认 `component`+`assets`），无 `runtime_shaders` |
| `crates\component\src\lib.rs` | 全部 `pub mod`（56 个）、`init(cx)` 内容、`locale`/`set_locale`、`rust_i18n::i18n!` |
| `crates\base\src\lib.rs` | base 的公开导出（`Root`、`Theme`、`Slider`、`Toast`、`Tree`、`dock`、`plot`…）与 `init(cx)` |
| `crates\base\Cargo.toml` | base 的依赖与 Windows 段（`SystemParametersInfoW`） |
| `crates\component\Cargo.toml` | 组件依赖、tree-sitter feature 全表、Windows 段（resvg + gdiplus） |
| `crates\component\src\theme\mod.rs` | `Theme` 全局、`Theme::update/change/sync_base/apply_config` 路径、`ThemeMode`、`ActiveTheme`、radius 族 |
| `crates\component\src\theme\schema.rs` | `ThemeSet`/`ThemeConfig`/`ThemeConfigColors`、`apply_config`、`window.border` 仅 Linux、渐变解析 |
| `crates\component\src\theme\registry.rs` | `ThemeRegistry::watch_dir/load_themes_from_str/sorted_themes`、默认主题来源 |
| `crates\component\src\theme\theme_color.rs` | 140+ `Hsla` token 全表（含 sidebar/`table*`/`list*`/`chart*`/`base.*`） |
| `themes\aurora.json:1-60` | 主题文件结构、`green-600` 具名色、`linear-gradient(...)` 值、`$schema` |
| `.theme-schema.json`（仓库根） | 主题 JSON 的 schema 入口 |
| `crates\component\src\icon.rs` | `IconName` 兼容枚举（仅默认图标）、`Icon::path/data/new`、`IconNameExt` |
| `crates\assets\README.md:11-16,51-53` | 101 vs 1830 图标预算（+1.02 MiB）、`icon_assets!`、全目录含 1818 Lucide |
| `crates\assets\default-icons.txt` | 默认 104 行（101 个 SVG），**不含** gamepad/mouse/keyboard |
| `examples\hello_world\src\main.rs` | 最小可运行样板原文 |
| `examples\sidebar\src\main.rs` | 侧边栏全套用法 + `WindowBounds::centered` |
| `examples\app_assets\src\main.rs` + `README.md` | 资产注册矩阵与自定义 `AssetSource` |
| `examples\window_title\src\main.rs` + `crates\component\src\title_bar.rs:15,59-91` | 自定义标题栏与 `window_options()` |
| `examples\system_monitor\src\main.rs:1-140` | `AreaChart` + `DataTable`/`TableDelegate` + `TabBar` + `sysinfo` 的实时监控范式 |
| `examples\brush\src\main.rs:36-50,196-280` | `SliderState` 用法 + `canvas`/`prepaint`/`paint_path` 自绘范式（设备热区的基础） |
| `examples\dock\src\main.rs:39-42,70-106` | `DockSkin::dock_area`、`DockEvent::LayoutChanged`、`cx.on_app_quit` 存盘 |
| `examples\ai_recipes\README.md` + `Cargo.toml` | "只依赖 gpui-kit 的消费方 crate"的官方定位 |
| `website\docs\installation.md` | Windows 前置（VS2022 C++、CMake、MSVC、Rust ≥1.92）、`[profile.dev.package]`、故障表 |
| `website\docs\getting-started.md:56-120` | 启动三段式、完整 Settings 配方 |
| `website\docs\i18n.md` | rust-i18n 集成、`extend!` 顺序、命名空间、重绘要求、RTL 不支持 |
| `website\component\index.md` | 官方 67 项组件目录原文 |
| `website\component\*.md`（78 个，含 index） | 目录漏掉的 10 个组件页 |
| `crates\component\src\setting\fields\mod.rs:154-270` | `SettingField::{switch,checkbox,input,dropdown,scrollable_dropdown,element,render,number_input}` 精确签名 |
| `crates\story\src\stories\settings_story.rs:131-230` | 设置页端到端范例（Global 状态 + 主题联动） |
| `crates\component\src\window_ext.rs:13-70` | `WindowExt`：`open_dialog/open_alert_dialog/open_sheet/push_notification/close_*` |
| `crates\component\src\dock\mod.rs:32-59,123-195` | Dock 全部公开导出 + `DockSkin` |
| `crates\component\src\chart\mod.rs:9-16` | 图表实际导出：Area/Bar/Candlestick/Line/Pie/**Radar**/**Sankey** |
| `crates\component\src\kbd.rs:13,30-41,105` | Kbd 只做展示与格式化，**无录制能力** |
| `C:\Program Files\Razer\...`（本机） | Synapse 4.0.662 = Electron；模块/服务/通知类别清单（§7.1） |
