//! Original `/release-patch-note/`: Ie renders firstPost as hr/h2 sections.
//! Content is read from the owning host's process-local MemoryStorage.
use gpui_kit::base::{Button as BaseButton, Dialog as BaseDialog, Link, TextView, TextViewStyle};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_storage::host::{HostStorage, HostStorageCall, HostStorageView, HostStoreKind};
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use serde_json::{Value, json};

#[cfg(test)]
#[path = "release_notes_tests.rs"]
mod tests;

pub fn open(window: &mut Window, cx: &mut App) -> Entity<ReleaseNotes> {
    open_for(NotesApp::Synapse, window, cx)
}

#[derive(Clone, Copy)]
pub enum NotesApp {
    Synapse,
    Chroma,
}

pub fn open_for(app: NotesApp, window: &mut Window, cx: &mut App) -> Entity<ReleaseNotes> {
    create(app, None, window, cx)
}

/// Use the same host store as the content producer, never a new worker or a
/// persisted draft. Without firstPost the original route renders nothing.
pub fn open_from_storage(
    app: NotesApp,
    storage: &mut HostStorage,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ReleaseNotes> {
    let id = match app {
        NotesApp::Synapse => 0xffff_0001,
        NotesApp::Chroma => 0xffff_0002,
    };
    storage.register_view(HostStorageView {
        id,
        url: match app {
            NotesApp::Synapse => "synapse-release-patch-notes",
            NotesApp::Chroma => "chroma-app-release-patch-notes",
        }
        .into(),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: true,
    });
    let data = storage
        .call(HostStorageCall {
            store: HostStoreKind::Memory,
            sender_id: id,
            action: "getMemoryStorageItem".into(),
            payload: json!({"key": "releaseNotePatchContent"}),
            target_url_array: Vec::new(),
        })
        .ok()
        .and_then(|reply| parse_storage(app, &reply.value).ok().flatten());
    storage.close_view(id);
    create(app, data, window, cx)
}

struct NotesContent {
    title: SharedString,
    sections: Vec<NotesSection>,
}

fn parse_storage(app: NotesApp, value: &Value) -> Result<Option<NotesContent>, String> {
    let Some(raw) = value.as_str().filter(|raw| !raw.is_empty()) else {
        return Ok(None);
    };
    let rows: Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if rows.is_null() {
        return Ok(None);
    }
    let encoded = rows
        .get(0)
        .and_then(|row| row.get("value"))
        .and_then(Value::as_str)
        .ok_or("releaseNotePatchContent has no string value")?;
    let all: Value = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
    let selected = &all[match app {
        NotesApp::Synapse => "synapse",
        NotesApp::Chroma => "chroma",
    }];
    let Some(first_post) = selected["firstPost"]
        .as_str()
        .filter(|text| !text.is_empty())
    else {
        return Ok(None);
    };
    Ok(Some(NotesContent {
        title: selected["title"].as_str().unwrap_or("").to_owned().into(),
        sections: sections(first_post),
    }))
}

fn create(
    app: NotesApp,
    content: Option<NotesContent>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ReleaseNotes> {
    let visible = content.is_some();
    let view = cx.new(|cx| ReleaseNotes {
        app,
        open: visible,
        content,
        focus: cx.focus_handle(),
        return_focus: window.focused(cx),
        scroll: ScrollHandle::new(),
    });
    if visible {
        let focus = view.read(cx).focus.clone();
        focus.focus(window, cx);
    }
    view
}

pub struct ReleaseNotes {
    app: NotesApp,
    open: bool,
    content: Option<NotesContent>,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    scroll: ScrollHandle,
}

struct NotesSection {
    offset: usize,
    subsections: Vec<(usize, SharedString)>,
}

fn js_trim(text: &str) -> &str {
    text.trim_matches(|character| {
        matches!(character,
            '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' |
            '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
    })
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
                // Preserve arbitrary
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

/// Ie's hr splitting, empty paragraph removal and h2 matches. Nonempty
/// sections without h2 still retain their source separator.
fn sections(first_post: &str) -> Vec<NotesSection> {
    use regex::Regex;
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<(Regex, Regex, Regex, Regex)> = OnceLock::new();
    let (rule, empty, heading, opening) = PATTERNS.get_or_init(|| {
        // ECMAScript \s differs from Rust's Unicode whitespace; its dot
        // also excludes CR and both Unicode line separators.
        let whitespace = r"[\t\n\x0B\x0C\r \u{00A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}]";
        (
            Regex::new(&format!(r"(?i)<hr{whitespace}*/?>")).unwrap(),
            Regex::new(&format!(r"(?i)<p>(&nbsp;|{whitespace})*</p>")).unwrap(),
            Regex::new(r"(?i)<h2[^>]*>[^\r\n\u{2028}\u{2029}]*?</h2>").unwrap(),
            Regex::new(r"(?i)<h2[^>]*>").unwrap(),
        )
    });
    let mut result = Vec::new();
    let mut offset = 0;
    for segment in rule.split(first_post) {
        let cleaned = empty.replace_all(segment, "");
        let html = js_trim(&cleaned);
        if !html.is_empty() {
            let mut subsections = Vec::new();
            let mut search = 0;
            while let Some(header) = heading.find_at(html, search) {
                let end = opening
                    .find_at(html, header.end())
                    .map_or(html.len(), |next| next.start());
                subsections.push((
                    offset + header.start(),
                    js_trim(&html[header.start()..end]).to_owned().into(),
                ));
                search = end;
            }
            result.push(NotesSection {
                offset,
                subsections,
            });
        }
        // Offsets only provide retained identity within this content snapshot.
        offset += segment.len() + 1;
    }
    result
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
    pub fn focus_if_open(&self, window: &mut Window, cx: &mut App) -> bool {
        if self.open {
            self.focus.focus(window, cx);
        }
        self.open
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.content = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
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
            .child(
                div()
                    .h(surface::css(24.))
                    .flex_shrink_0()
                    .mb(surface::css(20.))
                    .font_family("RazerF5")
                    .text_size(surface::css(20.))
                    .line_height(surface::css(20.))
                    .text_color(cx.theme().primary)
                    .child(
                        self.content
                            .as_ref()
                            .map(|content| content.title.to_uppercase())
                            .unwrap_or_default(),
                    ),
            );
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
        if let Some(content) = &self.content {
            let sections = &content.sections;
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
