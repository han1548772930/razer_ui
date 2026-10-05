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
                            view.child(source_link(
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

/// Presentation slots for rows supplied by service adapters. None is unobserved;
/// Some(empty) is observed empty. Snapshot devices lack installedDate,
/// needsUpgrade and installer receipts, so they cannot populate these groups.
/// ModulePreview remains a separate owner of explicitly labelled sample data.
#[derive(Default)]
struct ObservedModuleServiceRows {
    firmware_updates: Option<Vec<AnyView>>,
    new_devices: Option<Vec<AnyView>>,
    updated_recently: Option<Vec<AnyView>>,
}
impl ObservedModuleServiceRows {
    fn elements(rows: &Option<Vec<AnyView>>) -> Vec<AnyElement> {
        rows.iter()
            .flatten()
            .cloned()
            .map(IntoElement::into_any_element)
            .collect()
    }
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
pub(super) struct ModuleCatalog {
    expanded: BTreeSet<&'static str>,
    service_rows: ObservedModuleServiceRows,
    armory_features: Option<ObservedArmoryFeatures>,
    host_is_beta: Option<bool>,
}
pub(super) enum ModuleCatalogEvent {
    OpenModule(ModulePage),
    FirmwareUpdate {
        device: Option<Device>,
        preview: bool,
    },
}
impl EventEmitter<ModuleCatalogEvent> for ModuleCatalog {}
impl ModuleCatalog {
    pub(super) fn new() -> Self {
        Self {
            expanded: BTreeSet::new(),
            service_rows: ObservedModuleServiceRows::default(),
            armory_features: None,
            host_is_beta: None,
        }
    }
    /// Explicit UI samples: no installer transport or device mutation is involved.
    pub(super) fn open_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                                            this.expanded.insert(item.id);
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ne has five entries. Tour/profile migration remain Dashboard entries.
        let available = source_module_catalog()
            .ids
            .iter()
            .filter_map(|id| MODULES.iter().find(|item| item.source_id() == id.as_str()))
            .map(|item| self.module_row(item, cx))
            .collect();
        let groups = [
            module_group(
                "FIRMWARE_UPDATES",
                ObservedModuleServiceRows::elements(&self.service_rows.firmware_updates),
                cx,
            ),
            module_group(
                "NEW_DEVICES",
                ObservedModuleServiceRows::elements(&self.service_rows.new_devices),
                cx,
            ),
            module_group("AVAILABLE_MODULES", available, cx),
            module_group(
                "UPDATED_RECENTLY",
                ObservedModuleServiceRows::elements(&self.service_rows.updated_recently),
                cx,
            ),
        ];
        v_flex()
            .id("devices-modules")
            .test_support()
            .w_full()
            .children(groups.into_iter().flatten())
    }
}
