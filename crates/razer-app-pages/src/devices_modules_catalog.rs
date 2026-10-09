// Included by service_pages.rs: current 6505/44442 catalogue and H group tree.
#[derive(serde::Deserialize)]
struct SourceModuleCatalog {
    ids: Vec<String>,
    titles: BTreeMap<String, BTreeMap<String, String>>,
    descriptions: BTreeMap<String, Option<BTreeMap<String, String>>>,
    details: BTreeMap<String, SourceModuleDetail>,
}
#[derive(serde::Deserialize)]
struct SourceModuleDetail {
    size: u64,
}
fn source_module_catalog() -> &'static SourceModuleCatalog {
    static CATALOG: OnceLock<SourceModuleCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("devices_modules_catalog.json"))
            .expect("validated current 6505 module catalogue")
    })
}
fn localized_module_text(text: &BTreeMap<String, String>) -> String {
    let locale = i18n::locale().to_ascii_lowercase();
    text.get(&locale)
        .or_else(|| text.get("en"))
        .cloned()
        .unwrap_or_default()
}
impl Module {
    fn source_id(&self) -> &str {
        if self.id == "linked-games" {
            "linkedGames"
        } else {
            self.id
        }
    }
    fn title(&self) -> String {
        self.title_for(false, false)
    }
    fn title_for(&self, exchange: bool, beta: bool) -> String {
        let key = match self.source_id() {
            "armory" if exchange => "armory_exchange",
            "armory" => "armory_workshop",
            "feedback" if beta => "feedback_beta",
            key => key,
        };
        source_module_catalog()
            .titles
            .get(key)
            .map(localized_module_text)
            .unwrap_or_else(|| i18n::t(self.box_name))
    }
    fn icon_for(&self, exchange: bool) -> &'static str {
        if self.id == "armory" && exchange {
            "synapse/module-armory-exchange.svg"
        } else {
            self.icon
        }
    }
    fn description(&self) -> String {
        source_module_catalog()
            .descriptions
            .get(self.source_id())
            .and_then(Option::as_ref)
            .map(localized_module_text)
            .unwrap_or_default()
    }
    fn file_size(&self) -> String {
        let bytes = source_module_catalog()
            .details
            .get(self.source_id())
            .map_or(0, |detail| detail.size);
        if bytes == 0 {
            return "< 1 MB".into();
        }
        // Source u: Math.round ignores its extra precision argument.
        let index = ((bytes as f64).log(1024.).floor() as usize).min(4);
        format!(
            "{} {}",
            (bytes as f64 / 1024_f64.powi(index as i32)).round() as u64,
            ["Bytes", "KB", "MB", "GB", "TB"][index]
        )
    }
}

/// Shared w description subtree: both the preview and main route use the
/// current localized literals, static file size and source geometry.
fn module_description(item: &'static Module, cx: &App) -> AnyElement {
    h_flex()
        .items_start()
        .min_h(surface::css(202.))
        .p(surface::css(20.))
        .bg(MainPageColors.detail_surface())
        .when_some(item.image, |view, image| {
            view.child(
                img(image)
                    .w(surface::css(288.))
                    .h(surface::css(162.))
                    .flex_shrink_0()
                    .mr(surface::css(20.))
                    .object_fit(ObjectFit::Contain),
            )
        })
        .child(
            v_flex()
                .w(surface::css(592.))
                .flex_shrink_0()
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .text_color(rgb(0xcccccc))
                .child(
                    div()
                        .mb(surface::css(20.))
                        .whitespace_normal()
                        .child(item.description()),
                )
                .child(
                    h_flex()
                        .when_some(item.link, |view, url| {
                            view.child(razer_widgets::source_link::source_link(
                                "module-learn-more",
                                i18n::t("LEARN_MORE"),
                                url,
                                cx,
                            ))
                        })
                        .child(div().ml_auto().mr(surface::css(20.)).child(format!(
                            "{}: {}",
                            i18n::t("SIZE"),
                            item.file_size()
                        ))),
                ),
        )
        .into_any_element()
}
fn module_group_heading(key: &'static str, cx: &App) -> Div {
    div()
        .font_family("RazerF5")
        .text_size(surface::css(24.))
        .text_color(cx.theme().primary)
        .mb(surface::css(10.))
        .child(i18n::t(key).to_uppercase())
}
/// Current H returns null for an empty group. Preserve the four source groups
/// and their 40px spacing without adding empty headings or sample data.
fn module_group(key: &'static str, rows: Vec<AnyElement>, cx: &App) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    Some(
        v_flex()
            .id(key)
            .w(surface::css(1220.))
            .flex_shrink_0()
            .mx_auto()
            .mb(surface::css(40.))
            .child(module_group_heading(key, cx))
            .children(rows)
            .into_any_element(),
    )
}

/// 77989/i loads these flags together. Unknown retains the hook's initial
/// isExchangeEnabled=false, rather than inventing a completed flag response.
struct ObservedArmoryFeatures {
    profile_sharing: bool,
    macro_sharing: bool,
    chroma_sharing: bool,
    name_change_to_workshop: bool,
}
impl ObservedArmoryFeatures {
    fn exchange_enabled(&self) -> bool {
        (!self.profile_sharing && !self.macro_sharing && !self.chroma_sharing)
            || !self.name_change_to_workshop
    }
}
pub struct ModuleCatalog {
    expanded: BTreeSet<String>,
    service_snapshot: Option<ModuleServiceSnapshot>,
    service_groups: Option<service::ServiceGroups>,
    local_devices: Vec<Device>,
    local_page_devices: BTreeSet<(u32, String, String)>,
    removal: Option<String>,
    clear_settings: bool,
    removal_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    removal_task: Option<Task<()>>,
    armory_features: Option<ObservedArmoryFeatures>,
    host_is_beta: Option<bool>,
}
pub enum ModuleCatalogEvent {
    OpenModule(ModulePage),
    OpenDevice(String),
    ServiceCommand {
        action: &'static str,
        record: Record,
        clear_settings: bool,
    },
    FirmwareUpdate {
        device: Option<Device>,
        preview: bool,
    },
}
impl EventEmitter<ModuleCatalogEvent> for ModuleCatalog {}
impl ModuleCatalog {
    pub fn new(snapshot: Option<ModuleServiceSnapshot>, cx: &mut Context<Self>) -> Self {
        let groups = snapshot.as_ref().map(ModuleServiceSnapshot::groups);
        Self {
            expanded: BTreeSet::new(),
            service_snapshot: snapshot,
            service_groups: groups,
            local_devices: Vec::new(),
            local_page_devices: BTreeSet::new(),
            removal: None,
            clear_settings: false,
            removal_focus: cx.focus_handle(),
            return_focus: None,
            removal_task: None,
            armory_features: None,
            host_is_beta: None,
        }
    }
    pub fn service_snapshot(&self) -> Option<ModuleServiceSnapshot> {
        self.service_snapshot.clone()
    }
    pub fn sync_local_devices(
        &mut self,
        devices: &[Device],
        local_page_devices: BTreeSet<(u32, String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.local_devices = devices.to_vec();
        self.local_page_devices = local_page_devices;
        cx.notify();
    }
    /// Explicit UI samples: no installer transport or device mutation is involved.
    pub fn open_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        module_preview::open(cx.entity().downgrade(), window, cx);
    }
    fn module_row(&self, item: &'static Module, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.expanded.contains(item.id);
        let exchange = self
            .armory_features
            .as_ref()
            .is_some_and(ObservedArmoryFeatures::exchange_enabled);
        v_flex()
            .id(SharedString::from(format!("catalog-{}", item.id)))
            .test_support()
            .w_full()
            .child(
                h_flex()
                    .h(surface::css(80.))
                    .pl(surface::css(20.))
                    .pr(surface::css(30.))
                    .mb(surface::css(1.))
                    .bg(cx.theme().group_box)
                    .child(
                        img(item.icon_for(exchange))
                            .size(surface::css(40.))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .ml(surface::css(10.))
                            .w(surface::css(500.))
                            .flex_shrink_0()
                            .font_family("Roboto")
                            .text_size(surface::css(16.))
                            .text_color(rgb(0xcccccc))
                            .text_ellipsis()
                            .child(item.title_for(exchange, self.host_is_beta.unwrap_or(false))),
                    )
                    .child(div().flex_1().min_w_0().flex().items_center().when(
                        item.image.is_some(),
                        |view| {
                            view.child(
                                module_detail_action(
                                    SharedString::from(format!("module-details-{}", item.id)),
                                    i18n::t(if expanded {
                                        "CLOSE"
                                    } else {
                                        "MORE_INFORMATION"
                                    }),
                                    cx,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if !this.expanded.remove(item.id) {
                                            this.expanded.insert(item.id.into());
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                        },
                    ))
                    .when_some(item.native_page, |view, page| {
                        // Opening a compiled local page is not an installer receipt.
                        view.child(
                            div()
                                .id(SharedString::from(format!("module-open-wrap-{}", item.id)))
                                .test_support()
                                .child(
                                    module_action(
                                        SharedString::from(format!("module-open-{}", item.id)),
                                        i18n::t("TEXT_OPEN"),
                                        false,
                                        false,
                                        cx,
                                    )
                                    .ml(surface::css(30.))
                                    .on_click(cx.listener(
                                        move |_, _, _, cx| {
                                            cx.emit(ModuleCatalogEvent::OpenModule(page));
                                        },
                                    )),
                                ),
                        )
                    }),
            )
            .when(expanded && item.image.is_some(), |view| {
                view.child(module_description(item, cx))
            })
            .into_any_element()
    }
}
impl Render for ModuleCatalog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ne has five entries. Tour/profile migration remain Dashboard entries.
        let available = source_module_catalog()
            .ids
            .iter()
            .filter_map(|id| MODULES.iter().find(|item| item.source_id() == id.as_str()))
            .filter(|item| {
                !self
                    .service_snapshot
                    .as_ref()
                    .is_some_and(|s| s.installed_module(item.source_id()))
            })
            .map(|item| self.available_service_module(item, cx))
            .collect();
        let mut firmware = Vec::new();
        let mut new_devices = Vec::new();
        let mut recent = Vec::new();
        if let Some(groups) = &self.service_groups {
            firmware = groups
                .firmware
                .iter()
                .map(|row| self.firmware_service_row(row, cx))
                .collect();
            new_devices = groups
                .new_devices
                .iter()
                .map(|row| self.new_device_service_row(row, cx))
                .collect();
            // ae preserves this exact order; only normal devices sort connected-first.
            for removing in [true, false] {
                recent.extend(
                    groups
                        .installed_devices
                        .iter()
                        .filter(|r| (service::string(r, "status") == "uninstalling") == removing)
                        .map(|row| self.installed_service_row(row, None, window, cx)),
                );
                recent.extend(
                    groups
                        .installed_modules
                        .iter()
                        .filter(|r| (service::string(r, "status") == "uninstalling") == removing)
                        .filter_map(|row| {
                            let name = service::string(row, "moduleName");
                            MODULES
                                .iter()
                                .find(|m| {
                                    m.source_id() == name
                                        && source_module_catalog().ids.iter().any(|id| id == &name)
                                })
                                .map(|module| {
                                    // ae.f merges current ne/ie metadata after service data.
                                    let mut row = row.clone();
                                    row.insert(
                                        "title".into(),
                                        serde_json::Value::String(module.title_for(
                                            self.armory_features.as_ref().is_some_and(
                                                ObservedArmoryFeatures::exchange_enabled,
                                            ),
                                            self.host_is_beta.unwrap_or(false),
                                        )),
                                    );
                                    row.insert(
                                        "removable".into(),
                                        serde_json::Value::Bool(matches!(
                                            module.source_id(),
                                            "alexa" | "macro"
                                        )),
                                    );
                                    self.installed_service_row(&row, Some(module), window, cx)
                                })
                        }),
                );
            }
        }
        let groups = [
            module_group("FIRMWARE_UPDATES", firmware, cx),
            module_group("NEW_DEVICES", new_devices, cx),
            module_group("AVAILABLE_MODULES", available, cx),
            module_group("UPDATED_RECENTLY", recent, cx),
        ];
        v_flex()
            .id("devices-modules")
            .test_support()
            .w_full()
            .children(groups.into_iter().flatten())
    }
}
