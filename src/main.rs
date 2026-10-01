//! 雷云 4 的 Rust 原生替代 UI。
//!
//! 原版 UI 规格见 `docs/RAZER-SYNAPSE-UI-SPEC.md`，IPC 契约见 `docs/re/01-ipc-api-surface.md`。
//!
//! 架构现状（方案 2 的起点）：
//!   UI（gpui-kit）→ 本地配置；DLL 探测/调用仍由显式命令行路径触发。
//!
//! 目前 UI 跑在本机实测的设备快照上；引擎 DLL 的加载探测走命令行。

mod backend;
mod demo;
mod domain;
mod features;
mod i18n;
mod model;
mod nav;
mod preferences;
mod resources;
mod shell;
mod store;
mod ui;

// i18n：与 gpui-component 内部用的是同一套 rust-i18n。
//
// **必须放在 crate 根**：`t!` 展开后会引用 `crate::_rust_i18n_try_translate`，
// 而那个函数由本宏生成在当前模块；放在子模块里会导致其它模块找不到它。
//
// 语言文件在 `locales/`，按 rust-i18n 约定以**文件名作为语言代码**。
rust_i18n::i18n!("locales", fallback = "en");

use gpui_kit::component::{Theme, ThemeMode, TitleBar};
use gpui_kit::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--service-worker") {
        std::process::exit(backend::runtime::run_worker());
    }

    // 语言：`--lang <代码>`，默认中文。
    //
    // 用 rust-i18n 的全局语言设置（gpui-component 内部也是这一套），
    // 语言文件在 `locales/`。放在最前面，让自检等无窗路径也用同一语言。
    let lang = args
        .iter()
        .position(|arg| arg == "--lang")
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| {
            store::read_workspace(&store::store_path())
                .ok()
                .flatten()
                .map(|file| file.preferences.language)
                .unwrap_or_else(|| "zh-CN".to_string())
        });
    i18n::set_locale(&lang);

    // `--probe [引擎名]`：加载引擎 DLL 并统计符号后退出。
    //
    // 必须放在**独立进程**里逐个调用：第三方 DllMain 可能阻塞
    // （实测 `SysUtilsNative.dll` 会永久挂住），所以这条路径刻意与 UI 隔离。
    if let Some(pos) = args.iter().position(|arg| arg == "--probe") {
        use std::io::Write as _;

        let filter = args.get(pos + 1).filter(|arg| !arg.starts_with("--"));
        if backend::catalog().is_empty() {
            println!("引擎目录为空");
            return;
        }

        for spec in backend::catalog() {
            if let Some(name) = filter {
                if spec.stem != name.as_str() {
                    continue;
                }
            }
            // 已知会阻塞的引擎直接跳过，避免整个进程被挂住。
            if backend::blocks_load(spec.stem) {
                println!("探测 {:<20} ... 跳过（已知加载会阻塞）", spec.stem);
                if let Some(path) = backend::resolve_path(spec.stem) {
                    println!("      {}", path.display());
                }
                continue;
            }

            // 先打印再加载：这样即使卡住，也能从输出看出卡在哪个 DLL。
            print!("探测 {:<20} ... ", spec.stem);
            let _ = std::io::stdout().flush();

            let status = backend::probe_engine(spec);
            println!("{}", status.summary());
            if let Some(path) = &status.path {
                println!("      {}", path.display());
            }
        }
        return;
    }

    // `--lighting <子命令>`：方案 2 的引擎调用入口。
    //
    // 与 `--probe` 一样刻意放在**独立进程、UI 之外**：这些调用会 LoadLibrary
    // 并执行第三方 `DllMain`，可能阻塞；实测 `SysUtilsNative.dll` 会永久挂住。
    //
    // 子命令：
    //   version                              只读 DLL 版本，无副作用
    //   handshake <handle> [identifier]      跑一遍 注册→恢复→注销，**不下发效果**
    if let Some(pos) = args.iter().position(|arg| arg == "--lighting") {
        use std::io::Write as _;

        use backend::lighting::LightingDriver;

        let sub = args.get(pos + 1).map(String::as_str).unwrap_or("version");
        let paths = backend::engine_paths();

        // 先打印再加载：万一 DllMain 阻塞，也能从输出看出卡在哪一步。
        print!("加载 lighting_driver ... ");
        let _ = std::io::stdout().flush();
        let mut driver = match LightingDriver::load(&paths) {
            Ok(driver) => {
                println!("成功");
                driver
            }
            Err(err) => {
                println!("失败：{err}");
                return;
            }
        };
        println!("      {}", driver.path().display());

        match sub {
            "version" => match driver.version() {
                Some(version) => println!("GetDllVersion() = {version}"),
                None => println!("GetDllVersion() 无返回"),
            },
            "handshake" => {
                let handle = args
                    .get(pos + 2)
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(1);
                let identifier = args
                    .get(pos + 3)
                    .cloned()
                    .unwrap_or_else(|| "razer_ui_probe".to_string());

                println!("handshake: device_handle={handle} device_identifier={identifier}");
                println!("      该序列**不下发任何效果**，不会覆盖当前灯效。");
                println!("      但雷云若正在运行会争用设备，建议先退出雷云。");
                let _ = std::io::stdout().flush();

                match driver.handshake(handle, &identifier) {
                    Ok(steps) => {
                        for step in steps {
                            println!("      {step}");
                        }
                        println!("会话正常（注册 / 恢复 / 注销均已返回）");
                    }
                    Err(err) => println!("handshake 失败：{err}"),
                }
            }
            other => println!("未知子命令：{other}（可用：version / handshake）"),
        }
        return;
    }

    // `--selftest`：在不开窗的情况下验证功能层与持久化往返。
    //
    // 说明：真正的配置路径是 `%APPDATA%\razer_ui\profiles.json`，
    // 但自检写入显式路径，便于在受限环境里也能跑通。
    if args.iter().any(|arg| arg == "--selftest") {
        use domain::{Macro, MacroStep};

        // 语言包来自雷云真实前端，这里顺便验证它确实被载入了。
        println!("当前语言：{}", i18n::locale());
        for key in [
            "TAB_HOME",
            "TAB_CUSTOMIZE",
            "TAB_PERFORMANCE",
            "TAB_PAIRING",
            "DEBOUNCE_MODE",
            "BOSS_KEY",
        ] {
            println!("    {key} = {}", i18n::t(key));
        }
        println!();

        let mut devices = model::measured_devices();
        devices.push(demo::demo_keyboard());
        println!("设备数：{}（含合成演示键盘）\n", devices.len());

        for device in &mut devices {
            device.fill_defaults();
            let (min, max, step) = device.dpi_bounds();
            println!(
                "{} (productId {})",
                device.display_name(),
                device.product_id
            );
            println!(
                "    类别={:?}  鼠标={}  键盘={}  Chroma={}  电池={}",
                device.category,
                device.is_mouse(),
                device.is_keyboard(),
                device.is_chroma_device,
                device.has_battery
            );
            println!(
                "    DPI 范围 {min}–{max} 步进 {step}；夹取测试 clamp(1)={} clamp(999999)={}",
                device.clamp_dpi(1),
                device.clamp_dpi(999999)
            );
            println!(
                "    轮询率={}  抬升={:?}  键盘设置={}  灯光区域={}  电源={}  Hypershift 绑定={}",
                device.features.performance.polling_rate.label(),
                device.features.performance.lift_off.map(|v| v.zh()),
                device.features.keyboard.is_some(),
                device.features.lighting.len(),
                device.features.power.is_some(),
                device.features.hypershift_bindings.len(),
            );

            // 该设备类别应显示的标签页（实测自设备模块的常量块），
            // 以及对应功能块是否就位——这两者必须一致。
            let tabs = nav::Tab::for_product(device.product_id);
            let names: Vec<String> = tabs
                .iter()
                .map(|tab| format!("{}({})", tab.label(), tab.id()))
                .collect();
            println!(
                "    标签页 {} 个：{}",
                names.len(),
                if names.is_empty() {
                    "（该设备类别尚未逆向设备模块）".to_string()
                } else {
                    names.join(" · ")
                }
            );
            println!(
                "    对应功能块：滚动={} 校准={} 配对={}",
                device.features.scrolling.is_some(),
                device.features.calibration.is_some(),
                device.features.pairing.is_some(),
            );

            // 制造一处改动，用来验证序列化。
            if let Some(stages) = device.dpi_stages_mut() {
                stages.enable = true;
                if let Some(first) = stages.stages.first_mut() {
                    first.x = 1234;
                    first.y = 1234;
                }
            }
            device.features.macros.push(Macro {
                name: "自检宏".to_string(),
                steps: vec![MacroStep {
                    delay_ms: 20,
                    action: "key_down:A".to_string(),
                }],
                loop_until_release: true,
            });
            device.features.hypershift_enabled = true;
        }

        let path = std::path::Path::new(".ref/notes/selftest-store.json");
        match store::save_to(path, &devices) {
            Ok(()) => println!("\n已写入 {}", path.display()),
            Err(err) => {
                println!("\n写入失败：{err}");
                return;
            }
        }

        match store::load_from(path) {
            Some(reloaded) => {
                println!("回读成功，设备数：{}", reloaded.len());
                for device in &reloaded {
                    println!(
                        "    {}  宏={}  Hypershift={}  已初始化={}  DPI首档={:?}",
                        device.display_name(),
                        device.features.macros.len(),
                        device.features.hypershift_enabled,
                        device.features_initialized,
                        device
                            .active_profile_obj()
                            .and_then(|p| p.dpi_stages.as_ref())
                            .and_then(|s| s.stages.first())
                            .map(|s| s.x),
                    );
                }
            }
            None => println!("回读失败：文件无法解析"),
        }
        return;
    }

    // 注册图标源：**必须用 `AllAssets`**（完整 Lucide 图标集，1830 个 SVG）。
    // 默认的 `Assets` 只有少量图标，用它会导致大部分 `IconName` 渲染为空白。
    let application = gpui_kit::application().with_assets(resources::SynapseAssets);

    application.run(move |cx| {
        // 使用任何组件前必须先初始化。
        gpui_kit::init(cx);
        if let Err(error) = resources::register_fonts(cx) {
            eprintln!("字体加载失败：{error}");
        }

        // 暗色主题 + 雷蛇配色。
        //
        // 下列数值**逐条引自雷云设备模块的 CSS**（`.ref/devices/*/static/css/`），
        // 不是自拟的：
        //
        // ```css
        // .main-container      { background-color:#222 }
        // .body-wrapper        { background-image:radial-gradient(ellipse,#0000,#222 70%) }
        // .body-widgets .widget{ background-color:#111; border-radius:5px; font-size:14px }
        // .razer-button        { background-color:#44d62c; border:1px solid #000;
        //                        border-radius:3px; color:#000; text-transform:uppercase }
        // .button              { background:#707070; border:1px solid #0000004d;
        //                        border-radius:3px; font-family:Roboto; font-size:12px;
        //                        text-transform:uppercase }
        // .switch-button       { background-color:#333; border:1px solid #5d5d5d;
        //                        color:#ccc; font-size:14px; text-transform:uppercase }
        // .switch-button.active{ background-color:#44d62c; border-color:#44d62c; color:#111 }
        // .volume-item         { color:#999 }
        // .active              { border-color:#44d62c }
        // .nav:hover           { background-color:#2d2d2d }
        // .s3-dropdown         { border:1px solid #515151 }
        // ```
        //
        // **修正记录**：此前把通用边框写成 `#555`，全量统计后确认真实值是
        // **`#5d5d5d`（出现 157 次）**，`#555` 只有 7 次。
        // 另外此前**完全没设字体**——真实是 `Roboto`，标题用 `RazerF5`。
        //
        // 见 [`docs/screens/`](../docs/screens/README.md) 各页的「2.0.1 全局样式」。
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |theme| {
            // 字体与字号：`body,html { font-family:Roboto,sans-serif; font-size:16px }`，
            // 进入 `.widget` 后是 14px（由 `widgets::card()` 显式设回）。
            //
            // 基字号取真实的 **16px**：gpui-kit 的字号是从 `theme.font_size`
            // 按比例派生的（`sizing.rs:239` 的 `×0.875`），所以它**决定不了**
            // 具体档位，只能靠调用点显式指定（见 `widgets::geometry::TEXT_*`）。
            // 之前这里写 14px，与上面这行注释本身矛盾。
            theme.font_family = "Roboto".into();
            theme.font_size = px(16.);

            // 页面底色 #222，卡片 #111
            theme.colors.background = rgb(0x222222).into();
            theme.colors.title_bar = rgb(0x000000).into();
            theme.colors.title_bar_border = rgb(0x000000).into();
            theme.colors.group_box = rgb(0x111111).into();
            theme.colors.group_box_foreground = rgb(0xCCCCCC).into();
            theme.colors.secondary = rgb(0x333333).into();
            theme.colors.secondary_hover = rgb(0x2D2D2D).into();
            theme.colors.popover = rgb(0x111111).into();

            // 文字 #ccc，次要文字 #999（.volume-item{color:#999}）
            theme.colors.foreground = rgb(0xCCCCCC).into();
            theme.colors.muted_foreground = rgb(0x999999).into();
            theme.colors.list_hover = rgb(0x383838).into();
            theme.colors.slider_bar = rgb(0x44d62c).into();
            theme.colors.slider_thumb = rgb(0x44d62c).into();

            // 边框：**通用 #5d5d5d（157 次）**，下拉框 #515151
            theme.colors.border = rgb(0x5D5D5D).into();
            theme.colors.input = rgb(0x515151).into();
            theme.colors.sidebar_border = rgb(0x5D5D5D).into();

            // 按钮：默认 #707070 底 + #fff 字（`.thx-btn.secondary`）
            theme.colors.button = rgb(0x707070).into();
            theme.colors.button_foreground = rgb(0xFFFFFF).into();
            theme.colors.button_secondary = rgb(0x333333).into();
            theme.colors.button_secondary_hover = rgb(0x2D2D2D).into();
            theme.colors.button_secondary_foreground = rgb(0xCCCCCC).into();

            // 主色：雷蛇绿 #44d62c，前景黑（.thx-btn{background-color:#44d62c;color:#000}）
            theme.colors.primary = rgb(0x44D62C).into();
            // .nav.active{background-color:#44d62c;color:#111} —— 主色上的前景是 #111
            theme.colors.primary_foreground = rgb(0x111111).into();
            theme.colors.button_primary = rgb(0x44D62C).into();
            theme.colors.button_primary_foreground = rgb(0x000000).into();
            // 按钮的悬停/按下来自**透明度**，不是显式色值：
            //   `.thx-btn:hover { opacity:.8 }`、`.thx-btn:active { opacity:.6 }`
            // 因此按底色 #222 混合换算：
            //   hover : .8*#44d62c + .2*#222 = #3db22a
            //   active: .6*#44d62c + .4*#222 = #368e28
            theme.colors.button_primary_hover = rgb(0x3DB22A).into();
            theme.colors.button_primary_active = rgb(0x368E28).into();
            // 注：`.nav-tabs .nav:active` 用的是**另一个**显式值 `#3cbf27`
            // （产品级「主色按下」角色，全站 14 处）。它不等于上面的
            // `button_primary_hover`，因此不能共用令牌——
            // 见 `src/app.rs` 的 `NAV_ACTIVE_BG`。
            theme.colors.accent = rgb(0x44D62C).into();
            theme.colors.ring = rgb(0x44D62C).into();
            theme.colors.sidebar = rgb(0x222222).into();
            theme.colors.sidebar_foreground = rgb(0xCCCCCC).into();
            theme.colors.sidebar_primary = rgb(0x44D62C).into();
            theme.colors.sidebar_primary_foreground = rgb(0x111111).into();

            // 语义色：橙 #fd8611（提示/警告/分享）、红 #c8323c（危险）
            theme.colors.warning = rgb(0xFD8611).into();
            theme.colors.danger = rgb(0xC8323C).into();
            theme.colors.success = rgb(0x44D62C).into();

            // 开关。真实规格（`.ref/devices/777/static/css/main.e4bab2aa.css`）：
            //
            //   .switch        { width:32px; height:18px; padding:2px;
            //                    background-color:#707070;
            //                    border:1px solid #0000004d; border-radius:16px }
            //   .switch.on     { background-color:#44d62c }
            //   .switch .handle{ width:14px; height:14px;
            //                    background-color:#111; border-radius:7px }
            //   .switch:hover  { opacity:.7 }
            //   .switch.disabled { opacity:.3 }
            //
            // 关态轨道是 **#707070**（不是 #333），滑块是 **#111**（不是 #ccc）。
            // 打开态由 `primary`（#44d62c）承担。
            theme.colors.switch = rgb(0x707070).into();
            theme.colors.switch_thumb = rgb(0x111111).into();

            // 圆角：雷云是**两级**，正好对上主题的命名档位。
            //   .thx-btn { border-radius:3px }  → radius    → tokens.md（控件）
            //   .widget  { border-radius:5px }  → radius_lg → tokens.lg（卡片）
            // 见 docs/RAZER-SYNAPSE-UI-SPEC.md §3、§5。
            theme.radius = px(3.);
            theme.radius_lg = px(5.);
        });

        let window_options = WindowOptions {
            // 显式给初始尺寸，否则会退回到很小的默认值。
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
            window_min_size: Some(size(px(1080.), px(720.))),
            ..TitleBar::window_options()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| shell::AppShell::new(window, cx))
        })
        .expect("Failed to open window");
    });
}
