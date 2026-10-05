// Current Dashboard 44442/O + z and 15597/A. All rows are service observations;
// confirmation only emits a command, never an uninstalling/success result.

fn service_clear_settings_key(module: &Module) -> Option<&'static str> {
    Some(match module.source_id() {
        "alexa" => "REMOVE_ALEXA_SETTINGS",
        "chroma-connect" => "REMOVE_CHROMA_CONNECT_SETTINGS",
        "chroma-studio" => "REMOVE_CHROMA_STUDIO_SETTINGS",
        "audio-visualizer" => "REMOVE_AUDIO_VISUALIZER_SETTINGS",
        "philips-hue" => "REMOVE_PHILIPS_HUE_SETTINGS",
        "macro" => "REMOVE_MACRO_SETTINGS",
        "nanoleaf" => "REMOVE_NANOLEAF_SETTINGS",
        _ => return None,
    })
}

impl ModuleCatalog {
    fn removal_is_current(&self, row: &Record, module: Option<&Module>) -> bool {
        if !service::flag(row, "removable") || service::string(row, "status") == "uninstalling" {
            return false;
        }
        let Some(snapshot) = &self.service_snapshot else {
            return false;
        };
        if module.is_some_and(|m| m.source_id() == "macro") && !snapshot.macro_removable() {
            return false;
        }
        let Some(groups) = &self.service_groups else {
            return false;
        };
        let key = service_row_key(row, module);
        let rows = if module.is_some() {
            &groups.installed_modules
        } else {
            &groups.installed_devices
        };
        rows.iter().any(|current| {
            let same = if let Some(module) = module {
                service::string(current, "moduleName") == module.source_id()
            } else {
                service_row_key(current, None) == key
            };
            same && service::string(current, "status") != "uninstalling"
                && (module.is_some() || service::flag(current, "removable"))
        })
    }

    pub(super) fn dismiss_service_removal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.removal_task = None;
        self.removal = None;
        self.clear_settings = false;
        let restore = self.removal_focus.contains_focused(window, cx);
        if let Some(focus) = self.return_focus.take()
            && restore
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn show_service_removal(
        &mut self,
        row: Record,
        module: Option<&'static Module>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.removal_is_current(&row, module) {
            return;
        }
        self.removal = None;
        self.clear_settings = false;
        self.return_focus = window.focused(cx);
        // O.h waits 100ms before opening so the trigger click cannot be the
        // document click-away that immediately dismisses the new confirmation.
        self.removal_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.removal_is_current(&row, module) {
                    this.removal = Some(service_row_key(&row, module));
                    this.removal_focus.focus(window, cx);
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn installed_service_row(
        &self,
        row: &Record,
        module: Option<&'static Module>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = service_row_key(row, module);
        let title = service_title(row, module);
        let status = service::string(row, "status");
        let disconnected = status == "disconnected";
        let removing = status == "uninstalling";
        let removable = self.removal_is_current(row, module);
        let record = row.clone();
        let action = if removing {
            h_flex()
                .ml(surface::css(30.))
                .flex_shrink_1()
                .text_color(rgb(0x707070))
                // 43468/A names this exact SVG; bytes equal the current cache.
                .child(
                    img("synapse/alexa-spinner.svg")
                        .size(surface::css(26.))
                        .mr(surface::css(5.)),
                )
                .child(i18n::t("TEXT_REMOVING"))
                // The trailing 61252/A animated dotted SVG is not cached. Do
                // not invent its graphic or any progress percentage.
                .into_any_element()
        } else {
            BaseButton::new(SharedString::from(format!("service-remove-{key}")))
                .accessibility_label(i18n::t("REMOVE"))
                .disabled(!removable)
                .p_0()
                .h_auto()
                .w_auto()
                .ml(surface::css(30.))
                .flex_shrink_1()
                .underline()
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(rgb(0x707070))
                .when(!removable, |button| button.opacity(0.3))
                .when(removable, |button| {
                    button
                        .hover(|s| s.text_color(rgb(0xc8323c)))
                        .active(|s| s.text_color(rgb(0xc8323c)))
                        .focus_visible(|s| s.text_color(rgb(0xc8323c)))
                })
                .child(i18n::t("REMOVE"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.show_service_removal(record.clone(), module, window, cx)
                }))
                .into_any_element()
        };
        let open = self.removal.as_deref() == Some(key.as_str()) && removable;
        let opacity = motion::Presence::new(
            (SharedString::from(key.clone()), "service-remove-presence"),
            open,
        )
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
        div()
            .id(SharedString::from(format!("service-installed-{key}")))
            .relative()
            .w_full()
            .child(
                service_row_body()
                    .child(
                        div()
                            .size(surface::css(40.))
                            .flex_shrink_0()
                            .when(disconnected, |v| v.opacity(0.3))
                            .child(service_icon(row, module, cx)),
                    )
                    .child(
                        service_name(
                            if disconnected {
                                format!("{title} ({}) ", i18n::t("DISCONNECTED"))
                            } else {
                                format!("{title}  ")
                            },
                            false,
                            cx,
                        )
                        .when(disconnected, |v| v.opacity(0.3)),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .when(!removing, |view| {
                                view.child(
                                    div()
                                        .flex_1()
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(17.))
                                        .text_color(rgb(0x707070))
                                        .child(format!(
                                            "{}: {}",
                                            i18n::t("LAST_UPDATE"),
                                            service::date_label(
                                                row.get("lastUpdated"),
                                                &i18n::locale(),
                                                false
                                            )
                                        )),
                                )
                            })
                            .when(removing, |view| view.justify_end())
                            .child(action),
                    ),
            )
            .when(open, |view| {
                view.child(self.service_removal_panel(row, module, opacity, window, cx))
            })
            .into_any_element()
    }

    fn service_removal_panel(
        &self,
        row: &Record,
        module: Option<&'static Module>,
        opacity: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = service_row_key(row, module);
        let title = service_title(row, module);
        let is_macro = module.is_some_and(|m| m.source_id() == "macro");
        let clear_key = module
            .and_then(service_clear_settings_key)
            .filter(|_| !is_macro);
        let mut body = v_flex()
            .w_full()
            .mb(surface::css(10.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc))
            .whitespace_normal();
        if is_macro {
            let names = self
                .service_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.extra("macroAssignments"))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| {
                            value.as_str().map(str::to_string).or_else(|| {
                                let name = service::localized(
                                    value.get("name"),
                                    &i18n::locale().to_ascii_lowercase(),
                                );
                                (!name.is_empty()).then_some(name)
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            body = body
                .text_left()
                .child(i18n::t("REMOVE_MODULE_MACRO_MAIN_DESC"))
                .when(!names.is_empty(), |view| {
                    view.child(div().pb(surface::css(20.)).child(format!(
                        "{}:",
                        i18n::t("REMOVE_MODULE_MACRO_DEVICE_ASSIGNMENTS_DESC")
                    )))
                })
                .children(names.into_iter().map(|name| {
                    h_flex()
                        .items_start()
                        .child(div().pr(surface::css(4.)).child("•"))
                        .child(div().flex_1().child(name))
                }))
                .child(
                    div()
                        .pt(surface::css(20.))
                        .child(i18n::t("ADD_MODULE_ANYTIME")),
                );
        } else {
            body = body.text_center().child(
                i18n::t(if module.is_some() {
                    "REMOVE_MODULE_MSG"
                } else {
                    "REMOVE_DEVICE_MSG"
                })
                .replace("{{title}}", &title),
            );
            if let Some(clear_key) = clear_key {
                body = body.child(
                    surface::check_item(
                        SharedString::from(format!("service-clear-settings-{key}")),
                        i18n::t(clear_key),
                        self.clear_settings,
                        false,
                        window,
                        cx,
                    )
                    .mt(surface::css(22.))
                    .min_w(surface::css(260.))
                    .text_left()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clear_settings = !this.clear_settings;
                        cx.notify();
                    })),
                );
            }
        }
        let record = row.clone();
        let panel = v_flex()
            .id(SharedString::from(format!("service-removal-confirm-{key}")))
            .test_support()
            .role(Role::Dialog)
            .aria_label(i18n::t(if module.is_some() {
                "REMOVE_MODULE_TITLE"
            } else {
                "REMOVE_DEVICE_TITLE"
            }))
            .track_focus(&self.removal_focus)
            .tab_group()
            .absolute()
            .right(surface::css(30.))
            .top(surface::css(51.))
            .w_auto()
            .min_w(surface::css(300.))
            .when(is_macro, |view| view.max_w(surface::css(300.)))
            .p(surface::css(20.))
            .items_center()
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd4949))
            .rounded(surface::css(3.))
            .opacity(opacity)
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .shadow(vec![BoxShadow {
                color: rgba(0x00000033).into(),
                offset: point(px(0.), window.rem_size() * (6. / 16.)),
                blur_radius: window.rem_size() * (10. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_mouse_down_out(
                cx.listener(|this, _, window, cx| this.dismiss_service_removal(window, cx)),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.dismiss_service_removal(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .mb(surface::css(10.))
                    .text_center()
                    .text_color(rgb(0xfd4949))
                    .child(
                        i18n::t(if module.is_some() {
                            "REMOVE_MODULE_TITLE"
                        } else {
                            "REMOVE_DEVICE_TITLE"
                        })
                        .to_uppercase(),
                    ),
            )
            .child(body)
            .child(
                BaseButton::new(SharedString::from(format!("service-remove-confirm-{key}")))
                    .accessibility_label(i18n::t("REMOVE"))
                    .w_auto()
                    .min_w(surface::css(90.))
                    .h(surface::css(27.))
                    .px(surface::css(5.))
                    .py(surface::css(4.))
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(rgba(0x0000004d))
                    .bg(rgb(0xfd4949))
                    .text_color(rgb(0x111111))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .text_center()
                    .whitespace_nowrap()
                    .hover(|view| view.opacity(0.8))
                    .active(|view| view.opacity(0.6))
                    .child(i18n::t("REMOVE").to_uppercase())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let valid = this.removal_is_current(&record, module);
                        let clear_settings = clear_key.is_some() && this.clear_settings;
                        this.dismiss_service_removal(window, cx);
                        if valid {
                            cx.emit(ModuleCatalogEvent::ServiceCommand {
                                action: "remove",
                                record: record.clone(),
                                clear_settings,
                            });
                        }
                    })),
            );
        deferred(panel).priority(3).into_any_element()
    }
}
