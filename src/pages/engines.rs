//! 引擎接入状态页 —— 方案 2 的进度可视化。
//!
//! # 为什么这里不加载 DLL
//!
//! 实测教训：把引擎探测放在 `render()` 里会让界面**卡死**，
//! 因为第三方 `DllMain` 会阻塞（已定位到 `SysUtilsNative.dll`）。
//!
//! 所以本页只展示**静态目录 + 文件系统解析出的路径**（都是只读操作，线程安全），
//! 真正的加载验证走命令行 `razer_ui --probe <stem>`，在独立进程里逐个做。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::backend;

/// 渲染引擎接入状态。
pub fn render(cx: &mut Context<AppShell>) -> AnyElement {
    let engines = backend::catalog();

    v_flex()
        .gap_3()
        .p_4()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .font_bold()
                        .child("雷云引擎接入状态（方案 2：复用现有后端）"),
                )
                .child(div().text_sm().child(format!(
                    "共 {} 个引擎 DLL · 此处只解析路径，不加载 DLL",
                    engines.len()
                )))
                .child(div().text_xs().child(
                    "加载验证请在命令行执行：razer_ui --probe <引擎名>（如 --probe mapping_engine）",
                )),
        )
        .child(
            v_flex()
                .gap_2()
                .children(engines.iter().map(|spec| engine_row(spec, cx))),
        )
        .into_any_element()
}

/// 引擎清单里的一行。
///
/// 只展示 **文件系统解析出来的路径**（[`backend::resolve_path`]），
/// 不加载 DLL——本页在 UI 线程上渲染，加载第三方 `DllMain` 会卡死界面
/// （见模块文档的实测教训）。
fn engine_row(spec: &backend::EngineSpec, cx: &mut Context<AppShell>) -> AnyElement {
    let path = backend::resolve_path(spec.stem);
    let blocking = backend::blocks_load(spec.stem);

    v_flex()
        .w_full()
        .gap_1()
        .p_3()
        .rounded(px(3.))
        .bg(cx.theme().group_box)
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().text_sm().child(spec.label))
                        // 引擎名用更小的字，便于与 `--probe` 的输出对照。
                        .child(div().text_xs().child(spec.stem)),
                )
                // 用文字而不是图标表达状态：这是诊断信息，可读性优先。
                .child(
                    div()
                        .text_xs()
                        .text_color(if path.is_some() {
                            cx.theme().success
                        } else {
                            cx.theme().danger
                        })
                        .child(if path.is_some() { "已找到" } else { "未找到" }),
                ),
        )
        .child(div().text_xs().child(spec.purpose))
        .child(match &path {
            Some(path) => div().text_xs().child(path.display().to_string()),
            None => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("该版本的雷云没有这个引擎，或本机未安装雷云"),
        })
        .when(blocking, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    // 实测结论，不是猜测：必须标出来，否则用户会以为
                    // 「点一下就能探测」。
                    .child("⚠ 实测该 DLL 的 DllMain 会阻塞，只能在独立进程里探测"),
            )
        })
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "导出 {} 个：{}",
                    spec.symbols.len(),
                    spec.symbols.join(", ")
                )),
        )
        .into_any_element()
}
