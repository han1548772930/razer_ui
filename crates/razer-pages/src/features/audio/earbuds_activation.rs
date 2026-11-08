//! Local activation-code reading and installed application actions from Sound.
use super::*;
use razer_device::earbuds_auto_pause::{ActivationCode, Operation};
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as C};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(super) struct State {
    pub(super) code: Option<Result<ActivationCode, String>>,
    pub(super) loading: bool,
    open: bool,
    copied: bool,
    installed: Option<Result<bool, String>>,
    generation: u64,
    probe: Option<Task<()>>,
    launch: Option<Task<()>>,
    launch_id: Option<uuid::Uuid>,
    launch_guard: Option<Arc<AtomicBool>>,
    launch_error: Option<String>,
}

impl State {
    pub(super) fn clear_device(&mut self) {
        self.code = None;
        self.loading = false;
        self.open = false;
        self.copied = false;
    }
    pub(super) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.open = false;
        self.copied = false;
        self.probe = None;
        self.installed = None;
        if let Some(active) = self.launch_guard.take() {
            active.store(false, Ordering::Release);
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl AudioProductWorkspace {
    pub fn take_earbuds_activation_cleanup(&mut self) -> Option<Task<()>> {
        self.earbuds_activation.cancel();
        self.earbuds_activation.launch.take()
    }
    pub(super) fn sync_earbuds_activation(&mut self, cx: &mut Context<Self>) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id)
            || self.page != "TAB_SOUND"
        {
            self.earbuds_activation.cancel();
            return;
        }
        if self.earbuds_activation.installed.is_some() || self.earbuds_activation.probe.is_some() {
            return;
        }
        let generation = self.earbuds_activation.generation;
        self.earbuds_activation.probe = Some(cx.spawn(async move |owner, cx| {
            let result = cx
                .background_spawn(async {
                    razer_service::earbuds_activation::installed().map_err(|e| format!("{e:#}"))
                })
                .await;
            let _ = owner.update(cx, |owner, cx| {
                if generation == owner.earbuds_activation.generation {
                    owner.earbuds_activation.installed = Some(result);
                    owner.earbuds_activation.probe = None;
                    cx.notify();
                }
            });
        }));
    }

    fn open_earbuds_activation(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.active {
            return;
        }
        self.earbuds_activation.open = true;
        self.earbuds_activation.copied = false;
        self.earbuds_activation.loading = true;
        self.earbuds.queue_edit(Operation::ActivationCode);
        self.start_earbuds_request(cx);
        cx.notify();
    }

    fn launch_earbuds_thx(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.active
            || self.page != "TAB_SOUND"
            || self.earbuds_activation.launch_id.is_some()
            || !matches!(self.earbuds_activation.installed, Some(Ok(true)))
        {
            return;
        }
        let active = Arc::new(AtomicBool::new(true));
        let id = uuid::Uuid::new_v4();
        self.earbuds_activation.launch_id = Some(id);
        self.earbuds_activation.launch_guard = Some(active.clone());
        let generation = self.earbuds_activation.generation;
        self.earbuds_activation.launch = Some(cx.spawn(async move |owner, cx| {
            let result = cx
                .background_spawn(async move { razer_service::earbuds_activation::launch(active) })
                .await;
            let _ = owner.update(cx, |owner, cx| {
                if owner.earbuds_activation.launch_id != Some(id) {
                    return;
                }
                owner.earbuds_activation.launch_id = None;
                owner.earbuds_activation.launch = None;
                owner.earbuds_activation.launch_guard = None;
                if generation == owner.earbuds_activation.generation {
                    owner.earbuds_activation.launch_error = result.err().map(|e| format!("{e:#}"));
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn earbuds_activation_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut actions = h_flex()
            .mt(surface::css(20.))
            .justify_between()
            .items_center();
        if matches!(self.earbuds_activation.installed, Some(Ok(true))) {
            actions = actions.child(
                Button::new("earbuds-thx-launch")
                    .disabled(!self.earbuds.active || self.earbuds_activation.launch_id.is_some())
                    .h(surface::css(50.))
                    .flex()
                    .items_center()
                    .gap(surface::css(10.))
                    .px(surface::css(16.))
                    .border_1()
                    .border_color(C::text())
                    .rounded(surface::css(3.))
                    .bg(C::background())
                    .hover(|style| style.bg(C::mapping_black()))
                    .active(|style| style.bg(C::background()))
                    .child(img("synapse/earbuds-activation-thx.svg").size(surface::css(30.)))
                    .child(t("THX_SPATIAL_AUDIO_LAUNCH").to_uppercase())
                    .on_click(cx.listener(|this, _, _, cx| this.launch_earbuds_thx(cx))),
            );
        }
        actions = actions.child(
            Button::new("earbuds-activation-show")
                .flex()
                .items_center()
                .gap(surface::css(4.))
                .disabled(!self.earbuds.active)
                .text_size(surface::css(14.))
                .underline()
                .text_color(C::text())
                .hover(|style| style.text_color(C::selected()))
                .child(
                    razer_assets::resource_svg()
                        .path("synapse/earbuds-activation-eye.svg")
                        .size(surface::css(20.)),
                )
                .child(t("SHOW_ACTIVATION_CODE"))
                .on_click(cx.listener(|this, _, _, cx| this.open_earbuds_activation(cx))),
        );
        surface::panel(t("THX_SPATIAL_AUDIO"), cx)
            .id("earbuds-activation-card")
            .test_support()
            .relative()
            .child(t("THX_SPATIAL_AUDIO_DESC"))
            .child(actions)
            .when(self.earbuds_activation.open, |card| {
                card.child(self.earbuds_activation_modal(cx))
            })
            .into_any_element()
    }

    fn earbuds_activation_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let code = self
            .earbuds_activation
            .code
            .as_ref()
            .and_then(|value| value.as_ref().ok());
        let value = code.map(|code| code.value().to_string());
        let copy_value = value.clone();
        let content = h_flex()
            .gap(surface::css(20.))
            .justify_center()
            .p(surface::css(20.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .child(value.map(|value| value.to_uppercase()).unwrap_or_else(|| {
                        if self.earbuds_activation.loading {
                            "..."
                        } else {
                            "—"
                        }
                        .into()
                    })),
            )
            .child(
                v_flex()
                    .relative()
                    .child(
                        Button::new("earbuds-activation-copy")
                            .flex()
                            .items_center()
                            .gap(surface::css(8.))
                            .h(surface::css(27.))
                            .disabled(code.is_none())
                            .text_size(surface::css(12.))
                            .text_color(if self.earbuds_activation.copied {
                                C::activation_copy_pressed()
                            } else {
                                C::selected()
                            })
                            .when(
                                !self.earbuds_activation.copied && code.is_some(),
                                |button| {
                                    button
                                        .hover(|style| style.text_color(C::activation_copy_hover()))
                                        .active(|style| {
                                            style.text_color(C::activation_copy_pressed())
                                        })
                                },
                            )
                            .child(
                                razer_assets::resource_svg()
                                    .path("synapse/earbuds-activation-copy.svg")
                                    .size(surface::css(20.)),
                            )
                            .child(t("COPY").to_uppercase())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(value) = &copy_value {
                                    cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                                    this.earbuds_activation.copied = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .when(self.earbuds_activation.copied, |column| {
                        column.child(
                            div()
                                .absolute()
                                .top_full()
                                .left_0()
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .border_1()
                                .border_color(razer_widgets::theme::AudioEqualizerColors::border())
                                .bg(C::mapping_black())
                                .text_color(C::text())
                                .text_size(surface::css(14.))
                                .child(t("COPIED")),
                        )
                    }),
            );
        razer_assets::deferred_resource(
            div()
                .id("earbuds-activation-modal")
                .test_support()
                .absolute()
                .inset(surface::css(10.))
                .occlude()
                .bg(C::gesture_backdrop())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .w(surface::css(400.))
                        .max_w_full()
                        .bg(C::background())
                        .border_1()
                        .border_color(C::selected())
                        .rounded(surface::css(3.))
                        .child(
                            div()
                                .pt(surface::css(20.))
                                .px(surface::css(20.))
                                .text_center()
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .text_color(C::selected())
                                .child(t("ACTIVATION_CODE_FOR_THX_SPATIAL_AUDIO").to_uppercase()),
                        )
                        .child(content)
                        .child(
                            h_flex().justify_center().pb(surface::css(25.)).child(
                                Button::new("earbuds-activation-close")
                                    .h(surface::css(27.))
                                    .min_w(surface::css(90.))
                                    .bg(C::mapping_cancel())
                                    .hover(|style| style.bg(C::activation_close_hover()))
                                    .active(|style| style.bg(C::activation_close_pressed()))
                                    .text_color(C::mapping_white())
                                    .text_size(surface::css(12.))
                                    .child(t("CLOSE").to_uppercase())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.earbuds_activation.open = false;
                                        this.earbuds_activation.copied = false;
                                        cx.notify();
                                    })),
                            ),
                        ),
                ),
        )
        .with_priority(1001)
        .into_any_element()
    }
}
