//! Current mounted Performance Matcher panels and popups. Local view state and
//! observed service state stay separate; emitted actions are never fake replies.
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::{
    base::{ElementExt as _},
    component::{
        Disableable, IndexPath, h_flex,
        input::{Input, InputEvent, InputState},
        select::{SelectEvent, SelectItem, SelectState},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};
use razer_device::dpi_matcher::{Phase, Product};
use razer_device::{dpi_matcher_storage::Library, mouse_current_dpi::SourceStage};
use razer_i18n::t;
use razer_widgets::{scroll::SourceScrollable as _, surface};
use std::{cell::Cell, rc::Rc, time::Duration};
#[path = "mouse_dpi_matcher_state.rs"]
mod state;
pub use state::{MatcherAction, MatcherCommand, MatcherObservation, MatcherState, ProfileItem};

impl SelectItem for ProfileItem {
    type Value = String;
    fn title(&self) -> SharedString {
        self.name().to_owned().into()
    }
    fn value(&self) -> &String {
        self.value_id()
    }
    fn disabled(&self) -> bool {
        self.disabled()
    }
}

pub struct MouseDpiMatcher {
    product: Product,
    collapse_widget_margins: bool,
    state: MatcherState,
    initial_library: Library,
    selector: Entity<SelectState<Vec<ProfileItem>>>,
    rename_input: Entity<InputState>,
    syncing: bool,
    mounted: bool,
    popup_scroll: ScrollHandle,
    body_scroll: ScrollHandle,
    calibrate_bounds: Rc<Cell<Bounds<Pixels>>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<MatcherAction> for MouseDpiMatcher {}
impl MouseDpiMatcher {
    pub fn new(
        product: Product,
        collapse_widget_margins: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut state = MatcherState::default();
        state.observe_unmounted(MatcherObservation::Language(razer_i18n::locale()));
        let (items, index) = state.dataset(&t("NONE"));
        let initial_library = state.library().clone();
        let selector =
            cx.new(|cx| SelectState::new(items, Some(IndexPath::new(index)), window, cx));
        let rename_input = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let select_subscription =
            cx.subscribe_in(&selector, window, |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SelectEvent::Confirm(Some(guid)) = event {
                    this.emit(this.state.choose(guid), cx);
                    // Source reducers apply the local choice before middleware; it
                    // remains a draft until the real service returns an observation.
                    this.sync_selector(window, cx);
                }
            });
        let rename_subscription =
            cx.subscribe_in(&rename_input, window, |this, input, event, window, cx| {
                if this.syncing || this.state.rename().is_none() {
                    return;
                }
                match event {
                    InputEvent::Blur => {
                        let text = input.read(cx).value().to_string();
                        let actions = this.state.commit_rename(&text);
                        this.emit(actions, cx);
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => window.blur(cx),
                    _ => {}
                }
            });
        Self {
            product,
            collapse_widget_margins,
            state,
            initial_library,
            selector,
            rename_input,
            syncing: false,
            mounted: false,
            popup_scroll: ScrollHandle::default(),
            body_scroll: ScrollHandle::default(),
            calibrate_bounds: Rc::new(Cell::new(Bounds::default())),
            _subscriptions: vec![select_subscription, rename_subscription],
        }
    }

    fn element_id(&self, role: &'static str) -> ElementId {
        SharedString::from(format!(
            "mouse-matcher-{}-{role}",
            self.product.product_id()
        ))
        .into()
    }
    /// Invoke after the owner subscribes, only while the actual Performance
    /// component is mounted. Constructor/render do not emit GET_STATE.
    pub fn mount(&mut self, cx: &mut Context<Self>) {
        if self.mounted {
            return;
        }
        self.mounted = true;
        let actions = self.state.mount();
        self.emit(actions, cx);
    }
    pub fn unmount(&mut self, cx: &mut Context<Self>) {
        self.mounted = false;
        self.state.unmount();
        cx.notify();
    }
    pub fn observe(
        &mut self,
        observation: MatcherObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let actions = if self.mounted {
            self.state.observe(observation)
        } else {
            self.state.observe_unmounted(observation)
        };
        self.emit(actions, cx);
        self.sync_selector(window, cx);
        cx.notify();
    }
    pub fn state(&self) -> &MatcherState {
        &self.state
    }
    /// Local profile/discard restores retain the source's initial placeholder
    /// identity when no matcher library has been observed or edited yet.
    pub fn restore_profiles(
        &mut self,
        library: Option<Library>,
        selected: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let library = library.unwrap_or_else(|| self.initial_library.clone());
        self.observe(
            MatcherObservation::Profiles { library, selected },
            window,
            cx,
        );
    }
    pub fn observe_stages(
        &mut self,
        stages: Vec<SourceStage>,
        active: usize,
        cx: &mut Context<Self>,
    ) {
        self.state
            .observe(MatcherObservation::Stages { stages, active });
        cx.notify();
    }
    /// Parent's browser-equivalent window click adapter must call this for
    /// clicks outside this entity. Stopped close/cancel/reset clicks are excluded.
    pub fn window_click(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let inside = self.calibrate_bounds.get().contains(&position);
        let actions = self.state.window_click(inside);
        self.emit(actions, cx);
        cx.notify();
    }
    pub fn document_click(&mut self, inside_profile_actions: bool, cx: &mut Context<Self>) {
        self.state.document_click(inside_profile_actions);
        cx.notify();
    }
    fn emit(&mut self, actions: Vec<MatcherAction>, cx: &mut Context<Self>) {
        for action in actions {
            if let Err(error) = self.state.apply_local_action(&action) {
                eprintln!("[dpi-matcher] local reducer rejected action: {error:?}");
                continue;
            }
            cx.emit(action);
        }
        cx.notify();
    }
    fn sync_selector(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (items, index) = self.state.dataset(&t("NONE"));
        self.syncing = true;
        self.selector.update(cx, |select, cx| {
            select.set_items(items, window, cx);
            select.set_selected_index(Some(IndexPath::new(index)), window, cx);
        });
        self.syncing = false;
    }
    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(name) = self.state.begin_rename(&t("NONE")) {
            self.syncing = true;
            self.rename_input.update(cx, |input, cx| {
                input.set_value(name, window, cx);
                input.focus(window, cx);
                input.select_all(window, cx);
            });
            self.syncing = false;
            cx.notify();
        }
    }
    fn profile_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (items, index) = self.state.dataset(&t("NONE"));
        let sentinel = items.get(index).is_none_or(|item| item.guid() == "none");
        let disabled = items.len() < 2 && sentinel;
        let selector = if self.state.rename().is_some() {
            div()
                .ml(surface::css(15.))
                .mt(surface::css(1.))
                .w(surface::css(230.))
                .child(
                    Input::new(&self.rename_input)
                        .appearance(false)
                        .h(surface::css(27.))
                        .p(surface::css(5.))
                        .border_1()
                        .border_color(rgb(0x44d62c))
                        .rounded_none()
                        .bg(rgb(0x111111))
                        .text_color(rgb(0xcccccc))
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.)),
                )
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" && this.state.rename().is_some() {
                        cx.stop_propagation();
                        let actions = this.state.escape_rename();
                        this.emit(actions, cx);
                        cx.notify();
                    }
                }))
                .into_any_element()
        } else {
            surface::select(&self.selector)
                .items(items)
                .placeholder("None")
                .accessibility_label(t("SENSITIVITY_MATCHER"))
                .disabled(disabled)
                // The source dims the entire profile-bar ancestor instead.
                .opacity(1.)
                .w(surface::css(230.))
                .mx(surface::css(10.))
                .into_any_element()
        };
        let mut dots = BaseButton::new(self.element_id("profile-actions"))
            .accessibility_label(t("MORE"))
            .disabled(sentinel)
            .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
            .relative()
            .size(surface::css(26.))
            .mr(surface::css(10.))
            .border_1()
            .border_color(if self.state.actions_open() {
                rgb(0x44d62c)
            } else {
                rgb(0x222222)
            })
            .when(!sentinel, |button| {
                button
                    .hover(|style| style.border_color(rgb(0x44d62c)))
                    .active(|style| style.border_color(rgb(0x44d62c)))
            })
            .children(
                self.state
                    .rename()
                    .is_none()
                    .then(|| img("synapse/mouse-matcher-190/more.svg").size(surface::css(20.))),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.state.toggle_actions();
                cx.notify();
            }));
        if self.state.actions_open() {
            let menu = v_flex()
                .absolute()
                .top(surface::css(26.))
                .left(surface::css(-1.))
                .min_w(surface::css(76.))
                .py(surface::css(2.5))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .bg(rgb(0x000000))
                .child(
                    menu_button(self.element_id("rename"), "RENAME").on_click(cx.listener(
                        |this, _, window, cx| {
                            cx.stop_propagation();
                            this.begin_rename(window, cx);
                        },
                    )),
                )
                .child(
                    div()
                        .h(surface::css(1.))
                        .mx(surface::css(6.))
                        .mr(surface::css(11.))
                        .my(surface::css(2.5))
                        .bg(rgb(0x5d5d5d)),
                )
                .child(
                    menu_button(self.element_id("delete"), "DELETE").on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            let actions = this.state.delete_profile();
                            this.emit(actions, cx);
                            cx.notify();
                        },
                    )),
                )
                .child(
                    menu_button(self.element_id("delete-all"), "DELETE_ALL").on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            this.state.open_delete_all();
                            cx.notify();
                        },
                    )),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.state.document_click(false);
                        cx.notify();
                    }),
                );
            dots = dots.child(razer_assets::deferred_resource(menu).with_priority(111));
        }
        h_flex()
            .w(relative(0.95))
            .h(surface::css(26.))
            .mx_auto()
            .mb(surface::css(10.))
            .pl(relative(0.03))
            .justify_center()
            .text_size(surface::css(14.))
            .when(disabled, |row| row.opacity(0.3))
            .child(selector)
            .child(dots)
            .into_any_element()
    }
    fn delete_all_popup(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .absolute()
            .top(surface::css(205.))
            .right(surface::css(70.))
            .w(surface::css(300.))
            .items_center()
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd4949))
            .rounded(surface::css(3.))
            .shadow(vec![source_shadow(window, 6., 10., 0x00000033)])
            .child(
                div()
                    .text_color(rgb(0xfd4949))
                    .font_weight(FontWeight::BOLD)
                    .text_center()
                    .child(t("DELETE_ALL_SENSITIVITY_MATCHER_POPUP_HEADER").to_uppercase()),
            )
            .child(
                div()
                    .text_center()
                    .child(t("DELETE_ALL_SENSITIVITY_MATCHER_POPUP_DESC")),
            )
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(
                        delete_button(
                            self.element_id("delete-all-cancel"),
                            "CANCEL",
                            0x707070,
                            0xffffff,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.close_delete_all();
                            cx.notify();
                        })),
                    )
                    .child(
                        delete_button(
                            self.element_id("delete-all-confirm"),
                            "DELETE",
                            0xfd4949,
                            0x111111,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let actions = this.state.delete_all();
                            this.emit(actions, cx);
                            cx.notify();
                        })),
                    ),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.document_click(false);
                    cx.notify();
                }),
            )
            .into_any_element()
    }
    fn step_content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let phase = self.state.phase();
        if phase == Phase::Idle {
            return div().into_any_element();
        }
        let desc = match phase {
            Phase::Ready => "SENSITIVITY_MATCHER_ACTION_DESC_1",
            Phase::Completed => "SENSITIVITY_MATCHER_ACTION_DESC_3",
            _ => "SENSITIVITY_MATCHER_ACTION_DESC_2",
        };
        // The source uses block children (READY adds one unstyled div), not
        // a flex column: adjacent vertical margins must keep block behavior.
        let mut content = div().child(
            div()
                .mt(surface::css(50.))
                .mb(surface::css(10.))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_center()
                .child(t(desc)),
        );
        let mut artwork = h_flex()
            .items_center()
            .w(surface::css(450.))
            .h(surface::css(150.))
            .mt(surface::css(70.))
            .mx_auto();
        if phase == Phase::Completed {
            artwork = artwork.justify_center().child(
                img("synapse/mouse-matcher-190/completed.svg")
                    .w(surface::css(136.))
                    .h(surface::css(109.)),
            );
        } else {
            artwork = artwork.child(
                img("synapse/mouse-matcher-190/handmouse.svg")
                    .w(surface::css(150.))
                    .h(surface::css(136.)),
            );
            if phase == Phase::Ready {
                artwork = artwork.child(dotted(80.).mx_auto()).child(
                    img("synapse/mouse-matcher-190/synapse.svg")
                        .w(surface::css(158.))
                        .h(surface::css(103.)),
                );
            } else {
                artwork = artwork.child(
                    img("synapse/mouse-matcher-190/arrow.svg")
                        .w(surface::css(32.))
                        .h(surface::css(26.))
                        .ml(surface::css(50.))
                        .mb(surface::css(50.)),
                );
            }
        }
        content = content.child(artwork);
        if matches!(phase, Phase::Calibrating | Phase::Error) {
            let progress = gpui_kit::base::motion::transition(
                (self.element_id("progress"), "width"),
                self.state.process(),
                gpui_kit::base::motion::Transition::new(Duration::from_millis(500))
                    .easing(gpui_kit::base::motion::Easing::Ease),
                window,
                cx,
            );
            content = content.child(
                div()
                    .mx_auto()
                    .mt(surface::css(10.))
                    .w(surface::css(414.))
                    .h(surface::css(7.))
                    .bg(rgba(0x44d62c4d))
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress / 100.))
                            .bg(rgb(0x44d62c)),
                    ),
            );
        }
        if phase == Phase::Completed {
            content = content
                .child(
                    div()
                        .mx_auto()
                        .font_family("RazerF5")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(14.))
                        .h(surface::css(14.))
                        .w(surface::css(171.))
                        .text_center()
                        .child(t("CALIBRATION_INFORMATION")),
                )
                .child(dpi_table(
                    self.state.current_dpi(),
                    self.state.new_dpi(),
                    window,
                ))
                .child(
                    BaseButton::new(self.element_id("reset"))
                        .accessibility_label(t("RESET_PROFILE"))
                        .mx_auto()
                        .mt(surface::css(20.))
                        .h(surface::css(17.))
                        .font_weight(FontWeight::THIN)
                        .text_size(surface::css(14.))
                        .child(div().underline().child(t("RESET_PROFILE")))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            let actions = this.state.reset();
                            this.emit(actions, cx);
                            cx.notify();
                        })),
                );
        }
        content.into_any_element()
    }
    fn warning(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x27272799))
            .child(
                v_flex()
                    .w(surface::css(400.))
                    .min_h(surface::css(120.))
                    .p(surface::css(20.))
                    .gap(surface::css(17.))
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(rgb(0xfd8611))
                    .rounded(surface::css(3.))
                    .shadow(vec![source_shadow(window, 4., 8., 0x00000033)])
                    .child(
                        h_flex()
                            .justify_center()
                            .text_color(rgb(0xfd8611))
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .line_height(surface::css(15.))
                            .child(
                                img("synapse/mouse-matcher-190/warning.svg")
                                    .size(surface::css(20.))
                                    .mr(surface::css(10.)),
                            )
                            .child(t("SENSITIVITY_MATCHER_CALIBRATION_ERROR").to_uppercase()),
                    )
                    .child(
                        div()
                            .text_center()
                            .text_color(rgb(0x999999))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .max_w(surface::css(360.))
                            .child(t("SENSITIVITY_MATCHER_CALIBRATION_ERROR_DESC")),
                    )
                    .child(
                        h_flex()
                            .justify_center()
                            .gap(surface::css(5.))
                            .child(
                                warning_button(
                                    self.element_id("error-cancel"),
                                    "CANCEL",
                                    0x707070,
                                    0xffffff,
                                    window,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        cx.stop_propagation();
                                        let actions = this.state.cancel();
                                        this.emit(actions, cx);
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                warning_button(
                                    self.element_id("error-retry"),
                                    "RETRY",
                                    0x44d62c,
                                    0x222222,
                                    window,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        cx.stop_propagation();
                                        let actions = this.state.reset();
                                        this.emit(actions, cx);
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> ViewportLayer {
        let viewport = window.viewport_size();
        let popup_height = (viewport.height - window.rem_size() * (105. / 16.)).max(px(0.));
        let body_max_height = (popup_height - window.rem_size() * (36. / 16.)).max(px(0.));
        let steps = h_flex()
            .mx_auto()
            .relative()
            .top(surface::css(20.))
            .w(surface::css(283.))
            .h(surface::css(33.))
            .justify_between()
            .children((1..=3).flat_map(|step| {
                let mut children = vec![
                    div()
                        .size(surface::css(33.))
                        .rounded_full()
                        .bg(if self.state.step_active(step) {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x707070)
                        })
                        .child(
                            div()
                                .relative()
                                .top(surface::css(6.))
                                .left(surface::css(12.))
                                .w(surface::css(11.))
                                .h(surface::css(21.))
                                .text_color(rgb(0x000000))
                                .text_size(surface::css(18.))
                                .line_height(surface::css(21.))
                                .child(step.to_string()),
                        )
                        .into_any_element(),
                ];
                if step != 3 {
                    children.push(dotted(80.).into_any_element());
                }
                children
            }));
        let popup = div()
            .id(self.element_id("popup"))
            .relative()
            .w(surface::css(800.))
            .min_w(surface::css(546.))
            .h(popup_height)
            .max_h(popup_height)
            .bg(rgb(0x222222))
            .rounded_t(surface::css(5.))
            .child(
                div()
                    .relative()
                    .h(surface::css(36.))
                    .flex_shrink_0()
                    .shadow(vec![source_shadow(window, 1., 0., 0x5d5d5dff)])
                    .text_center()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .pt(surface::css(9.))
                    .pb(surface::css(8.))
                    .text_color(rgb(0x999999))
                    .child(t("SENSITIVITY_MATCHER").to_uppercase())
                    .child(
                        BaseButton::new(self.element_id("close"))
                            .accessibility_label(t("CLOSE"))
                            .absolute()
                            .right_0()
                            .top_0()
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|style| style.bg(rgba(0xffffff1a)))
                            .active(|style| style.bg(rgba(0x0000001a)))
                            .child(
                                img("synapse/mouse-matcher-190/close.svg").size(surface::css(20.)),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                let actions = this.state.close();
                                this.emit(actions, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id(self.element_id("popup-body"))
                    .min_h(surface::css(569.))
                    .max_h(body_max_height)
                    .pt(surface::css(20.))
                    .pb(surface::css(30.))
                    .pl(surface::css(25.))
                    .scrollable_y()
                    .track_scroll(&self.body_scroll)
                    .child(
                        div()
                            .child(
                                div()
                                    .text_center()
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(17.))
                                    .mb(surface::css(10.))
                                    .child(t("SENSITIVITY_MATCHER_STEP_DESC")),
                            )
                            .child(steps)
                            .child(self.step_content(window, cx))
                            .child(
                                h_flex()
                                    // .extra-style !important overrides inline120
                                    // when dS renders the completed state.
                                    .mt(surface::css(if self.state.phase() == Phase::Completed {
                                        50.
                                    } else {
                                        120.
                                    }))
                                    .justify_center()
                                    .child(
                                        footer_button(
                                            self.element_id("cancel"),
                                            "CANCEL",
                                            0x707070,
                                            0xffffff,
                                            false,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                cx.stop_propagation();
                                                let actions = this.state.cancel();
                                                this.emit(actions, cx);
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(
                                        footer_button(
                                            self.element_id("apply"),
                                            "APPLY",
                                            0x44d62c,
                                            0x000000,
                                            self.state.phase() != Phase::Completed,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                let actions = this.state.apply();
                                                this.emit(actions, cx);
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                let actions = this.state.window_click(false);
                this.emit(actions, cx);
                cx.notify();
            }))
            .children((self.state.phase() == Phase::Error).then(|| {
                razer_assets::deferred_resource(self.warning(window, cx)).with_priority(999)
            }))
            // dS.componentDidMount applies overflow:auto to the popup itself.
            // Keep the body's own min-height and scroll limit on the body;
            // short viewports also need to scroll the outer popup.
            .scrollable_both()
            .track_scroll(&self.popup_scroll);
        let backdrop = div()
            .id(self.element_id("backdrop"))
            .w(viewport.width)
            .h(viewport.height)
            .bg(rgba(0x00000080))
            .relative()
            .child(
                img("synapse/mouse-matcher-190/glow.svg")
                    .absolute()
                    .bottom_0()
                    .w_full(),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                let actions = this.state.window_click(false);
                this.emit(actions, cx);
                cx.notify();
            }));
        ViewportLayer {
            backdrop: backdrop.into_any_element(),
            popup: popup.into_any_element(),
        }
    }
}
impl Render for MouseDpiMatcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.calibrate_bounds.clone();
        surface::panel_with_help(
            t("SENSITIVITY_MATCHER"),
            surface::help_control(self.element_id("help"), t("SENSITIVITY_MATCHER_TOOLTIP")),
            cx,
        )
        .when(self.collapse_widget_margins, |panel| panel.mb_0())
        .id(self.element_id("panel"))
        .child(t("SENSITIVITY_MATCHER_DESC"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .relative()
                .child(
                    BaseButton::new(self.element_id("calibrate"))
                        .accessibility_label(t("CALIBRATE"))
                        .mt(surface::css(5.))
                        .h(surface::css(28.))
                        .min_w(surface::css(90.))
                        .px(surface::css(16.))
                        .pt(surface::css(7.))
                        .pb(surface::css(6.))
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .bg(rgb(0x44d62c))
                        .text_color(rgb(0x212121))
                        .text_size(surface::css(12.))
                        .child(t("CALIBRATE"))
                        .on_prepaint(move |rect, _, _| bounds.set(rect))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.state.open();
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .mt(surface::css(20.))
                        .w_full()
                        .child(t("SENSITIVITY_MATCHER_PROFILE_DESC")),
                )
                .child(
                    div()
                        .ml(surface::css(-16.))
                        .mt(surface::css(10.))
                        .child(self.profile_bar(window, cx)),
                )
                .children(self.state.delete_all_open().then(|| {
                    razer_assets::deferred_resource(self.delete_all_popup(window, cx))
                        .with_priority(112)
                })),
        )
        .children(
            self.state.popup().then(|| {
                razer_assets::deferred_resource(self.overlay(window, cx)).with_priority(106)
            }),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            let actions = this.state.window_click(false);
            this.emit(actions, cx);
            cx.notify();
        }))
    }
}
fn menu_button(id: impl Into<ElementId>, key: &str) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .h(surface::css(19.))
        .px(surface::css(6.))
        .py(surface::css(2.5))
        .justify_start()
        .text_size(surface::css(12.))
        .line_height(surface::css(17.))
        .text_color(rgb(0x999999))
        .bg(rgb(0x000000))
        .hover(|style| style.bg(rgb(0x1a1a1a)))
        .child(t(key))
}
fn delete_button(
    id: impl Into<ElementId>,
    key: &str,
    background: u32,
    foreground: u32,
) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .px(surface::css(5.))
        .py(surface::css(4.))
        .font_family("Arial")
        .text_size(surface::css(12.))
        .line_height(surface::css(17.))
        .text_center()
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .hover(|style| style.opacity(0.8))
        .child(t(key).to_uppercase())
}
// The source's three action groups have different selectors and box metrics.
// Keep those distinctions instead of normalizing them through one button.
fn footer_button(
    id: impl Into<ElementId>,
    key: &str,
    background: u32,
    foreground: u32,
    disabled: bool,
) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .disabled(disabled)
        .flex_shrink_0()
        .h(surface::css(27.))
        .min_w(surface::css(100.))
        .mr(surface::css(10.))
        .pt(surface::css(6.))
        .pr(surface::css(10.))
        .pb(surface::css(7.))
        .pl(surface::css(6.))
        .border_1()
        .border_color(rgb(0x000000))
        .rounded(surface::css(3.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .text_center()
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .when(!disabled, |button| {
            button
                .hover(|style| style.opacity(0.8))
                .active(|style| style.opacity(0.6))
        })
        .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
        .child(t(key).to_uppercase())
}
fn warning_button(
    id: impl Into<ElementId>,
    key: &str,
    background: u32,
    foreground: u32,
    window: &Window,
) -> BaseButton {
    // Native <button> has its UA font and normal line height; the warning
    // selector sets neither a fixed height nor the footer's 14px line-height.
    let mut font = window.text_style().font();
    font.family = "Arial".into();
    font.weight = FontWeight::NORMAL;
    let font_id = window.text_system().resolve_font(&font);
    let font_size = window.rem_size() * (12. / 16.);
    let line_height = window.text_system().ascent(font_id, font_size)
        + window.text_system().descent(font_id, font_size);
    BaseButton::new(id)
        .accessibility_label(t(key))
        .w(surface::css(90.))
        .px(surface::css(9.))
        .py(surface::css(6.))
        .rounded(surface::css(3.))
        .font_family("Arial")
        .text_size(surface::css(12.))
        .line_height(line_height)
        .text_center()
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .child(t(key))
}
fn source_shadow(window: &Window, y: f32, blur: f32, color: u32) -> BoxShadow {
    let css_pixel = window.rem_size() / 16.;
    BoxShadow {
        color: rgba(color).into(),
        offset: point(px(0.), css_pixel * y),
        blur_radius: css_pixel * blur,
        spread_radius: px(0.),
        inset: false,
    }
}
fn dotted(width: f32) -> Div {
    h_flex()
        .w(surface::css(width))
        .h(surface::css(2.))
        .justify_between()
        .children((0..(width / 4.) as usize).map(|_| {
            div()
                .size(surface::css(2.))
                .rounded_full()
                .bg(rgb(0x707070))
        }))
}
fn dpi_table(current: Option<f64>, new: f64, window: &Window) -> Div {
    let labels = [t("CURRENT_DPI"), t("NEW_DPI")];
    let values = [current, Some(new)].map(|value| {
        // The initial reducer omits currentDpi; React renders that child empty.
        value.map_or_else(String::new, razer_device::source_string::number_text)
    });
    // dS uses a nowrap HTML table with two shared columns, not independently
    // justified rows. Account for UA 2px border spacing and 1px td padding.
    let label_min = labels
        .iter()
        .map(|label| surface::label_width(label, 14., window) + 2.)
        .fold(0_f32, f32::max);
    let value_min = values
        .iter()
        .map(|value| surface::label_width(value, 14., window) + 2.)
        .fold(0_f32, f32::max);
    let width = 118_f32.max(label_min + value_min + 6.);
    let label_width = (width - 6.) * label_min / (label_min + value_min);
    let column = |width| {
        v_flex()
            .w(surface::css(width))
            .flex_shrink_0()
            .gap(surface::css(2.))
            .text_right()
    };
    h_flex()
        .w(surface::css(width))
        .mx_auto()
        .mt(surface::css(10.))
        .p(surface::css(2.))
        .gap(surface::css(2.))
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .whitespace_nowrap()
        .child(
            column(label_width).children(
                labels
                    .into_iter()
                    .map(|label| div().p(surface::css(1.)).child(label)),
            ),
        )
        .child(
            column(width - 6. - label_width).children(values.into_iter().enumerate().map(
                |(index, value)| {
                    div()
                        .p(surface::css(1.))
                        .text_color(if index == 1 { rgb(0x44d62c) } else { rgb(0xcccccc) })
                        .child(value)
                },
            )),
        )
}
struct ViewportLayer {
    backdrop: AnyElement,
    popup: AnyElement,
}
impl IntoElement for ViewportLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ViewportLayer {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let backdrop = self.backdrop.request_layout(window, cx);
        let popup = self.popup.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [backdrop, popup],
                cx,
            ),
            popup,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        popup: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = window.layout_bounds(*popup).size;
        let viewport = window.viewport_size();
        window.with_element_offset(point(px(0.), px(0.)) - bounds.origin, |window| {
            self.backdrop.prepaint(window, cx)
        });
        window.with_element_offset(
            point(
                (viewport.width - size.width) / 2.,
                window.rem_size() * (105. / 16.),
            ) - bounds.origin,
            |window| self.popup.prepaint(window, cx),
        );
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.backdrop.paint(window, cx);
        self.popup.paint(window, cx);
    }
}
