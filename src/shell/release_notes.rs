//! Original `/release-patch-note/`: Ie renders firstPost as hr/h2 sections.
//! The installed version and its host-provided content are not read locally.
use crate::ui::scroll::SourceScrollable as _;
use crate::{i18n, ui::surface};
use gpui_kit::base::{Button as BaseButton, Dialog as BaseDialog, Link, TextView, TextViewStyle};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};

#[cfg(test)]
#[path = "release_notes_tests.rs"]
mod tests;

pub(super) fn open(window: &mut Window, cx: &mut App) -> Entity<ReleaseNotes> {
    open_for(NotesApp::Synapse, window, cx)
}

#[derive(Clone, Copy)]
pub(super) enum NotesApp {
    Synapse,
    Chroma,
}

pub(super) fn open_for(app: NotesApp, window: &mut Window, cx: &mut App) -> Entity<ReleaseNotes> {
    let view = cx.new(|cx| ReleaseNotes {
        app,
        open: true,
        preview: None,
        focus: cx.focus_handle(),
        return_focus: window.focused(cx),
        scroll: ScrollHandle::new(),
    });
    let focus = view.read(cx).focus.clone();
    focus.focus(window, cx);
    view
}

pub(super) struct ReleaseNotes {
    app: NotesApp,
    open: bool,
    preview: Option<Vec<NotesSection>>,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    scroll: ScrollHandle,
}

struct NotesSection {
    offset: usize,
    subsections: Vec<(usize, SharedString)>,
}

/// Keep top-level blocks intact so a list can use its source line height
/// without expanding ordinary paragraphs. TextView still owns inline HTML.
fn html_blocks(html: &str) -> Vec<(usize, &str, &str)> {
    let lower = html.to_ascii_lowercase();
    let mut blocks = Vec::new();
    let mut start = 0;
    while start < html.len() {
        let whitespace = html[start..].len() - html[start..].trim_start().len();
        start += whitespace;
        if start == html.len() {
            break;
        }
        let Some(tag_end) = lower[start..].find('>').map(|end| start + end) else {
            blocks.push((start, "", &html[start..]));
            break;
        };
        let tag = lower[start..=tag_end]
            .strip_prefix('<')
            .unwrap_or("")
            .split(|character: char| !character.is_ascii_alphanumeric())
            .next()
            .unwrap_or("");
        if !matches!(tag, "h2" | "p" | "ul" | "ol" | "li") {
            blocks.push((start, "", &html[start..]));
            break;
        }
        let closing = format!("</{tag}>");
        let Some(end) = lower[tag_end + 1..]
            .find(&closing)
            .map(|end| tag_end + 1 + end + closing.len())
        else {
            blocks.push((start, "", &html[start..]));
            break;
        };
        let kind = match tag {
            "h2" => "h2",
            "p" => "p",
            "ul" => "ul",
            "ol" => "ol",
            _ => "li",
        };
        blocks.push((start, kind, &html[start..end]));
        start = end;
    }
    blocks
}

fn rich_block(id: usize, html: &str, style: &TextViewStyle, list: bool) -> TextView {
    TextView::html(("release-notes-block", id), html.to_owned())
        .style(style.clone())
        .scrollable(false)
        .text_size(surface::css(14.))
        .line_height(surface::css(if list { 25. } else { 16.8 }))
}

fn subsection(offset: usize, html: &str, style: &TextViewStyle) -> AnyElement {
    v_flex()
        .pb(surface::css(20.))
        .children(html_blocks(html).into_iter().map(|(start, kind, block)| {
            if matches!(kind, "ul" | "ol") {
                let inner_start = block.find('>').unwrap() + 1;
                let inner_end = block.rfind("</").unwrap();
                let items = html_blocks(&block[inner_start..inner_end]);
                // The available local example uses flat lists. Preserve arbitrary
                // markup in the rich renderer if a future readback contains nesting.
                if items.iter().all(|(_, tag, html)| {
                    *tag == "li" && !html.contains("<ul") && !html.contains("<ol")
                }) {
                    return v_flex()
                        .mt(surface::css(10.))
                        .mb(surface::css(14.))
                        .children(items.into_iter().enumerate().map(
                            |(index, (item_start, _, item))| {
                                let content_start = item.find('>').unwrap() + 1;
                                let content_end = item.rfind("</").unwrap();
                                h_flex()
                                    .items_start()
                                    .child(
                                        div()
                                            .w(surface::css(25.))
                                            .flex_shrink_0()
                                            .text_center()
                                            .line_height(surface::css(25.))
                                            .child(if kind == "ol" {
                                                format!("{}.", index + 1)
                                            } else {
                                                "•".to_owned()
                                            }),
                                    )
                                    .child(div().flex_1().min_w_0().child(rich_block(
                                        offset + start + inner_start + item_start,
                                        &item[content_start..content_end],
                                        style,
                                        true,
                                    )))
                            },
                        ))
                        .into_any_element();
                }
            }
            div()
                .when(kind == "p", |view| view.mb(surface::css(8.)))
                .child(rich_block(
                    offset + start,
                    block,
                    style,
                    matches!(kind, "ul" | "ol"),
                ))
                .into_any_element()
        }))
        .into_any_element()
}

/// Match the source's case-insensitive `<hr\s*/?>`, then keep complete h2
/// sections. IDs use source byte offsets rather than translated heading text.
fn sections(first_post: &str) -> Vec<NotesSection> {
    let lower = first_post.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut section_start = 0;
    let mut search = 0;
    while let Some(relative) = lower[search..].find("<hr") {
        let start = search + relative;
        let Some(end) = lower[start..].find('>').map(|end| start + end + 1) else {
            break;
        };
        let tail = lower[start + 3..end - 1].trim();
        if tail.is_empty() || tail == "/" {
            ranges.push(section_start..start);
            section_start = end;
        }
        search = end;
    }
    ranges.push(section_start..first_post.len());
    ranges
        .into_iter()
        .filter_map(|range| {
            let mut starts = Vec::new();
            let mut search = range.start;
            while let Some(relative) = lower[search..range.end].find("<h2") {
                let start = search + relative;
                let after = start + 3;
                let boundary = lower.as_bytes().get(after).copied();
                search = after;
                if !boundary.is_some_and(|byte| byte == b'>' || byte.is_ascii_whitespace()) {
                    continue;
                }
                if lower[after..range.end].contains("</h2>") {
                    starts.push(start);
                }
            }
            if starts.is_empty() {
                return None;
            }
            let subsections = starts
                .iter()
                .enumerate()
                .map(|(index, start)| {
                    let end = starts.get(index + 1).copied().unwrap_or(range.end);
                    (
                        *start,
                        SharedString::from(first_post[*start..end].trim().to_owned()),
                    )
                })
                .collect();
            Some(NotesSection {
                offset: range.start,
                subsections,
            })
        })
        .collect()
}

fn example_first_post() -> String {
    (1..=4).map(|section| {
        let items = (1..=5).map(|item| format!(
            "<li>示例条目 {section}.{item}：此内容用于查看长文本、列表与滚动效果，不代表真实的软件更新。</li>"
        )).collect::<String>();
        format!("<h2>示例章节 {section}</h2><p>以下均为界面预览内容，未读取已安装版本的发行记录。</p><ul>{items}</ul>")
    }).collect::<Vec<_>>().join("<hr />")
}

fn official_url(app: NotesApp) -> &'static str {
    // Ie: rzr.to/{synapse|chroma}-{win|mac}-{prod|beta}-patch-notes.
    match (app, cfg!(target_os = "macos")) {
        (NotesApp::Synapse, true) => "https://rzr.to/synapse-mac-prod-patch-notes",
        (NotesApp::Synapse, false) => "https://rzr.to/synapse-win-prod-patch-notes",
        (NotesApp::Chroma, true) => "https://rzr.to/chroma-mac-prod-patch-notes",
        (NotesApp::Chroma, false) => "https://rzr.to/chroma-win-prod-patch-notes",
    }
}

impl ReleaseNotes {
    pub(super) fn focus_if_open(&self, window: &mut Window, cx: &mut App) -> bool {
        if self.open {
            self.focus.focus(window, cx);
        }
        self.open
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.preview = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn toggle_preview(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.preview = if self.preview.is_some() {
            None
        } else {
            Some(sections(&example_first_post()))
        };
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn body(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex()
            .id("release-notes-body")
            .test_support()
            .flex_1()
            .min_h_0()
            .w_full()
            .py(surface::css(if compact { 0. } else { 20. }))
            .pl(surface::css(if compact { 12. } else { 30. }))
            .pr(surface::css(if compact { 12. } else { 16. }))
            .child(div()
                .h(surface::css(24.))
                .flex_shrink_0()
                .mb(surface::css(20.))
                .font_family("RazerF5")
                .text_size(surface::css(20.))
                .line_height(surface::css(20.))
                .text_color(cx.theme().primary)
                .child(if self.preview.is_some() { "界面预览 · 示例发布说明" } else { "发布说明尚未读取" }))
            .child(v_flex()
                .flex_shrink_0()
                .gap(surface::css(10.))
                .mb(surface::css(20.))
                .child(if self.preview.is_some() {
                    "示例内容不代表已安装版本或真实发行记录。"
                } else {
                    match self.app {
                        NotesApp::Synapse => "尚未读取已安装的 Synapse 版本和对应发布说明。可通过下方官方链接查看发布记录。",
                        NotesApp::Chroma => "尚未读取已安装的 Chroma 版本和对应发布说明。可通过下方官方链接查看发布记录。",
                    }
                })
                .child(BaseButton::new("release-notes-preview")
                    .self_start()
                    .px(surface::css(10.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(surface::css(3.))
                    .text_size(surface::css(12.))
                    .hover(|button| button.border_color(cx.theme().primary))
                    .child(if self.preview.is_some() { "结束预览" } else { "预览章节布局" })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_preview(window, cx)))));
        let text_style = TextViewStyle::default()
            .with_foreground(cx.theme().foreground)
            .with_muted_foreground(cx.theme().muted_foreground)
            .with_link(cx.theme().primary)
            .with_selection(cx.theme().selection)
            .with_paragraph_gap(rems(8. / 16.))
            .with_dark(true)
            .with_heading(|_| {
                StyleRefinement::default()
                    .font_family("RazerF5")
                    .font_weight(FontWeight::NORMAL)
                    .text_size(surface::css(18.))
                    .line_height(surface::css(18.))
                    .pb_0()
                    .mt_0()
                    .mb(surface::css(5.))
            });
        if let Some(sections) = &self.preview {
            for section in sections {
                body = body.child(
                    v_flex()
                        .id(("release-notes-section", section.offset))
                        .test_support()
                        .flex_shrink_0()
                        .children(
                            section
                                .subsections
                                .iter()
                                .map(|(offset, html)| subsection(*offset, html, &text_style)),
                        )
                        .when(section.offset != sections.last().unwrap().offset, |view| {
                            view.child(
                                div()
                                    .h(surface::css(1.))
                                    .mb(surface::css(7.))
                                    .bg(cx.theme().border),
                            )
                        }),
                );
            }
        }
        body.scrollable_y()
            .track_scroll(&self.scroll)
            .into_any_element()
    }
}

impl Render for ReleaseNotes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let viewport = window.viewport_size();
        let height = (viewport.height - window.rem_size() * (106. / 16.))
            .max(px(0.))
            .min(window.rem_size() * (932. / 16.));
        let width = viewport.width.min(window.rem_size() * (850. / 16.));
        let compact = viewport.width <= window.rem_size() * (600. / 16.);
        let panel = v_flex()
            .id("release-notes-panel")
            .test_support()
            .relative()
            .occlude()
            .w(width)
            .h(height)
            .bg(cx.theme().sidebar)
            .rounded_t(surface::css(5.))
            .text_color(cx.theme().foreground)
            .text_size(surface::css(14.))
            .line_height(surface::css(16.8))
            .child(
                h_flex()
                    .h(surface::css(36.))
                    .w_full()
                    .flex_shrink_0()
                    .justify_center()
                    .px(surface::css(if compact { 12. } else { 16. }))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .font_family("RazerF5")
                    .text_color(cx.theme().muted_foreground)
                    .child("RELEASE NOTES"),
            )
            .child(self.body(compact, cx))
            .child(
                h_flex()
                    .h(surface::css(47.))
                    .flex_shrink_0()
                    .justify_between()
                    .px(surface::css(if compact { 12. } else { 30. }))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Link::new("release-notes-official")
                            .href(official_url(self.app))
                            .open_with(|url, _, _, cx| cx.open_url(url))
                            .flex()
                            .items_center()
                            .gap(surface::css(5.))
                            .text_color(cx.theme().foreground)
                            .underline()
                            .child(
                                i18n::t("RELEASE_PATCH_NOTE_VIEW")
                                    .replace("{{releasePatchNote}}", "Release Notes"),
                            )
                            .child(
                                img("synapse/release-notes-external.svg").size(surface::css(20.)),
                            ),
                    )
                    .child(
                        BaseButton::new("release-notes-close")
                            .h(surface::css(27.))
                            .min_w(surface::css(90.))
                            .px(surface::css(if compact { 12. } else { 16. }))
                            .rounded(surface::css(3.))
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().sidebar)
                            .text_size(surface::css(if compact { 13. } else { 14. }))
                            .child(i18n::t("CLOSE"))
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .child(
                BaseButton::new("release-notes-close-x")
                    .absolute()
                    .top_0()
                    .right_0()
                    .w(surface::css(40.))
                    .h(surface::css(36.))
                    .rounded_tr(surface::css(5.))
                    .bg(cx.theme().sidebar)
                    .flex()
                    .items_center()
                    .justify_center()
                    .accessibility_label(i18n::t("CLOSE"))
                    .child(img("synapse/mapping-close.svg").size(surface::css(24.)))
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            );
        BaseDialog::new(cx)
            .layer(2, true)
            .focus_handle(self.focus.clone())
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().title_bar.opacity(0.7)),
            )
            .popup(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w(viewport.width)
                    .h(height)
                    .flex()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}
