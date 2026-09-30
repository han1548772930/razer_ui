//! 引擎诊断页。
//!
//! 这里不是雷云的产品功能页，也不承诺灯光、映射或音频已经接入。页面只做两件事：
//! 读取本机文件系统中的 DLL 路径，以及展示必须在独立进程执行的探测边界。
//! UI 线程绝不调用 `LoadLibrary`，避免第三方 `DllMain` 阻塞主窗口。

use gpui_kit::component::*;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::backend;
use crate::shell::AppShell;
use crate::ui::widgets::SettingRow;

/// 渲染只读的引擎诊断状态。
pub fn render(cx: &mut Context<AppShell>) -> AnyElement {
    let engines = backend::catalog();
    let resolved = engines
        .iter()
        .filter(|spec| backend::resolve_path(spec.stem).is_some())
        .count();
    let blocked = engines
        .iter()
        .filter(|spec| backend::blocks_load(spec.stem))
        .count();

    v_flex()
        .size_full()
        .gap_3()
        .p(px(24.))
        .overflow_y_scrollbar()
        .child(div().text_xl().font_bold().child("引擎诊断"))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("只读诊断页面：路径解析不等于引擎已加载，也不等于产品功能已经接入。"),
        )
        .child(
            v_flex()
                .gap_2()
                .p(px(16.))
                .rounded(px(5.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(div().font_bold().child("当前诊断范围"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child("不会在 UI 线程加载 DLL"),
                        ),
                )
                .child(SettingRow::new(
                    "引擎目录",
                    format!("{resolved}/{} 个可解析", engines.len()),
                ))
                .child(SettingRow::new(
                    "需要独立进程探测",
                    format!("{} 个", blocked),
                ))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                    "真正的加载、导出符号和依赖验证只能通过命令行探测；页面不会提供会卡死窗口的“测试”按钮。",
                )),
        )
        .child(
            v_flex()
                .gap_2()
                .children(engines.iter().map(|spec| engine_row(spec, cx))),
        )
        .into_any_element()
}

/// 引擎目录中的一条只读诊断记录。
fn engine_row(spec: &backend::EngineSpec, cx: &mut Context<AppShell>) -> AnyElement {
    let path = backend::resolve_path(spec.stem);
    let blocking = backend::blocks_load(spec.stem);
    let warning = cx.theme().warning;
    let path_label = if path.is_some() {
        "路径已解析"
    } else {
        "未解析到路径"
    };

    v_flex()
        .w_full()
        .gap_2()
        .p(px(16.))
        .rounded(px(5.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().font_bold().child(spec.label))
                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                            spec.stem,
                        )),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(if path.is_some() {
                            cx.theme().success
                        } else {
                            cx.theme().danger
                        })
                        .child(path_label),
                ),
        )
        .child(div().text_sm().child(spec.purpose))
        .child(match &path {
            Some(path) => v_flex()
                .gap_1()
                .child(div().text_xs().child("解析路径"))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                    path.display().to_string(),
                ))
                .into_any_element(),
            None => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("本机未安装该引擎，或当前雷云版本不包含该文件。")
                .into_any_element(),
        })
        .when(blocking, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(warning)
                    .child("需要独立进程探测：该 DLL 的 DllMain 可能阻塞，不能从页面线程加载。"),
            )
        })
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "期望导出 {} 个；命令行：razer_ui --probe {}",
                    spec.symbols.len(),
                    spec.stem
                )),
        )
        .into_any_element()
}
