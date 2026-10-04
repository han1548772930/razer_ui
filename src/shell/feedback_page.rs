//! Current Feedback application (`feedback-synapse`) root.
//!
//! The source package is a standalone 500px form.  Submission and log export
//! are service boundaries in this native replica, so the controls remain
//! visible and report an unavailable state instead of making network or
//! process calls.
use crate::{
    features::Choice,
    i18n,
    ui::surface,
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    select::{SelectEvent, SelectState},
    checkbox::Checkbox,
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
    value.chars().take_while(|character| {
        let next = used + character.len_utf16();
        if next > limit { false } else { used = next; true }
    }).collect()
}

fn valid_email(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 3
        && value.len() <= EMAIL_LIMIT
        && value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        })
}

fn category_choices() -> Vec<Choice> {
    vec![
        Choice::new("1", tr("TEXT_FEEDBACK_W_FEATURE_REQUEST")),
        Choice::new("3", format!("{} / {}", tr("TEXT_CUSTOMER_SUPPORT"), tr("TEXT_REPORT_BUG"))),
        Choice::new("4", tr("TEXT_PRIVACY_ENQUIRES")),
    ]
}

fn app_choices() -> Vec<Choice> {
    vec![Choice::new("s", "Synapse"), Choice::new("Other", tr("TEXT_OTHER"))]
}

fn device_choices() -> Vec<Choice> {
    vec![Choice::new("0", tr("TEXT_OTHER_RAZER_DEVICE"))]
}

pub(super) struct FeedbackPage {
    focus: FocusHandle,
    apps: Entity<SelectState<Vec<Choice>>>,
    category: Entity<SelectState<Vec<Choice>>>,
    device: Entity<SelectState<Vec<Choice>>>,
    title: Entity<InputState>,
    email: Entity<InputState>,
    description: Entity<TextareaState>,
    with_logs: bool,
    error_email: bool,
    unavailable: bool,
    subscriptions: Vec<Subscription>,
}

impl FeedbackPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let apps = cx.new(|cx| {
            SelectState::new(
                app_choices(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let category = cx.new(|cx| SelectState::new(category_choices(), None, window, cx));
        let device = cx.new(|cx| SelectState::new(device_choices(), None, window, cx));
        let title = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr("TEXT_SUBJECT"))
        });
        let email = cx.new(|cx| InputState::new(window, cx).placeholder(tr("TEXT_EMAIL")));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder(tr("TEXT_DETAIL_YOUR_FEEDBACK"))
        });
        let mut this = Self {
            focus: cx.focus_handle(),
            apps,
            category,
            device,
            title,
            email,
            description,
            with_logs: false,
            error_email: false,
            unavailable: false,
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
        this.subscriptions.push(cx.subscribe_in(
            &this.title,
            window,
            |_this, input, event, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let value = input.read(cx).value().to_string();
                if utf16_len(&value) > TITLE_LIMIT {
                    let clipped = clip_utf16(&value, TITLE_LIMIT);
                    input.update(cx, |state, cx| state.set_value(clipped, window, cx));
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.email,
            window,
            |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error_email = !input.read(cx).value().is_empty()
                        && !valid_email(&input.read(cx).value());
                    cx.notify();
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.description,
            window,
            |_this, input, event, window, cx| {
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
            |_this, _, event, _, cx| {
                if matches!(event, SelectEvent::Confirm(Some(_))) {
                    cx.notify();
                }
            },
        ));
        this
    }

    fn selected_category(&self, cx: &App) -> Option<String> {
        self.category.read(cx).selected_value().cloned()
    }

    fn valid(&self, cx: &App) -> bool {
        let category = self.selected_category(cx);
        let email = self.email.read(cx).value();
        let email_required = category.as_deref() == Some("3");
        self.apps.read(cx).selected_value().is_some()
            && category.is_some()
            && !self.title.read(cx).value().trim().is_empty()
            && !self.description.read(cx).value().trim().is_empty()
            && ((!email_required && email.is_empty()) || valid_email(&email))
            && !self.error_email
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.valid(cx) {
            return;
        }
        // Jn(...) in the source is the remote feedback service. Keep this
        // boundary explicit and reviewable in the local UI.
        self.unavailable = true;
        cx.notify();
    }

    fn form(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
            .h(surface::css(27.))
            .px(surface::css(6.))
            .bg(rgb(0x000000))
            .border_1()
            .border_color(cx.theme().border)
            .into_any_element();
        let email = Input::new(&self.email)
            .appearance(false)
            .id("feedback-email")
            .aria_label(tr("TEXT_EMAIL"))
            .w_full()
            .h(surface::css(27.))
            .px(surface::css(6.))
            .bg(rgb(0x000000))
            .border_1()
            .border_color(cx.theme().border)
            .into_any_element();
        let description = Textarea::new(&self.description)
            .appearance(false)
            .aria_label(tr("TEXT_DETAIL_YOUR_FEEDBACK"))
            .w_full()
            .h(surface::css(213.))
            .p(surface::css(6.))
            .bg(rgb(0x000000))
            .border_1()
            .border_color(cx.theme().border)
            .into_any_element();
        let category_select = surface::select(&category_state)
            .id("feedback-category")
            .items(category_choices())
            .w_full()
            .accessibility_label(tr("TEXT_SELECT_TYPE"));
        let app_select = surface::select(&apps)
            .id("feedback-app")
            .items(app_choices())
            .w_full()
            .accessibility_label(tr("TEXT_SELECT_SOFTWARE"));
        let device_select = surface::select(&device)
            .id("feedback-device")
            .items(device_choices())
            .w_full()
            .accessibility_label(tr("TEXT_DEVICE_NAME"));
        let submit = BaseButton::new("feedback-submit")
            .accessibility_label(tr("TEXT_SUBMIT"))
            .px(surface::css(16.))
            .py(surface::css(7.))
            .bg(cx.theme().primary)
            .text_color(cx.theme().primary_foreground)
            .disabled(!self.valid(cx))
            .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
            .child(tr("TEXT_SUBMIT"));

        let mut root = v_flex()
            .id("feedback-form")
            .w_full()
            .gap(surface::css(10.))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(category_select)
                    .when(category.as_deref() != Some("4"), |view| view.child(app_select)),
            );
        if app.as_deref() == Some("s") {
            root = root.child(device_select);
        }
        if category.as_deref() != Some("4") {
            root = root.child(email);
            if self.error_email {
                root = root.child(
                    div()
                        .text_size(surface::css(12.))
                        .text_color(cx.theme().primary)
                        .child("Please enter a valid email address"),
                );
            }
        }
        root = root.child(title).child(description).child(
            div()
                .w_full()
                .text_color(cx.theme().muted_foreground)
                .text_size(surface::css(12.))
                .text_right()
                .child(format!(
                    "{} / {}",
                    utf16_len(&self.description.read(cx).value()),
                    DESCRIPTION_LIMIT
                )),
        );
        if category.as_deref() == Some("3") {
            root = root.child(
                Checkbox::new("feedback-with-logs")
                    .label(tr("TEXT_SEND_LOG_FILE"))
                    .checked(self.with_logs)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        this.with_logs = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if category.as_deref() == Some("4") {
            root = root.child(
                v_flex()
                    .gap(surface::css(8.))
                    .p(surface::css(12.))
                    .bg(rgb(0x222222))
                    .child(
                        div()
                            .text_color(cx.theme().primary)
                            .child(tr("TEXT_PRIVACY_ENQUIRY_TITLE")),
                    )
                    .child(div().child(tr("TEXT_PRIVACY_ENQUIRES_DESCRIPTION")))
                    .child(crate::shell::service_pages::source_link(
                        "feedback-privacy-link",
                        tr("TEXT_CONTACT_US"),
                        "https://www.razer.com/privacy-enquiries",
                        cx,
                    )),
            );
        }
        root = root.child(h_flex().w_full().justify_center().child(submit));
        if self.unavailable {
            root = root.child(
                div()
                    .w_full()
                    .p(surface::css(10.))
                    .border_1()
                    .border_color(cx.theme().primary)
                    .text_color(cx.theme().primary)
                    .child("服务未连接；草稿仍保留在本地。"),
            );
        }
        root.into_any_element()
    }
}

impl Render for FeedbackPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("feedback-container")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .child(
                div()
                    .id("feedback-page")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .items_center()
                    .pt(surface::css(30.))
                    .pb(surface::css(20.))
                    .child(
                        v_flex()
                            .w(surface::css(500.))
                            .max_w_full()
                            .gap(surface::css(12.))
                            .child(self.form(window, cx)),
                    ),
            )
            .child(
                v_flex()
                    .id("feedback-footer")
                    .w_full()
                    .h(surface::css(60.))
                    .items_center()
                    .justify_end()
                    .pb(surface::css(20.))
                    .text_size(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(tr("TEXT_EARN_REWARD"))
                            .child(crate::shell::service_pages::source_link(
                                "feedback-bug-bounty-link",
                                tr("TEXT_JOIN_THE_BUG_BOUNTY_PROGRAM"),
                                "https://www.razer.com/security/bug-bounty-program",
                                cx,
                            )),
                    ),
            )
    }
}
