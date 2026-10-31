//! Window-owned keyboard guide from the shared `rz-accessibility` runtime.
use crate::{surface::css, theme::KeyboardGuideColors as Colors};
use gpui_kit::base::{
    Root, RootPlugin, TestSupportExt as _,
    motion::{self, Easing, Transition},
};
use gpui_kit::*;
use razer_state::ui_preferences::{
    InitialTabFinished, InitialTabObservation, InitialTabRequest, UiPreferences,
};
use std::{collections::HashSet, time::Duration};

mod source;
pub use source::{DocumentNavigation, HostPage, ProductId, SourcePage};
mod document;
pub use document::{DocumentScope, document_scope};

mod focus;
pub use focus::{
    FocusBehavior, FocusRing, FocusTarget, focus_adjacent_to, focus_input, focus_target,
    using_keyboard,
};
mod button;
pub use button::Button;

const SHORTCUTS: [(&str, &str); 9] = [
    (
        "TAB_SHORTCUTS_MOVE_FOCUS_FORWARD",
        "TAB_SHORTCUTS_MOVE_FOCUS_FORWARD_KEYS",
    ),
    (
        "TAB_SHORTCUTS_MOVE_FOCUS_BACKWARD",
        "TAB_SHORTCUTS_MOVE_FOCUS_BACKWARD_KEYS",
    ),
    (
        "TAB_SHORTCUTS_CLOSE_CURRENT_TAB",
        "TAB_SHORTCUTS_CLOSE_CURRENT_TAB_KEYS",
    ),
    (
        "TAB_SHORTCUTS_REOPEN_LAST_CLOSED",
        "TAB_SHORTCUTS_REOPEN_LAST_CLOSED_KEYS",
    ),
    (
        "TAB_SHORTCUTS_JUMP_TO_SPECIFIC",
        "TAB_SHORTCUTS_JUMP_TO_SPECIFIC_KEYS",
    ),
    (
        "TAB_SHORTCUTS_JUMP_NEXT_TAB",
        "TAB_SHORTCUTS_JUMP_NEXT_TAB_KEYS",
    ),
    (
        "TAB_SHORTCUTS_JUMP_PREVIOUS_TAB",
        "TAB_SHORTCUTS_JUMP_PREVIOUS_TAB_KEYS",
    ),
    (
        "TAB_SHORTCUTS_MOVE_TABS_LEFT",
        "TAB_SHORTCUTS_MOVE_TABS_LEFT_KEYS",
    ),
    (
        "TAB_SHORTCUTS_MOVE_TABS_RIGHT",
        "TAB_SHORTCUTS_MOVE_TABS_RIGHT_KEYS",
    ),
];

fn text(key: &str) -> String {
    razer_i18n::t_locale(
        &format!("KEYBOARD_GUIDE.{key}"),
        guide_language(&razer_i18n::locale()),
    )
}

fn guide_language(language: &str) -> &'static str {
    for supported in [
        "en", "de", "es", "fr", "ja", "kr", "pt-BR", "ru", "zh-CN", "zh-TW",
    ] {
        if language == supported {
            return supported;
        }
    }
    let language = language.to_ascii_lowercase();
    for (prefix, supported) in [
        ("de", "de"),
        ("es", "es"),
        ("fr", "fr"),
        ("ja", "ja"),
        ("ko", "kr"),
        ("pt", "pt-BR"),
    ] {
        if language.starts_with(prefix) {
            return supported;
        }
    }
    if let Some(chinese) = language.strip_prefix("zh") {
        return if ["tw", "hant", "hk", "mo"]
            .iter()
            .any(|part| chinese.contains(part))
        {
            "zh-TW"
        } else {
            "zh-CN"
        };
    }
    if language.starts_with("ru") { "ru" } else { "en" }
}

#[derive(Default)]
struct GuideState {
    open: bool,
    generation: u64,
    pending_tab: Option<InitialTabRequest>,
}
impl GuideState {
    fn open(&mut self) -> Option<u64> {
        if self.open {
            return None;
        }
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Guide generation exhausted");
        self.open = true;
        Some(self.generation)
    }
    fn close(&mut self) {
        self.open = false;
        self.pending_tab = None;
    }
    fn accepts_focus(&self, generation: u64) -> bool {
        self.open && self.generation == generation
    }
    fn observe_tab(
        &mut self,
        request: InitialTabRequest,
        result: &Result<InitialTabObservation, String>,
    ) -> bool {
        if self.pending_tab != Some(request) {
            return false;
        }
        self.pending_tab = None;
        matches!(result, Ok(InitialTabObservation::First { .. }))
    }
}

struct KeyboardNavigation {
    guide: GuideState,
    dialog_focus: FocusHandle,
    close_focus: FocusHandle,
    preferences: Entity<UiPreferences>,
    next_tab: u64,
    keyboard: HashSet<ElementId>,
    pending_document: Option<document::Document>,
    guide_document: Option<document::Document>,
    close_hovered: bool,
    focus_frame: focus::SharedFrame,
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn init(cx: &mut App) {
    Root::register_plugin::<KeyboardNavigation>(cx, KeyboardNavigation::new);
}

/// Open the same guide in any registered product, settings or independent window.
pub fn open_guide(window: &mut Window, cx: &mut App) {
    let document = focus::active_document(window, cx)
        .filter(|document| document.page.navigation().installed());
    let owner = Root::read(window, cx)
        .plugin::<KeyboardNavigation>()
        .expect("Keyboard navigation must be initialized before opening windows");
    owner.update(cx, |owner, cx| {
        if let Some(document) = document {
            owner.open(document, window, cx);
        }
    });
}

impl KeyboardNavigation {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferences = UiPreferences::shared(cx);
        let window_id = window.window_handle().window_id();
        let weak = cx.weak_entity();
        let keys = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() != window_id {
                return;
            }
            let _ = weak.update(cx, |owner, cx| {
                owner.keystroke(&event.keystroke, window, cx)
            });
        });
        let initial = cx.subscribe_in(
            &preferences,
            window,
            |owner, _, event: &InitialTabFinished, window, cx| {
                if owner.guide.observe_tab(event.request(), event.result()) {
                    if let Some(document) = owner.pending_document.take().filter(|document| {
                        focus::document_is_mounted_in(document, &owner.focus_frame)
                    }) {
                        owner.open(document, window, cx);
                    }
                } else if owner.guide.pending_tab.is_none() {
                    owner.pending_document = None;
                }
            },
        );
        Self {
            guide: GuideState::default(),
            dialog_focus: cx.focus_handle(),
            close_focus: cx.focus_handle().tab_stop(true),
            preferences,
            next_tab: 0,
            keyboard: HashSet::new(),
            pending_document: None,
            guide_document: None,
            close_hovered: false,
            focus_frame: Default::default(),
            _subscriptions: vec![keys, initial],
        }
    }

    fn keystroke(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        if key.key == "escape" && self.guide.open {
            self.close(window, cx);
            cx.stop_propagation();
            return;
        }
        let Some(document) = focus::active_document_in(&self.focus_frame, window, cx)
            .filter(|document| document.page.navigation().installed())
        else {
            return;
        };
        if matches!(
            key.key.as_str(),
            "tab" | "up" | "down" | "left" | "right" | "home" | "end" | "enter" | "space"
        ) && self.keyboard.insert(document.id.clone())
        {
            cx.notify();
            window.refresh();
        }
        if key.key == "tab"
            && document.page.navigation().initial_guide()
            && self.guide.pending_tab.is_none()
        {
            self.next_tab = self
                .next_tab
                .checked_add(1)
                .expect("Initial Tab request sequence exhausted");
            let request = InitialTabRequest::new(cx.entity_id(), self.next_tab);
            if self.preferences.update(cx, |preferences, cx| {
                preferences.record_initial_tab(request, cx)
            }) {
                self.guide.pending_tab = Some(request);
                self.pending_document = Some(document);
            }
        }
    }

    fn open(&mut self, document: document::Document, window: &mut Window, cx: &mut Context<Self>) {
        let Some(generation) = self.guide.open() else {
            return;
        };
        self.guide_document = Some(document);
        self.close_hovered = false;
        let weak = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = weak.update(cx, |owner, cx| {
                if owner.guide.accepts_focus(generation) {
                    owner.close_focus.focus(window, cx);
                }
            });
        });
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.guide.close();
        self.guide_document = None;
        self.pending_document = None;
        self.close_hovered = false;
        // Source St removes the focused dialog; it neither traps Tab nor
        // restores the trigger. Base Dialog's different focus policy is not used.
        if self.dialog_focus.contains_focused(window, cx) || self.close_focus.is_focused(window) {
            window.blur(cx);
        }
        cx.notify();
    }

    fn prune_documents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .guide_document
            .as_ref()
            .is_some_and(|document| !focus::document_is_mounted_in(document, &self.focus_frame))
        {
            self.close(window, cx);
        } else if self
            .pending_document
            .as_ref()
            .is_some_and(|document| !focus::document_is_mounted_in(document, &self.focus_frame))
        {
            self.guide.pending_tab = None;
            self.pending_document = None;
        }
    }
}

impl RootPlugin for KeyboardNavigation {
    fn decorate(
        &self,
        surface: AnyElement,
        root: &Root,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        let owner = root
            .plugin::<Self>()
            .expect("Retained keyboard navigation owner");
        focus::scope(
            div()
                .size_full()
                .capture_any_mouse_down(move |event, window, cx| {
                    let document = focus::document_at(event.position, window, cx);
                    owner.update(cx, |owner, cx| {
                        if let Some(document) = document
                            && owner.keyboard.remove(&document.id)
                        {
                            cx.notify();
                            window.refresh();
                        }
                    });
                })
                .on_key_down(focus::navigate)
                .child(surface),
            self.focus_frame.clone(),
        )
    }
}

impl Render for KeyboardNavigation {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.guide.open {
            return Empty.into_any_element();
        }
        let Some(document) = self.guide_document.clone() else {
            return Empty.into_any_element();
        };
        let opacity = motion::transition(
            "keyboard-guide-close-opacity",
            if self.close_hovered { 0.8 } else { 1. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let close_text = text("CLOSE").to_uppercase();
        let close = Button::new("keyboard-guide-close")
            .track_focus(&self.close_focus)
            .accessibility_label(close_text.clone())
            .flex()
            .items_center()
            .justify_center()
            .min_w(css(90.))
            .px(css(16.))
            .pt(css(7.))
            .pb(css(6.))
            .border_1()
            .border_color(Colors::button_border())
            .rounded(css(3.))
            .bg(Colors::accent())
            .text_color(Colors::button_text())
            .font_family("Roboto")
            .text_size(css(12.))
            .line_height(css(14.))
            .cursor_pointer()
            .opacity(opacity)
            .on_hover(cx.listener(|owner, hovered, _, cx| {
                owner.close_hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|owner, _, window, cx| owner.close(window, cx)))
            .child(close_text);
        let panel = div()
            .id("keyboard-guide-dialog")
            .test_support()
            .role(Role::Dialog)
            .aria_label(text("TAB_SHORTCUTS_TITLE"))
            .track_focus(&self.dialog_focus)
            .occlude()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .gap(css(30.))
            .w(css(600.))
            .max_w_full()
            .p(css(30.))
            .border_1()
            .border_color(Colors::accent())
            .rounded(css(3.))
            .bg(Colors::surface())
            .shadow(vec![BoxShadow {
                color: Colors::shadow(),
                offset: point(px(0.), window.rem_size() * (6. / 16.)),
                blur_radius: window.rem_size() * (10. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w_full()
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .text_color(Colors::accent())
                    .child(text("TAB_SHORTCUTS_TITLE").to_uppercase()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .border_t_1()
                    .border_color(Colors::border())
                    .children(SHORTCUTS.map(|(action, keys)| {
                        let cell = || {
                            div()
                                .flex()
                                .items_center()
                                .p(css(15.))
                                .border_b_1()
                                .border_color(Colors::border())
                                .font_family("Roboto")
                                .text_size(css(14.))
                                .line_height(css(17.))
                        };
                        div()
                            .id(action)
                            .flex()
                            .items_stretch()
                            .w_full()
                            .child(
                                cell()
                                    .w(css(260.))
                                    .flex_shrink_0()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Colors::action())
                                    .child(text(action)),
                            )
                            .child(
                                cell()
                                    .flex_grow(1.)
                                    .flex_shrink_0()
                                    .flex_basis(relative(0.))
                                    .min_w_0()
                                    .border_l_1()
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(Colors::keys())
                                    .child(text(keys)),
                            )
                    })),
            )
            .child(close);
        document::overlay(
            document,
            razer_assets::deferred_resource(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("keyboard-guide-backdrop")
                        .test_support()
                        .absolute()
                        .w(window.viewport_size().width)
                        .h(window.viewport_size().height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .occlude()
                        .bg(Colors::backdrop())
                        .on_click(cx.listener(|owner, _, window, cx| owner.close(window, cx)))
                        .child(panel),
                ),
            )
            .with_priority(9999),
        )
        .into_any_element()
    }
}

#[cfg(test)]
#[path = "keyboard_navigation/tests.rs"]
mod tests;
