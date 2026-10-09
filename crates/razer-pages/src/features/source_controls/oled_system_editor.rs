//! Product 691's `CustomizeSystemInfo` editor.
//!
//! The current OLED chunk keeps this editor in a local draft and sends only
//! the committed system-information payload when Apply is pressed.  The
//! service owns telemetry values (CPU/GPU/memory/battery and clock text), so
//! this module edits the source-owned layout and display preferences only.
use super::*;
use gpui_kit::component::button::ButtonVariants;
use razer_i18n::t;
use razer_widgets::theme::OledColors;

const SLIDE_INTERVALS: [u8; 3] = [3, 5, 10];
const DATE_FORMATS: [&str; 3] = ["mm/dd/yyyy", "dd/mm/yyyy", "yyyy/mm/dd"];
const TIME_FORMATS: [&str; 2] = ["12H", "24H"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OledSystemTag {
    id: &'static str,
    label: &'static str,
    device_label: &'static str,
    type_id: u8,
}

impl OledSystemTag {
    fn value(self) -> Value {
        // `type` is the field consumed by the current MW OLED protocol.  The
        // labels are retained for the local preview and localized source UI.
        serde_json::json!({
            "id": self.id,
            "label": t(self.label),
            "deviceLabel": t(self.device_label),
            "deviceLabelKey": self.device_label,
            "showLabel": !matches!(self.id, "date" | "time"),
            "type": self.type_id,
        })
    }
}

/// `SYSTEM_INFO_TAGS_DEVICE_SPECIFIC` for product 691 (keyboard battery is
/// appended by the product module).  The source's preview SVGs are absent
/// from this repository's registered assets; the editor therefore uses a
/// text preview while preserving each source tag's protocol type.
const SYSTEM_TAGS: [OledSystemTag; 9] = [
    OledSystemTag {
        id: "cpuUsage",
        label: "OLED_CPU",
        device_label: "OLED_DEVICE_LABEL_CPU",
        type_id: 1,
    },
    OledSystemTag {
        id: "cpuTemp",
        label: "OLED_CPU_TEMP",
        device_label: "OLED_DEVICE_LABEL_CPU_TEMP",
        type_id: 2,
    },
    OledSystemTag {
        id: "gpuUsage",
        label: "OLED_GPU",
        device_label: "OLED_DEVICE_LABEL_GPU",
        type_id: 3,
    },
    OledSystemTag {
        id: "gpuTemp",
        label: "OLED_GPU_TEMP",
        device_label: "OLED_DEVICE_LABEL_GPU_TEMP",
        type_id: 4,
    },
    OledSystemTag {
        id: "memory",
        label: "OLED_MEMORY",
        device_label: "OLED_MEMORY",
        type_id: 5,
    },
    OledSystemTag {
        id: "date",
        label: "OLED_DATE",
        device_label: "OLED_DATE",
        type_id: 6,
    },
    OledSystemTag {
        id: "time",
        label: "OLED_TIME",
        device_label: "OLED_TIME",
        type_id: 7,
    },
    OledSystemTag {
        id: "laptopBattery",
        label: "OLED_LAPTOP_BATTERY",
        device_label: "OLED_DEVICE_LABEL_LAPTOP",
        type_id: 8,
    },
    OledSystemTag {
        id: "keyboardBattery",
        label: "OLED_KEYBOARD_BATTERY",
        device_label: "OLED_KEYBOARD_BATTERY",
        type_id: 9,
    },
];

#[derive(Clone, Copy, Debug)]
struct OledSystemSlide {
    left: Option<OledSystemTag>,
    right: Option<OledSystemTag>,
}

#[derive(Clone, Debug)]
struct OledSystemDraft {
    selected_slide: usize,
    time_between_slides: u8,
    temperature_unit: &'static str,
    time_format: usize,
    date_format: usize,
    slides: [OledSystemSlide; 3],
    /// Runtime telemetry is service-owned. Preserve an existing info payload
    /// in the device-owned OLED mirror while editing layout preferences.
    info: Value,
}

impl Default for OledSystemDraft {
    fn default() -> Self {
        Self {
            selected_slide: 0,
            time_between_slides: 3,
            temperature_unit: "celsius",
            time_format: 0,
            date_format: 0,
            slides: [
                OledSystemSlide {
                    left: Some(SYSTEM_TAGS[0]),
                    right: Some(SYSTEM_TAGS[1]),
                },
                OledSystemSlide {
                    left: Some(SYSTEM_TAGS[2]),
                    right: Some(SYSTEM_TAGS[3]),
                },
                OledSystemSlide {
                    left: Some(SYSTEM_TAGS[5]),
                    right: Some(SYSTEM_TAGS[6]),
                },
            ],
            info: serde_json::json!({
                "cpuUsage": {"label": "CPU", "value": "25%"},
                "cpuTemp": {"label": "CPU Temp", "value": "35°C"},
                "gpuUsage": {"label": "GPU", "value": "35%"},
                "gpuTemp": {"label": "GPU Temp", "value": "45°C"},
                "memory": {"label": "Memory", "value": "15%"},
                "date": {"label": "", "value": "07/15/2024"},
                "time": {"label": "", "value": "13:00"},
                "laptopBattery": {"label": "Laptop", "value": 100},
                "keyboardBattery": {"label": "Keyboard", "value": 100},
            }),
        }
    }
}

impl OledSystemDraft {
    fn from_value(value: &Value) -> Self {
        let mut draft = Self::default();
        let system = value;
        if !system.is_object() {
            return draft;
        }
        draft.selected_slide = system
            .get("selectedSlideIdx")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(2) as usize;
        draft.time_between_slides = system
            .get("timeBetweenSlides")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| SLIDE_INTERVALS.contains(value))
            .unwrap_or(3);
        draft.temperature_unit = match system.get("temperatureUnit").and_then(Value::as_str) {
            Some("fahrenheit") => "fahrenheit",
            _ => "celsius",
        };
        draft.time_format = system
            .get("timeFormat")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(1) as usize;
        draft.date_format = system
            .get("dateFormat")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(2) as usize;
        if let Some(slides) = system.get("slides").and_then(Value::as_array) {
            for (ix, slide) in slides.iter().take(3).enumerate() {
                draft.slides[ix] = OledSystemSlide {
                    left: tag_from_value(slide.get("left")),
                    right: tag_from_value(slide.get("right")),
                };
            }
        }
        if let Some(info) = system.get("info") {
            draft.info = info.clone();
        }
        draft
    }

    fn value(&self) -> Value {
        serde_json::json!({
            "timeBetweenSlides": self.time_between_slides,
            "selectedSlideIdx": self.selected_slide,
            "triggerHomeScreenSlide": false,
            "temperatureUnit": self.temperature_unit,
            "timeFormat": self.time_format,
            "dateFormat": self.date_format,
            "info": self.info.clone(),
            "slides": self.slides.iter().map(|slide| serde_json::json!({
                "left": slide.left.map(OledSystemTag::value),
                "right": slide.right.map(OledSystemTag::value),
            })).collect::<Vec<_>>(),
        })
    }

    fn assign(&mut self, side: bool, tag: Option<OledSystemTag>) {
        if side {
            self.slides[self.selected_slide].right = tag;
        } else {
            self.slides[self.selected_slide].left = tag;
        }
    }

    fn preview(&self) -> AnyElement {
        let slide = self.slides[self.selected_slide];
        let slot = |tag: Option<OledSystemTag>| {
            let Some(tag) = tag else {
                return div().flex_1().into_any_element();
            };
            v_flex()
                .flex_1()
                .items_center()
                .child(
                    div()
                        .text_size(surface::css(11.))
                        .child(t(tag.device_label)),
                )
                .child(
                    div()
                        .text_size(surface::css(14.))
                        .text_color(rgb(0x44d62c))
                        .child(match tag.id {
                            "cpuUsage" => "25%",
                            "cpuTemp" => "35°C",
                            "gpuUsage" => "35%",
                            "gpuTemp" => "45°C",
                            "memory" => "15%",
                            "date" => "07/15/2024",
                            "time" => "13:00",
                            "laptopBattery" | "keyboardBattery" => "100%",
                            _ => "--",
                        }),
                )
                .into_any_element()
        };
        div()
            .w(surface::css(236.))
            .h(surface::css(68.))
            .p(surface::css(2.))
            .bg(OledColors::screen())
            .border_1()
            .border_color(OledColors::border())
            .child(
                h_flex()
                    .w_full()
                    .h_full()
                    .items_center()
                    .child(slot(slide.left))
                    .child(slot(slide.right)),
            )
            .into_any_element()
    }
}

pub(super) fn system_preview(value: Option<&Value>) -> AnyElement {
    OledSystemDraft::from_value(value.unwrap_or(&Value::Null)).preview()
}

fn tag_from_value(value: Option<&Value>) -> Option<OledSystemTag> {
    let id = value
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)?;
    SYSTEM_TAGS.iter().copied().find(|tag| tag.id == id)
}

struct OledSystemEditor {
    draft: OledSystemDraft,
}

impl OledSystemEditor {
    fn new(value: Option<&Value>) -> Self {
        Self {
            draft: OledSystemDraft::from_value(value.unwrap_or(&Value::Null)),
        }
    }

    fn slot(&self, side: bool, cx: &mut Context<Self>) -> AnyElement {
        let slide = self.draft.slides[self.draft.selected_slide];
        let current = if side { slide.right } else { slide.left };
        let prefix = if side { "right" } else { "left" };
        let current_label = current.map_or_else(|| "—".to_owned(), |tag| t(tag.label));
        v_flex()
            .flex_1()
            .gap_1()
            .child(
                div()
                    .text_size(surface::css(13.))
                    .child(prefix.to_uppercase()),
            )
            .child(
                Button::new(SharedString::from(format!("oled-system-slot-{prefix}")))
                    .label(current_label)
                    .outline()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let slide = this.draft.slides[this.draft.selected_slide];
                        let current = if side { slide.right } else { slide.left };
                        let next = current
                            .and_then(|tag| {
                                let ix =
                                    SYSTEM_TAGS.iter().position(|candidate| *candidate == tag)?;
                                SYSTEM_TAGS.get((ix + 1) % SYSTEM_TAGS.len()).copied()
                            })
                            .or(Some(SYSTEM_TAGS[0]));
                        this.draft.assign(side, next);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("oled-system-clear-{prefix}")))
                    .label(t("CLEAR"))
                    .outline()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft.assign(side, None);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

impl Render for OledSystemEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.draft.selected_slide;
        let unit = self.draft.temperature_unit;
        let date_format = self.draft.date_format;
        let time_format = self.draft.time_format;
        let interval = self.draft.time_between_slides;
        v_flex()
            .w_full()
            .gap(surface::css(16.))
            .text_color(rgb(0xcccccc))
            .child(
                h_flex()
                    .items_end()
                    .gap(surface::css(20.))
                    .child(self.draft.preview())
                    .child(div().child(t("CUSTOMIZE_SYSTEM_INFO_TOOLTIP"))),
            )
            .child(h_flex().gap(surface::css(8.)).children((0..3).map(|ix| {
                Button::new(SharedString::from(format!("oled-system-slide-{ix}")))
                    .label(t(&format!("OLED_SLIDE_{}", ix + 1)))
                    .when(selected == ix, |button| button.primary())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft.selected_slide = ix;
                        cx.notify();
                    }))
            })))
            .child(
                h_flex()
                    .gap(surface::css(12.))
                    .child(self.slot(false, cx))
                    .child(self.slot(true, cx)),
            )
            .child(
                v_flex()
                    .gap(surface::css(8.))
                    .child(div().child(t("CUSTOMIZE_SYSTEM_INFO_TIME_BETWEEN_SLIDE_LABEL")))
                    .child(h_flex().gap(surface::css(8.)).children(
                        SLIDE_INTERVALS.into_iter().map(|value| {
                            Button::new(SharedString::from(format!("oled-system-interval-{value}")))
                                .label(format!("{value}s"))
                                .when(interval == value, |button| button.primary())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.draft.time_between_slides = value;
                                    cx.notify();
                                }))
                        }),
                    ))
                    .child(surface::note(
                        t("CUSTOMIZE_SYSTEM_INFO_TIME_BETWEEN_SLIDE_DESC"),
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .gap(surface::css(8.))
                    .child(div().child(t("CUSTOMIZE_SYSTEM_INFO_TEMPERATURE_LABEL")))
                    .child(h_flex().gap(surface::css(8.)).children(
                        ["celsius", "fahrenheit"].into_iter().map(|value| {
                            Button::new(SharedString::from(format!(
                                "oled-system-temperature-{value}"
                            )))
                            .label(if value == "celsius" { "°C" } else { "°F" })
                            .when(unit == value, |button| button.primary())
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.draft.temperature_unit = value;
                                    cx.notify();
                                },
                            ))
                        }),
                    ))
                    .child(surface::note(
                        t("CUSTOMIZE_SYSTEM_INFO_TEMPERATURE_DESC"),
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .gap(surface::css(24.))
                    .child(
                        v_flex()
                            .gap(surface::css(8.))
                            .child(div().child(t("CUSTOMIZE_SYSTEM_INFO_DATE_FORMAT_LABEL")))
                            .child(h_flex().gap(surface::css(6.)).children(
                                DATE_FORMATS.into_iter().enumerate().map(|(ix, value)| {
                                    Button::new(SharedString::from(format!(
                                        "oled-system-date-{ix}"
                                    )))
                                    .label(value)
                                    .when(date_format == ix, |button| button.primary())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.draft.date_format = ix;
                                        cx.notify();
                                    }))
                                }),
                            )),
                    )
                    .child(
                        v_flex()
                            .gap(surface::css(8.))
                            .child(div().child(t("CUSTOMIZE_SYSTEM_INFO_TIME_FORMAT_LABEL")))
                            .child(h_flex().gap(surface::css(6.)).children(
                                TIME_FORMATS.into_iter().enumerate().map(|(ix, value)| {
                                    Button::new(SharedString::from(format!(
                                        "oled-system-time-{ix}"
                                    )))
                                    .label(value)
                                    .when(time_format == ix, |button| button.primary())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.draft.time_format = ix;
                                        cx.notify();
                                    }))
                                }),
                            )),
                    ),
            )
    }
}

impl SourceControls {
    /// Opens the source-faithful system-information editor.  The editor is
    /// staged locally and only writes `/oled/system` after Apply.
    pub(super) fn open_oled_system(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_ble
            || self.page != "OLED"
            || self.draft.pointer("/oled/homeScreenDisplay/enabled") != Some(&Value::Bool(true))
        {
            return;
        }
        let editor = cx.new(|_| OledSystemEditor::new(self.draft.pointer("/oled/system")));
        let editor_for_apply = editor.clone();
        let parent = cx.weak_entity();
        let width = surface::css(850.).to_pixels(window.rem_size());
        window.open_dialog(cx, move |dialog, _, _| {
            let editor_for_apply = editor_for_apply.clone();
            let parent = parent.clone();
            dialog
                .title(t("CUSTOMIZE_MODAL_SYSTEM_INFO_TITLE"))
                .width(width)
                .child(editor.clone())
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("oled-system-cancel")
                                .label(t("CANCEL"))
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("oled-system-apply")
                                .label(t("APPLY"))
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let value = editor_for_apply.read(cx).draft.value();
                                    let _ = parent.update(cx, |this, cx| {
                                        if !this.is_ble
                                            && this.page == "OLED"
                                            && this.draft.pointer("/oled/homeScreenDisplay/enabled")
                                                == Some(&Value::Bool(true))
                                        {
                                            this.draft["oled"]["system"] = value;
                                            cx.emit(SourceControlsChanged);
                                            cx.notify();
                                        }
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
}
