//! Source-projected, actually mounted four-step AnalogTutorial.
use super::*;

#[derive(serde::Deserialize)]
struct Tutorial {
    product_id: u32,
    steps: Vec<Step>,
    locales: BTreeMap<String, BTreeMap<String, String>>,
    storage_read: String,
    storage_writes: Vec<String>,
}
#[derive(serde::Deserialize)]
struct Step {
    heading: Node,
    content: Node,
    asset: String,
}
#[derive(serde::Deserialize)]
struct Node {
    kind: String,
    #[serde(default)]
    key: Option<String>,
    #[serde(rename = "className", default)]
    class_name: Option<String>,
    #[serde(default)]
    children: Vec<Node>,
}
fn tutorials() -> &'static [Tutorial] {
    static DATA: OnceLock<Vec<Tutorial>> = OnceLock::new();
    DATA.get_or_init(|| crate::data::source_help_analog_data!())
}
fn tutorial(pid: u32) -> Option<&'static Tutorial> {
    tutorials().iter().find(|row| row.product_id == pid)
}
pub(super) fn supports(pid: u32) -> bool {
    tutorial(pid).is_some()
}
pub(super) fn storage_key(pid: u32) -> Option<String> {
    tutorial(pid).map(|row| row.storage_read.clone())
}
pub(super) fn storage_writes(pid: u32) -> Vec<(String, Value)> {
    tutorial(pid)
        .map(|row| {
            row.storage_writes
                .iter()
                .map(|key| (key.clone(), Value::Bool(true)))
                .collect()
        })
        .unwrap_or_default()
}
pub(super) fn message(pid: u32, key: &str) -> Option<String> {
    let row = tutorial(pid)?;
    row.locales
        .iter()
        .find(|(locale, _)| locale.eq_ignore_ascii_case(&i18n::locale()))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| row.locales.get("en").and_then(|values| values.get(key)))
        .cloned()
}
fn render_node(node: &Node, pid: u32) -> AnyElement {
    if node.kind == "text" {
        return div()
            .child(super::message(pid, node.key.as_deref().unwrap_or_default()))
            .into_any_element();
    }
    if node.kind == "icon" {
        return svg()
            .path("synapse/help-analog-info.svg")
            .size(surface::css(20.))
            .flex_shrink_0()
            .into_any_element();
    }
    let class = node.class_name.as_deref().unwrap_or_default();
    let mut element = div()
        .when(class.contains("_text-wrapper__"), |box_| {
            box_.flex()
                .flex_col()
                .gap(surface::css(5.))
                .font_family("Roboto")
                .text_size(surface::css(13.5))
                .line_height(surface::css(17.))
        })
        .when(class.contains("_header__"), |box_| {
            box_.font_family("RazerF5")
                .text_size(surface::css(14.))
                .line_height(surface::css(14.))
                .text_color(Colors::accent())
        })
        .when(class.contains("_note__"), |box_| {
            box_.flex()
                .items_start()
                .gap(surface::css(10.))
                .text_color(Colors::note_text())
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
        });
    for child in &node.children {
        element = element.child(if class.contains("_header__") && child.kind == "text" {
            div()
                .child(super::message(pid, child.key.as_deref().unwrap_or_default()).to_uppercase())
                .into_any_element()
        } else {
            render_node(child, pid)
        });
    }
    element.into_any_element()
}
impl SourceHelpOwner {
    pub(super) fn analog_overlay(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let Some(row) = tutorial(self.product_id) else {
            return div().into_any_element();
        };
        let step = &row.steps[self.analog_step.saturating_sub(1).min(3)];
        let asset = step.asset.as_str();
        let mut content = v_flex().gap(surface::css(20.)).child(heading(
            self.product_id,
            step.heading.key.as_deref().unwrap_or_default(),
        ));
        // The memo's JSX Fragment has no DOM node: retain each original child.
        for node in &step.content.children {
            content = content.child(render_node(node, self.product_id));
        }
        let mut indicators = h_flex().gap(surface::css(10.)).h(surface::css(20.));
        for index in 1..=4 {
            indicators =
                indicators.child(div().size(surface::css(4.)).rounded(surface::css(2.)).bg(
                    if index == self.analog_step {
                        Colors::plain_button_text()
                    } else {
                        Colors::button()
                    },
                ));
        }
        let last = self.analog_step == 4;
        let button = |id: &'static str, key: &str, primary: bool| {
            Button::new(id)
                .accessibility_label(super::message(self.product_id, key))
                .w(surface::css(90.))
                .h(surface::css(27.))
                .p_0()
                .rounded(surface::css(3.))
                .text_size(surface::css(12.))
                .bg(if primary {
                    Colors::accent()
                } else {
                    Colors::button()
                })
                .text_color(if primary {
                    Colors::button_text()
                } else {
                    Colors::plain_button_text()
                })
                .hover(|style| style.opacity(0.8))
                .child(super::message(self.product_id, key).to_uppercase())
        };
        let controls = v_flex().child(indicators).child(
            h_flex()
                .gap(surface::css(10.))
                .child(
                    button("help-analog-back", "BACK", false)
                        .disabled(self.analog_step == 1)
                        .when(self.analog_step == 1, |b| b.opacity(0.3))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.analog_step > 1 {
                                this.analog_step -= 1;
                                cx.notify();
                            }
                        })),
                )
                .child(
                    button(
                        "help-analog-next",
                        if last { "CLOSE" } else { "NEXT" },
                        true,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if last {
                            this.close(HelpOwnerOverlay::Tutorial, cx);
                        } else if this.analog_step < 4 {
                            this.analog_step += 1;
                            cx.notify();
                        }
                    })),
                ),
        );
        let close = Button::new("help-analog-close")
            .accessibility_label(super::message(self.product_id, "CLOSE"))
            .absolute()
            .top(surface::css(10.))
            .right(surface::css(10.))
            .size(surface::css(24.))
            .p_0()
            .child(
                svg()
                    .path("synapse/help-owner-close.svg")
                    .size(surface::css(24.)),
            )
            .on_click(cx.listener(|this, _, _, cx| this.close(HelpOwnerOverlay::Tutorial, cx)));
        let card = h_flex()
            .id("help-owner-analog-tutorial")
            .relative()
            .w(surface::css(800.))
            .min_h(surface::css(420.))
            .items_stretch()
            .bg(Colors::background())
            .text_color(Colors::text())
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .occlude()
            .shadow(vec![BoxShadow {
                color: Colors::accent(),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: window.rem_size() / 16. * 1.5,
                inset: false,
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .w(surface::css(320.))
                    .min_h(surface::css(420.))
                    .max_h(surface::css(420.))
                    .m(surface::css(1.))
                    .child(
                        image_cache(razer_assets::resource_image_cache())
                            .absolute()
                            .inset_0()
                            .child(
                                img(asset)
                                    .id(SharedString::from(format!(
                                        "help-analog-video-{}",
                                        self.analog_step
                                    )))
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .justify_between()
                    .p(surface::css(50.))
                    .child(content)
                    .child(controls),
            )
            .child(close);
        let size = window.viewport_size();
        let host_tab = window.rem_size() / 16. * 42.;
        let height = (size.height - host_tab).max(px(0.));
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position(point(px(0.), host_tab))
                .child(
                    div().w(size.width).h(height).bg(Colors::shade()).child(
                        deferred(CenteredAt {
                            center: point(size.width / 2., host_tab + height * 0.45),
                            child: card.into_any_element(),
                        })
                        .priority(106),
                    ),
                ),
        )
        .priority(105)
        .into_any_element()
    }
}
