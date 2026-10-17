use super::super::artwork::pairing_device_image as device_image;
use super::*;
use gpui_kit::base::{Button as BaseButton, Radio as BaseRadio};
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::theme::HyperfluxColors as Colors;

fn message(key: &str) -> String {
    static STRINGS: OnceLock<BTreeMap<String, BTreeMap<String, String>>> = OnceLock::new();
    let strings = STRINGS.get_or_init(|| read_hyperflux_207_pairing_strings_catalog());
    let locale = razer_i18n::locale();
    strings
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(&locale))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| strings["en"].get(key))
        .map(String::as_str)
        .unwrap_or(key)
        .into()
}
fn name(device: &Value) -> String {
    let locale = razer_i18n::locale().to_lowercase();
    device["productName"]
        .get(&locale)
        .filter(|value| truthy(value))
        .or_else(|| device["productName"].get("en"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
fn scalar(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|v| v.to_string()))
}
fn action(id: impl Into<ElementId>, key: &str, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(message(key))
        .h(surface::css(28.))
        .min_w(surface::css(90.))
        .px(surface::css(10.))
        .border_1()
        .border_color(Colors::black())
        .rounded(surface::css(3.))
        .bg(if primary { Colors::green() } else { Colors::cancel() })
        .text_color(if primary {
            Colors::button_text()
        } else {
            Colors::white()
        })
        .text_size(surface::css(12.))
        .opacity(if primary { 1. } else { 0.7 })
        .hover(move |s| s.opacity(if primary { 0.7 } else { 1. }))
        .child(message(key).to_uppercase())
}
#[derive(IntoElement)]
struct PairingSpinner {
    slot: usize,
}
impl RenderOnce for PairingSpinner {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        svg()
            .path("synapse/hyperflux-207/progress_spinner.svg")
            .size(surface::css(24.))
            .text_color(Colors::green())
            .with_animation(
                ("hyperflux-pair-spinner", self.slot as u64),
                Animation::new(Duration::from_secs(1))
                    .repeat()
                    .with_easing(linear),
                |icon, phase| icon.with_transformation(Transformation::rotate(percentage(phase))),
            )
    }
}
// Source 520x30 SVG path, dash arrays and the 500ms CSS keyframe. Draw
// segments directly to preserve root scaleX(-1) and unscaled background crop.
fn pairing_line(slot: usize, connected: u64) -> AnyElement {
    let view = div()
        .relative()
        .w(surface::css(440.))
        .h_full()
        .overflow_hidden()
        .when(slot == 1, |v| v.left(surface::css(-3.)));
    if connected == 0 {
        return view.into_any_element();
    }
    let paint = move |phase: f32| {
        canvas(
            |_, _, _| (),
            move |area, _, window, _| {
                let unit = f32::from(window.rem_size()) / 16.;
                let origin = area.origin
                    + point(
                        if slot == 0 {
                            px(0.)
                        } else {
                            area.size.width - px(520. * unit)
                        },
                        px(0.),
                    );
                let length = 284.;
                let period = if connected == 1 { 22. } else { 4. };
                let offset = if connected == 1 { 22. * (1. - phase) } else { 0. };
                let coordinate = |distance: f32| {
                    let (x, y) = if distance <= 269. {
                        (126. + distance, 15.)
                    } else {
                        (395., 15. + distance - 269.)
                    };
                    let x = if slot == 0 { 520. - x } else { x };
                    origin + point(px(x * unit), px(y * unit))
                };
                let mut start = -offset;
                while start < length {
                    let from = start.max(0.);
                    let to = (start + 2.).min(length);
                    if to > from {
                        let mut path = PathBuilder::stroke(px(2. * unit));
                        path.move_to(coordinate(from));
                        if from < 269. && to > 269. {
                            path.line_to(coordinate(269.));
                        }
                        path.line_to(coordinate(to));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, Colors::green());
                        }
                    }
                    start += period;
                }
            },
        )
        .size_full()
    };
    if connected == 1 {
        view.with_animation(
            ("hyperflux-pair-ants", slot as u64),
            Animation::new(Duration::from_millis(500))
                .repeat()
                .with_easing(linear),
            move |element, phase| element.child(paint(phase)),
        )
        .into_any_element()
    } else {
        view.child(paint(0.)).into_any_element()
    }
}
impl SourceControls {
    fn render_pairing_scans(
        &self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let devices = self.hyperflux.pairing.scans(slot);
        let count = devices.len();
        let mut rows = v_flex()
            .id(("hyperflux-scan-list", slot as u64))
            .w_full()
            .h(surface::css(104.))
            .overflow_y_scroll()
            .overflow_x_hidden();
        for (ix, device) in devices.into_iter().enumerate() {
            let selected = self.hyperflux.pairing.selected_ix == ix;
            let owner = cx.entity().downgrade();
            let id = SharedString::from(format!(
                "hyperflux-scan-{}-{}-{ix}",
                category(slot),
                scalar(&device["productId"]).unwrap_or_default()
            ));
            let ratio = gpui_kit::base::motion::transition(
                (id.clone(), "selected-scale"),
                if selected { 1. } else { 0. },
                gpui_kit::base::motion::Transition::new(Duration::from_millis(200))
                    .easing(gpui_kit::base::motion::Easing::Ease),
                window,
                cx,
            );
            rows = rows.child(
                BaseRadio::new(id)
                    .checked(selected)
                    .accessibility_label(name(device))
                    .flex()
                    .items_start()
                    .min_h(surface::css(20.))
                    .text_color(Colors::text())
                    .line_height(surface::css(20.))
                    .child(
                        div()
                            .relative()
                            .size(surface::css(20.))
                            .mr(surface::css(10.))
                            .flex_shrink_0()
                            .border_1()
                            .border_color(Colors::radio_border())
                            .rounded(surface::css(10.))
                            .child(
                                div()
                                    .absolute()
                                    .left(surface::css(9. - 5. * ratio))
                                    .top(surface::css(9. - 5. * ratio))
                                    .size(surface::css(10. * ratio))
                                    .rounded(surface::css(5.))
                                    .bg(Colors::green())
                                    .opacity(ratio),
                            ),
                    )
                    .child(name(device).to_uppercase())
                    .on_change(move |_, _, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.hyperflux.pairing.selected_ix = ix;
                            cx.notify();
                        });
                    }),
            );
        }
        v_flex()
            .w_full()
            .child(
                div()
                    .my(surface::css(10.))
                    .h(surface::css(20.))
                    .text_center()
                    .child(format!(
                        "{}{}",
                        if count == 1 {
                            String::new()
                        } else {
                            format!("{count} ")
                        },
                        message("DUALLINK_SCAN_DEVICE_FOUND")
                    )),
            )
            .child(rows)
            .into_any_element()
    }
    fn render_pairing_artwork(
        &self,
        slot: usize,
        device: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let status = self.hyperflux.pairing.status[slot];
        let can_modify = self.hyperflux.pairing.can_modify(slot);
        let valid = truthy(&device["valid"]);
        let empty = !valid || status == Status::JustUnpair;
        let mut art = div()
            .relative()
            .w_full()
            .h(surface::css(120.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center();
        if empty {
            art = art.justify_start().pt(surface::css(20.)).child(
                img(if slot == 0 {
                    "synapse/hyperflux-207/icon_category_keyboard.svg"
                } else {
                    "synapse/hyperflux-207/icon_category_mouse.svg"
                })
                .size(surface::css(44.))
                .flex_shrink_0()
                .mb(surface::css(10.)),
            );
            if matches!(
                status,
                Status::Scanning | Status::Pairing | Status::JustUnpair
            ) {
                art = art.child(
                    div()
                        .w(surface::css(90.))
                        .h(surface::css(28.))
                        .flex()
                        .justify_center()
                        .child(PairingSpinner { slot }),
                );
            } else {
                // Wm uses undefined PAIRED_FAILED, so actual PairFailed falls back to ADD.
                let key =
                    if status == Status::Scanned && self.hyperflux.pairing.scans(slot).is_empty() {
                        "RESCAN"
                    } else {
                        "ADD"
                    };
                art = art.child(
                    BaseButton::new(("hyperflux-pair-add", slot as u64))
                        .disabled(!can_modify)
                        .accessibility_label(message(key))
                        .min_w(surface::css(90.))
                        .h(surface::css(28.))
                        .px(surface::css(5.))
                        .text_size(surface::css(12.))
                        .line_height(surface::css(28.))
                        .rounded(surface::css(2.))
                        .bg(Colors::green())
                        .text_color(Colors::button_text())
                        .opacity(if can_modify { 1. } else { 0.45 })
                        .child(message(key).to_uppercase())
                        .on_click(cx.listener(move |this, _, _, cx| this.scan_pairing(slot, cx))),
                );
            }
        } else {
            if let Some(image) = device_image(device) {
                art = art.child(
                    img(image)
                        .h(surface::css(120.))
                        .w_full()
                        .object_fit(ObjectFit::Contain),
                );
            }
            if matches!(status, Status::Paired | Status::JustPaired) {
                art = art.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(
                            BaseButton::new(("hyperflux-pair-unpair", slot as u64))
                                .disabled(!can_modify)
                                .accessibility_label(message("UNPAIR"))
                                .h(surface::css(28.))
                                .px(surface::css(10.))
                                .rounded(surface::css(2.))
                                .bg(Colors::cancel())
                                .text_color(Colors::white())
                                .text_size(surface::css(12.))
                                .opacity(if can_modify { 1. } else { 0.5 })
                                .hover(|s| s.bg(Colors::cancel_hover()))
                                .child(message("UNPAIR").to_uppercase())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.hyperflux.pairing.status[slot] = Status::Unpairing;
                                    cx.notify();
                                })),
                        ),
                );
            }
            if status == Status::Unpairing {
                art = art.opacity(0.);
            }
        }
        art.into_any_element()
    }
    fn render_pairing_box(
        &self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let status = self.hyperflux.pairing.status[slot];
        let device = self.pairing_slave(slot);
        let selected_list =
            status == Status::Scanned && self.hyperflux.pairing.scans(slot).len() > 1;
        let mut inner = v_flex()
            .relative()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .h(surface::css(28.))
                    .pt(surface::css(10.))
                    .line_height(surface::css(18.))
                    .text_center()
                    .text_color(Colors::cancel())
                    .child(message(category(slot)).to_uppercase()),
            );
        inner = inner.child(if selected_list {
            self.render_pairing_scans(slot, window, cx)
        } else {
            self.render_pairing_artwork(slot, &device, cx)
        });
        let tag = match status {
            Status::Scanning => Some(("SCANNING", Colors::green())),
            Status::Scanned if self.hyperflux.pairing.scans(slot).is_empty() => {
                Some(("NO_DEVICE_FOUND", Colors::warning()))
            }
            Status::Pairing => Some(("PAIRING", Colors::green())),
            Status::PairFailed => Some(("PAIRING_FAILED", Colors::warning())),
            Status::JustUnpair => Some(("UNPAIRING", Colors::green())),
            _ => None,
        };
        if selected_list {
            inner = inner.child(
                h_flex()
                    .justify_center()
                    .child(
                        action(("hyperflux-scan-cancel", slot as u64), "CANCEL", false)
                            .mr(surface::css(10.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pairing_message(
                                    "DUALLINK_BIND_INFO",
                                    serde_json::json!({}),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        action(("hyperflux-scan-pair", slot as u64), "PAIR", true).on_click(
                            cx.listener(move |this, _, _, cx| this.bind_selected_pairing(slot, cx)),
                        ),
                    ),
            );
        } else if let Some((key, color)) = tag {
            inner = inner.child(
                div()
                    .w_full()
                    .line_height(surface::css(16.))
                    .text_center()
                    .text_color(color)
                    .child(message(key)),
            );
        } else if truthy(&device["valid"]) {
            inner = inner.child(
                div()
                    .h(surface::css(60.))
                    .w_full()
                    .line_height(surface::css(16.))
                    .text_center()
                    .child(name(&device).to_uppercase())
                    .when(status == Status::UnpairFailed, |v| {
                        v.child(
                            div()
                                .h(surface::css(20.))
                                .line_height(surface::css(20.))
                                .text_color(Colors::warning())
                                .child(message("UNPAIRING_FAILED")),
                        )
                    }),
            );
        }
        if status == Status::Unpairing {
            let selected = device.clone();
            inner = inner.child(
                div()
                    .absolute()
                    .left(surface::css(10.))
                    .top(surface::css(10.))
                    .w(surface::css(210.))
                    .pt(surface::css(18.))
                    .pb(surface::css(20.))
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(Colors::warning())
                    .bg(Colors::panel())
                    .child(
                        div()
                            .w(surface::css(190.))
                            .mx_auto()
                            .mt(surface::css(3.))
                            .mb(surface::css(9.))
                            .line_height(surface::css(17.))
                            .text_center()
                            .child(message("UNPAIR_CONFIRM_TEXT")),
                    )
                    .child(
                        h_flex()
                            .justify_center()
                            .child(
                                action(
                                    ("hyperflux-pair-unpair-cancel", slot as u64),
                                    "CANCEL",
                                    false,
                                )
                                .mr(surface::css(10.))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.hyperflux.pairing.status[slot] = Status::Paired;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action(
                                    ("hyperflux-pair-unpair-confirm", slot as u64),
                                    "CONFIRM",
                                    true,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.unbind_pairing(slot, &selected, cx),
                                )),
                            ),
                    ),
            );
        }
        div()
            .w(surface::css(250.))
            .h(surface::css(210.))
            .px(surface::css(10.))
            .bg(Colors::panel())
            .rounded(surface::css(5.))
            .when(device["connected"] == 2, |v| {
                v.shadow(vec![BoxShadow {
                    color: Colors::green(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: window.rem_size() / 16. * 2.,
                    inset: false,
                }])
            })
            .child(inner)
            .into_any_element()
    }
    fn render_pairing_guidance(&self) -> AnyElement {
        let status = self.hyperflux.pairing.status;
        if status
            .iter()
            .any(|s| matches!(s, Status::JustPaired | Status::Unpaired))
        {
            return div()
                .w(surface::css(520.))
                .h(surface::css(20.))
                .mx_auto()
                .mt(surface::css(20.))
                .text_center()
                .text_color(Colors::green())
                .child(message(if status.contains(&Status::Unpaired) {
                    "UNPAIRING_COMPLETE"
                } else {
                    "PAIRING_COMPLETE"
                }))
                .into_any_element();
        }
        if status.iter().any(|s| {
            matches!(
                s,
                Status::Loaded
                    | Status::Scanning
                    | Status::PairFailed
                    | Status::UnpairFailed
                    | Status::Scanned
            )
        }) {
            return h_flex()
                .items_center()
                .w(surface::css(520.))
                .min_h(surface::css(85.))
                .mx_auto()
                .mt(surface::css(20.))
                .px(surface::css(12.))
                .child(
                    img("synapse/hyperflux-207/icon_mousemat_ensure.svg")
                        .size(surface::css(60.))
                        .flex_shrink_0(),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .ml(surface::css(25.))
                        .text_color(Colors::secondary())
                        .children(
                            [
                                "HYPER_SPEED_MULTI_DEVICE_DONGLE_DUALLINK_NOTE_ONE",
                                "DUALLINK_NOTE_TWO",
                                "DUALLINK_NOTE_THREE",
                                "DUALLINK_NOTE_FOUR_MOUSE_MAT",
                                "DUALLINK_NOTE_FIVE",
                            ]
                            .into_iter()
                            .map(|key| div().child(format!("• {}", message(key)))),
                        ),
                )
                .into_any_element();
        }
        div().into_any_element()
    }
    fn render_pairing_content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.hyperflux.pairing.status[0] == Status::Loading {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child("loading")
                .into_any_element();
        }
        let mut dongle = v_flex().items_center().text_center().w(relative(0.5));
        if let Some(master) = self.pairing_master() {
            if let Some(image) = device_image(&master) {
                dongle = dongle.child(
                    img(image)
                        .size(surface::css(80.))
                        .mb(surface::css(10.))
                        .object_fit(ObjectFit::Contain),
                );
            }
            dongle = dongle.child(name(&master).to_uppercase());
        }
        let mut lines = h_flex()
            .w(surface::css(520.))
            .h(surface::css(30.))
            .mx_auto()
            .mt(surface::css(10.))
            .mb(surface::css(13.));
        for slot in 0..2 {
            if slot == 1 {
                lines = lines.child(
                    img("synapse/hyperflux-207/icon_marching_ants_master_line.svg")
                        .w(surface::css(4.))
                        .h(relative(0.58))
                        .self_start(),
                );
            }
            let connected = self.pairing_slave(slot)["connected"].as_u64().unwrap_or(0);
            lines = lines.child(pairing_line(slot, connected));
        }
        v_flex()
            .w_full()
            .bg(Colors::modal())
            .child(
                div()
                    .w(surface::css(790.))
                    .max_w_full()
                    .mx_auto()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .text_center()
                    .child(message("DUAL_LINK_TWO_DEVICES_DESCRIPTION")),
            )
            .child(
                div()
                    .w(surface::css(600.))
                    .mx_auto()
                    .child(
                        h_flex()
                            .justify_center()
                            .px(surface::css(10.))
                            .child(dongle),
                    )
                    .child(lines)
                    .child(
                        h_flex()
                            .justify_center()
                            .items_center()
                            .child(self.render_pairing_box(0, window, cx))
                            .child(
                                div()
                                    .ml(surface::css(20.))
                                    .child(self.render_pairing_box(1, window, cx)),
                            ),
                    ),
            )
            .child(self.render_pairing_guidance())
            .into_any_element()
    }
    pub(in crate::features::source_controls) fn render_hyperflux_pairing_modal(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.hyperflux.pairing.open {
            return div().into_any_element();
        }
        let size = window.viewport_size();
        let unit = window.rem_size() / 16.;
        // qM's fixed coordinates and 100vh belong to the product WebContents
        // below Electron's 42px TabUI, as for the current 179 pairing modal.
        let content_top = unit * 42.;
        let content_height = (size.height - content_top).max(px(0.));
        let preferred = unit * 850.;
        let width = if preferred < size.width {
            preferred
        } else {
            size.width
        };
        let header = h_flex()
            .relative()
            .w_full()
            .h(surface::css(36.))
            .flex_shrink_0()
            .justify_center()
            .bg(Colors::modal())
            .text_color(Colors::secondary())
            .border_b_1()
            .border_color(Colors::border())
            .font_family("RazerF5")
            .text_size(surface::css(16.))
            .line_height(surface::css(19.))
            .child(message("HYPER_SPEED_MULTI_DEVICE_PAIRING_UTILITY").to_uppercase())
            .child(
                surface::keymap_close_button("hyperflux-pairing-close", "CLOSE", window, cx)
                    .absolute()
                    .right_0()
                    .top_0()
                    .on_click(cx.listener(|this, _, _, cx| this.close_pairing(true, cx))),
            );
        let body = v_flex()
            .id("hyperflux-pairing-body")
            .flex_1()
            .min_h_0()
            .overflow_x_hidden()
            .scrollable_y()
            .child(
                div()
                    .pb(surface::css(150.))
                    .child(self.render_pairing_content(window, cx)),
            );
        let modal = v_flex()
            .w(width)
            .h(content_height)
            .rounded(surface::css(5.))
            .bg(Colors::modal())
            .overflow_hidden()
            .occlude()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::text())
            .child(header)
            .child(body);
        let mut root = div()
            .child(
                razer_assets::deferred_resource(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .position(point(px(0.), content_top))
                        .child(
                            div()
                                .w(size.width)
                                .h(content_height)
                                .bg(Colors::shade())
                                .occlude(),
                        ),
                )
                .priority(1000),
            )
            .child(
                razer_assets::deferred_resource(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .position(point((size.width - width) / 2., content_top + unit * 100.))
                        .child(modal),
                )
                .priority(1001),
            );
        if self.hyperflux.pairing.warning {
            let warning =
                v_flex()
                    .w(surface::css(400.))
                    .p(surface::css(20.))
                    .gap(surface::css(10.))
                    .border_1()
                    .border_color(Colors::warning())
                    .rounded(surface::css(3.))
                    .bg(Colors::panel())
                    .shadow(vec![BoxShadow {
                        color: Colors::black().opacity(0.2),
                        offset: point(px(0.), window.rem_size() / 16. * 6.),
                        blur_radius: window.rem_size() / 16. * 10.,
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .text_center()
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .text_color(Colors::text())
                    .child(
                        div()
                            .text_color(Colors::warning())
                            .child(message("DUAL_LINK_WARNING_HEADER").to_uppercase()),
                    )
                    .child(div().mb(surface::css(5.)).child(format!(
                        "{}\n\n{}\n\n{}",
                        message("DUAL_LINK_WARNING_DESCRIPTION_1"),
                        message("DUAL_LINK_WARNING_DESCRIPTION_2"),
                        message("DUAL_LINK_WARNING_CONFIRM")
                    )))
                    .child(
                        h_flex()
                            .justify_center()
                            .items_end()
                            .child(
                                action(
                                    "hyperflux-pairing-warning-continue",
                                    "CONTINUE_PAIRING",
                                    false,
                                )
                                .h_auto()
                                .min_w_0()
                                .max_w(surface::css(190.))
                                .mt(surface::css(10.))
                                .mr(surface::css(10.))
                                .px(surface::css(16.))
                                .py(surface::css(7.))
                                .line_height(surface::css(14.))
                                .opacity(1.)
                                .hover(|s| s.opacity(0.8))
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.confirm_pairing_warning(true, cx),
                                )),
                            )
                            .child(
                                action(
                                    "hyperflux-pairing-warning-cancel",
                                    "USE_KEYBOARD_DONGLE",
                                    true,
                                )
                                .h_auto()
                                .min_w_0()
                                .max_w(surface::css(190.))
                                .px(surface::css(16.))
                                .py(surface::css(7.))
                                .line_height(surface::css(14.))
                                .text_color(Colors::modal())
                                .hover(|s| s.opacity(0.8))
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.confirm_pairing_warning(false, cx),
                                )),
                            ),
                    );
            root = root.child(
                razer_assets::deferred_resource(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .position(point(px(0.), content_top + content_height * 0.035))
                        .child(
                            div()
                                .w(size.width)
                                .h(content_height)
                                .bg(Colors::warning_shade())
                                .occlude()
                                .overflow_hidden()
                                .flex()
                                .justify_center()
                                // DualLinkWarning's full-height inner flex
                                // wrapper has margin-top:-3.5%; CSS resolves
                                // that percentage against the containing width.
                                .child(h_flex().h_full().mt(-size.width * 0.035).child(warning)),
                        ),
                )
                .priority(2000),
            );
        }
        root.into_any_element()
    }
}

// Generated catalog reader: razer-pages/hyperflux_207_pairing_strings.
fn read_hyperflux_207_pairing_strings_catalog() -> BTreeMap<String, BTreeMap<String, String>> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_HYPERFLUX_207_PAIRING_STRINGS,
        0,
        "25f6e47ca020ed308b78c2a0c69ed7935963bd8ca72819108beea4306f3240c7",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod hyperflux_207_pairing_strings_catalog_tests {
    #[test]
    fn packaged_catalog_hyperflux_207_pairing_strings_decodes() {
        let _ = super::read_hyperflux_207_pairing_strings_catalog();
    }
}
// End generated catalog reader: razer-pages/hyperflux_207_pairing_strings.
