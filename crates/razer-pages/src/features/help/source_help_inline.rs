//! Current Help `On`/`In`: 300px card-local profile-del alert, one red button.
use super::*;
use razer_widgets::theme::SourceHelpPopupColors as Colors;

impl SourceHelp {
    pub(in crate::features) fn observe_product_names(
        &mut self,
        names: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !super::super::source_help_owner::supports_product_names(self.device.product_id) {
            return false;
        }
        let Some(values) = names.as_object() else {
            return false;
        };
        let (Some(en), Some(zh)) = (
            values.get("en").and_then(serde_json::Value::as_str),
            values.get("zh-cn").and_then(serde_json::Value::as_str),
        ) else {
            return false;
        };
        self.localized_product_names = Some(BTreeMap::from([
            ("en".into(), en.into()),
            ("zh-cn".into(), zh.into()),
        ]));
        cx.notify();
        true
    }
    fn popup_title(&self, uppercase: bool) -> String {
        let name =
            if super::super::source_help_owner::supports_product_names(self.device.product_id) {
                // Original name helper starts with en/zh-cn empty strings. A model
                // name or edited display name is not its observed source value.
                let language = if i18n::locale().eq_ignore_ascii_case("zh-cn") {
                    "zh-cn"
                } else {
                    "en"
                };
                self.localized_product_names
                    .as_ref()
                    .and_then(|values| values.get(language))
                    .cloned()
                    .unwrap_or_default()
            } else {
                self.device.display_name()
            };
        if uppercase { name.to_uppercase() } else { name }
    }
    pub(super) fn reset_popup(&self, cx: &Context<Self>) -> AnyElement {
        let Some((_, page)) = self.metadata() else {
            return div().into_any_element();
        };
        let oled = matches!(page.popup_kind.as_deref(), Some("oled" | "audio-oled"))
            && !self.observed_ble.unwrap_or(self.device.use_ble);
        let legacy = page.popup_kind.as_deref() == Some("legacy-delete");
        let plain = page.popup_kind.as_deref() == Some("plain-delete");
        let mixer = page.popup_kind.as_deref() == Some("mixer-remove");
        let firmware = page.firmware_reset.is_some();
        let remove = page.popup_kind.as_deref() == Some("normal-remove") || mixer;
        let confirm_label = i18n::t(if legacy || plain {
            "DELETE"
        } else if remove {
            "REMOVE"
        } else {
            "RESET"
        });
        let message = i18n::t(if let Some(message) = page.popup_message.as_deref() {
            message
        } else if page.obm {
            "FACTORY_RESET_MSG"
        } else {
            "FACTORY_RESET_MSG_NO_OBM_DEVICE"
        });
        let popup_bounds = self.popup_layout.popup_bounds.clone();
        let popup = v_flex()
            .id("source-help-reset-popup")
            .absolute()
            .when(!oled && !legacy && !mixer && !firmware, |popup| {
                popup.left_0()
            })
            // Source top:auto follows the reset button and its top margin.
            .when(!oled && !legacy && !mixer && !firmware, |popup| {
                popup.top_full()
            })
            .when(mixer, |popup| {
                popup
                    .left(relative(0.5))
                    .ml(surface::css(-150.))
                    .top(surface::css(42.))
            })
            .when(legacy, |popup| {
                popup.left(surface::css(40.)).top(surface::css(42.))
            })
            .when(oled || firmware, |popup| {
                popup.left(surface::css(140.)).top(surface::css(125.))
            })
            .w(surface::css(300.))
            .p(surface::css(20.))
            .items_center()
            .bg(Colors::background())
            .border_1()
            .border_color(Colors::alert())
            .when(legacy, |popup| popup.border_color(Colors::legacy_alert()))
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .on_prepaint(move |bounds, _, _| popup_bounds.set(bounds))
            .when(!self.inline_reset, |popup| popup.invisible())
            .when(self.inline_reset, |popup| popup.occlude())
            .shadow(vec![BoxShadow {
                inset: false,
                color: Colors::shadow(),
                offset: point(px(0.), surface::css(6.).to_pixels(cx.theme().font_size)),
                blur_radius: surface::css(10.).to_pixels(cx.theme().font_size),
                spread_radius: px(0.),
            }])
            .child(
                div()
                    .mb(surface::css(10.))
                    .text_color(Colors::alert())
                    .when(legacy, |title| {
                        title
                            .text_color(Colors::legacy_title())
                            .font_weight(FontWeight::BOLD)
                    })
                    .when(plain, |title| title.font_weight(FontWeight::BOLD))
                    .text_center()
                    .child(self.popup_title(!legacy)),
            )
            .when(!oled, |popup| {
                popup.child(
                    div()
                        .when(!firmware, |body| body.mb(surface::css(10.)))
                        .when(plain, |body| {
                            body.mt(surface::css(10.)).mx(surface::css(10.))
                        })
                        .text_center()
                        .child(message),
                )
            })
            .when(firmware, |popup| {
                popup.child(
                    div()
                        .mt(surface::css(10.))
                        .mb(surface::css(16.))
                        .mx_auto()
                        .text_center()
                        .text_color(rgb(0x999999))
                        .when(
                            page.firmware_reset
                                .as_ref()
                                .and_then(|v| v.get("shouldPowerOff"))
                                .and_then(|v| v.as_bool())
                                == Some(true),
                            |body| body.child(i18n::t("RESET_WARNING_AUTO_POWER_OFF")),
                        ),
                )
            })
            .when(oled, |popup| {
                popup
                    .child(
                        div()
                            .mb(surface::css(10.))
                            .text_center()
                            .child(i18n::t("FACTORY_RESET_MSG_WITH_MINUTES")),
                    )
                    .child(
                        div()
                            .mb(surface::css(10.))
                            .text_center()
                            .child(i18n::t("FACTORY_RESET_MSG_WITH_MINUTES_DESC")),
                    )
            })
            .child(
                BaseButton::new("source-help-reset-confirm")
                    .accessibility_label(confirm_label.clone())
                    .disabled(!self.inline_reset)
                    .focusable(self.inline_reset)
                    .h(surface::css(27.))
                    .min_w(surface::css(90.))
                    .px(surface::css(5.))
                    .py(surface::css(4.))
                    .border_1()
                    .border_color(Colors::button_border())
                    .rounded(surface::css(3.))
                    .bg(Colors::alert())
                    .text_color(Colors::button_text())
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .when(legacy, |button| {
                        button
                            .bg(Colors::legacy_alert())
                            .text_color(Colors::legacy_button_text())
                            .px_0()
                            .line_height(surface::css(17.))
                    })
                    .when(plain || firmware, |button| {
                        button.px_0().line_height(surface::css(17.))
                    })
                    .hover(|style| style.opacity(0.8))
                    .active(|style| style.opacity(0.6))
                    .child(confirm_label.to_uppercase())
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.confirm_inline_reset(window, cx);
                    })),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if !this.inline_reset {
                    return;
                }
                this.inline_reset = false;
                if this
                    .metadata()
                    .is_some_and(|(_, page)| page.firmware_reset.is_some())
                {
                    this.popup_layout.unmount();
                }
                this.confirmation_generation = this.confirmation_generation.wrapping_add(1);
                cx.notify();
            }));
        if self.inline_reset {
            popup
                .with_animation(
                    SharedString::from(format!(
                        "source-help-reset-open-{}",
                        self.confirmation_generation
                    )),
                    Animation::new(Duration::from_millis(300)).with_easing(linear),
                    |popup, phase| popup.opacity(phase),
                )
                .into_any_element()
        } else {
            popup.into_any_element()
        }
    }

    fn confirm_inline_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, page)) = self.metadata() else {
            return;
        };
        if !page.inline_confirmation
            || !self.inline_reset
            || self.reset_cooldown
            || self.factory_reset_disabled(page)
        {
            return;
        }
        self.inline_reset = false;
        if page.firmware_reset.is_some() {
            self.popup_layout.unmount();
        }
        self.pending_reset = true;
        self.reset_error = None;
        self.reset_generation = self.reset_generation.wrapping_add(1);
        self.reset_overlay_acknowledged = false;
        if page.show_factory_reset_overlay {
            self.reset_overlay_focus.focus(window, cx);
        }
        let callback_allowed =
            !self.observed_ble.unwrap_or(self.device.use_ble) || page.obm_reset_during_ble;
        if callback_allowed
            && page
                .reset_callback
                .as_ref()
                .is_some_and(|callback| callback.action == "SOURCE_CALLBACK_ERROR")
        {
            // The mounted owner calls a missing action prop (585 obmResetDevice,
            // or 1330 addProfile). The actual caller throws synchronously.
            // confirmDel has already closed the popup and disabled the button;
            // the synchronous TypeError prevents its later timeout/submission.
            self.pending_reset = false;
            self.reset_cooldown = true;
            self.reset_cooldown_task = None;
            cx.notify();
            return;
        }
        let oled = page.popup_kind.as_deref() == Some("oled") && callback_allowed;
        let (action, payload) = if oled {
            ("SET_OLED_RESET", false)
        } else if callback_allowed {
            match page.reset_callback.as_ref() {
                Some(callback) => (
                    match callback.action.as_str() {
                        "ON_RESET_OBM" => "ON_RESET_OBM",
                        "ON_RESET_DEVICE" => "ON_RESET_DEVICE",
                        "ADD_PROFILE" => "ADD_PROFILE",
                        _ => return,
                    },
                    callback.payload,
                ),
                // Previously audited special Help variants retain their
                // explicit branch until their callback receipt is refreshed.
                None if page.obm => ("ON_RESET_OBM", true),
                None => ("ON_RESET_DEVICE", false),
            }
        } else {
            ("ON_RESET_DEVICE", false)
        };
        self.reset_source_action = action;
        self.reset_source_payload = payload;
        self.reset_source_payload_value = callback_allowed
            .then(|| {
                page.reset_callback
                    .as_ref()
                    .and_then(|callback| callback.payload_value.as_ref())
            })
            .flatten();
        self.reset_oled_obm = oled.then_some(true);
        let request = HelpResetRequest {
            generation: self.reset_generation,
            audio_streams: false,
            source_action: (!oled).then_some(self.reset_source_action),
            source_payload: self.reset_source_payload,
            source_payload_value: self.reset_source_payload_value,
            oled_reset_obm: self.reset_oled_obm,
            firmware_hardware_reset: page.firmware_reset.is_some(),
            oled_factory_reset: page.popup_kind.as_deref() == Some("audio-oled"),
            oled_source_clone_failure: page.popup_kind.as_deref() == Some("audio-oled")
                && callback_allowed,
        };
        self.reset_cooldown = true;
        let timeout_ms = if page.wait_for_obm_reset_completion {
            4000
        } else {
            page.cooldown_ms.unwrap_or(2000)
        };
        self.reset_cooldown_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(timeout_ms))
                .await;
            let _ = view.update(cx, |view, cx| {
                if view.reset_generation == request.generation {
                    view.reset_cooldown = false;
                    cx.notify();
                }
            });
        }));
        cx.emit(HelpResetEvent::Requested(request));
        cx.notify();
    }
}
