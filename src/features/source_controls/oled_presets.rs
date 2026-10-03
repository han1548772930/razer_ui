//! Local preset selection follows OLED wt/Vt. The dialog owns an isolated
//! draft; only Apply changes the device-owned OLED settings.
use super::*;
use crate::{i18n::t, ui::theme::OledColors};
use gpui_kit::component::button::ButtonVariants;
use serde::Serialize;

#[derive(Clone, Copy)]
enum PresetKind {
    Animation,
    Image,
}
impl PresetKind {
    fn key(self) -> &'static str {
        match self {
            Self::Animation => "animation",
            Self::Image => "image",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Animation => "OLED_HOME_SCREEN_DISPLAY_TITLE_ANIMATION",
            Self::Image => "OLED_HOME_SCREEN_DISPLAY_TITLE_IMAGE",
        }
    }
    fn asset(self, ix: usize) -> SharedString {
        let ext = match self {
            Self::Animation => "webp",
            Self::Image => "png",
        };
        format!("synapse/oled-home-{}-{}.{ext}", self.key(), ix + 1).into()
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Preset {
    id: String,
    custom: bool,
    enabled: bool,
}
#[derive(Clone, Deserialize, Serialize)]
struct PresetSelection {
    #[serde(rename = "selectedIdx")]
    selected_ix: usize,
    list: Vec<Preset>,
}
impl PresetSelection {
    fn normalize(&mut self) {
        // Older local drafts may contain no enabled item or an invalid index.
        if !self.list.iter().any(|item| item.enabled) {
            if let Some(first) = self.list.first_mut() {
                first.enabled = true;
            }
        }
        if !self.list.get(self.selected_ix).is_some_and(|p| p.enabled) {
            self.selected_ix = self.list.iter().position(|p| p.enabled).unwrap_or(0);
        }
    }
    fn set_enabled(&mut self, ix: usize, enabled: bool) {
        if !enabled && self.list.iter().filter(|p| p.enabled).count() == 1 {
            return;
        }
        if let Some(item) = self.list.get_mut(ix) {
            item.enabled = enabled;
            self.normalize();
        }
    }
}

struct PresetEditor {
    kind: PresetKind,
    selection: PresetSelection,
}
impl Render for PresetEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let last_enabled = self.selection.list.iter().filter(|p| p.enabled).count() == 1;
        h_flex()
            .w_full()
            .items_start()
            .flex_wrap()
            .gap(surface::css(20.))
            .children(self.selection.list.iter().enumerate().map(|(ix, preset)| {
                let selected = self.selection.selected_ix == ix;
                let enabled = preset.enabled;
                let id = preset.id.clone();
                v_flex()
                    .w(surface::css(236.))
                    .gap_1()
                    .child(
                        gpui_kit::base::Button::new(SharedString::from(format!("select-{id}")))
                            .accessibility_label(format!("{} {}", t(self.kind.title()), ix + 1))
                            .disabled(!enabled)
                            .w(surface::css(236.))
                            .h(surface::css(68.))
                            .border_2()
                            .border_color(if selected {
                                cx.theme().primary
                            } else {
                                OledColors::border()
                            })
                            .bg(OledColors::screen())
                            .hover(|s| s.border_color(cx.theme().primary))
                            .focus_visible(|s| s.border_color(cx.theme().primary))
                            .child(
                                img(self.kind.asset(ix))
                                    .w(surface::css(232.))
                                    .h(surface::css(64.))
                                    .object_fit(ObjectFit::Contain)
                                    .when(!enabled, |image| image.opacity(0.1)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.selection.list[ix].enabled {
                                    this.selection.selected_ix = ix;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        surface::SynapseSwitch::new(SharedString::from(format!("enable-{id}")))
                            .label(format!("{} {}", t(self.kind.title()), ix + 1))
                            .checked(enabled)
                            .disabled(last_enabled && enabled)
                            .on_change(cx.listener(move |this, enabled, _, cx| {
                                this.selection.set_enabled(ix, *enabled);
                                cx.notify();
                            })),
                    )
            }))
    }
}

impl SourceControls {
    fn preset_selection(&self, kind: PresetKind) -> PresetSelection {
        let mut selection: PresetSelection =
            serde_json::from_value(self.draft["oled"][kind.key()].clone())
                .expect("validated OLED preset selection");
        selection.normalize();
        selection
    }

    pub(super) fn render_oled_presets(&self, disabled: bool, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .gap(surface::css(10.))
            .flex_wrap()
            .children(
                [PresetKind::Animation, PresetKind::Image]
                    .into_iter()
                    .map(|kind| {
                        let selection = self.preset_selection(kind);
                        v_flex()
                            .w(surface::css(236.))
                            .gap_1()
                            .when(disabled, |view| view.opacity(0.2))
                            .child(div().text_size(surface::css(14.)).child(t(kind.title())))
                            .child(
                                img(kind.asset(selection.selected_ix))
                                    .w(surface::css(236.))
                                    .h(surface::css(68.))
                                    .bg(OledColors::screen())
                                    .border_2()
                                    .border_color(OledColors::border())
                                    .object_fit(ObjectFit::Contain),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "oled-edit-{}",
                                    kind.key()
                                )))
                                .label(t("EDIT"))
                                .outline()
                                .disabled(disabled || self.is_ble)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.open_oled_presets(kind, window, cx);
                                    },
                                )),
                            )
                    }),
            )
            .into_any_element()
    }

    fn open_oled_presets(&mut self, kind: PresetKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_ble
            || self.draft.pointer("/oled/homeScreenDisplay/enabled") != Some(&Value::Bool(true))
        {
            return;
        }
        let selection = self.preset_selection(kind);
        let editor = cx.new(|_| PresetEditor { kind, selection });
        let parent = cx.weak_entity();
        let width = surface::css(800.).to_pixels(window.rem_size());
        window.open_dialog(cx, move |dialog, _, _| {
            let editor_for_apply = editor.clone();
            let parent = parent.clone();
            dialog
                .title(t(kind.title()))
                .width(width)
                .child(editor.clone())
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("oled-preset-cancel")
                                .label(t("CANCEL"))
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("oled-preset-apply")
                                .label(t("APPLY"))
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let selection = editor_for_apply.read(cx).selection.clone();
                                    let _ = parent.update(cx, |this, cx| {
                                        if !this.is_ble
                                            && this.page == "OLED"
                                            && this.draft.pointer("/oled/homeScreenDisplay/enabled")
                                                == Some(&Value::Bool(true))
                                        {
                                            this.draft["oled"][kind.key()] =
                                                serde_json::to_value(selection)
                                                    .expect("OLED selection serializes");
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

#[cfg(test)]
mod tests {
    use super::{Preset, PresetSelection};

    fn selection() -> PresetSelection {
        PresetSelection {
            selected_ix: 1,
            list: (0..3)
                .map(|ix| Preset {
                    id: format!("image-{}", ix + 1),
                    custom: false,
                    enabled: true,
                })
                .collect(),
        }
    }

    #[test]
    fn disabling_selected_preset_moves_to_first_enabled() {
        let mut draft = selection();
        draft.set_enabled(1, false);
        assert_eq!(draft.selected_ix, 0);
        draft.set_enabled(0, false);
        assert_eq!(draft.selected_ix, 2);
        draft.set_enabled(2, false);
        assert!(draft.list[2].enabled);
        assert_eq!(draft.selected_ix, 2);
    }

    #[test]
    fn editing_clone_does_not_change_committed_selection() {
        let committed = selection();
        let mut draft = committed.clone();
        draft.set_enabled(1, false);
        assert_eq!(committed.selected_ix, 1);
        assert!(committed.list.iter().all(|p| p.enabled));
    }

    #[test]
    fn malformed_saved_selection_recovers_enabled_item() {
        let mut draft = selection();
        draft.selected_ix = usize::MAX;
        draft.list.iter_mut().for_each(|p| p.enabled = false);
        draft.normalize();
        assert_eq!(draft.selected_ix, 0);
        assert!(draft.list[0].enabled);
        draft.set_enabled(2, true);
        assert_eq!(draft.selected_ix, 0);
        assert_eq!(serde_json::to_value(&draft).unwrap()["selectedIdx"], 0);
    }
}
