// Included by mapping_editor.rs. Current 182/653 MapMacro and local library adapter.
use crate::features::macro_library::{EntryKind, MacroLibraryFile, MacroType};
use crate::ui::stepper::{Stepper, StepperEvent};

#[derive(Deserialize)]
struct MacroPlaybackChoice {
    id: String,
    content: String,
}
#[derive(Deserialize)]
struct MacroPlaybackSets {
    standard: Vec<MacroPlaybackChoice>,
    wheel: Vec<MacroPlaybackChoice>,
    sequence: Vec<MacroPlaybackChoice>,
    phased: Vec<MacroPlaybackChoice>,
}
#[derive(Deserialize)]
struct ProductMacroCapability {
    product_id: u32,
    wheel_inputs: Vec<String>,
    no_held_inputs: Vec<String>,
    input_aliases: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct ProductMacroData {
    playback: MacroPlaybackSets,
    products: Vec<ProductMacroCapability>,
}
fn product_macro_data() -> &'static ProductMacroData {
    static DATA: std::sync::OnceLock<ProductMacroData> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("mapping_macro_data.json"))
            .expect("audited product Macro playback data")
    })
}

#[derive(Clone)]
struct MappingMacroEntry {
    id: u64,
    name: String,
    macro_type: MacroType,
}

pub(super) struct MacroMappingState {
    catalog: Vec<MappingMacroEntry>,
    selector: Entity<SelectState<Vec<Choice>>>,
    playback: Entity<SelectState<Vec<Choice>>>,
    repeat: Entity<Stepper>,
    repeat_value: f64,
}

impl MacroMappingState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<DeviceWorkspace>) -> Self {
        Self {
            catalog: Vec::new(),
            selector: cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx)),
            playback: cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx)),
            repeat: cx.new(|cx| {
                Stepper::new(
                    "mapping-macro-repeat",
                    2.,
                    (1., 99., 1.),
                    false,
                    false,
                    Some(2),
                    window,
                    cx,
                )
                .in_custom_keymapping()
                .live_update()
            }),
            repeat_value: 2.,
        }
    }
}

// Current shared comparator 3466/eh: stable first-character rank ascending,
// second-character rank descending, then UTF-16 comparison. No locale guessing.
fn mapping_macro_name_order(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let rank = |value: Option<u16>| {
        value
            .and_then(|value| {
                "*!@_.()#^&%-=+01234567989abcdefghijklmnopqrstuvwxyz"
                    .encode_utf16()
                    .position(|candidate| candidate == value)
            })
            .map_or(-1, |value| value as i32)
    };
    let (mut aa, mut bb) = (a.encode_utf16(), b.encode_utf16());
    let (a0, a1, b0, b1) = (
        rank(aa.next()),
        rank(aa.next()),
        rank(bb.next()),
        rank(bb.next()),
    );
    if a0 == b0 {
        if a1 == b1 || a1 == -1 || b1 == -1 {
            a.encode_utf16().cmp(b.encode_utf16())
        } else {
            b1.cmp(&a1)
        }
    } else if a0 == -1 || b0 == -1 {
        a.encode_utf16().cmp(b.encode_utf16())
    } else {
        a0.cmp(&b0)
    }
}

impl DeviceWorkspace {
    pub(crate) fn set_mapping_macro_library(
        &mut self,
        file: &MacroLibraryFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mapping_macro.catalog = file
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Macro)
            .map(|entry| MappingMacroEntry {
                id: entry.id,
                name: entry.name.clone(),
                macro_type: entry.macro_type,
            })
            .collect();
        self.mapping_macro.catalog.sort_by_key(|entry| entry.id);
        self.mapping_macro
            .catalog
            .sort_by(|a, b| mapping_macro_name_order(&a.name, &b.name));
        if let Some(mut action @ Assignment::Macro { macro_id: 0, .. }) = self.mapping_action() {
            self.initialize_macro_assignment(&mut action);
            self.update_mapping(cx, |current| *current = action);
        }
        // Existing nonzero references survive rename/delete/type changes.
        // Catalog observation never commits or silently retargets a binding.
        self.sync_mapping_macro_controls(window, cx);
        cx.notify();
    }

    fn mapping_macro_choices(&self, kind: MacroType) -> Vec<&'static MacroPlaybackChoice> {
        let data = product_macro_data();
        let Some(capability) = data
            .products
            .iter()
            .find(|spec| spec.product_id == self.pid())
        else {
            return vec![];
        };
        let Some(draft) = &self.mapping else {
            return vec![];
        };
        let input = capability
            .input_aliases
            .get(&draft.input)
            .unwrap_or(&draft.input);
        let choices = match kind {
            MacroType::Sequence => &data.playback.sequence,
            MacroType::Phased => &data.playback.phased,
            MacroType::Standard if capability.wheel_inputs.contains(input) => &data.playback.wheel,
            MacroType::Standard => &data.playback.standard,
        };
        choices
            .iter()
            .filter(|choice| {
                !capability.no_held_inputs.contains(input) || choice.id != "ContinuousHeld"
            })
            .collect()
    }

    fn initialize_macro_assignment(&self, action: &mut Assignment) {
        if let Some(entry) = self.mapping_macro.catalog.first()
            && let Some(choice) = self.mapping_macro_choices(entry.macro_type).first()
        {
            *action = Assignment::Macro {
                macro_id: entry.id,
                name: entry.name.clone(),
                playback: choice.id.clone(),
                repeat_count: 2,
            };
        }
    }

    fn mapping_macro_error(&self, action: &Assignment) -> Option<&'static str> {
        let Assignment::Macro {
            macro_id,
            playback,
            repeat_count,
            ..
        } = action
        else {
            return None;
        };
        let Some(entry) = self
            .mapping_macro
            .catalog
            .iter()
            .find(|entry| entry.id == *macro_id)
        else {
            return Some(if *macro_id == 0 {
                "请先创建或选择一个本地宏。"
            } else {
                "引用的本地宏已不存在，请重新选择。"
            });
        };
        if entry.name.trim().is_empty() {
            return Some("所选宏尚无有效名称，请在宏编辑器中命名。");
        }
        if !self
            .mapping_macro_choices(entry.macro_type)
            .iter()
            .any(|choice| choice.id == *playback)
        {
            return Some("此宏类型或输入不支持已存播放方式，请重新选择。");
        }
        if playback == "NTimes" && !(1..=99).contains(repeat_count) {
            return Some("播放次数须为 1–99 的整数。");
        }
        if playback != "NTimes" && *repeat_count != 2 {
            return Some("请重新选择播放方式以恢复有效次数。");
        }
        None
    }

    fn select_mapping_macro(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self
            .mapping_macro
            .catalog
            .iter()
            .find(|entry| entry.id.to_string() == id)
            .cloned()
        else {
            return;
        };
        if !matches!(self.mapping_action(), Some(Assignment::Macro { .. })) {
            return;
        }
        let choices = self.mapping_macro_choices(entry.macro_type);
        let Some(first) = choices.first() else { return };
        self.update_mapping(cx, |action| {
            if let Assignment::Macro {
                macro_id,
                name,
                playback,
                repeat_count,
            } = action
            {
                *macro_id = entry.id;
                *name = entry.name;
                if !choices.iter().any(|choice| choice.id == *playback) {
                    *playback = first.id.clone();
                }
                if playback != "NTimes" || !(1..=99).contains(repeat_count) {
                    *repeat_count = 2;
                }
            }
        });
        self.reset_mapping_macro_repeat(window, cx);
        self.sync_mapping_macro_controls(window, cx);
    }

    fn install_mapping_macro_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.mapping_macro.selector,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.select_mapping_macro(id, window, cx);
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.mapping_macro.playback,
            window,
            |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(option)) = event else {
                    return;
                };
                let Some(Assignment::Macro {
                    macro_id,
                    playback,
                    repeat_count,
                    ..
                }) = this.mapping_action()
                else {
                    return;
                };
                if playback == *option
                    && if option == "NTimes" {
                        (1..=99).contains(&repeat_count)
                    } else {
                        repeat_count == 2
                    }
                {
                    return;
                }
                let Some(entry) = this
                    .mapping_macro
                    .catalog
                    .iter()
                    .find(|entry| entry.id == macro_id)
                else {
                    return;
                };
                if !this
                    .mapping_macro_choices(entry.macro_type)
                    .iter()
                    .any(|choice| choice.id == *option)
                {
                    return;
                }
                this.update_mapping(cx, |action| {
                    if let Assignment::Macro {
                        playback,
                        repeat_count,
                        ..
                    } = action
                    {
                        *playback = option.clone();
                        *repeat_count = 2;
                    }
                });
                this.reset_mapping_macro_repeat(window, cx);
                this.sync_mapping_macro_controls(window, cx);
            },
        ));
        self.subscriptions.push(cx.subscribe(
            &self.mapping_macro.repeat,
            |this, _, event: &StepperEvent, cx| {
                let Some(Assignment::Macro {
                    macro_id, playback, ..
                }) = this.mapping_action()
                else {
                    return;
                };
                if playback != "NTimes"
                    || !this
                        .mapping_macro
                        .catalog
                        .iter()
                        .any(|entry| entry.id == macro_id)
                {
                    return;
                }
                let draft = event
                    .draft
                    .clone()
                    .unwrap_or_else(|| event.value.to_string());
                let value = draft.parse::<u32>().unwrap_or(0);
                this.mapping_macro.repeat_value = event.value;
                this.update_mapping(cx, |action| {
                    if let Assignment::Macro { repeat_count, .. } = action {
                        *repeat_count = value;
                    }
                });
                cx.notify();
            },
        ));
    }

    fn sync_mapping_macro_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Assignment::Macro {
            macro_id,
            playback,
            repeat_count,
            ..
        }) = self.mapping_action()
        else {
            self.mapping_macro.repeat.update(cx, |stepper, cx| {
                stepper.sync_value(self.mapping_macro.repeat_value, 1., true, window, cx);
            });
            return;
        };
        let mut items: Vec<Choice> = self
            .mapping_macro
            .catalog
            .iter()
            .map(|entry| Choice::new(entry.id.to_string(), entry.name.clone()))
            .collect();
        let entry = self
            .mapping_macro
            .catalog
            .iter()
            .find(|entry| entry.id == macro_id);
        if items.is_empty() {
            items.push(Choice::new("empty", " "));
        }
        let selected = if macro_id == 0 {
            "empty".into()
        } else {
            macro_id.to_string()
        };
        self.mapping_macro.selector.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_value(&selected, window, cx);
        });
        let choices =
            self.mapping_macro_choices(entry.map_or(MacroType::Standard, |entry| entry.macro_type));
        self.mapping_macro.playback.update(cx, |state, cx| {
            state.set_items(
                choices
                    .iter()
                    .map(|choice| Choice::new(choice.id.clone(), i18n::t(&choice.content)))
                    .collect(),
                window,
                cx,
            );
            state.set_selected_value(&playback, window, cx);
        });
        self.mapping_macro.repeat.update(cx, |stepper, cx| {
            stepper.sync_value(
                if (1..=99).contains(&repeat_count) {
                    repeat_count as f64
                } else {
                    self.mapping_macro.repeat_value
                },
                1.,
                entry.is_none() || playback != "NTimes",
                window,
                cx,
            );
        });
    }

    fn reset_mapping_macro_repeat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mapping_macro.repeat_value = match self.mapping_action() {
            Some(Assignment::Macro { repeat_count, .. }) if (1..=99).contains(&repeat_count) => {
                repeat_count as f64
            }
            _ => 2.,
        };
        self.mapping_macro.repeat.update(cx, |stepper, cx| {
            stepper.reset_value(self.mapping_macro.repeat_value, window, cx);
        });
    }

    fn render_mapping_macro(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Assignment::Macro {
            macro_id,
            playback,
            repeat_count,
            ..
        }) = self.mapping_action()
        else {
            return div().into_any_element();
        };
        let entry = self
            .mapping_macro
            .catalog
            .iter()
            .find(|entry| entry.id == macro_id);
        let available = !self.mapping_macro.catalog.is_empty();
        let choices =
            self.mapping_macro_choices(entry.map_or(MacroType::Standard, |entry| entry.macro_type));
        v_flex()
            .id("mapping-macro")
            .test_support()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                surface::select(&self.mapping_macro.selector)
                    .id("mapping-macro-selector")
                    .accessibility_label(i18n::t("MACRO"))
                    .w_full()
                    .disabled(
                        !available || self.mapping_macro.catalog.len() == 1 && entry.is_some(),
                    ),
            )
            .child(
                div()
                    .mb(surface::css(5.))
                    .opacity(if available { 1. } else { 0.3 })
                    .child(i18n::t("PLAYBACK_OPTION")),
            )
            .child(
                surface::select(&self.mapping_macro.playback)
                    .id("mapping-macro-playback")
                    .accessibility_label(i18n::t("PLAYBACK_OPTION"))
                    .w_full()
                    .disabled(
                        entry.is_none()
                            || choices.len() == 1 && choices[0].id == playback && repeat_count == 2,
                    ),
            )
            .when(playback == "NTimes", |view| {
                view.child(div().mb(surface::css(5.)).child(i18n::t("NUMBER_OF_TIMES")))
                    .child(self.mapping_macro.repeat.clone())
            })
            .when(!available, |view| {
                view.child(
                    div()
                        .mb(surface::css(20.))
                        .child(i18n::t("CREATE_MACRO_MSG")),
                )
            })
            .child(
                BaseButton::new("mapping-configure-macros")
                    .accessibility_label(i18n::t("CONFIGURE_MACROS"))
                    .text_left()
                    .underline()
                    .mb(surface::css(20.))
                    .child(i18n::t("CONFIGURE_MACROS"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkspaceEvent::OpenMacro))),
            )
            .into_any_element()
    }
}
