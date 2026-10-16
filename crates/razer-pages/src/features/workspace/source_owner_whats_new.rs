//! Actually mounted Analog owner WhatsNewDialog, separate from tutorial state.
use super::*;
use razer_widgets::scroll::SourceScrollable as _;

#[derive(serde::Deserialize)]
struct WhatsNew {
    product_id: u32,
    content: Node,
    locales: BTreeMap<String, BTreeMap<String, String>>,
    button_key: String,
    storage_key: String,
    image: String,
}
#[derive(serde::Deserialize)]
struct Node {
    kind: String,
    key: Option<String>,
    #[serde(rename = "className")]
    class_name: Option<String>,
    #[serde(default)]
    children: Vec<Node>,
}
fn records() -> &'static [WhatsNew] {
    static DATA: OnceLock<Vec<WhatsNew>> = OnceLock::new();
    DATA.get_or_init(|| read_source_owner_whats_new_catalog())
}
fn record(pid: u32) -> Option<&'static WhatsNew> {
    records().iter().find(|row| row.product_id == pid)
}
pub(super) fn storage_key(pid: u32) -> Option<&'static str> {
    record(pid).map(|row| row.storage_key.as_str())
}
fn message(row: &WhatsNew, key: &str) -> String {
    row.locales
        .iter()
        .find(|(locale, _)| locale.eq_ignore_ascii_case(&i18n::locale()))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| row.locales.get("en").and_then(|values| values.get(key)))
        .cloned()
        .unwrap_or_default()
}
fn render_node(node: &Node, row: &WhatsNew, uppercase: bool) -> AnyElement {
    if node.kind == "text" {
        let text = message(row, node.key.as_deref().unwrap_or_default());
        return div()
            .child(if uppercase { text.to_uppercase() } else { text })
            .into_any_element();
    }
    let class = node.class_name.as_deref().unwrap_or_default();
    let title = class.contains("_head__") || class.contains("_header__");
    let element = div()
        .when(class.contains("_root__"), |element| {
            element
                .flex()
                .flex_col()
                .gap(surface::css(20.))
                .min_h_0()
                .mr(surface::css(-52.))
                .pr(surface::css(52.))
        })
        .when(class.contains("_head__"), |element| {
            element
                .text_size(surface::css(20.))
                .line_height(surface::css(20.))
        })
        .when(class.contains("_text-wrapper__"), |element| {
            element
                .flex()
                .flex_col()
                .gap(surface::css(5.))
                .font_family("Roboto")
                .text_size(surface::css(13.5))
                .line_height(surface::css(17.))
        })
        .when(class.contains("_header__"), |element| {
            element
                .font_family("RazerF5")
                .text_size(surface::css(14.))
                .line_height(surface::css(14.))
                .text_color(Colors::accent())
        })
        .children(
            node.children
                .iter()
                .map(|child| render_node(child, row, title)),
        );
    if class.contains("_root__") {
        element
            .id("owner-whats-new-content")
            .scrollable_y()
            .into_any_element()
    } else {
        element.into_any_element()
    }
}
impl SourceHelpOwner {
    pub(super) fn whats_new_overlay(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let Some(row) = record(self.product_id) else {
            return div().into_any_element();
        };
        let size = window.viewport_size();
        let unit = window.rem_size() / 16.;
        let host_tab = unit * 42.;
        let height = (size.height - host_tab).max(px(0.));
        let card = h_flex()
            .items_stretch()
            .relative()
            .w(surface::css(800.))
            .min_h(surface::css(420.))
            .max_h((height * 0.87 - unit * 60.).max(px(0.)))
            .bg(Colors::background())
            .rounded(surface::css(3.))
            .overflow_hidden()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(Colors::text())
            .child(
                div()
                    .relative()
                    .w(surface::css(320.))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(
                        img(row.image.clone())
                            .absolute()
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .py(surface::css(50.))
                    .px(surface::css(52.))
                    .child(render_node(&row.content, row, false))
                    .child(
                        h_flex().gap(surface::css(10.)).pt(surface::css(30.)).child(
                            Button::new("owner-whats-new-get-started")
                                .accessibility_label(message(row, &row.button_key))
                                .h(surface::css(27.))
                                .min_w(surface::css(111.))
                                .p_0()
                                .bg(Colors::accent())
                                .text_color(Colors::button_text())
                                .hover(|style| style.opacity(0.8))
                                .child(message(row, &row.button_key).to_uppercase())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close(HelpOwnerOverlay::WhatsNew, cx)
                                })),
                        ),
                    ),
            )
            .child(
                Button::new("owner-whats-new-close")
                    .accessibility_label("Close")
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.))
                    .size(surface::css(24.))
                    .p_0()
                    .child(
                        svg()
                            .path("synapse/owner-whats-new-close.svg")
                            .size(surface::css(24.)),
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| this.close(HelpOwnerOverlay::WhatsNew, cx)),
                    ),
            );
        // CSS outline sits outside the original 800px card; it does not reduce
        // the 320px media and 480px content columns as an internal border would.
        let outlined = div()
            .p(surface::css(1.5))
            .bg(Colors::accent())
            .rounded(surface::css(4.5))
            .child(card);
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position(point(px(0.), host_tab))
                .child(
                    div().w(size.width).h(height).bg(Colors::shade()).child(
                        deferred(CenteredAt {
                            center: point(size.width / 2., host_tab + height / 2. + unit * 42.),
                            child: outlined.into_any_element(),
                        })
                        .priority(106),
                    ),
                ),
        )
        .priority(105)
        .into_any_element()
    }
}

// Generated catalog reader: razer-pages/source_owner_whats_new_data.
fn read_source_owner_whats_new_catalog() -> Vec<WhatsNew> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<WhatsNew>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<WhatsNew> {
        let value = match r.tag()? {
            0 => WhatsNew {
                product_id: r.child(decode_2)?,
                content: r.child(decode_3)?,
                locales: r.child(decode_7)?,
                button_key: r.child(decode_4)?,
                storage_key: r.child(decode_4)?,
                image: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Node> {
        let value = match r.tag()? {
            0 => Node {
                kind: r.child(decode_4)?,
                key: r.child(decode_5)?,
                class_name: None,
                children: Default::default(),
            },
            1 => Node {
                kind: r.child(decode_4)?,
                key: None,
                class_name: r.child(decode_5)?,
                children: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_4)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Node>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SOURCE_OWNER_WHATS_NEW_DATA,
        0,
        "aa66b469dd268e1bff1c7eea5015cfe1d24ef45c34d7bc3a6aef51f877a9077d",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_source_owner_whats_new_data_decodes() {
    let _ = read_source_owner_whats_new_catalog();
}
// End generated catalog reader: razer-pages/source_owner_whats_new_data.
