use crate::settings_page::{preview_editions, preview_layout_label, preview_layouts_for_edition};
use gpui_kit::component::{
    button::Button,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    select::{Select, SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::{model::Device, product_preview};
use razer_pages::features::Choice;
use std::sync::Arc;

#[derive(Clone)]
struct ProductEntry {
    pid: u32,
    search: String,
    categories: Vec<&'static str>,
    available_pages: Vec<&'static str>,
    missing_pages: Vec<&'static str>,
}

fn category_key(source: &str) -> &'static str {
    match source {
        "MOUSE" => "LOCAL_PRODUCTS_CATEGORY_MICE",
        "MOUSEMAT" => "LOCAL_PRODUCTS_CATEGORY_MOUSE_MATS",
        "MOUSEPLUSMAT" => "LOCAL_PRODUCTS_CATEGORY_MOUSE_MAT_SETS",
        "KEYBOARD" => "LOCAL_PRODUCTS_CATEGORY_KEYBOARDS",
        "KEYPAD" => "LOCAL_PRODUCTS_CATEGORY_KEYPADS",
        "AUDIO"
        | "AUDIO_EARBUDS"
        | "AUDIO_EARPHONES"
        | "AUDIO_MIXER_AUDIO"
        | "AUDIO_SPEAKER"
        | "AUDIO_SPEAKER_HEAD_CUSHION"
        | "AUDIO_SPEAKER_HEAD_CUSHION_CLIO_X" => "LOCAL_PRODUCTS_CATEGORY_AUDIO",
        "BROADCASTER_CAMERA" | "CAMERA_CAMERA" => "LOCAL_PRODUCTS_CATEGORY_CAMERAS",
        "BROADCASTER_GAME_CAPTURE_CARD" => "LOCAL_PRODUCTS_CATEGORY_CAPTURE_CARDS",
        "BROADCASTER_MICROPHONE" => "LOCAL_PRODUCTS_CATEGORY_MICROPHONES",
        "BROADCASTER_STREAM_CONTROLLER" => "LOCAL_PRODUCTS_CATEGORY_STREAM_CONTROLLERS",
        "GAMEPAD" | "GAMEPAD_ARCADE_CONTROLLER" => "LOCAL_PRODUCTS_CATEGORY_GAME_CONTROLLERS",
        "HUE" | "HUE_HUE" => "LOCAL_PRODUCTS_CATEGORY_HUE",
        "IOT_BULB" | "IOT_KEY_LIGHT" | "IOT_LAMP" | "IOT_STRIP" => "LOCAL_PRODUCTS_CATEGORY_LIGHTS",
        "ACCESSORY_ARGB_CONTROLLER" => "LOCAL_PRODUCTS_CATEGORY_LIGHTING_CONTROLLERS",
        "ACCESSORY_FAN_CONTROLLER" | "ACCESSORY_LIQUID_CONTROLLER" => {
            "LOCAL_PRODUCTS_CATEGORY_COOLING_CONTROLLERS"
        }
        "ACCESSORY_MAINBOARD" => "LOCAL_PRODUCTS_CATEGORY_MAINBOARDS",
        "ACCESSORY_CHAIR" | "CHAIR_CHAIR" => "LOCAL_PRODUCTS_CATEGORY_CHAIRS",
        "ACCESSORY" => "LOCAL_PRODUCTS_CATEGORY_ACCESSORIES",
        "CASE" => "LOCAL_PRODUCTS_CATEGORY_CASES",
        "EGPU" => "LOCAL_PRODUCTS_CATEGORY_EXTERNAL_GRAPHICS",
        "MONITOR" => "LOCAL_PRODUCTS_CATEGORY_MONITORS",
        "SYSTEM" => "LOCAL_PRODUCTS_CATEGORY_SYSTEMS",
        _ => "LOCAL_PRODUCTS_CATEGORY_OTHER",
    }
}

fn product_categories(source: &[&str]) -> Vec<&'static str> {
    let mut categories = source
        .iter()
        .map(|category| category_key(category))
        .collect::<std::collections::BTreeSet<_>>();
    if categories.len() > 1 {
        categories.remove("LOCAL_PRODUCTS_CATEGORY_OTHER");
    }
    if categories.is_empty() {
        categories.insert("LOCAL_PRODUCTS_CATEGORY_OTHER");
    }
    categories.into_iter().collect()
}

fn catalog() -> Vec<ProductEntry> {
    razer_catalog::registry()
        .iter()
        .map(|product| {
            let mut available_pages = Vec::new();
            let mut missing_pages = Vec::new();
            if let Some(navigation) = product.primary_navigation() {
                for page in navigation
                    .pages()
                    .iter()
                    .filter(|page| page.role() != razer_catalog::ProductPageRole::Help)
                {
                    let key = page.kind().key();
                    if razer_pages::features::has_local_product_page(product.id(), key) {
                        available_pages.push(key);
                    } else {
                        missing_pages.push(key);
                    }
                }
            }
            let search = format!(
                "{} {} {}",
                product_preview::preview_search_text(product.id()),
                product.name(),
                product.categories().join(" ")
            )
            .to_lowercase();
            ProductEntry {
                pid: product.id(),
                search,
                categories: product_categories(product.categories()),
                available_pages,
                missing_pages,
            }
        })
        .collect()
}

fn filtered(catalog: &[ProductEntry], query: &str, category: &str) -> Vec<usize> {
    let terms = query.to_lowercase();
    catalog
        .iter()
        .enumerate()
        .filter(|(_, product)| {
            (category.is_empty() || product.categories.contains(&category))
                && terms
                    .split_whitespace()
                    .all(|term| product.search.contains(term))
        })
        .map(|(index, _)| index)
        .collect()
}

pub fn first_local_page(pid: u32) -> Option<&'static str> {
    razer_catalog::registered(pid)?
        .primary_navigation()?
        .pages()
        .iter()
        .find(|page| {
            page.role() != razer_catalog::ProductPageRole::Help
                && razer_pages::features::has_local_product_page(pid, page.kind().key())
        })
        .map(|page| page.kind().key())
}

pub fn preview_device(pid: u32, edition: u32, layout: u32) -> Option<Device> {
    let editions = preview_editions(pid);
    let layouts = preview_layouts_for_edition(pid, edition);
    if !editions.contains(&edition) || !layouts.contains(&layout) {
        return None;
    }
    let mut device = product_preview::mouse_mat_preview(pid)
        .or_else(|| product_preview::registered_preview(pid))?;
    device.edition_id = edition;
    device.layout_id = layout;
    let legacy_default_layout = if pid == 653 { 1 } else { 0 };
    if editions.first() != Some(&edition) || layout != legacy_default_layout {
        device.serial_number = format!("PREVIEW-{pid}-{edition}-{layout}");
        device.device_container_id = format!("preview-{pid}-{edition}-{layout}");
        for profile in &mut device.profiles {
            let id = format!("{}-{edition}-{layout}", profile.id);
            if device.active_profile == profile.id {
                device.active_profile = id.clone();
            }
            profile.id = id.clone();
            profile.guid = id;
        }
    }
    product_preview::apply_preview_names(&mut device);
    Some(device)
}

fn selected_preview_variant(pid: u32, edition: u32, previous: Option<u32>) -> Option<(u32, u32)> {
    let layouts = preview_layouts_for_edition(pid, edition);
    previous
        .filter(|layout| layouts.contains(layout))
        .or_else(|| layouts.first().copied())
        .map(|layout| (edition, layout))
}

pub fn matching_connected_device(device: &Device, pid: u32, edition: u32, layout: u32) -> bool {
    use razer_model::model::DeviceConnectionObservation;
    let serial = device.serial_number.to_ascii_uppercase();
    device.product_id == pid
        && device.edition_id == edition
        && device.layout_id == layout
        && !serial.starts_with("PREVIEW-")
        && !serial.starts_with("DEMO-")
        && matches!(
            device.dashboard.connection_observation,
            Some(
                DeviceConnectionObservation::UsbPresent
                    | DeviceConnectionObservation::HidPresent
                    | DeviceConnectionObservation::ReceiverPeer(1)
            )
        )
}

fn artwork(pid: u32, edition: u32, layout: u32) -> Option<&'static str> {
    if !preview_editions(pid).contains(&edition) {
        return None;
    }
    [3, 1]
        .into_iter()
        .find_map(|density| {
            razer_assets::dashboard_plugin_image(pid, edition, edition, layout, false, density)
        })
        .or_else(|| razer_assets::dashboard_image(pid, edition, layout))
        .or_else(|| {
            let observation = razer_pages::features::keyboard_products::SourceArtworkObservation {
                product_id: pid,
                edition_id: edition,
                layout_id: layout,
                wristrest_connected: None,
            };
            [3., 1.].into_iter().find_map(|scale| {
                razer_pages::features::keyboard_products::source_preview_image_for_scale(
                    observation,
                    scale,
                )
            })
        })
}

/// Keep the user-added local product card consistent with the Settings catalog.
/// Only a catalog preview may use its own current ProductImage when the
/// Dashboard PluginImages request is unavailable. Real device observations
/// retain the Dashboard's original image producer and error retry.
pub fn local_preview_artwork(device: &Device) -> Option<&'static str> {
    if !device.dashboard.local_snapshot
        || device.dashboard.connection_observation.is_some()
        || !device.serial_number.starts_with("PREVIEW-")
    {
        return None;
    }
    let layouts = preview_layouts_for_edition(device.product_id, device.edition_id);
    let layout = if layouts.contains(&device.layout_id) {
        device.layout_id
    } else {
        let (_, layout) = product_preview::presentation_variant(device);
        if !layouts.contains(&layout) {
            return None;
        }
        layout
    };
    artwork(device.product_id, device.edition_id, layout)
}

pub struct OpenProduct {
    pid: u32,
    edition: u32,
    layout: u32,
}
impl OpenProduct {
    pub fn product_id(&self) -> u32 {
        self.pid
    }
    pub fn edition_id(&self) -> u32 {
        self.edition
    }
    pub fn layout_id(&self) -> u32 {
        self.layout
    }
}

struct ProductsList {
    catalog: Arc<Vec<ProductEntry>>,
    visible: Vec<usize>,
    query: String,
    category: String,
    selected: Option<u32>,
}
impl ProductsList {
    fn product(&self, index: IndexPath) -> Option<&ProductEntry> {
        self.catalog.get(*self.visible.get(index.row)?)
    }
    fn refilter(&mut self) -> Option<IndexPath> {
        self.visible = filtered(&self.catalog, &self.query, &self.category);
        self.visible
            .iter()
            .position(|index| Some(self.catalog[*index].pid) == self.selected)
            .map(IndexPath::new)
    }
}
impl ListDelegate for ProductsList {
    type Item = ListItem;
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.visible.len()
    }
    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        if let Some(product) = index.and_then(|index| self.product(index)) {
            self.selected = Some(product.pid);
        }
    }
    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_owned();
        let selected_pid = self.selected;
        let selected = self.refilter();
        cx.defer_in(window, move |state, window, cx| {
            state.delegate_mut().selected = selected_pid;
            state.set_selected_index(selected, window, cx);
            cx.notify();
        });
        Task::ready(())
    }
    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let product = self.product(index)?;
        let edition = *preview_editions(product.pid).first()?;
        let layout = preview_layouts_for_edition(product.pid, edition)
            .first()
            .copied();
        let name = product_preview::source_product_name(product.pid, edition, &i18n::locale());
        let image = layout.and_then(|layout| artwork(product.pid, edition, layout));
        Some(
            ListItem::new(("local-product", product.pid))
                .accessibility_label(name.clone())
                .h_16()
                .gap_3()
                .px_3()
                .child(
                    div()
                        .w_16()
                        .h_12()
                        .flex_shrink_0()
                        .when_some(image, |view, image| {
                            view.child(img(image).size_full().object_fit(ObjectFit::Contain))
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(div().text_sm().truncate().child(name))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "PID {} · {}",
                                    product.pid,
                                    i18n::t(if product.available_pages.is_empty() {
                                        "LOCAL_PRODUCTS_UNAVAILABLE"
                                    } else {
                                        "LOCAL_PRODUCTS_PREVIEW"
                                    })
                                )),
                        ),
                ),
        )
    }
    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .p_4()
            .text_color(cx.theme().muted_foreground)
            .child(i18n::t("LOCAL_PRODUCTS_EMPTY"))
    }
}

pub struct ProductsView {
    catalog: Arc<Vec<ProductEntry>>,
    list: Entity<ListState<ProductsList>>,
    category: Entity<SelectState<Vec<Choice>>>,
    edition: Entity<SelectState<Vec<Choice>>>,
    layout: Entity<SelectState<Vec<Choice>>>,
    selected: Option<u32>,
    selected_edition: Option<u32>,
    variants: std::collections::BTreeMap<u32, (u32, u32)>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<OpenProduct> for ProductsView {}
impl ProductsView {
    pub(crate) fn relocalize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected_category = self.list.read(cx).delegate().category.clone();
        let categories = self
            .catalog
            .iter()
            .flat_map(|product| product.categories.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        let choices = std::iter::once(Choice::new("", i18n::t("LOCAL_PRODUCTS_ALL_CATEGORIES")))
            .chain(
                categories
                    .into_iter()
                    .map(|category| Choice::new(category, i18n::t(category))),
            )
            .collect();
        self.category.update(cx, |state, cx| {
            state.set_items(choices, window, cx);
            state.set_selected_value(&selected_category, window, cx);
        });
        if let Some(pid) = self.selected.take() {
            let edition = self.selected_edition;
            self.select(pid, window, cx);
            if let Some(edition) = edition.filter(|edition| preview_editions(pid).contains(edition))
            {
                self.edition.update(cx, |state, cx| {
                    state.set_selected_value(&edition.to_string(), window, cx);
                });
                self.select_edition(pid, edition, window, cx);
            }
        }
        self.list.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let catalog = Arc::new(catalog());
        let list = cx.new(|cx| {
            ListState::new(
                ProductsList {
                    visible: (0..catalog.len()).collect(),
                    catalog: catalog.clone(),
                    query: String::new(),
                    category: String::new(),
                    selected: None,
                },
                window,
                cx,
            )
        });
        let categories = catalog
            .iter()
            .flat_map(|product| product.categories.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        let choices = std::iter::once(Choice::new("", i18n::t("LOCAL_PRODUCTS_ALL_CATEGORIES")))
            .chain(
                categories
                    .into_iter()
                    .map(|category| Choice::new(category, i18n::t(category))),
            )
            .collect::<Vec<_>>();
        let category = cx.new(|cx| SelectState::new(choices, Some(IndexPath::new(0)), window, cx));
        let edition = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let layout = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&list, window, |this: &mut Self, list, event, window, cx| {
                if let ListEvent::Select(index) | ListEvent::Confirm(index) = event {
                    let pid = list
                        .read(cx)
                        .delegate()
                        .product(*index)
                        .map(|product| product.pid);
                    if let Some(pid) = pid {
                        this.select(pid, window, cx);
                    }
                }
            }),
            cx.subscribe_in(
                &category,
                window,
                |this: &mut Self, _, event, window, cx| {
                    if let SelectEvent::Confirm(Some(category)) = event {
                        this.list.update(cx, |state, cx| {
                            state.delegate_mut().category = category.clone();
                            let selected = state.delegate_mut().refilter();
                            state.set_selected_index(selected, window, cx);
                            state.scroll_to_selected_item(window, cx);
                            cx.notify();
                        });
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(&edition, window, |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let (Some(pid), Ok(value)) = (this.selected, value.parse()) {
                        if preview_editions(pid).contains(&value) {
                            this.select_edition(pid, value, window, cx);
                        }
                    }
                }
            }),
            cx.subscribe(&layout, |this: &mut Self, _, event, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let (Some(pid), Ok(value)) = (this.selected, value.parse()) {
                        if let Some(variant) = this.variants.get_mut(&pid) {
                            if preview_layouts_for_edition(pid, variant.0).contains(&value) {
                                variant.1 = value;
                            }
                        }
                        cx.notify();
                    }
                }
            }),
            cx.observe(&list, |_, _, cx| cx.notify()),
        ];
        Self {
            catalog,
            list,
            category,
            edition,
            layout,
            selected: None,
            variants: Default::default(),
            selected_edition: None,
            _subscriptions: subscriptions,
        }
    }
    fn select(&mut self, pid: u32, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected == Some(pid) {
            return;
        }
        let editions = preview_editions(pid);
        let Some(default) = editions.first().copied() else {
            return;
        };
        let edition = self
            .variants
            .get(&pid)
            .map(|variant| variant.0)
            .filter(|edition| editions.contains(edition))
            .unwrap_or(default);
        self.selected = Some(pid);
        self.edition.update(cx, |state, cx| {
            state.set_items(
                editions
                    .into_iter()
                    .map(|edition| {
                        Choice::new(
                            edition.to_string(),
                            product_preview::preview_edition_label(pid, edition),
                        )
                    })
                    .collect(),
                window,
                cx,
            );
            state.set_selected_value(&edition.to_string(), window, cx);
        });
        self.select_edition(pid, edition, window, cx);
    }
    fn select_edition(
        &mut self,
        pid: u32,
        edition: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let layouts = preview_layouts_for_edition(pid, edition);
        let variant =
            selected_preview_variant(pid, edition, self.variants.get(&pid).map(|value| value.1));
        self.selected_edition = Some(edition);
        if let Some(variant) = variant {
            self.variants.insert(pid, variant);
        } else {
            self.variants.remove(&pid);
        }
        self.layout.update(cx, |state, cx| {
            state.set_items(
                layouts
                    .into_iter()
                    .map(|layout| {
                        Choice::new(layout.to_string(), preview_layout_label(pid, layout))
                    })
                    .collect(),
                window,
                cx,
            );
            if let Some((_, layout)) = variant {
                state.set_selected_value(&layout.to_string(), window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
        cx.notify();
    }
    fn open(&self, cx: &mut Context<Self>) {
        let Some(pid) = self.selected else {
            return;
        };
        let Some(&(edition, layout)) = self.variants.get(&pid) else {
            return;
        };
        if first_local_page(pid).is_some()
            && preview_layouts_for_edition(pid, edition).contains(&layout)
        {
            cx.emit(OpenProduct {
                pid,
                edition,
                layout,
            });
        }
    }
    fn details(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(product) = self
            .selected
            .and_then(|pid| self.catalog.iter().find(|product| product.pid == pid))
        else {
            return div()
                .p_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(i18n::t("LOCAL_PRODUCTS_SELECT"))
                .into_any_element();
        };
        let variant = self.variants.get(&product.pid).copied();
        let Some(edition) = self.selected_edition else {
            return div().into_any_element();
        };
        let image = variant.and_then(|(_, layout)| artwork(product.pid, edition, layout));
        let name = product_preview::source_product_name(product.pid, edition, &i18n::locale());
        v_flex()
            .gap_3()
            .p_4()
            .min_w_0()
            .child(div().text_lg().child(name))
            .child(
                div()
                    .h_32()
                    .w_full()
                    .when_some(image, |view, image| {
                        view.child(img(image).size_full().object_fit(ObjectFit::Contain))
                    })
                    .when(image.is_none(), |view| {
                        view.child(i18n::t("LOCAL_PRODUCTS_NO_IMAGE"))
                    }),
            )
            .child(div().text_sm().child(i18n::t("LOCAL_PRODUCTS_EDITION")))
            .child(Select::new(&self.edition).w_full())
            .child(div().text_sm().child(i18n::t("LOCAL_PRODUCTS_LAYOUT")))
            .child(
                Select::new(&self.layout)
                    .w_full()
                    .disabled(variant.is_none()),
            )
            .when(variant.is_none(), |view| {
                view.child(div().text_sm().child(i18n::t("LOCAL_PRODUCTS_NO_LAYOUT")))
            })
            .child(
                Button::new("local-products-open")
                    .label(i18n::t("LOCAL_PRODUCTS_OPEN"))
                    .outline()
                    .disabled(product.available_pages.is_empty() || variant.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.open(cx))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::t("LOCAL_PRODUCTS_DRAFT_NOTICE")),
            )
            .child(
                div()
                    .text_sm()
                    .child(i18n::t(if product.available_pages.is_empty() {
                        "LOCAL_PRODUCTS_UNAVAILABLE_DETAIL"
                    } else {
                        "LOCAL_PRODUCTS_PARTIAL_NOTICE"
                    })),
            )
            .when(!product.missing_pages.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(i18n::t("LOCAL_PRODUCTS_MISSING_PAGES"))
                        .child(
                            product
                                .missing_pages
                                .iter()
                                .map(|key| i18n::t_or(key, key))
                                .collect::<Vec<_>>()
                                .join(" · "),
                        ),
                )
            })
            .into_any_element()
    }
}
impl Render for ProductsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.list.read(cx).delegate().visible.len();
        v_flex()
            .id("all-products")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .size_full()
            .child(
                h_flex()
                    .p_4()
                    .gap_3()
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex_1()
                            .text_lg()
                            .child(i18n::t("LOCAL_PRODUCTS_TITLE")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{visible} / {}", self.catalog.len())),
                    )
                    .child(Select::new(&self.category).w_48()),
            )
            .child(
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(div().flex_1().min_w_0().min_h_0().h_full().child(
                        List::new(&self.list).search_placeholder(i18n::t("LOCAL_PRODUCTS_SEARCH")),
                    ))
                    .child(
                        div()
                            .id("local-products-details")
                            .w_72()
                            .max_w(relative(0.5))
                            .h_full()
                            .overflow_y_scroll()
                            .border_l_1()
                            .border_color(cx.theme().border)
                            .child(self.details(cx)),
                    ),
            )
    }
}

#[cfg(test)]
#[path = "products/tests.rs"]
mod tests;
