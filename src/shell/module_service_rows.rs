// Production 44442/w/L rows. No preview records or successful command responses.
#[derive(serde::Deserialize)]
struct ModuleServiceSource {
    categories: Vec<String>,
    subcategory_parents: Vec<String>,
}
fn module_service_source() -> &'static ModuleServiceSource {
    static SOURCE: OnceLock<ModuleServiceSource> = OnceLock::new();
    SOURCE.get_or_init(|| {
        serde_json::from_str(include_str!("module_service_source.json"))
            .expect("validated module service category source")
    })
}
fn service_row_key(row: &Record, module: Option<&Module>) -> String {
    module
        .map(|m| format!("module-{}", m.source_id()))
        .unwrap_or_else(|| format!("device-{}", service::identity(row)))
}
fn service_title(row: &Record, module: Option<&Module>) -> String {
    if row.get("title").is_some_and(|v| !v.is_null()) {
        service::localized(row.get("title"), &i18n::locale().to_ascii_lowercase())
    } else if let Some(module) = module {
        module.title()
    } else {
        service::localized(row.get("productName"), &i18n::locale().to_ascii_lowercase())
    }
}
fn service_icon(row: &Record, module: Option<&Module>, cx: &App) -> AnyElement {
    service_icon_with_warning(row, module, false, cx)
}
fn service_icon_with_warning(
    row: &Record,
    module: Option<&Module>,
    warning: bool,
    cx: &App,
) -> AnyElement {
    if let Some(module) = module {
        return img(module.icon)
            .size(surface::css(40.))
            .flex_shrink_0()
            .into_any_element();
    }
    let category = service::string(row, "category");
    let category = if category.is_empty() {
        service::string(row, "icon")
    } else {
        category
    };
    let sub = service::string(row, "subCategory");
    let source = module_service_source();
    let first = if source.subcategory_parents.contains(&category) && !sub.is_empty() {
        &sub
    } else {
        &category
    };
    let selected = if source.categories.contains(first) {
        Some(first)
    } else if !sub.is_empty() && source.categories.contains(&category) {
        Some(&category)
    } else {
        None
    };
    div()
        .size(surface::css(40.))
        .flex_shrink_0()
        .when_some(selected, |view, key| {
            view.child(
                svg()
                    .path(SharedString::from(format!(
                        "synapse/service-category-{}.svg",
                        key.to_ascii_lowercase()
                    )))
                    .size_full()
                    .text_color(if warning {
                        MainPageColors.tutorial_accent()
                    } else {
                        cx.theme().foreground
                    }),
            )
        })
        .into_any_element()
}
fn service_row_body() -> Div {
    h_flex()
        .h(surface::css(80.))
        .w_full()
        .pl(surface::css(20.))
        .pr(surface::css(30.))
        .mb(surface::css(1.))
        .bg(crate::ui::theme::DockPairingColors::card())
}
fn service_name(title: String, warning: bool, cx: &App) -> Div {
    div()
        .ml(surface::css(10.))
        .w(surface::css(500.))
        .flex_shrink_0()
        .font_family("Roboto")
        .text_size(surface::css(16.))
        .text_color(if warning {
            MainPageColors.tutorial_accent()
        } else {
            cx.theme().foreground
        })
        .text_ellipsis()
        .child(title)
}
fn service_external_link(
    id: String,
    label: String,
    url: String,
    firmware: bool,
    cx: &App,
) -> AnyElement {
    gpui_kit::base::Link::new(SharedString::from(id))
        .href(url)
        .accessibility_label(label.clone())
        .open_with(|url, _, _, cx| cx.open_url(url))
        .cursor_pointer()
        .text_size(surface::css(14.))
        .text_color(cx.theme().foreground)
        .underline()
        .hover(|v| v.text_color(cx.theme().primary))
        .focus_visible(|v| v.text_color(cx.theme().primary))
        .child(label)
        .when(firmware, |link| {
            // CSS decorates the inline text anchor, not the full right column.
            link.self_start().relative().child(
                img("synapse/external-link.svg")
                    .absolute()
                    .right(surface::css(-25.))
                    .top(surface::css(-2.))
                    .size(surface::css(20.)),
            )
        })
        .into_any_element()
}

/// 75551 changes only the painted width. Accessibility continues to report
/// the observed percentage, and this state never modifies an installer record.
#[derive(IntoElement)]
struct ServiceProgressBar {
    id: SharedString,
    percent: f32,
    label: String,
}

/// 44442/w mounts this sibling `.tip` inside `.item-tooltip`. Keeping it in
/// the row preserves the source anchor and 300ms fade; Component Tooltip's
/// delayed portal has different geometry, typography and visibility rules.
#[derive(IntoElement)]
struct ServiceOfflineAction {
    id: SharedString,
    button: ModuleAction,
    show_tip: bool,
}
impl RenderOnce for ServiceOfflineAction {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(
            (ElementId::from(self.id.clone()), "offline-hover"),
            cx,
            |_, _| false,
        );
        let visible = self.show_tip && *state.read(cx);
        let opacity = motion::Presence::new((self.id.clone(), "offline-opacity"), visible)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        div()
            .id(self.id)
            .relative()
            .flex_shrink_0()
            .on_hover(window.listener_for(&state, |hovered, next, _, cx| {
                *hovered = *next;
                cx.notify();
            }))
            .child(self.button)
            .when(self.show_tip, |view| {
                view.child(
                    div()
                        .absolute()
                        .right(surface::css(-160.))
                        .top(surface::css(32.))
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .bg(rgb(0x000000))
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .text_color(rgb(0xcccccc))
                        .whitespace_normal()
                        .opacity(opacity)
                        .when(!visible, |tip| tip.invisible())
                        .child(i18n::t("INTERNET_CONNECTION_REQUIRED")),
                )
            })
    }
}
struct ServiceProgressMotion {
    observed: f32,
    from: f32,
    started: std::time::Instant,
    stages: Vec<(f32, Duration)>,
}
impl ServiceProgressMotion {
    fn sample(&self, now: std::time::Instant) -> (f32, bool) {
        let mut elapsed = now.saturating_duration_since(self.started);
        let mut from = self.from;
        for &(target, duration) in &self.stages {
            if elapsed < duration {
                let phase = elapsed.as_secs_f32() / duration.as_secs_f32();
                return (from + (target - from) * phase, true);
            }
            elapsed = elapsed.saturating_sub(duration);
            from = target;
        }
        (from, false)
    }
}
impl RenderOnce for ServiceProgressBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let now = cx.background_executor().now();
        let target = self.percent.clamp(0., 100.);
        let reduced = cx.reduce_motion();
        let state = window.use_keyed_state(
            (ElementId::from(self.id.clone()), "source-width"),
            cx,
            |_, _| ServiceProgressMotion {
                observed: 0.,
                from: 0.,
                started: now,
                stages: Vec::new(),
            },
        );
        let (width, running) = state.update(cx, |state, _| {
            let (sampled, _) = state.sample(now);
            if reduced {
                state.observed = target;
                state.from = target;
                state.stages.clear();
                return (target, false);
            }
            if state.observed != target {
                let previous = state.observed;
                state.observed = target;
                state.from = sampled;
                state.started = now;
                state.stages.clear();
                if target < previous {
                    // Source transitionend chain: complete current phase,
                    // reset with data-transition=0, then show the next phase.
                    if previous < 100. {
                        state.stages.push((100., Duration::from_millis(300)));
                    }
                    state.stages.push((0., Duration::from_millis(1)));
                }
                state.stages.push((target, Duration::from_millis(300)));
            }
            state.sample(now)
        });
        if running {
            window.request_animation_frame();
        }
        gpui_kit::base::Progress::new(self.id)
            .value(self.percent)
            .accessibility_label(self.label)
            .child(
                gpui_kit::base::ProgressTrack::new()
                    .relative()
                    .w_full()
                    .h(surface::css(8.))
                    .rounded(surface::css(4.))
                    .overflow_hidden()
                    .bg(MainPageColors.service_progress_track())
                    .child(
                        gpui_kit::base::ProgressIndicator::new()
                            .absolute()
                            .left_0()
                            .top_0()
                            .h_full()
                            .w(relative(width / 100.))
                            .rounded(surface::css(15.))
                            .bg(MainPageColors.service_progress_fill()),
                    ),
            )
    }
}
impl ModuleCatalog {
    fn local_service_device(&self, row: &Record) -> Option<&Device> {
        let pid = service::number(row, "productId");
        let serial = service::string(row, "serialNumber");
        let container = service::string(row, "deviceContainerId");
        self.local_devices.iter().find(|device| {
            device.product_id == pid
                && crate::features::has_product_workspace(pid)
                && if !container.is_empty() {
                    device.device_container_id == container
                } else {
                    !serial.is_empty() && device.serial_number == serial
                }
        })
    }
    fn toggle_service_details(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.expanded.remove(&key) {
            self.expanded.insert(key);
        }
        cx.notify();
    }
    fn service_details_action(&self, key: String, cx: &mut Context<Self>) -> BaseButton {
        let expanded = self.expanded.contains(&key);
        module_detail_action(
            SharedString::from(format!("service-details-{key}")),
            i18n::t(if expanded {
                "CLOSE"
            } else {
                "MORE_INFORMATION"
            }),
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_service_details(key.clone(), cx)))
    }
    fn service_progress(&self, key: &str, row: &Record, _: &App) -> AnyElement {
        let Some(progress) = self.service_snapshot.as_ref().and_then(|s| s.progress(key)) else {
            return div().into_any_element();
        };
        let phase = service::string(progress, "phase");
        if phase == "error" {
            return div()
                // `.second-from-left.info-text` supplies the 14px text size.
                .text_size(surface::css(14.))
                .text_color(crate::ui::theme::ProfileAlertColors::new().danger())
                .child(i18n::t("INSTALLATION_FAILED"))
                .into_any_element();
        }
        if phase != "downloading" && phase != "installing" {
            return div().into_any_element();
        }
        let amount = service::number(
            progress,
            if phase == "downloading" {
                "downloadedPercent"
            } else {
                "installedFiles"
            },
        );
        let total = if phase == "downloading" {
            100
        } else {
            service::number(progress, "totalFiles")
        };
        let percent = if total == 0 {
            0.
        } else {
            (amount as f32 / total as f32 * 100.).round()
        };
        let label = if phase == "installing" {
            format!("{}...", i18n::t("INSTALLING"))
        } else if percent == 100. {
            format!("{}...", i18n::t("SAVING"))
        } else {
            let bytes = service::number(row, "size");
            let display = if bytes == 0 {
                String::new()
            } else {
                let ix = ((bytes as f64).log(1024.).floor() as usize).min(4);
                let scaled = (bytes as f64 / 1024_f64.powi(ix as i32)).round();
                format!(
                    "{}/{} {}",
                    (amount as f64 * scaled / 100.).round() as u64,
                    scaled as u64,
                    ["Bytes", "KB", "MB", "GB", "TB"][ix]
                )
            };
            format!("{} {display}", i18n::t("DOWNLOADING"))
        };
        v_flex()
            .pt(surface::css(20.))
            .w(surface::css(200.))
            .child(ServiceProgressBar {
                id: format!("service-progress-{key}").into(),
                percent,
                label: label.clone(),
            })
            .child(
                div()
                    .mt(surface::css(5.))
                    .text_size(surface::css(12.))
                    .text_color(MainPageColors.card_caption())
                    .child(label),
            )
            .into_any_element()
    }
    fn install_service_action(&self, row: &Record, cx: &mut Context<Self>) -> AnyElement {
        if let Some(device) = self.local_service_device(row) {
            let container = device.device_container_id.clone();
            return module_action(
                SharedString::from(format!("open-{}", service::identity(row))),
                i18n::t("TEXT_OPEN"),
                false,
                false,
                cx,
            )
            .ml(surface::css(30.))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(ModuleCatalogEvent::OpenDevice(container.clone()))
            }))
            .into_any_element();
        }
        let key = service::number(row, "productId").to_string();
        let status = self
            .service_snapshot
            .as_ref()
            .and_then(|s| s.progress(&key));
        let phase = status
            .map(|p| service::string(p, "phase"))
            .unwrap_or_default();
        let online = self
            .service_snapshot
            .as_ref()
            .is_some_and(ModuleServiceSnapshot::online);
        let (label, action, disabled, primary) = match phase.as_str() {
            "installing" => ("CANCEL", "cancel", true, false),
            "downloading" => (
                "CANCEL",
                "cancel",
                status.is_some_and(|p| service::number(p, "downloadedPercent") == 100),
                false,
            ),
            "error" => ("RETRY", "retry", !online, true),
            _ => ("INSTALL", "install", !online, true),
        };
        let record = row.clone();
        let button = module_action(
            SharedString::from(format!("install-{}", service::identity(row))),
            i18n::t(label),
            primary,
            disabled,
            cx,
        )
        .ml(surface::css(30.))
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.emit(ModuleCatalogEvent::ServiceCommand {
                action,
                record: record.clone(),
                clear_settings: false,
            })
        }));
        ServiceOfflineAction {
            id: format!("install-tooltip-{}", service::identity(row)).into(),
            button,
            // Only default/error branches mount a hovered `.no-internet`.
            // Downloading/installing and the canceled offline branch do not.
            show_tip: !online
                && !matches!(phase.as_str(), "downloading" | "installing" | "canceled"),
        }
        .into_any_element()
    }
    fn available_service_module(
        &self,
        module: &'static Module,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let progress = self
            .service_snapshot
            .as_ref()
            .and_then(|s| s.progress(module.source_id()));
        if progress.is_none() {
            return self.module_row(module, cx);
        }
        let key = service_row_key(&Record::new(), Some(module));
        let record = Record::from_iter([("size".into(), serde_json::Value::from(0))]);
        let page = module.native_page;
        v_flex()
            .id(SharedString::from(key.clone()))
            .w_full()
            .child(
                service_row_body()
                    .child(service_icon(&record, Some(module), cx))
                    .child(service_name(module.title(), false, cx))
                    .child(div().flex_1().when(module.image.is_some(), |v| {
                        v.child(self.service_details_action(key.clone(), cx))
                    }))
                    .child(self.service_progress(module.source_id(), &record, cx))
                    .when_some(page, |v, page| {
                        v.child(
                            module_action(
                                SharedString::from(format!("open-{key}")),
                                i18n::t("TEXT_OPEN"),
                                false,
                                false,
                                cx,
                            )
                            .ml(surface::css(30.))
                            .on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(ModuleCatalogEvent::OpenModule(page)),
                            )),
                        )
                    }),
            )
            .when(
                self.expanded.contains(&key) && module.image.is_some(),
                |v| v.child(module_description(module, cx)),
            )
            .into_any_element()
    }
    fn new_device_service_row(&self, row: &Record, cx: &mut Context<Self>) -> AnyElement {
        let key = service_row_key(row, None);
        let details = row.get("detail").and_then(serde_json::Value::as_object);
        v_flex()
            .id(SharedString::from(format!("new-{key}")))
            .w_full()
            .child(
                service_row_body()
                    .child(service_icon(row, None, cx))
                    .child(service_name(service_title(row, None), false, cx))
                    .child(div().flex_1().when(details.is_some(), |v| {
                        v.child(self.service_details_action(key.clone(), cx))
                    }))
                    .child(self.service_progress(
                        &service::number(row, "productId").to_string(),
                        row,
                        cx,
                    ))
                    .child(self.install_service_action(row, cx)),
            )
            .when(self.expanded.contains(&key), |view| {
                view.when_some(details, |view, details| {
                    view.child(self.device_service_description(row, details, cx))
                })
            })
            .into_any_element()
    }
    fn device_service_description(&self, row: &Record, details: &Record, cx: &App) -> AnyElement {
        let locale = i18n::locale().to_ascii_lowercase();
        // w reads detail.srcImage, not the ae.v dashboard thumbnail. There is
        // no verified detail-srcImage-to-embedded-asset mapping yet; preserve
        // its exact image slot instead of showing unrelated product artwork.
        h_flex()
            .items_start()
            .min_h(surface::css(202.))
            .p(surface::css(20.))
            .bg(MainPageColors.detail_surface())
            .child(
                div()
                    .w(surface::css(288.))
                    .h(surface::css(162.))
                    .mr(surface::css(20.))
                    .flex_shrink_0(),
            )
            .child(
                v_flex()
                    .w(surface::css(592.))
                    .text_size(surface::css(14.))
                    .child(
                        gpui_kit::base::TextView::html(
                            SharedString::from(format!("description-{}", service::identity(row))),
                            service::localized(details.get("description"), &locale),
                        )
                        .mb(surface::css(20.)),
                    )
                    .child(
                        h_flex()
                            .when(!service::string(details, "learnMoreURL").is_empty(), |v| {
                                v.child(service_external_link(
                                    format!("learn-{}", service::identity(row)),
                                    i18n::t("LEARN_MORE"),
                                    service::string(details, "learnMoreURL"),
                                    false,
                                    cx,
                                ))
                            })
                            .child(div().ml_auto().mr(surface::css(20.)).child(format!(
                                "{}: {}",
                                i18n::t("SIZE"),
                                service::file_size(service::number(details, "size") as u64)
                            )))
                            .when(service::flag(details, "restartRequired"), |view| {
                                view.child(i18n::t("RESTART_SYNAPSE_REQUIRED"))
                            }),
                    ),
            )
            .into_any_element()
    }
    fn firmware_service_row(&self, row: &Record, cx: &mut Context<Self>) -> AnyElement {
        let key = format!("firmware-{}", service::identity(row));
        // L prefers productName; ordinary w/O rows use the projected title.
        let title = match row.get("productName") {
            Some(serde_json::Value::Object(_)) => {
                service::localized(row.get("productName"), &i18n::locale().to_ascii_lowercase())
            }
            Some(serde_json::Value::String(value)) if !value.is_empty() => value.clone(),
            _ => service_title(row, None),
        };
        let release = service::firmware_release(row);
        let warning = release.is_some_and(|r| service::string(r, "severity") == "warning");
        let (enabled, warning_text) = service::firmware_entry(row);
        let title_display = if warning {
            format!(
                "{title} ({})",
                i18n::t("FIRMWARE_UPDATE_REQUIRED")
                    .to_lowercase()
                    .split(' ')
                    .map(|part| {
                        let mut letters = part.chars();
                        letters
                            .next()
                            .map(|c| c.to_uppercase().collect::<String>() + letters.as_str())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        } else {
            title.clone()
        };
        let record = row.clone();
        let local = self.local_service_device(row).cloned();
        v_flex()
            .id(SharedString::from(key.clone()))
            .w_full()
            .child(
                service_row_body()
                    .child(service_icon_with_warning(row, None, warning, cx))
                    .child(service_name(title_display, warning, cx))
                    .child(
                        div()
                            .flex_1()
                            .child(self.service_details_action(key.clone(), cx)),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .justify_end()
                            .when_some(warning_text, |v, key| {
                                v.child(
                                    h_flex()
                                        .max_w(surface::css(314.))
                                        .max_h(surface::css(56.))
                                        .child(
                                            img("synapse/service-firmware-warning.svg")
                                                .w(surface::css(20.))
                                                .h(surface::css(27.))
                                                .mr(surface::css(10.)),
                                        )
                                        .child(
                                            div()
                                                .text_size(surface::css(14.))
                                                .text_color(MainPageColors.card_caption())
                                                .whitespace_normal()
                                                .child(i18n::t(key)),
                                        ),
                                )
                            }),
                    )
                    .child(
                        module_action(
                            SharedString::from(format!("launch-{key}")),
                            i18n::t("LAUNCH_UPDATER"),
                            false,
                            !enabled,
                            cx,
                        )
                        .ml(surface::css(30.))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            if service::string(&record, "upgradeMode") == "SDK" {
                                cx.emit(ModuleCatalogEvent::FirmwareUpdate {
                                    device: local.clone(),
                                    preview: false,
                                });
                            } else if let Some(url) = record
                                .get("firmwareUpdateInfo")
                                .and_then(|v| v.get("guide"))
                                .and_then(serde_json::Value::as_str)
                                .filter(|s| !s.is_empty())
                            {
                                cx.open_url(url);
                            }
                        })),
                    ),
            )
            .when(self.expanded.contains(&key), |v| {
                v.child(self.firmware_service_description(row, &key, &title, cx))
            })
            .into_any_element()
    }
    fn firmware_service_description(
        &self,
        row: &Record,
        key: &str,
        title: &str,
        cx: &App,
    ) -> AnyElement {
        let locale = i18n::locale().to_ascii_lowercase();
        let release = service::firmware_release(row);
        let version = release
            .map(|r| service::string(r, "targetFWVersion"))
            .unwrap_or_default();
        let date = service::date_label(release.and_then(|r| r.get("releaseDate")), &locale, true);
        let size = service::file_size(release.map_or(0, |r| service::number(r, "size")) as u64);
        let url = row
            .get("firmwareUpdateInfo")
            .and_then(|v| v.get("guide"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let notes = release
            .and_then(|r| r.get("releaseNotes"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .map(|v| service::localized(Some(v), &locale).trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        h_flex()
            .items_start()
            .p(surface::css(20.))
            .bg(MainPageColors.detail_surface())
            .text_size(surface::css(14.))
            .child(
                v_flex()
                    .w(surface::css(200.))
                    .flex_shrink_0()
                    .gap(surface::css(10.))
                    .child(i18n::t("VERSION_NUMBER").replace("{{version}}", &version))
                    .child(format!("{}: {date}", i18n::t("RELEASED")))
                    .when(service::string(row, "upgradeMode") != "SDK", |v| {
                        v.child(format!("{}: {size}", i18n::t("SIZE")))
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .when(!url.is_empty(), |v| {
                        v.child(service_external_link(
                            format!("guide-{key}"),
                            i18n::t(if service::string(row, "icon") == "SYSTEM" {
                                "BIOS_UPDATE_GUIDE"
                            } else {
                                "FIRMWARE_UPDATE_GUIDE"
                            })
                            .replace("{{deviceName}}", title)
                            .replace("{{version}}", &version),
                            url.into(),
                            true,
                            cx,
                        ))
                    })
                    .child(
                        v_flex()
                            .pl(surface::css(15.))
                            .my(surface::css(14.))
                            .children(
                                notes.into_iter().map(|text| {
                                    div().whitespace_normal().child(format!("• {text}"))
                                }),
                            ),
                    ),
            )
            .into_any_element()
    }
}
