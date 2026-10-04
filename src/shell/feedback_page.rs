//! Current Feedback application (`feedback-synapse`) root.
//!
//! The current application mounts a 500px guest form and footer. Submission
//! and log collection remain service boundaries: either log choice preserves
//! the local draft and reports unavailable without network or process calls.
use crate::{
    features::Choice,
    i18n,
    ui::{surface, theme::FeedbackColors},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    select::{SelectEvent, SelectState},
    text::TextView,
    *,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

const TITLE_LIMIT: usize = 50;
const EMAIL_LIMIT: usize = 255;
const DESCRIPTION_LIMIT: usize = 32_000;

fn tr(key: &str) -> String {
    i18n::t(&format!("FEEDBACK_SOURCE.{key}"))
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn clip_utf16(value: &str, limit: usize) -> String {
    let mut used = 0;
    value
        .chars()
        .take_while(|character| {
            let next = used + character.len_utf16();
            if next > limit {
                false
            } else {
                used = next;
                true
            }
        })
        .collect()
}

fn valid_email(value: &str) -> bool {
    // Module 4496 gn: ASCII local part, single interior dots, and a TLD
    // beginning with a letter and containing at least two alphanumerics.
    // Keep the source's unusual allowance for hyphens before a domain dot.
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    let atom = |byte: u8| byte.is_ascii_alphanumeric() || b"-!#$%&'*+/=?^_{|}~".contains(&byte);
    let local = local.as_bytes();
    if local.first().is_none_or(|byte| !atom(*byte)) {
        return false;
    }
    let mut index = 1;
    while index < local.len() {
        if local[index] == b'.' {
            index += 1;
        }
        if local
            .get(index)
            .is_none_or(|byte| !atom(*byte) && *byte != b'`')
        {
            return false;
        }
        index += 1;
    }
    let Some((host, suffix)) = domain.rsplit_once('.') else {
        return false;
    };
    let host = host.as_bytes();
    if host
        .first()
        .is_none_or(|byte| !byte.is_ascii_alphanumeric())
    {
        return false;
    }
    let mut index = 1;
    while index < host.len() {
        while host.get(index) == Some(&b'-') {
            index += 1;
        }
        if host.get(index) == Some(&b'.') {
            index += 1;
        }
        if host
            .get(index)
            .is_none_or(|byte| !byte.is_ascii_alphanumeric())
        {
            return false;
        }
        index += 1;
    }
    let suffix = suffix.as_bytes();
    if suffix.len() < 2 || !suffix[0].is_ascii_alphabetic() {
        return false;
    }
    let mut index = 1;
    while index < suffix.len() {
        if suffix[index] == b'-' {
            index += 1;
        }
        if suffix
            .get(index)
            .is_none_or(|byte| !byte.is_ascii_alphanumeric())
        {
            return false;
        }
        index += 1;
    }
    true
}

fn has_content(value: &str) -> bool {
    // ECMAScript \S (module 4496 hn), including BOM but excluding U+0085.
    value.chars().any(|ch| !matches!(ch, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
}

fn email_placeholder(required: bool) -> String {
    let qualifier = tr(if required {
        "TEXT_REQUIRE"
    } else {
        "TEXT_OPTIONAL"
    });
    let mut chars = qualifier.chars();
    let qualifier = chars
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default();
    format!("{} ({qualifier})", tr("TEXT_EMAIL"))
}

fn linked_copy(id: &'static str, key: &str, url: &'static str) -> TextView {
    let markdown = tr(key)
        .replace("{{a}}", "[")
        .replace("{{aEnd}}", &format!("]({url})"));
    TextView::markdown(id, markdown)
}

fn input_frame(input: AnyElement, height: f32, focused: bool, error: bool) -> AnyElement {
    div()
        .w_full()
        .h(surface::css(height))
        .border_1()
        .border_color(if error {
            FeedbackColors::error()
        } else if focused {
            FeedbackColors::primary()
        } else {
            FeedbackColors::border()
        })
        .when(!error, |view| {
            view.hover(|style| style.border_color(FeedbackColors::primary()))
        })
        .child(input)
        .into_any_element()
}

fn category_choices() -> Vec<Choice> {
    vec![
        Choice::new("1", tr("TEXT_FEEDBACK_W_FEATURE_REQUEST")),
        Choice::new(
            "3",
            format!("{}/{}", tr("TEXT_CUSTOMER_SUPPORT"), tr("TEXT_REPORT_BUG")),
        ),
        Choice::new("4", tr("TEXT_PRIVACY_ENQUIRES")),
    ]
}

fn app_choices() -> Vec<Choice> {
    vec![
        Choice::new("synapse", "Synapse"),
        Choice::new("Other", tr("TEXT_OTHER")),
    ]
}

fn device_choices() -> Vec<Choice> {
    vec![Choice::new("0", tr("TEXT_OTHER_RAZER_DEVICE"))]
}

pub(super) struct FeedbackPage {
    focus: FocusHandle,
    logs_focus: FocusHandle,
    submit_focus: FocusHandle,
    apps: Entity<SelectState<Vec<Choice>>>,
    category: Entity<SelectState<Vec<Choice>>>,
    device: Entity<SelectState<Vec<Choice>>>,
    title: Entity<InputState>,
    email: Entity<InputState>,
    description: Entity<TextareaState>,
    with_logs: bool,
    error_email: bool,
    unavailable: bool,
    show_log_request: bool,
    subscriptions: Vec<Subscription>,
}

impl FeedbackPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let apps =
            cx.new(|cx| SelectState::new(app_choices(), Some(IndexPath::new(0)), window, cx));
        let category = cx.new(|cx| SelectState::new(category_choices(), None, window, cx));
        let device = cx.new(|cx| SelectState::new(device_choices(), None, window, cx));
        let title = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr("TEXT_SUBJECT"))
                .validate(|value, _| utf16_len(value) <= TITLE_LIMIT)
        });
        let email = cx.new(|cx| InputState::new(window, cx).placeholder(email_placeholder(false)));
        let description = cx
            .new(|cx| TextareaState::new(window, cx).placeholder(tr("TEXT_DETAIL_YOUR_FEEDBACK")));
        let mut this = Self {
            focus: cx.focus_handle(),
            logs_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            apps,
            category,
            device,
            title,
            email,
            description,
            with_logs: false,
            error_email: false,
            unavailable: false,
            show_log_request: false,
            subscriptions: Vec::new(),
        };
        this.subscriptions
            .push(cx.observe(&this.apps, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.category, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.device, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.title, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.email, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.description, |_, _, cx| cx.notify()));
        this.subscriptions.push(
            cx.subscribe_in(&this.title, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Blur) {
                    this.validate_email_on_blur(cx);
                }
            }),
        );
        this.subscriptions.push(cx.subscribe_in(
            &this.email,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    if utf16_len(&value) > EMAIL_LIMIT {
                        input.update(cx, |state, cx| {
                            state.set_value(clip_utf16(&value, EMAIL_LIMIT), window, cx)
                        });
                    }
                    // The source only updates errorEmail for nonempty input.
                    let value = input.read(cx).value();
                    if !value.is_empty() {
                        this.error_email = !valid_email(&value);
                    }
                    cx.notify();
                } else if matches!(event, InputEvent::Blur) {
                    this.validate_email_on_blur(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.description,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Blur) {
                    this.validate_email_on_blur(cx);
                }
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let value = input.read(cx).value().to_string();
                if utf16_len(&value) > DESCRIPTION_LIMIT {
                    let clipped = clip_utf16(&value, DESCRIPTION_LIMIT);
                    input.update(cx, |state, cx| state.set_value(clipped, window, cx));
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.category,
            window,
            |this, _, event, window, cx| {
                if matches!(event, SelectEvent::Confirm(Some(_))) {
                    let required = this.selected_category(cx).as_deref() == Some("3");
                    let email = this.email.read(cx).value();
                    if !required && email.is_empty() {
                        this.error_email = false;
                    } else if required && !email.is_empty() {
                        this.error_email = !valid_email(&email);
                    }
                    this.email.update(cx, |state, cx| {
                        state.set_placeholder(email_placeholder(required), window, cx)
                    });
                    cx.notify();
                }
            },
        ));
        this
    }

    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_log_request {
            self.logs_focus.focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
    }

    fn selected_category(&self, cx: &App) -> Option<String> {
        self.category.read(cx).selected_value().cloned()
    }

    fn valid(&self, cx: &App) -> bool {
        let category = self.selected_category(cx);
        let email = self.email.read(cx).value();
        let email_required = category.as_deref() == Some("3");
        self.apps.read(cx).selected_value().is_some()
            && matches!(category.as_deref(), Some("1" | "3"))
            && has_content(&self.title.read(cx).value())
            && has_content(&self.description.read(cx).value())
            && ((!email_required && email.is_empty()) || valid_email(&email))
    }

    fn validate_email_on_blur(&mut self, cx: &mut Context<Self>) {
        let email = self.email.read(cx).value();
        self.error_email = (self.selected_category(cx).as_deref() == Some("3")
            || !email.is_empty())
            && !valid_email(&email);
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.valid(cx) {
            return;
        }
        if self.selected_category(cx).as_deref() == Some("3") && !self.with_logs {
            self.show_log_request = true;
            self.logs_focus.focus(window, cx);
            cx.notify();
            return;
        }
        if self.selected_category(cx).as_deref() != Some("3") {
            self.with_logs = false;
        }
        // Jn(...) is the remote feedback service; the draft stays local.
        self.unavailable = true;
        cx.notify();
    }

    fn form(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let category = self.category.read(cx).selected_value().cloned();
        let app = self.apps.read(cx).selected_value().cloned();
        let apps = self.apps.clone();
        let category_state = self.category.clone();
        let device = self.device.clone();
        let title = Input::new(&self.title)
            .appearance(false)
            .id("feedback-title")
            .aria_label(tr("TEXT_SUBJECT"))
            .w_full()
            .h_full()
            .px(surface::css(6.))
            .bg(FeedbackColors::input())
            .border_0()
            .into_any_element();
        let email = Input::new(&self.email)
            .appearance(false)
            .id("feedback-email")
            .aria_label(tr("TEXT_EMAIL"))
            .w_full()
            .h_full()
            .px(surface::css(6.))
            .bg(FeedbackColors::input())
            .border_0()
            .into_any_element();
        let description = Textarea::new(&self.description)
            .appearance(false)
            .aria_label(tr("TEXT_DETAIL_YOUR_FEEDBACK"))
            .w_full()
            .h_full()
            .p(surface::css(6.))
            .bg(FeedbackColors::input())
            .border_0()
            .into_any_element();
        let title = input_frame(
            title,
            27.,
            self.title.focus_handle(cx).is_focused(window),
            false,
        );
        let email = input_frame(
            email,
            27.,
            self.email.focus_handle(cx).is_focused(window),
            self.error_email,
        );
        let description = input_frame(
            description,
            213.,
            self.description.focus_handle(cx).is_focused(window),
            false,
        );
        let category_select = surface::select(&category_state)
            .id("feedback-category")
            .items(category_choices())
            .placeholder(tr("TEXT_SELECT_TYPE"))
            .w_full()
            .accessibility_label(tr("TEXT_SELECT_TYPE"));
        let app_select = surface::select(&apps)
            .id("feedback-app")
            .items(app_choices())
            .placeholder(tr("TEXT_SELECT_SOFTWARE"))
            .w_full()
            .accessibility_label(tr("TEXT_SELECT_SOFTWARE"));
        let device_select = surface::select(&device)
            .id("feedback-device")
            .items(device_choices())
            .placeholder(tr("TEXT_DEVICE_NAME"))
            .w_full()
            .accessibility_label(tr("TEXT_DEVICE_NAME"));
        let submit = BaseButton::new("feedback-submit")
            .track_focus(&self.submit_focus)
            .accessibility_label(tr("TEXT_SUBMIT"))
            .px(surface::css(16.))
            .py(surface::css(7.))
            .bg(FeedbackColors::primary())
            .text_color(FeedbackColors::primary_foreground())
            .disabled(!self.valid(cx))
            .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
            .focus_visible(|style| style.border_1().border_color(FeedbackColors::foreground()))
            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
            .child(tr("TEXT_SUBMIT"));

        let mut root = v_flex().id("feedback-form").w_full().child(
            h_flex()
                .w_full()
                .mb(surface::css(10.))
                .gap(surface::css(10.))
                .child(div().min_w_0().flex_1().child(category_select))
                .when(category.as_deref() != Some("4"), |view| {
                    view.child(div().min_w_0().flex_1().child(app_select))
                })
                .when(category.as_deref() == Some("4"), |view| {
                    view.gap_0().child(div().flex_1())
                }),
        );
        // In the browser this is an opaque .bug-bounty layer over the remaining
        // fields, from top:35px through bottom:0. Keep those drafts retained,
        // and expose only the visible privacy content to native keyboard focus.
        if category.as_deref() == Some("4") {
            return root.child(self.privacy(cx)).into_any_element();
        }
        root = root.child(
            div()
                .w_full()
                .when(!self.error_email, |view| view.mb(surface::css(10.)))
                .child(email),
        );
        if self.error_email {
            root = root.child(
                div()
                    .mt(surface::css(5.))
                    .mb(surface::css(10.))
                    .text_size(surface::css(12.))
                    .text_color(FeedbackColors::error())
                    .child("Please enter a valid email address"),
            );
        }
        if app.as_deref() == Some("synapse") {
            root = root.child(div().w_full().mb(surface::css(10.)).child(device_select));
        }
        root = root
            .child(div().w_full().mb(surface::css(10.)).child(title))
            .child(div().w_full().mb(surface::css(10.)).child(description))
            .child(
                div()
                    .w_full()
                    .text_color(FeedbackColors::secondary())
                    .text_right()
                    .child(format!(
                        "{} / {}",
                        utf16_len(&self.description.read(cx).value()),
                        DESCRIPTION_LIMIT
                    )),
            );
        if category.as_deref() == Some("3") {
            root = root.child(
                v_flex()
                    .mb(surface::css(10.))
                    .child(
                        Checkbox::new("feedback-with-logs")
                            .label(tr("TEXT_SEND_LOG_FILE"))
                            .checked(self.with_logs)
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.with_logs = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .ml(surface::css(24.))
                            .mt(surface::css(3.))
                            .text_size(surface::css(12.))
                            .text_color(FeedbackColors::secondary())
                            .child(tr("TEXT_SEND_LOG_FILE_HELPER")),
                    ),
            );
        }
        root = root.child(h_flex().w_full().justify_center().child(submit));
        if self.unavailable {
            root = root.child(
                div()
                    .w_full()
                    .p(surface::css(10.))
                    .border_1()
                    .mt(surface::css(10.))
                    .border_color(FeedbackColors::warning())
                    .text_color(FeedbackColors::warning())
                    .child("服务未连接；草稿仍保留在本地。"),
            );
        }
        root.into_any_element()
    }

    fn privacy(&self, cx: &App) -> AnyElement {
        v_flex()
            .id("feedback-privacy")
            .w_full()
            .min_h(surface::css(372.))
            .px(surface::css(10.))
            .py(surface::css(20.))
            .bg(FeedbackColors::background())
            .text_color(FeedbackColors::privacy_text())
            .child(
                div()
                    .mb(surface::css(10.))
                    .font_family("RazerF5")
                    .text_size(surface::css(18.))
                    .text_color(FeedbackColors::primary())
                    .child(tr("TEXT_PRIVACY_ENQUIRY_TITLE")),
            )
            .child(div().child(tr("TEXT_PRIVACY_ENQUIRES_DESCRIPTION")))
            .child(div().mt(surface::css(20.)).child(linked_copy(
                "feedback-policy-copy",
                "TEXT_PRIVACY_ENQUIRES_DESCRIPTION_2",
                "https://www.razer.com/legal/customer-privacy-policy",
            )))
            .child(
                div()
                    .mt(surface::css(20.))
                    .mb(surface::css(20.))
                    .child(tr("TEXT_PRIVACY_ENQUIRES_DESCRIPTION_3")),
            )
            .child(
                h_flex().justify_center().child(
                    crate::shell::service_pages::source_link(
                        "feedback-privacy-link",
                        tr("TEXT_CONTACT_US"),
                        "https://www.razer.com/privacy-enquiries",
                        cx,
                    )
                    .px(surface::css(16.))
                    .py(surface::css(7.))
                    .bg(FeedbackColors::primary())
                    .text_color(FeedbackColors::primary_foreground()),
                ),
            )
            .into_any_element()
    }

    fn close_log_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_log_request = false;
        self.submit_focus.focus(window, cx);
        cx.notify();
    }

    fn confirm_logs(&mut self, with_logs: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.with_logs = with_logs;
        self.unavailable = true;
        self.close_log_request(window, cx);
    }

    fn log_request(&self, cx: &mut Context<Self>) -> AnyElement {
        let panel = v_flex()
            .w(surface::css(520.))
            .max_w_full()
            .h(surface::css(175.))
            .p(surface::css(20.))
            .bg(FeedbackColors::background())
            .border_1()
            .border_color(FeedbackColors::warning())
            .text_color(FeedbackColors::foreground())
            .text_size(surface::css(14.))
            .child(
                div()
                    .mb(surface::css(20.))
                    .child(tr("TEXT_SUBMITTING_FORM")),
            )
            .child(div().child(tr("TEXT_SUBMITTING_FORM_1")))
            .child(
                div()
                    .text_color(FeedbackColors::placeholder())
                    .child(tr("TEXT_SUBMITTING_FORM_2")),
            )
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .justify_center()
                    .child(
                        BaseButton::new("feedback-without-logs")
                            .accessibility_label(tr("TEXT_SUBMITTING_FORM_ACTION_1"))
                            .px(surface::css(16.))
                            .py(surface::css(7.))
                            .bg(cx.theme().button)
                            .text_color(cx.theme().button_foreground)
                            .focus_visible(|style| {
                                style.border_1().border_color(FeedbackColors::primary())
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_logs(false, window, cx)
                            }))
                            .child(tr("TEXT_SUBMITTING_FORM_ACTION_1")),
                    )
                    .child(
                        BaseButton::new("feedback-include-logs")
                            .accessibility_label(tr("TEXT_SUBMITTING_FORM_ACTION_2"))
                            .px(surface::css(16.))
                            .py(surface::css(7.))
                            .bg(FeedbackColors::primary())
                            .text_color(FeedbackColors::primary_foreground())
                            .focus_visible(|style| {
                                style.border_1().border_color(FeedbackColors::foreground())
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_logs(true, window, cx)
                            }))
                            .child(tr("TEXT_SUBMITTING_FORM_ACTION_2")),
                    ),
            );
        gpui_kit::base::Dialog::new(cx)
            .layer(1, true)
            .focus_handle(self.logs_focus.clone())
            .close_on_backdrop_press(true)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close_log_request(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(FeedbackColors::background().opacity(0.8)),
            )
            // The popup owns hit testing; a press on its explanatory text
            // must not reach the dismissing backdrop beneath it.
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .items_end()
                    .pb(surface::css(98.))
                    .child(gpui_kit::base::DialogPopup::new().child(panel)),
            )
            .into_any_element()
    }
}

impl Render for FeedbackPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("feedback-container")
            .size_full()
            .bg(FeedbackColors::background())
            .text_color(FeedbackColors::foreground())
            .text_size(surface::css(14.))
            .track_focus(&self.focus)
            .child(
                v_flex()
                    .id("feedback-page")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .items_center()
                    .pt(surface::css(30.))
                    .child(
                        v_flex()
                            .w(surface::css(500.))
                            .max_w_full()
                            .flex_1()
                            .flex_shrink_0()
                            .mb(surface::css(20.))
                            .child(self.form(window, cx)),
                    )
                    .child(
                        v_flex()
                            .id("feedback-footer")
                            .w(surface::css(500.))
                            .max_w_full()
                            .h(surface::css(60.))
                            .flex_shrink_0()
                            .items_center()
                            .justify_end()
                            .pb(surface::css(20.))
                            .text_color(FeedbackColors::foreground())
                            .child(tr("TEXT_EARN_REWARD"))
                            .child(linked_copy(
                                "feedback-bug-bounty-link",
                                "TEXT_JOIN_THE_BUG_BOUNTY_PROGRAM",
                                "https://www.razer.com/security/bug-bounty-program",
                            )),
                    ),
            )
            .when(self.show_log_request, |view| {
                view.child(self.log_request(cx))
            })
    }
}
