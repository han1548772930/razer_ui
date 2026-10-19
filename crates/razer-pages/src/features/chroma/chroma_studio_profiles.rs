//! Current Studio 8480 profile commands, 7759 local projection producer and
//! 2502 unsaved-change choice. Persisted content belongs to StudioSession.
use super::*;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use studio_dropdown::StudioDropdown;

impl ChromaStudio {
    pub fn set_profiles(
        &mut self,
        profiles: Vec<StudioProfileItem>,
        selected: Option<String>,
        pending: bool,
        cx: &mut Context<Self>,
    ) {
        self.profiles = profiles;
        self.selected_profile = selected;
        self.profiles_pending = pending;
        cx.notify();
    }
    pub fn next_profile_name(&self) -> String {
        self.profile_name(None)
    }
    /// The current storage listener dispatches the same selection command as
    /// the profile menu, including its unsaved-change choice (main/8480 O/B).
    pub fn select_external_profile(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_profile(StudioProfileCommand::Select { id }, window, cx);
    }
    pub fn has_unsaved_changes(&self) -> bool {
        self.document != self.saved
    }
    fn profile_name(&self, base: Option<&str>) -> String {
        let names = self
            .profiles
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<Vec<_>>();
        if let Some(base) = base.filter(|base| !base.is_empty()) {
            if !names.contains(&base) {
                return base.into();
            }
            let base = studio_layers::suffix_base(base);
            for number in 1..u64::MAX {
                let name = format!("{base} ({number})");
                if !names.contains(&name.as_str()) {
                    return name;
                }
            }
        } else {
            for number in 1..u64::MAX {
                let name = format!("{} {number}", label("TEXT_CHROMA_EFFECT"));
                if !names.contains(&name.as_str()) {
                    return name;
                }
            }
        }
        unreachable!("finite Studio profile inventory")
    }
    fn request_profile(
        &mut self,
        command: StudioProfileCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.profiles_pending {
            return;
        }
        if matches!(&command,StudioProfileCommand::Select {id} if self.selected_profile.as_ref()==Some(id))
        {
            return;
        }
        if self.has_unsaved_changes()
            && matches!(
                command,
                StudioProfileCommand::Add { .. }
                    | StudioProfileCommand::Select { .. }
                    | StudioProfileCommand::Duplicate { .. }
            )
        {
            let owner = cx.entity().downgrade();
            window.open_dialog(cx, move |dialog, _, _| {
                let save_owner = owner.clone();
                let discard_owner = owner.clone();
                let save_command = command.clone();
                let discard_command = command.clone();
                dialog
                    .title(razer_assets::resource_scope(label(
                        "SAVE_CHROMA_EFFECT_TITLE",
                    )))
                    .overlay_closable(true)
                    .child(razer_assets::resource_scope(
                        v_flex()
                            .gap(surface::css(14.))
                            .child(label("SAVE_CHROMA_EFFECT_DESC_1"))
                            .child(label("SAVE_CHROMA_EFFECT_DESC_2")),
                    ))
                    .footer(razer_assets::resource_scope(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                Button::new("studio-profile-save-switch")
                                    .label(label("SAVE"))
                                    .on_click(move |_, window, cx| {
                                        if let Some(owner) = save_owner.upgrade() {
                                            owner.update(cx, |page, cx| {
                                                cx.emit(ChromaStudioEvent::Save(page.snapshot()));
                                                cx.emit(ChromaStudioEvent::Profile(
                                                    save_command.clone(),
                                                ));
                                            });
                                        }
                                        window.close_dialog(cx);
                                    }),
                            )
                            .child(
                                Button::new("studio-profile-discard-switch")
                                    .label(label("DONT_SAVE"))
                                    .on_click(move |_, window, cx| {
                                        if let Some(owner) = discard_owner.upgrade() {
                                            owner.update(cx, |_, cx| {
                                                cx.emit(ChromaStudioEvent::Profile(
                                                    discard_command.clone(),
                                                ))
                                            });
                                        }
                                        window.close_dialog(cx);
                                    }),
                            ),
                    ))
            });
        } else {
            cx.emit(ChromaStudioEvent::Profile(command));
        }
    }
    fn rename_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == self.selected_profile.as_ref())
        else {
            return;
        };
        let id = profile.id.clone();
        let name = profile.name.clone();
        let input = cx.new(|cx| {
            let mut input = source_input::input_state(32, window, cx);
            source_input::set_value(&mut input, name, 32, window, cx);
            input
        });
        self.profile_rename_subscription = Some(cx.subscribe(&input, |page, _, event, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                let Some((id, input)) = page.profile_rename.take() else {
                    return;
                };
                page.profile_rename_subscription = None;
                let value = input.read(cx).value();
                let name = studio_layers::renamed_title(
                    &value,
                    page.profiles.iter().map(|profile| profile.name.as_str()),
                );
                if let Some(name) = name.map(str::to_owned) {
                    cx.emit(ChromaStudioEvent::Profile(StudioProfileCommand::Rename {
                        id,
                        name,
                    }));
                }
                cx.notify();
            }
        }));
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.profile_rename = Some((id, input));
        cx.notify();
    }
    pub(super) fn cancel_profile_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_rename.take().is_none() {
            return;
        }
        self.profile_rename_subscription = None;
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn profile_header(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut list = v_flex();
        for (position, profile) in self.profiles.iter().enumerate() {
            let id = profile.id.clone();
            let menu = self.profile_dropdown.clone();
            list = list.child(
                button(
                    ElementId::Name(format!("studio-profile-{}", profile.id).into()),
                    "CHANGE_EFFECT",
                )
                .w_full()
                .px(surface::css(10.))
                .py(surface::css(5.))
                .justify_start()
                .when(
                    Some(&profile.id) == self.selected_profile.as_ref(),
                    |button| button.text_color(Colors::selected()),
                )
                .child(format!("{}. {}", position + 1, profile.name))
                .on_click(cx.listener(move |page, _, window, cx| {
                    menu.update(cx, |menu, cx| menu.set_open(false, cx));
                    page.request_profile(
                        StudioProfileCommand::Select { id: id.clone() },
                        window,
                        cx,
                    );
                })),
            );
        }
        let selected = self
            .profiles
            .iter()
            .enumerate()
            .find(|(_, profile)| Some(&profile.id) == self.selected_profile.as_ref());
        let title = selected
            .map(|(position, profile)| format!("{}. {}", position + 1, profile.name))
            .unwrap_or_default();
        let selector = if let Some((_, input)) = &self.profile_rename {
            div()
                .w(surface::css(180.))
                .capture_action(cx.listener(
                    |this, _: &gpui_kit::component::input::Escape, window, cx| {
                        this.cancel_profile_rename(window, cx);
                        cx.stop_propagation();
                    },
                ))
                .child(
                    Input::new(input)
                        .id("studio-profile-rename-input")
                        .source_max_length(input, 32),
                )
                .into_any_element()
        } else {
            StudioDropdown::new(
                "studio-profiles",
                &self.profile_dropdown,
                !self.profiles_pending && self.profiles.len() > 1,
                button("studio-profiles-trigger", "CHANGE_EFFECT")
                    .w(surface::css(180.))
                    .h(surface::css(27.))
                    .bg(Colors::black())
                    .border_1()
                    .border_color(Colors::input_border())
                    .child(div().flex_1().truncate().child(title))
                    .child(studio_dropdown::arrow(
                        self.profile_dropdown.read(cx).is_open(),
                        "studio-profiles-arrow".into(),
                        window,
                        cx,
                    )),
                list.into_any_element(),
                180.,
                0.,
            )
            .into_any_element()
        };
        let mut actions = v_flex();
        if self.profile_delete_confirm {
            let menu = self.profile_actions.clone();
            let id = self.selected_profile.clone();
            actions = actions
                .p(surface::css(10.))
                .gap(surface::css(10.))
                .child(label("DELETE_CHROMA_EFFECT"))
                .child(label("DELETE_CHROMA_EFFECT_DESC"))
                .child(
                    button("studio-profile-delete-confirm", "DELETE")
                        .child(label("DELETE"))
                        .on_click(cx.listener(move |page, _, window, cx| {
                            page.profile_delete_confirm = false;
                            menu.update(cx, |menu, cx| menu.set_open(false, cx));
                            if let Some(id) = &id {
                                page.request_profile(
                                    StudioProfileCommand::Delete { id: id.clone() },
                                    window,
                                    cx,
                                );
                            }
                        })),
                );
        } else {
            for key in ["ADD", "RENAME", "DUPLICATE", "DELETE"] {
                let menu = self.profile_actions.clone();
                let disabled = self.profiles_pending
                    || (key != "ADD" && self.selected_profile.is_none())
                    || (key == "DELETE" && self.profiles.len() <= 1);
                actions = actions.child(
                    button(
                        ElementId::Name(format!("studio-profile-action-{key}").into()),
                        key,
                    )
                    .w_full()
                    .px(surface::css(10.))
                    .py(surface::css(5.))
                    .justify_start()
                    .disabled(disabled)
                    .child(label(key))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        if key == "DELETE" {
                            page.profile_delete_confirm = true;
                            cx.notify();
                            return;
                        }
                        menu.update(cx, |menu, cx| menu.set_open(false, cx));
                        if key == "RENAME" {
                            page.rename_profile(window, cx);
                            return;
                        }
                        let command = if key == "ADD" {
                            Some(StudioProfileCommand::Add {
                                name: page.profile_name(None),
                            })
                        } else {
                            page.selected_profile
                                .as_ref()
                                .and_then(|id| {
                                    page.profiles.iter().find(|profile| profile.id == *id)
                                })
                                .map(|profile| StudioProfileCommand::Duplicate {
                                    id: profile.id.clone(),
                                    name: page.profile_name(Some(&profile.name)),
                                })
                        };
                        if let Some(command) = command {
                            page.request_profile(command, window, cx);
                        }
                    })),
                );
            }
        }
        h_flex()
            .gap(surface::css(10.))
            .child(selector)
            .child(StudioDropdown::new(
                "studio-profile-actions",
                &self.profile_actions,
                !self.profiles_pending,
                button("studio-profile-actions-trigger", "CHANGE_EFFECT")
                    .w(surface::css(27.))
                    .h(surface::css(27.))
                    .bg(Colors::black())
                    .border_1()
                    .border_color(Colors::input_border())
                    .child("…"),
                actions.into_any_element(),
                170.,
                0.,
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod rename_window_tests {
    use super::super::{
        ChromaStudio, ChromaStudioEvent, ChromaStudioSnapshot, StudioProfileCommand,
        StudioProfileItem,
    };
    use gpui_kit::base::Dialog;
    use gpui_kit::component::Root;
    use gpui_kit::prelude::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext as _, Context, Entity, FocusHandle, IntoElement, Render, Subscription,
        TestAppContext, Window, WindowHandle, div, px, size,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        time::Duration,
    };

    // The production Studio window has no Dialog ancestor. This fixture checks
    // the actual Kit action consumer that runs before raw key-down handlers.
    struct DialogHost {
        studio: Entity<ChromaStudio>,
        cancelled: Rc<Cell<usize>>,
        focus: FocusHandle,
    }
    impl Render for DialogHost {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let cancelled = self.cancelled.clone();
            Dialog::new(cx)
                .focus_handle(self.focus.clone())
                .on_cancel(move |_, _, _| {
                    cancelled.set(cancelled.get() + 1);
                    false
                })
                .popup(div().size_full().child(self.studio.clone()))
        }
    }

    struct RenameWindow {
        handle: WindowHandle<Root>,
        studio: Entity<ChromaStudio>,
        cancelled: Rc<Cell<usize>>,
        events: Rc<RefCell<Vec<ChromaStudioEvent>>>,
        initial: ChromaStudioSnapshot,
        _subscription: Subscription,
    }
    impl RenameWindow {
        fn new(cx: &mut TestAppContext) -> Self {
            cx.update(|cx| {
                gpui_kit::init(cx);
                cx.set_reduce_motion(true);
            });
            let cancelled = Rc::new(Cell::new(0));
            let mut initial = ChromaStudioSnapshot::default_profile().unwrap();
            initial.document.layers[0].title =
                "Source layer name beyond thirty-two UTF16 units".into();
            let mut retained = None;
            let handle = cx.open_window(size(px(1280.), px(800.)), |window, cx| {
                let studio = cx.new(ChromaStudio::new);
                studio.update(cx, |studio, cx| {
                    studio.restore(initial.clone(), cx);
                    studio.set_profiles(
                        vec![StudioProfileItem {
                            id: "source-profile".into(),
                            name: "Source profile name beyond thirty-two UTF16 units".into(),
                        }],
                        Some("source-profile".into()),
                        false,
                        cx,
                    );
                    studio.focus(window, cx);
                });
                retained = Some(studio.clone());
                let host = cx.new(|cx| DialogHost {
                    studio,
                    cancelled: cancelled.clone(),
                    focus: cx.focus_handle(),
                });
                Root::new(host, window, cx)
            });
            let studio = retained.unwrap();
            let events = Rc::new(RefCell::new(Vec::new()));
            let recorded = events.clone();
            let subscription = cx.update(|cx| {
                cx.subscribe(&studio, move |_, event: &ChromaStudioEvent, _| {
                    recorded.borrow_mut().push(event.clone());
                })
            });
            Self {
                handle,
                studio,
                cancelled,
                events,
                initial,
                _subscription: subscription,
            }
        }
        fn open_profile_rename(&self, cx: &mut TestAppContext) {
            cx.update_window(self.handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.click("studio-profile-actions-trigger", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.run_until_parked();
            cx.update_window(self.handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.click("studio-profile-action-RENAME", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.run_until_parked();
        }
    }

    #[gpui_kit::test]
    fn studio_profile_escape_cancels_before_parent_dialog_and_later_enter_submits_once(
        cx: &mut TestAppContext,
    ) {
        let scene = RenameWindow::new(cx);
        scene.open_profile_rename(cx);
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let input = window.find("studio-profile-rename-input");
            assert_eq!(input.focused(), Some(true));
            assert_eq!(
                input.value(),
                Some("Source profile name beyond thirty-two UTF16 units")
            );
            window.input("Discarded profile draft", cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("studio-profile-rename-input").is_none());
            let studio = scene.studio.read(cx);
            assert!(studio.profile_rename.is_none());
            assert!(studio.profile_rename_subscription.is_none());
            assert_eq!(
                studio.profiles[0].name,
                "Source profile name beyond thirty-two UTF16 units"
            );
            assert_eq!(studio.snapshot(), scene.initial);
            assert!(!studio.has_unsaved_changes());
            assert!(studio.focus.is_focused(window));
        })
        .unwrap();
        assert_eq!(scene.cancelled.get(), 0);
        assert!(scene.events.borrow().is_empty());

        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
        assert_eq!(scene.cancelled.get(), 1);

        scene.open_profile_rename(cx);
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.input("  Accepted profile  ", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("studio-profile-rename-input").is_none());
            window.click(("studio-layer", 1_u64), cx);
        })
        .unwrap();
        cx.run_until_parked();
        let events = scene.events.borrow();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            ChromaStudioEvent::Profile(StudioProfileCommand::Rename { id, name })
                if id == "source-profile" && name == "Accepted profile"
        ));
        assert_eq!(scene.cancelled.get(), 1);
    }

    #[gpui_kit::test]
    fn studio_layer_escape_cancels_before_parent_dialog_and_later_enter_changes_only_draft(
        cx: &mut TestAppContext,
    ) {
        let scene = RenameWindow::new(cx);
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(("studio-layer", 1_u64), cx);
            window.press("ctrl-e", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let input = window.find(("studio-layer-rename-input", 1_u64));
            assert_eq!(input.focused(), Some(true));
            assert_eq!(
                input.value(),
                Some("Source layer name beyond thirty-two UTF16 units")
            );
            window.input("Discarded layer draft", cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .try_find(("studio-layer-rename-input", 1_u64))
                    .is_none()
            );
            let studio = scene.studio.read(cx);
            assert!(studio.renaming.is_none());
            assert!(studio.rename_subscription.is_none());
            assert_eq!(studio.snapshot(), scene.initial);
            assert!(!studio.has_unsaved_changes());
            assert!(studio.past.is_empty());
            assert!(studio.focus.is_focused(window));
        })
        .unwrap();
        assert_eq!(scene.cancelled.get(), 0);
        assert!(scene.events.borrow().is_empty());

        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
        assert_eq!(scene.cancelled.get(), 1);

        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.press("ctrl-e", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.input("  Accepted layer  ", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(scene.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .try_find(("studio-layer-rename-input", 1_u64))
                    .is_none()
            );
            window.click("studio-profile-actions-trigger", cx);
            let studio = scene.studio.read(cx);
            assert_eq!(studio.document.layers[0].title, "Accepted layer");
            assert_eq!(studio.saved, *scene.initial.document());
            assert_eq!(studio.past.len(), 1);
            assert!(studio.has_unsaved_changes());
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(scene.cancelled.get(), 1);
        assert!(scene.events.borrow().is_empty());
    }
}
