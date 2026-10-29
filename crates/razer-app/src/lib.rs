//! 雷云 4 的 Rust 原生替代 UI。
//!
//! 当前架构、来源和验证边界见 `docs/README.md`。
//!
//! UI 编辑使用本地草稿；启动及 USB/HID 拓扑变化会触发后台设备发现。
//! 经当前源核实的原生查询由共享本地服务执行，结果通过观察事件进入 UI。
//! 接口存在不等于设备配置已读取；历史快照和静态能力不能冒充当前读值。
//! 执行限制和仅限维护工具的原厂对比测试见 AGENTS.md。

mod arguments;
mod diagnostics;
mod service_runtime;

use razer_assets as resources;
use razer_i18n as i18n;
use razer_service as backend;
use razer_shell::shell;
use razer_storage::workspace as store;

// Localization and source assets are owned by their dedicated library crates.

use gpui_kit::component::{Theme, ThemeMode, TitleBar};
use gpui_kit::*;

pub fn run() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if let Err(error) = arguments::validate(&args) {
        eprintln!("{error}");
        return std::process::ExitCode::from(2);
    }
    if let Some(operation) = arguments::driver_installation(&args) {
        let result = match operation {
            arguments::DriverInstallation::Install => {
                razer_platform::hardware_driver::install_from_application_directory()
            }
            arguments::DriverInstallation::Uninstall => {
                razer_platform::hardware_driver::uninstall_owned_installation()
            }
        };
        return match result {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Hardware driver installation: {error:#}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    if args.iter().any(|arg| arg == "--hid-broker") {
        return std::process::ExitCode::from(backend::runtime::run_hid_broker() as u8);
    }
    if args.iter().any(|arg| arg == "--service-worker") {
        return std::process::ExitCode::from(backend::runtime::run_worker() as u8);
    }
    let paths = razer_storage::store_path().and_then(|workspace| {
        Ok((
            workspace,
            razer_storage::runtime_log_path()?,
            razer_storage::panic_log_path()?,
        ))
    });
    let (workspace_path, runtime_log, panic_log) = match paths {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("User configuration directory: {error:#}");
            return std::process::ExitCode::FAILURE;
        }
    };
    // The owner drains startup diagnostics and outlives the application cleanup.
    let _diagnostics = diagnostics::Runtime::install(runtime_log);
    diagnostics::install_panic_log(panic_log);

    // 语言：`--lang <代码>`，默认中文。
    //
    // 词表由安装的本地目录库提供。先确定语言，使无窗口路径和 UI
    // 使用同一份已校验工作区快照。
    // The shell consumes this validated snapshot so language and profile
    // selection use the same file revision without reparsing on the UI thread.
    let workspace = store::read_workspace(&workspace_path);
    let lang = args
        .iter()
        .position(|arg| arg == "--lang")
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| {
            workspace
                .as_ref()
                .ok()
                .and_then(|file| file.as_ref())
                .map(|file| file.preferences.language.clone())
                .unwrap_or_else(|| "zh-CN".to_string())
        });
    i18n::set_locale(&lang);
    let start_hidden = razer_platform::startup::starts_hidden(args.iter().map(String::as_str));

    if let Err(error) = resources::validate() {
        log::error!("资源索引无法加载：{error:#}");
        return std::process::ExitCode::FAILURE;
    }
    if let Err(error) = razer_catalog::emoji::validate() {
        log::error!("本地表情数据库无法加载：{error:#}");
        return std::process::ExitCode::FAILURE;
    }
    if let Err(error) = razer_catalog::initialize_registry() {
        log::error!("本地产品目录无法加载：{error:#}");
        return std::process::ExitCode::FAILURE;
    }
    let resource_runtime = match resources::ResourceRuntime::start() {
        Ok(runtime) => runtime,
        Err(error) => {
            log::error!("资源服务无法创建：{error:#}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let _service_runtime = match service_runtime::Runtime::start() {
        Ok(runtime) => runtime,
        Err(error) => {
            log::error!("设备服务无法创建：{error:#}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let assets = resources::SynapseAssets::new(resource_runtime.client());
    let application = gpui_kit::application().with_assets(assets);

    application.run(move |cx| {
        // 使用任何组件前必须先初始化。
        gpui_kit::init(cx);
        razer_widgets::init(cx);
        if let Err(error) = resources::start_resources(resource_runtime, cx) {
            log::error!("资源加载器无法初始化：{error:#}");
        }
        if let Err(error) = resources::register_fonts(cx) {
            log::error!("字体加载失败：{error}");
        }

        // 暗色主题 + 雷蛇配色。
        //
        // 共享主题为页面提供基础颜色；具体产品的呈现由所属页面令牌覆盖。
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
        // 当前来源与逐页验证边界见 docs/re/ui-readonly-first-roadmap.md。
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |theme| {
            // 字体与字号：`body,html { font-family:Roboto,sans-serif; font-size:16px }`，
            // 进入 `.widget` 后是 14px（由 `widgets::card()` 显式设回）。
            //
            // 基字号取真实的 **16px**：gpui-kit 的字号是从 `theme.font_size`
            // 按比例派生的（`sizing.rs:239` 的 `×0.875`），所以它**决定不了**
            // 具体档位，只能靠调用点显式指定（见 `widgets::geometry::TEXT_*`）。
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

            // 开关的共享主题基线：
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
            // 公共主题基线；产品页面按各自当前 CSS 覆盖。
            theme.radius = px(3.);
            theme.radius_lg = px(5.);
        });

        let window_options = WindowOptions {
            show: !start_hidden,
            focus: !start_hidden,
            // 显式给初始尺寸，否则会退回到很小的默认值。
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
            window_min_size: Some(size(px(1080.), px(720.))),
            ..TitleBar::window_options()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| shell::AppShell::new(workspace, window, cx))
        })
        .expect("Failed to open window");
    });
    std::process::ExitCode::SUCCESS
}
