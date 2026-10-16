//! Source owner shortcut modal and its mounted simultaneous-audio trigger.
use super::*;
use gpui_kit::component::radio::Radio;
use razer_widgets::{scroll::SourceScrollable as _, theme::SmartSwitchColors as SmartColors};

#[derive(serde::Deserialize)]
struct SmartSwitch {
    product_id: u32,
    header: String,
    description: String,
    note: String,
    headset: String,
    hero: String,
    rows: Vec<Row>,
    locales: BTreeMap<String, BTreeMap<String, String>>,
    navigation: String,
    tooltip_asset: String,
    card: Card,
}
#[derive(serde::Deserialize)]
struct Row {
    icon: String,
    description: String,
    action: Option<String>,
    chips: Vec<Chip>,
}
#[derive(serde::Deserialize)]
struct Chip {
    label: String,
    color: Option<String>,
    tip: Option<String>,
    asterisk: bool,
    icon: Option<String>,
}
#[derive(serde::Deserialize)]
struct Card {
    page: String,
    initial: Value,
    title: String,
    tip: String,
    description: String,
    note: String,
    help: String,
    options: Vec<OptionRow>,
    support_button: bool,
    mutation: String,
}
#[derive(serde::Deserialize)]
struct OptionRow {
    value: u8,
    name: String,
}
fn records() -> &'static [SmartSwitch] {
    static DATA: OnceLock<Vec<SmartSwitch>> = OnceLock::new();
    DATA.get_or_init(|| crate::data::source_owner_smart_switch_data!())
}
fn record(pid: u32) -> Option<&'static SmartSwitch> {
    records().iter().find(|row| row.product_id == pid)
}
pub(super) fn message(pid: u32, key: &str) -> Option<String> {
    let row = record(pid)?;
    row.locales
        .iter()
        .find(|(locale, _)| locale.eq_ignore_ascii_case(&i18n::locale()))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| row.locales.get("en").and_then(|values| values.get(key)))
        .cloned()
}
pub(super) fn initial_mode(pid: u32) -> Option<Value> {
    record(pid).map(|row| row.card.initial.clone())
}
pub(super) fn page(pid: u32) -> Option<&'static str> {
    record(pid).map(|row| row.card.page.as_str())
}
pub(super) fn mutation(pid: u32) -> Option<&'static str> {
    record(pid).map(|row| row.card.mutation.as_str())
}
pub(super) fn supports(pid: u32) -> bool {
    record(pid).is_some()
}

impl SourceHelpOwner {
    pub(super) fn set_simultaneous_audio(&mut self, value: u8, cx: &mut Context<Self>) {
        let Some(row) = record(self.product_id) else {
            return;
        };
        if !row.card.options.iter().any(|option| option.value == value) {
            return;
        }
        self.simultaneous_mode = Some(serde_json::json!(value));
        // This is the source Redux state, not a native device acknowledgement.
        cx.emit(self.request(HelpOwnerCommand::SetSimultaneousAudio));
        cx.notify();
    }
    pub(in crate::features) fn observe_simultaneous_audio(
        &mut self,
        request: &HelpOwnerRequested,
        payload: Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request) || !supports(self.product_id) {
            return false;
        }
        // Older owners destructure payload.mode; the three newer owners also
        // accept a primitive payload. Keep undefined/null fallback separate.
        self.simultaneous_mode = if matches!(self.product_id, 1398 | 2638 | 4124) {
            payload.get("mode").cloned()
        } else if payload.is_object() {
            payload.get("mode").cloned()
        } else {
            Some(payload)
        };
        cx.notify();
        true
    }
    pub(in crate::features) fn smart_switch_card(
        owner: &Entity<Self>,
        cx: &App,
    ) -> Option<AnyElement> {
        let state = owner.read(cx);
        let row = record(state.product_id)?;
        let pid = state.product_id;
        let mode = state
            .simultaneous_mode
            .as_ref()
            .filter(|value| !value.is_null());
        let effective = mode.cloned().unwrap_or_else(|| serde_json::json!(0));
        let msg = |key: &str| message(pid, key).unwrap_or_default();
        let card = surface::panel_with_help(
            msg(&row.card.title),
            surface::help_control("source-simultaneous-tip", msg(&row.card.tip)),
            cx,
        )
        .child(
            div()
                .mb(surface::css(20.))
                .child(msg(&row.card.description)),
        )
        .child(
            v_flex()
                .gap(surface::css(10.))
                .children(row.card.options.iter().map(|option| {
                    let value = option.value;
                    let target = owner.clone();
                    Radio::new(SharedString::from(format!(
                        "source-simultaneous-mode-{value}"
                    )))
                    .label(msg(&option.name))
                    .checked(effective == serde_json::json!(value))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |owner, cx| owner.set_simultaneous_audio(value, cx));
                    })
                })),
        )
        .child(
            h_flex()
                .mt(surface::css(20.))
                .flex_wrap()
                .text_color(Colors::note_text())
                .child(msg(&row.card.note))
                .when(row.card.support_button, |footer| {
                    let target = owner.clone();
                    footer.child(
                        Button::new("source-simultaneous-show-shortcuts")
                            .accessibility_label(msg(&row.card.help))
                            .p_0()
                            .ml(surface::css(3.))
                            .underline()
                            .text_color(Colors::text())
                            .hover(|style| style.text_color(Colors::accent()))
                            .child(msg(&row.card.help))
                            .child(
                                svg()
                                    .path("synapse/owner-smart-switch-support.svg")
                                    .w(surface::css(20.))
                                    .h(surface::css(16.)),
                            )
                            .on_click(move |_, _, cx| {
                                target.update(cx, |owner, cx| {
                                    owner.smart_switch_shown = true;
                                    cx.notify();
                                })
                            }),
                    )
                }),
        );
        Some(card.into_any_element())
    }
    fn smart_switch_chip(&self, chip: &Chip, row: &SmartSwitch, cx: &Context<Self>) -> AnyElement {
        let mut element = h_flex().gap(surface::css(10.));
        if let Some(icon) = &chip.icon {
            element = element.child(
                img(icon.clone())
                    .size(surface::css(16.))
                    .object_fit(ObjectFit::Contain),
            );
        } else {
            element = element.child(
                div()
                    .size(surface::css(12.))
                    .rounded_full()
                    .flex_shrink_0()
                    .bg(match chip.color.as_deref() {
                        Some("blue") => SmartColors::bluetooth(),
                        Some("white") => Colors::plain_button_text(),
                        _ => Colors::accent(),
                    }),
            );
        }
        element = element.child(
            h_flex()
                .max_w(surface::css(165.))
                .text_color(Colors::note_text())
                .child(message(self.product_id, &chip.label).unwrap_or_default())
                .when(chip.asterisk, |label| {
                    label.child(
                        div()
                            .ml(surface::css(4.))
                            .text_color(SmartColors::note_mark())
                            .child("*"),
                    )
                }),
        );
        if let Some(tip) = &chip.tip {
            element = element.child(
                div()
                    .relative()
                    .size(surface::css(15.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("smart-switch-help-icon")
                            .size(surface::css(15.))
                            .rounded_full()
                            .bg(SmartColors::tooltip_icon())
                            .on_hover(cx.listener(|this, hovered, _, cx| {
                                this.smart_switch_tip_hovered = *hovered;
                                cx.notify();
                            }))
                            .child(
                                svg()
                                    .path(row.tooltip_asset.clone())
                                    .size(surface::css(15.)),
                            ),
                    )
                    .when(self.smart_switch_tip_hovered, |help| {
                        help.child(
                            div()
                                .absolute()
                                .left(surface::css(13.))
                                .top(surface::css(14.))
                                .w(surface::css(300.))
                                .py(surface::css(8.))
                                .px(surface::css(10.))
                                .bg(SmartColors::tooltip_background())
                                .border_1()
                                .border_color(SmartColors::tooltip_border())
                                .text_color(Colors::text())
                                .text_size(surface::css(14.))
                                .line_height(surface::css(18.))
                                .child(message(self.product_id, tip).unwrap_or_default()),
                        )
                    }),
            );
        }
        element.into_any_element()
    }
    pub(super) fn smart_switch_overlay(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let Some(row) = record(self.product_id) else {
            return div().into_any_element();
        };
        let msg = |key: &str| message(self.product_id, key).unwrap_or_default();
        let card = v_flex()
            .id("source-smart-switch-card")
            .relative()
            .w(surface::css(720.))
            .mx_auto()
            .mt(surface::css(108.))
            .mb(surface::css(23.))
            .p(surface::css(30.))
            .border_1()
            .border_color(Colors::accent())
            .rounded(surface::css(3.))
            .bg(Colors::background())
            .text_color(Colors::text())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(18.))
            .on_mouse_down_out(
                cx.listener(|this, _, _, cx| this.close(HelpOwnerOverlay::SmartSwitch, cx)),
            )
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(Colors::accent())
                    .child(msg(&row.header).to_uppercase()),
            )
            .child(
                div()
                    .w(surface::css(640.))
                    .mt(surface::css(7.))
                    .text_color(Colors::note_text())
                    .line_height(surface::css(17.))
                    .child(msg(&row.description)),
            )
            .child(
                div()
                    .relative()
                    .w(surface::css(660.))
                    .h(surface::css(90.))
                    .mt(surface::css(30.))
                    .child(
                        img(row.headset.clone())
                            .absolute()
                            .left(surface::css(290.))
                            .top(surface::css(10.))
                            .size(surface::css(80.))
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        img(row.hero.clone())
                            .absolute()
                            .w(surface::css(660.))
                            .h(surface::css(90.))
                            .object_fit(ObjectFit::Contain),
                    ),
            )
            .child(
                v_flex()
                    .gap(surface::css(4.))
                    .mt(surface::css(30.))
                    .children(row.rows.iter().map(|entry| {
                        // Original title is display:none. Keep its source translation in
                        // evidence/data without adding a visible heading to the row.
                        h_flex()
                            .min_h(surface::css(99.))
                            .p(surface::css(20.))
                            .rounded(surface::css(3.))
                            .bg(SmartColors::row())
                            .child(
                                h_flex().w(surface::css(64.)).flex_shrink_0().child(
                                    img(entry.icon.clone())
                                        .w(surface::css(44.))
                                        .h(surface::css(59.))
                                        .object_fit(ObjectFit::Contain),
                                ),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w(surface::css(310.))
                                    .child(
                                        div()
                                            .w(surface::css(310.))
                                            .text_color(Colors::note_text())
                                            .line_height(surface::css(17.))
                                            .child(msg(&entry.description)),
                                    )
                                    .when_some(entry.action.as_ref(), |center, key| {
                                        center.child(
                                            Button::new("smart-switch-remap-sound")
                                                .accessibility_label(msg(key))
                                                .p_0()
                                                .self_start()
                                                .mt(surface::css(4.))
                                                .underline()
                                                .text_color(Colors::text())
                                                .hover(|style| style.text_color(Colors::accent()))
                                                .child(msg(key))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.emit(HelpOwnerPageRequested(
                                                        row.navigation.as_str(),
                                                    ));
                                                    this.close(HelpOwnerOverlay::SmartSwitch, cx);
                                                })),
                                        )
                                    }),
                            )
                            .child(
                                v_flex()
                                    .min_w(surface::css(252.))
                                    .pl(surface::css(21.))
                                    .gap(surface::css(8.))
                                    .children(
                                        entry
                                            .chips
                                            .iter()
                                            .map(|chip| self.smart_switch_chip(chip, row, cx)),
                                    ),
                            )
                    })),
            )
            .child(
                h_flex()
                    .items_start()
                    .mt(surface::css(29.))
                    .text_color(Colors::note_text())
                    .line_height(surface::css(17.))
                    .child(
                        div()
                            .mr(surface::css(4.))
                            .text_color(SmartColors::note_mark())
                            .child("*"),
                    )
                    .child(msg(&row.note)),
            )
            .child(
                Button::new("smart-switch-close")
                    .accessibility_label("Close")
                    .p_0()
                    .absolute()
                    .right(surface::css(11.))
                    .top(surface::css(11.))
                    .size(surface::css(22.))
                    .text_color(Colors::note_text())
                    .hover(|style| style.text_color(Colors::text()))
                    .child(
                        svg()
                            .path("synapse/owner-smart-switch-close.svg")
                            .size(surface::css(22.)),
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| this.close(HelpOwnerOverlay::SmartSwitch, cx)),
                    ),
            );
        let size = window.viewport_size();
        let host_tab = window.rem_size() / 16. * 42.;
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position(point(px(0.), host_tab))
                .child(
                    div()
                        .id("smart-switch-scroll")
                        .w(size.width)
                        .h((size.height - host_tab).max(px(0.)))
                        .bg(Colors::shade())
                        .child(card)
                        .scrollable_y(),
                ),
        )
        .priority(105)
        .into_any_element()
    }
}
