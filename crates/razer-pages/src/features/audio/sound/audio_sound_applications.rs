//! Current SP -> xL application output and installed-program selection.
//! Service queries and original source calls never invent application records.
use super::sound::{Product, js_truthy, message, tick};
use super::*;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    spinner::Spinner,
};
use razer_widgets::{scroll::SourceScrollable as _, theme::AudioEnhancementColors as Colors};

impl AudioProductWorkspace {
    fn source_app_outputs(&self, row: &Product) -> Vec<Value> {
        let mut outputs = row
            .configs
            .application_outputs
            .iter()
            .map(|output| json!(output))
            .collect::<Vec<_>>();
        let enabled = self.draft["sound"]["cloudEQ"]["enabledPresets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !enabled.is_empty() || self.sound.apps_cloud_observed {
            let names = enabled
                .iter()
                .filter(|p| p["cloudEqualizerId"].as_u64().is_some_and(|id| id < 4))
                .filter_map(|p| p["name"]["en"].as_str())
                .map(str::to_lowercase)
                .collect::<std::collections::BTreeSet<_>>();
            outputs.retain(|output| {
                let name = output["name"].as_str().unwrap_or_default();
                name == "FOLLOW_EXISTING_AUDIO_EQ"
                    || name == "divider"
                    || (name == "GAME_PROFILE_DEFAULT" && names.contains("default"))
                    || names.contains(&name.to_lowercase())
            });
        }
        outputs
    }
    fn sound_app_thx_support<'a>(
        &self,
        app: &Value,
        row: &'a Product,
    ) -> Option<&'a super::sound::fixed::SoundPayloadProductsitemConfigsApplicationThxDatasetentry>
    {
        let dataset = &row.configs.application_thx_dataset;
        let names = if let Some(names) = app["exeName"].as_array() {
            names.iter().filter_map(Value::as_str).collect::<Vec<_>>()
        } else {
            app["exeName"].as_str().into_iter().collect()
        };
        names
            .into_iter()
            .find_map(|name| dataset.get(&name.to_lowercase()))
    }
    fn sound_app_outputs(&self, app: &Value, row: &Product) -> Vec<Value> {
        let mut outputs = self.source_app_outputs(row);
        if let Some(support) = self.sound_app_thx_support(app, row) {
            let immersive = support.imm.is_some();
            let competitive = support.cmm.is_some();
            if immersive || competitive {
                outputs.push(json!({"name":"divider"}));
            }
            if immersive {
                outputs
                    .push(json!({"name":"IMM_MODE","guid":"THX Immersive Mode","type":"thx_env"}));
            }
            if competitive {
                outputs.push(
                    json!({"name":"COM_MODE","guid":"THX Competitive Mode","type":"thx_comp"}),
                );
            }
        }
        outputs
    }
    fn query_sound_apps(&self, cx: &mut Context<Self>) {
        self.emit_sound(vec![json!({"sourceCall":{"method":"getInstalledApplicationList","args":[]},"responseChain":["JSON.parse","getApplicationIcons(JSON.stringify(paths),80,80)","setInstalledApplicationList"]})],cx);
    }
    fn refresh_sound_apps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sound.apps_refresh = true;
        self.sound.apps_searching = false;
        self.sound.apps_refresh_generation = self.sound.apps_refresh_generation.wrapping_add(1);
        self.sound.apps_refresh_task = None;
        if let Some(search) = &self.sound.apps_search {
            search.update(cx, |state, cx| state.set_value("", window, cx));
        }
        self.query_sound_apps(cx);
        cx.notify();
    }
    fn open_sound_apps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sound.apps_shown = true;
        if self.sound.apps_search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx));
            self.subscriptions
                .push(cx.subscribe(&input, |_, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                }));
            self.sound.apps_search = Some(input);
        }
        self.query_sound_apps(cx);
        cx.notify();
    }
    fn close_sound_apps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sound.apps_shown = false;
        self.sound.apps_searching = false;
        if let Some(search) = &self.sound.apps_search {
            search.update(cx, |state, cx| state.set_value("", window, cx));
        }
        cx.notify();
    }
    fn add_sound_app(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.emit_sound(vec![json!({"sourceCall":{"method":"audioAddApplication","args":[path]}}),
            json!({"sourceCall":{"method":"onSendProfileChangeLinkedGamesEvent","args":[path]}}),
            json!({"sourceCall":{"method":"addNewApplication","args":[path]}})],cx);
        self.close_sound_apps(window, cx);
    }
    fn set_sound_app_output(&mut self, app: Value, output: Value, cx: &mut Context<Self>) {
        let kind = output["type"].as_str().unwrap_or_default();
        let output = json!({"type":kind,"guid":if matches!(kind,"imm"|"cmm"){output["env_name"].clone()}else{output["guid"].clone()}});
        if app.get("appType").is_some_and(js_truthy) {
            let payload = json!({"appType":app["appType"],"output":output});
            self.emit_sound_action("updateGeneralApplicationOutput", payload.clone(), cx);
            self.emit_sound(vec![json!({"transport":{"type":"ON_SET_GENERAL_SPATIAL_OUTPUT","timerTick":tick(),"payload":{"changedSetting":payload}}})],cx);
        } else {
            let payload = json!({"gameId":app["gameId"],"name":app["name"],"exeName":app["exeName"],"output":output});
            let mut settings = self.draft["sound"]["specificInputSetting"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let existing = settings.iter().position(|value| {
                value["gameId"] == app["gameId"] && value["exeName"] == app["exeName"]
            });
            if let Some(index) = existing {
                settings[index] = payload.clone();
            } else {
                settings.push(payload.clone());
            }
            self.draft["sound"]["specificInputSetting"] = json!(settings);
            self.emit_sound_action("updateSpecificApplicationOutput", payload.clone(), cx);
            self.emit_sound(vec![json!({"transport":{"type":"UPDATE_SPECIFIC_APPLICATION_OUTPUT","timerTick":tick(),"payload":{"applicationPreset":payload,"isNewSetting":existing.is_none()}}})],cx);
        }
        self.sound.apps_dropdown = None;
        self.capture_sound_device();
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn request_remove_sound_app(
        &mut self,
        app: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sound.apps_remove = Some(app);
        self.sound.apps_remove_shown = false;
        self.sound.apps_remove_generation = self.sound.apps_remove_generation.wrapping_add(1);
        let generation = self.sound.apps_remove_generation;
        self.sound.apps_remove_task = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let _ = owner.update_in(cx, |this, _, cx| {
                if this.sound.apps_remove_generation == generation
                    && this.sound.apps_remove.is_some()
                {
                    this.sound.apps_remove_shown = true;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }
    fn confirm_remove_sound_app(&mut self, cx: &mut Context<Self>) {
        let Some(app) = self.sound.apps_remove.take() else {
            return;
        };
        self.sound.apps_remove_shown = false;
        self.emit_sound_action("addRemoveAudioApplication", app, cx);
        // Original linkedGames reducer owns removedApp/storage/native writes.
        // The actual application observation is required to refresh this row.
        cx.notify();
    }
    fn current_sound_app_row(
        &self,
        app: &Value,
        index: usize,
        outputs: &[Value],
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        let open = self.sound.apps_dropdown.as_ref() == Some(app);
        let removing = app.clone();
        let opening = app.clone();
        let specific = self.draft["sound"]["specificInputSetting"]
            .as_array()
            .and_then(|rows| {
                rows.iter().find(|setting| {
                    setting["gameId"] == app["gameId"] && setting["exeName"] == app["exeName"]
                })
            });
        let selected = specific
            .and_then(|setting| {
                outputs
                    .iter()
                    .find(|output| output["guid"] == setting["output"]["guid"])
            })
            .or_else(|| outputs.first());
        let icon = app["icon"]
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| row.media.placeholder_game_icon.as_str())
            .to_string();
        let name = app["name"].as_str().unwrap_or_default();
        let utf16 = name.encode_utf16().collect::<Vec<_>>();
        let name = if js_truthy(&app["shouldTranslate"]) {
            message(row, name)
        } else if utf16.len() > 27 {
            format!("{}...", String::from_utf16_lossy(&utf16[..27]))
        } else {
            name.to_string()
        };
        let hover_name = self.sound.apps_hover_name == Some(index);
        let hover_icon = self.sound.apps_hover_icon == Some(index);
        let thx_supported = self.sound_app_thx_support(app, row).is_some();
        v_flex()
            .relative()
            .child(
                h_flex()
                    .h(surface::css(40.))
                    .gap(surface::css(10.))
                    .child(
                        razer_widgets::keyboard_navigation::Button::new(SharedString::from(
                            format!("current-app-remove-{index}"),
                        ))
                        .size(surface::css(30.))
                        .p_0()
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.sound.apps_hover_icon = hovered.then_some(index);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .relative()
                                .size_full()
                                .child(img(icon).size_full().object_fit(ObjectFit::Contain))
                                .when(hover_name || hover_icon, |body| {
                                    body.child(
                                        img(if hover_name {
                                            row.media.icon_close_enclosed_small.clone()
                                        } else {
                                            row.media.icon_close_enclosed_a_small.clone()
                                        })
                                        .absolute()
                                        .inset_0()
                                        .size_full(),
                                    )
                                }),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.request_remove_sound_app(removing.clone(), window, cx)
                            },
                        )),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("current-app-name-{index}")))
                            .flex_1()
                            .child(name)
                            .when(thx_supported, |body| {
                                body.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "current-app-thx-supported-{index}"
                                        )))
                                        .child(
                                            img(row.media.appmix_thx.clone())
                                                .size(surface::css(16.)),
                                        )
                                        .on_hover(cx.listener(
                                            move |this, hovered: &bool, _, cx| {
                                                this.sound.apps_thx_hover =
                                                    hovered.then_some(index);
                                                cx.notify();
                                            },
                                        )),
                                )
                            })
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                this.sound.apps_hover_name = hovered.then_some(index);
                                cx.notify();
                            })),
                    )
                    .child(
                        razer_widgets::keyboard_navigation::Button::new(SharedString::from(
                            format!("current-app-output-{index}"),
                        ))
                        .w(surface::css(180.))
                        .disabled(js_truthy(&app["disabled"]))
                        .child(
                            selected
                                .and_then(|output| output["name"].as_str())
                                .map(|name| message(row, name))
                                .unwrap_or_default(),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.sound.apps_dropdown = Some(opening.clone());
                            cx.notify();
                        })),
                    ),
            )
            .when(
                thx_supported && self.sound.apps_thx_hover == Some(index),
                |body| {
                    body.child(
                        div()
                            .absolute()
                            .left(surface::css(50.))
                            .top(surface::css(35.))
                            .max_w(surface::css(270.))
                            .p(surface::css(10.))
                            .bg(rgb(0x000000))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .text_color(rgb(0xcccccc))
                            .text_size(surface::css(14.))
                            .child(message(row, "THX_GAME_MODE")),
                    )
                },
            )
            .when(open, |body| {
                body.child(
                    v_flex()
                        .border_1()
                        .border_color(Colors::border())
                        .bg(Colors::background())
                        .children(outputs.iter().enumerate().map(|(option_index, output)| {
                            let selecting = app.clone();
                            let option = output.clone();
                            razer_widgets::keyboard_navigation::Button::new(SharedString::from(
                                format!("current-app-option-{index}-{option_index}"),
                            ))
                            .disabled(output["name"] == json!("divider"))
                            .child(message(row, output["name"].as_str().unwrap_or_default()))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_sound_app_output(selecting.clone(), option.clone(), cx)
                                },
                            ))
                        })),
                )
            })
            .into_any_element()
    }
    pub(super) fn current_sound_applications(
        &self,
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut apps = self.draft["sound"]["applicationList"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        apps.retain(|app| !js_truthy(&app["isMissing"]));
        apps.sort_by_key(|app| app["name"].as_str().unwrap_or_default().to_uppercase());
        surface::panel_with_help(
            message(row, "GAME_PROFILES"),
            surface::help_control(
                "current-game-profile-tip",
                message(row, "GAME_PROFILES_TIPS"),
            ),
            cx,
        )
        .child(div().child(message(row, "GAME_PROFILES_DESC")))
        .child(
            gpui_kit::base::Link::new("current-thx-game-list")
                .href("https://www.razer.com/thx-spatial-audio/game-list")
                .open_with(|url, _, _, cx| cx.open_url(url))
                .text_color(Colors::text())
                .child(message(row, "THX_GAME_PROFILES")),
        )
        .child(
            h_flex()
                .mt(surface::css(20.))
                .mb(surface::css(10.))
                .child(div().flex_1().child(message(row, "GAMES_PROGRAMS")))
                .child(
                    div()
                        .w(surface::css(180.))
                        .child(message(row, "AUDIO_OUTPUT")),
                ),
        )
        .child(
            v_flex()
                .id("current-audio-app-list")
                .max_h(surface::css(340.))
                .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                    this.sound.apps_dropdown = None;
                    this.sound.apps_remove_shown = false;
                    this.sound.apps_remove_generation =
                        this.sound.apps_remove_generation.wrapping_add(1);
                    cx.notify();
                }))
                .scrollable_y()
                .children(apps.iter().enumerate().map(|(index, app)| {
                    let outputs = self.sound_app_outputs(app, row);
                    self.current_sound_app_row(app, index, &outputs, row, cx)
                }))
                .child(
                    razer_widgets::keyboard_navigation::Button::new("current-audio-add-app")
                        .h(surface::css(48.))
                        .p_0()
                        .child(img(row.media.icon_add.clone()).size(surface::css(30.)))
                        .child(message(row, "ADD_GAME_AND_PROGRAM"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_sound_apps(window, cx)),
                        ),
                ),
        )
        .when(self.sound.apps_remove_shown, |body| {
            body.child(
                v_flex()
                    .w(surface::css(300.))
                    .p(surface::css(15.))
                    .bg(Colors::background())
                    .border_1()
                    .border_color(Colors::border())
                    .child(
                        self.sound
                            .apps_remove
                            .as_ref()
                            .and_then(|app| app["name"].as_str())
                            .unwrap_or_default()
                            .to_string(),
                    )
                    .child(message(row, "REMOVE_GAME_MESSAGE"))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                razer_widgets::keyboard_navigation::Button::new(
                                    "current-app-remove-cancel",
                                )
                                .child(message(row, "CANCEL"))
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.sound.apps_remove_shown = false;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                razer_widgets::keyboard_navigation::Button::new(
                                    "current-app-remove-confirm",
                                )
                                .child(message(row, "CONFIRM"))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.confirm_remove_sound_app(cx)),
                                ),
                            ),
                    ),
            )
        })
        .into_any_element()
    }
    fn current_sound_apps_search(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        if !self.sound.apps_searching {
            return razer_widgets::keyboard_navigation::Button::new(
                "current-installed-apps-search",
            )
            .size(surface::css(26.))
            .p_0()
            .active(|style| style.opacity(0.7))
            .child(
                img(row
                    .media
                    .css
                    .get(if self.sound.apps_search_hovered {
                        "icon_search.green"
                    } else {
                        "icon_search"
                    })
                    .cloned()
                    .unwrap_or_default())
                .size(surface::css(20.)),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.sound.apps_search_hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, window, cx| {
                this.sound.apps_searching = true;
                if let Some(input) = &this.sound.apps_search {
                    input.update(cx, |state, cx| state.focus(window, cx));
                }
                cx.notify();
            }))
            .into_any_element();
        }
        let text = self
            .sound
            .apps_search
            .as_ref()
            .map(|input| input.read(cx).value())
            .unwrap_or_default();
        div()
            .id("current-installed-app-search-wrapper")
            .relative()
            .w(surface::css(133.))
            .h(surface::css(26.))
            .when_some(self.sound.apps_search.as_ref(), |body, input| {
                body.child(razer_widgets::keyboard_navigation::focus_input(
                    Input::new(input).w_full(),
                    input,
                    cx,
                ))
            })
            .child(
                img(row
                    .media
                    .css
                    .get("icon_search.grey")
                    .cloned()
                    .unwrap_or_default())
                .absolute()
                .left(surface::css(3.))
                .top(surface::css(2.))
                .size(surface::css(20.)),
            )
            .when(!text.is_empty(), |body| {
                body.child(
                    razer_widgets::keyboard_navigation::Button::new(
                        "current-installed-apps-clear-search",
                    )
                    .absolute()
                    .right_0()
                    .top_0()
                    .p_0()
                    .size(surface::css(25.))
                    .child(
                        img(row
                            .media
                            .css
                            .get(if self.sound.apps_clear_hovered {
                                "icon_close_enclosed_a"
                            } else {
                                "icon_close_enclosed"
                            })
                            .cloned()
                            .unwrap_or_default())
                        .size(surface::css(20.)),
                    )
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.sound.apps_clear_hovered = *hovered;
                        cx.notify();
                    }))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(input) = &this.sound.apps_search {
                            input.update(cx, |state, cx| state.set_value("", window, cx));
                        }
                    })),
                )
            })
            .into_any_element()
    }
    pub(super) fn current_sound_apps_modal(
        &self,
        row: &Product,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let search = self
            .sound
            .apps_search
            .as_ref()
            .map(|state| state.read(cx).value().to_lowercase().trim().to_string())
            .unwrap_or_default();
        let mut apps = self.draft["sound"]["installedAppList"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        apps.retain(|app| {
            app["name"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&search)
        });
        apps.sort_by_key(|app| app["name"].as_str().unwrap_or_default().to_uppercase());
        razer_assets::deferred_resource(div().absolute().inset_0().flex().items_center().justify_center().bg(rgba(0x000000b3)).child(
            v_flex().w(surface::css(950.)).max_h(window.viewport_size().height*0.98).bg(rgb(0x222222)).p(surface::css(20.)).border_1().border_color(Colors::border())
                .child(h_flex().justify_between().child(message(row,"ADD_GAME_TITLE"))
                    .child(razer_widgets::keyboard_navigation::Button::new("current-installed-apps-close").child("×").on_click(cx.listener(|this,_,window,cx|this.close_sound_apps(window,cx)))))
                .child(h_flex().gap(surface::css(10.))
                    .child(razer_widgets::keyboard_navigation::Button::new("current-installed-apps-refresh").size(surface::css(26.)).p_0()
                        .active(|style| style.opacity(0.7))
                        .when(self.sound.apps_refresh, |body| body.child(Spinner::new().small()))
                        .when(!self.sound.apps_refresh, |body| body.child(img(row.media.css.get(if self.sound.apps_refresh_hovered {"icon_refresh.green"} else {"icon_refresh"}).cloned().unwrap_or_default()).size(surface::css(20.))))
                        .on_hover(cx.listener(|this,hovered,_,cx| {this.sound.apps_refresh_hovered=*hovered;cx.notify();}))
                        .on_click(cx.listener(|this,_,window,cx|this.refresh_sound_apps(window,cx))))
                    .child(self.current_sound_apps_search(row,cx))
                    .child(message(row,"STILL_DONT_SEE_YOUR_GAME"))
                    .child(razer_widgets::keyboard_navigation::Button::new("current-installed-apps-browse").child(message(row,"BROWSE"))
                        .on_click(cx.listener(|this,_,_,cx|this.emit_sound(vec![json!({"sourceCall":{"method":"showFileOpenDialog","args":["Open",true,"[\".exe\"]"]},"responseChain":"JSON.parse then addApp for each selected path"})],cx)))))
                .child(v_flex().id("current-installed-apps-list").scrollable_y().max_h(surface::css(500.))
                    .children(apps.iter().enumerate().map(|(index,app)|{let path=app["path"].as_str().unwrap_or_default().to_string();
                        razer_widgets::keyboard_navigation::Button::new(SharedString::from(format!("current-installed-app-{index}"))).w_full().h(surface::css(80.)).p(surface::css(10.))
                            .when_some(app["logo"].as_str().filter(|logo| !logo.is_empty()), |body, logo| body.child(img(logo.to_string()).size(surface::css(80.)).object_fit(ObjectFit::Contain)))
                            .child(v_flex().child(app["name"].as_str().unwrap_or_default().to_string()).child(path.clone()))
                            .on_click(cx.listener(move|this,_,window,cx|this.add_sound_app(path.clone(),window,cx)))
                    })))
        )).with_priority(1100).into_any_element()
    }
}
