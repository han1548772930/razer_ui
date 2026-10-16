use super::*;
use controls::spinner;
use gpui_kit::base::motion;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallState {
    Unknown,
    Detecting,
    Available,
    Waiting,
    Downloading,
    Saving,
    Installing,
    Error,
}
impl InstallState {
    pub fn busy(self) -> bool {
        matches!(
            self,
            Self::Waiting | Self::Downloading | Self::Saving | Self::Installing
        )
    }
    pub fn disabled(self) -> bool {
        matches!(
            self,
            Self::Unknown | Self::Detecting | Self::Saving | Self::Installing
        )
    }
    pub fn progress(self) -> f32 {
        match self {
            Self::Downloading => 0.4,
            Self::Saving | Self::Installing => 1.,
            _ => 0.,
        }
    }
    pub fn status(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting...",
            Self::Downloading => "Downloading 4 MB/10 MB",
            Self::Saving => "Saving...",
            Self::Installing => "Installing...",
            _ => "",
        }
    }
    pub fn after_action(self) -> Self {
        match self {
            Self::Waiting | Self::Downloading => Self::Available,
            Self::Available | Self::Error => Self::Waiting,
            _ => self,
        }
    }
}

pub fn progress_bar(
    id: &'static str,
    progress: f32,
    height: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let progress = motion::transition(
        (id, "progress"),
        progress,
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    );
    div()
        .w_full()
        .h(css(height))
        .rounded(css(4.))
        .overflow_hidden()
        .bg(AlexaColors::progress_track())
        .child(
            div()
                .h_full()
                .w(relative(progress))
                .rounded(css(3.))
                .bg(cx.theme().primary),
        )
        .into_any_element()
}

impl AlexaPage {
    pub fn install_page(
        &self,
        state: InstallState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let header = h_flex()
            .w_full()
            .p(css(20.))
            .pr(css(30.))
            .bg(cx.theme().group_box);
        if matches!(state, InstallState::Unknown | InstallState::Detecting) {
            return v_flex()
                .w_full()
                .max_w(css(940.))
                .mx_auto()
                .mt(css(20.))
                .child(
                    header
                        .when(state == InstallState::Detecting, |view| {
                            view.child(
                                div()
                                    .size(css(40.))
                                    .mr(css(10.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(spinner(34.)),
                            )
                        })
                        .child(div().flex_1().child(if state == InstallState::Unknown {
                            "尚未读取 Alexa 模块安装状态"
                        } else {
                            "Detecting module..."
                        })),
                )
                .when(state == InstallState::Unknown, |view| {
                    view.child(
                        div()
                            .p(css(20.))
                            .bg(razer_widgets::theme::MainPageColors.mobile_action())
                            .child("安装服务未连接。使用上方本地预览查看原版安装界面。"),
                    )
                })
                .into_any_element();
        }
        v_flex()
            .w_full()
            .max_w(css(940.))
            .mx_auto()
            .mt(css(20.))
            .when(state == InstallState::Error, |view| {
                view.child(
                    h_flex()
                        .mb(css(20.))
                        .p(css(10.))
                        .bg(cx.theme().danger)
                        .child(
                            div()
                                .flex_1()
                                .child(format!("{}（本地示例）", text("TEXT_ERROR_UNEXPECTED"))),
                        )
                        .child(
                            text_button("alexa-install-error-close", text("TEXT_CLOSE"), cx)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.installer = Some(InstallState::Available);
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                header
                    .child(
                        img("synapse/module-alexa.svg")
                            .size(css(40.))
                            .mr(css(10.))
                            .flex_shrink_0(),
                    )
                    .child(div().flex_1().child("ALEXA"))
                    .when(state.busy(), |view| {
                        view.child(
                            v_flex()
                                .relative()
                                .w(css(200.))
                                .mr(css(30.))
                                .child(progress_bar(
                                    "alexa-installer-progress",
                                    state.progress(),
                                    8.,
                                    window,
                                    cx,
                                ))
                                .child(
                                    div()
                                        .absolute()
                                        .top_full()
                                        .mt(css(5.))
                                        .w_full()
                                        .text_right()
                                        .text_size(css(12.))
                                        .text_color(
                                            razer_widgets::theme::SettingsButtonColors::background(
                                            ),
                                        )
                                        .child(state.status()),
                                ),
                        )
                    })
                    .child(
                        source_button(
                            "alexa-install-action",
                            if state.busy() { "Cancel" } else { "Install" },
                            if state.busy() {
                                SourceButtonKind::Gray
                            } else {
                                SourceButtonKind::Green
                            },
                            state.disabled(),
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(state) = this.installer {
                                this.installer = Some(state.after_action());
                                cx.notify();
                            }
                        })),
                    ),
            )
            .child(
                h_flex()
                    .items_start()
                    .p(css(20.))
                    .bg(razer_widgets::theme::MainPageColors.mobile_action())
                    .child(
                        div()
                            .w(css(288.))
                            .h(css(162.))
                            .mr(css(20.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                img("synapse/module-alexa.png")
                                    .size_full()
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().mb(css(20.)).child(format!(
                                "{} {}",
                                text("ALEXA_PAGE_DESC_1"),
                                text("ALEXA_PAGE_DESC_2")
                            )))
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap(css(20.))
                                    .child(
                                        Link::new("alexa-install-learn-more")
                                            .href("https://www.razer.com/chroma/alexa")
                                            .open_with(|url, _, _, cx| cx.open_url(url))
                                            .underline()
                                            .mr_auto()
                                            .child("Learn More"),
                                    )
                                    .child("Size: 10 MB（示例）")
                                    .child("Restart Synapse required"),
                            )
                            .child(
                                div()
                                    .mt(css(12.))
                                    .text_size(css(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "图像及大小用于本地界面示例，不代表已安装模块或可用更新。",
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
