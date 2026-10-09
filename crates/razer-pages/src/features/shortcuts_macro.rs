// Current Dashboard 82508/k. The picker consumes the shared local library;
// local document identities are never passed off as native service GUIDs.
#[derive(Clone)]
struct MacroCatalogItem {
    id: u64,
    name: String,
    macro_type: super::macro_library::MacroType,
}

// 3466/p: first-character rank is ascending; second-character rank descending.
// The 9 before 8 and repeated 9 in this literal belong to the source.
fn macro_name_order(a: &str, b: &str) -> std::cmp::Ordering {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let rank = |value: Option<u16>| {
        value
            .and_then(|value| {
                "*!@_.()#^&%-=+01234567989abcdefghijklmnopqrstuvwxyz"
                    .encode_utf16()
                    .position(|candidate| candidate == value)
            })
            .map_or(-1, |value| value as i32)
    };
    let mut aa = a.encode_utf16();
    let mut bb = b.encode_utf16();
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

fn macro_playback_choices(
    kind: super::macro_library::MacroType,
) -> &'static [SourceShortcutChoice] {
    use super::macro_library::MacroType;
    match kind {
        MacroType::Standard => &source_shortcuts().macro_playback,
        MacroType::Sequence => &source_shortcuts().macro_sequence_playback,
        MacroType::Phased => &source_shortcuts().macro_phased_playback,
    }
}

impl Shortcuts {
    pub fn set_macro_library(
        &mut self,
        file: &super::macro_library::MacroLibraryFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = self
            .draft
            .as_ref()
            .and_then(|draft| match &draft.value.output {
                ShortcutOutput::Macro { macro_id, .. } => Some(*macro_id),
                _ => None,
            });
        self.macro_catalog = file
            .entries
            .iter()
            .filter(|entry| entry.kind == super::macro_library::EntryKind::Macro)
            .map(|entry| MacroCatalogItem {
                id: entry.id,
                name: entry.name.clone(),
                macro_type: entry.macro_type,
            })
            .collect();
        // Establish source creation order before its stable name comparator.
        self.macro_catalog.sort_by_key(|entry| entry.id);
        self.macro_catalog
            .sort_by(|a, b| macro_name_order(&a.name, &b.name));
        if selected == Some(0) {
            // Source selects the first result when an empty library becomes
            // available. A deleted nonzero identity must still be repaired
            // explicitly instead of silently binding a different document.
            self.choose_default_macro();
        } else if selected.is_some() {
            let current = self
                .macro_catalog
                .iter()
                .find(|item| Some(item.id) == selected)
                .cloned();
            if let Some(draft) = &mut self.draft {
                if let ShortcutOutput::Macro {
                    name,
                    playback,
                    repeat_count,
                    ..
                } = &mut draft.value.output
                {
                    // A removed document keeps its identity until the user
                    // explicitly chooses another; never persist a retarget.
                    if let Some(item) = current {
                        *name = item.name;
                        let choices = macro_playback_choices(item.macro_type);
                        if !choices.iter().any(|choice| choice.id == *playback) {
                            *playback = choices[0].id.clone();
                            *repeat_count = 2;
                        }
                    }
                }
            }
        }
        self.sync_macro_controls(window, cx);
        cx.notify();
    }

    fn subscribe_macro_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.macro_selector,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    let Some(item) = this
                        .macro_catalog
                        .iter()
                        .find(|item| item.id.to_string() == *id)
                        .cloned()
                    else {
                        return;
                    };
                    if let Some(draft) = &mut this.draft {
                        if let ShortcutOutput::Macro {
                            macro_id,
                            name,
                            playback,
                            repeat_count,
                        } = &mut draft.value.output
                        {
                            *macro_id = item.id;
                            *name = item.name;
                            let choices = macro_playback_choices(item.macro_type);
                            if !choices.iter().any(|choice| choice.id == *playback) {
                                *playback = choices[0].id.clone();
                                *repeat_count = 2;
                            }
                        }
                    }
                    this.sync_macro_controls(window, cx);
                    this.changed(cx);
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.playback,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(option)) = event {
                    if !this
                        .selected_macro_playback()
                        .iter()
                        .any(|item| item.id == *option)
                    {
                        return;
                    }
                    if let Some(draft) = &mut this.draft {
                        if let ShortcutOutput::Macro {
                            playback,
                            repeat_count,
                            ..
                        } = &mut draft.value.output
                        {
                            *playback = option.clone();
                            *repeat_count = if option == "NTimes" {
                                this.macro_repeat_draft.parse().unwrap_or(2)
                            } else {
                                2
                            };
                        }
                    }
                    this.sync_macro_controls(window, cx);
                    this.changed(cx);
                }
            },
        ));
        self.subscriptions.push(cx.subscribe(
            &self.macro_repeat,
            |this, _, event: &razer_widgets::stepper::StepperEvent, cx| {
                this.macro_repeat_draft = event
                    .draft
                    .clone()
                    .unwrap_or_else(|| event.value.to_string());
                if let Ok(value) = this.macro_repeat_draft.parse::<u8>() {
                    if (1..=99).contains(&value) {
                        if let Some(draft) = &mut this.draft {
                            if let ShortcutOutput::Macro {
                                playback,
                                repeat_count,
                                ..
                            } = &mut draft.value.output
                            {
                                if playback == "NTimes" {
                                    *repeat_count = value;
                                }
                            }
                        }
                    }
                }
                this.changed(cx);
            },
        ));
    }

    fn choose_default_macro(&mut self) {
        if let Some(draft) = &mut self.draft {
            if let ShortcutOutput::Macro {
                macro_id,
                name,
                playback,
                repeat_count,
            } = &mut draft.value.output
            {
                if let Some(item) = self.macro_catalog.first() {
                    *macro_id = item.id;
                    *name = item.name.clone();
                    *playback = macro_playback_choices(item.macro_type)[0].id.clone();
                    *repeat_count = 2;
                }
            }
        }
    }

    fn sync_macro_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let mut items = self
            .macro_catalog
            .iter()
            .map(|item| Choice::new(item.id.to_string(), item.name.clone()))
            .collect::<Vec<_>>();
        if items.is_empty() {
            items.push(Choice::new("empty", " "));
        }
        let (id, playback) = match self.draft.as_ref().map(|draft| &draft.value.output) {
            Some(ShortcutOutput::Macro {
                macro_id, playback, ..
            }) => (macro_id.to_string(), playback.clone()),
            _ => ("empty".into(), "Once".into()),
        };
        self.macro_selector.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_value(&id, window, cx);
        });
        let choices = self
            .selected_macro_playback()
            .iter()
            .map(|choice| Choice::new(choice.id.clone(), i18n::t(&choice.content)))
            .collect();
        self.playback.update(cx, |state, cx| {
            state.set_items(choices, window, cx);
            state.set_selected_value(&playback, window, cx);
        });
        if let Ok(value) = self.macro_repeat_draft.parse::<f64>() {
            self.macro_repeat.update(cx, |stepper, cx| {
                stepper.sync_value(value, 1., false, window, cx)
            });
        }
    }

    fn selected_macro_playback(&self) -> &'static [SourceShortcutChoice] {
        let id = match self.draft.as_ref().map(|draft| &draft.value.output) {
            Some(ShortcutOutput::Macro { macro_id, .. }) => *macro_id,
            _ => 0,
        };
        macro_playback_choices(
            self.macro_catalog
                .iter()
                .find(|item| item.id == id)
                .map_or(super::macro_library::MacroType::Standard, |item| {
                    item.macro_type
                }),
        )
    }

    fn output_label(&self, output: &ShortcutOutput) -> String {
        if let ShortcutOutput::Macro { macro_id, name, .. } = output {
            return self
                .macro_catalog
                .iter()
                .find(|item| item.id == *macro_id)
                .map_or_else(|| name.clone(), |item| item.name.clone());
        }
        output.label()
    }

    fn macro_function(&self, cx: &mut Context<Self>) -> AnyElement {
        let available = !self.macro_catalog.is_empty();
        let valid_selection = matches!(self.draft.as_ref().map(|draft| &draft.value.output),
            Some(ShortcutOutput::Macro { macro_id, .. })
                if self.macro_catalog.iter().any(|item| item.id == *macro_id));
        let repeat = matches!(self.draft.as_ref().map(|draft| &draft.value.output),
            Some(ShortcutOutput::Macro { playback, .. }) if playback == "NTimes");
        v_flex()
            .child(
                surface::select(&self.macro_selector)
                    .id("shortcut-macro")
                    .w_full()
                    // 3844 disables a single choice. Keep it actionable when
                    // repairing a deleted reference to that remaining choice.
                    .disabled(!available || self.macro_catalog.len() == 1 && valid_selection),
            )
            .child(
                div()
                    .mb(surface::css(5.))
                    .opacity(if available { 1. } else { 0.3 })
                    .child(i18n::t("PLAYBACK_OPTION")),
            )
            .child(
                surface::select(&self.playback)
                    .id("shortcut-macro-playback")
                    .w_full()
                    .disabled(!available || self.selected_macro_playback().len() <= 1),
            )
            .when(repeat, |view| {
                view.child(div().mb(surface::css(5.)).child(i18n::t("NUMBER_OF_TIMES")))
                    .child(self.macro_repeat.clone())
            })
            .when(!available, |view| {
                view.child(
                    div()
                        .mb(surface::css(20.))
                        .child(i18n::t("CREATE_MACRO_MSG")),
                )
            })
            .child(
                ShortcutButton::new("shortcut-configure-macros")
                    .text_left()
                    .mb(surface::css(20.))
                    .underline()
                    .child(i18n::t("CONFIGURE_MACROS"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ShortcutsOpenModule::Macro))),
            )
            .into_any_element()
    }
}
