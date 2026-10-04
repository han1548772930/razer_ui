//! Current Macro 21700 U/M/I, with product-owned physical input catalogs.
//! Associations belong to this MacroPage session; no service GUID, hardware
//! mapping, profile activation or recorder state is synthesized by this view.
use super::{palette::BindingColors as Colors, *};
use crate::{
    features::{
        Choice, ProductWorkspace,
        macro_inputs::{self, MacroInput, MacroInputLayout},
    },
    model::{Device, DeviceCategory},
    resources,
    ui::{
        keyboard_geometry::KeyRegion,
        stepper::{Stepper, StepperEvent},
        surface,
    },
};
use gpui_kit::component::select::{SelectEvent, SelectItem, SelectState};

#[derive(Clone)]
pub(super) struct LocalBinding {
    macro_id: u64,
    device: String,
    profile: String,
    input: String,
    hypershift: bool,
    playback: String,
    repeat_count: u32,
}
impl LocalBinding {
    fn key(&self) -> String {
        serde_json::to_string(&(&self.device, &self.profile, &self.input, self.hypershift))
            .expect("local binding identity")
    }
    fn same_target(&self, other: &Self) -> bool {
        self.device == other.device
            && self.profile == other.profile
            && self.input == other.input
            && self.hypershift == other.hypershift
    }
}

#[derive(Clone)]
struct BindingEvent(LocalBinding);

fn local_text(zh: &'static str, en: &'static str) -> &'static str {
    if i18n::locale().to_lowercase().starts_with("zh") {
        zh
    } else {
        en
    }
}
fn session_note() -> &'static str {
    local_text(
        "仅保存在此会话，尚未写入设备。",
        "Saved for this session; not written to the device.",
    )
}
fn device_name(device: &Device) -> String {
    device.name.get(&i18n::locale().to_lowercase()).to_string()
}
fn profile_choices(device: &Device) -> Vec<Choice> {
    device
        .profiles
        .iter()
        .map(|profile| Choice::new(&profile.id, &profile.name))
        .collect()
}
fn eligible(device: &Device) -> bool {
    matches!(
        device.category,
        DeviceCategory::Mouse | DeviceCategory::Keyboard | DeviceCategory::Keypad
    ) || device.product_id == 3907
        || macro_inputs::for_device(device).is_some()
}

impl MacroPage {
    pub(in crate::shell) fn set_devices(
        &mut self,
        devices: Vec<Entity<ProductWorkspace>>,
        cx: &mut Context<Self>,
    ) {
        // Replacing this bounded vector drops obsolete observers on refresh.
        self.device_subscriptions = devices
            .iter()
            .map(|device| cx.observe(device, |_, _, cx| cx.notify()))
            .collect();
        self.devices = devices;
        if let Some(dialog) = &self.binding_dialog {
            dialog.update(cx, |dialog, cx| {
                dialog.set_devices(self.devices.clone(), cx)
            });
        }
        cx.notify();
    }

    fn open_bindings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(macro_id) = self.current else { return };
        if self.devices.is_empty() {
            return;
        }
        let dialog = cx.new(|cx| {
            BindingDialog::new(
                macro_id,
                self.selected_name(),
                self.devices.clone(),
                self.local_bindings.clone(),
                window,
                cx,
            )
        });
        self.binding_subscription = Some(cx.subscribe(
            &dialog,
            |this, dialog, event: &BindingEvent, cx| {
                let binding = &event.0;
                if !this
                    .entries
                    .iter()
                    .any(|entry| entry.id == binding.macro_id && entry.kind == EntryKind::Macro)
                {
                    return;
                }
                // A physical input has one local assignment per device/profile/layer.
                this.local_bindings
                    .retain(|other| !binding.same_target(other));
                this.local_bindings.push(binding.clone());
                dialog.update(cx, |dialog, cx| {
                    dialog.bindings = this.local_bindings.clone();
                    cx.notify();
                });
                cx.notify();
            },
        ));
        self.binding_dialog = Some(dialog);
        self.binding_menu = None;
        cx.notify();
    }

    pub(super) fn forget_bindings(&mut self, macros: &[u64]) {
        self.local_bindings
            .retain(|binding| !macros.contains(&binding.macro_id));
        self.binding_menu = None;
    }

    pub(super) fn key_binds(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(macro_id) = self.current else {
            return div().into_any_element();
        };
        let no_devices = self.devices.is_empty();
        let cards: Vec<_> = self
            .local_bindings
            .iter()
            .filter(|binding| binding.macro_id == macro_id)
            .filter_map(|binding| {
                let workspace = self
                    .devices
                    .iter()
                    .find(|workspace| workspace.read(cx).identity(cx) == binding.device)?;
                let device = workspace.read(cx).device(cx);
                let profile = device
                    .profiles
                    .iter()
                    .find(|profile| profile.id == binding.profile)?;
                let layout = macro_inputs::for_device(device)?;
                let input = layout
                    .inputs()
                    .iter()
                    .find(|input| input.id() == binding.input)?;
                let key = binding.key();
                let open = self.binding_menu.as_ref() == Some(&key);
                let owner = cx.entity().downgrade();
                let remove_key = key.clone();
                let menu_key = key.clone();
                let more = gpui_kit::base::Popover::new(SharedString::from(format!(
                    "macro-binding-menu-{key}"
                )))
                .anchor(Anchor::TopLeft)
                .offset(window.rem_size() * (2. / 16.))
                .open(open)
                .trigger_with(move |_, _, _| {
                    BaseButton::new(SharedString::from(format!("macro-binding-more-{menu_key}")))
                        .accessibility_label(tr("TEXT_REMOVE_KEY_BIND"))
                        .selected(open)
                        .size(css(30.))
                        .p_0()
                        .rounded_tr(css(5.))
                        .when(open, |button| button.bg(Colors::selected_menu()))
                        .hover(|style| style.bg(Colors::hover()))
                        .focus_visible(|style| style.border_1().border_color(Colors::accent()))
                        .child(img("synapse/macro/binding-more.svg").w(css(16.)).h(css(4.)))
                        .into_any_element()
                })
                .on_open_change(cx.listener(move |this, open: &bool, _, cx| {
                    this.binding_menu = open.then(|| key.clone());
                    cx.notify();
                }))
                .content(move |_, _, _| {
                    let owner = owner.clone();
                    let key = remove_key.clone();
                    BaseButton::new("macro-remove-binding")
                        .w(css(130.))
                        .px(css(10.))
                        .py(css(5.))
                        .bg(Colors::card())
                        .border_1()
                        .border_color(Colors::hover())
                        .text_color(Colors::text())
                        .text_size(css(14.))
                        .hover(|style| style.bg(Colors::hover()))
                        .focus_visible(|style| style.border_color(Colors::accent()))
                        .child(tr("TEXT_REMOVE_KEY_BIND"))
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.local_bindings.retain(|binding| binding.key() != key);
                                this.binding_menu = None;
                                cx.notify();
                            });
                        })
                        .into_any_element()
                });
                let image = resources::dashboard_image(
                    device.product_id,
                    device.edition_id,
                    device.layout_id,
                );
                Some(
                    v_flex()
                        .relative()
                        .top(css(13.))
                        .w(css(290.))
                        .h(css(230.))
                        .m(css(15.))
                        .px(css(20.))
                        .py(css(8.))
                        .bg(Colors::card())
                        .rounded(css(5.))
                        .items_center()
                        .text_center()
                        .flex_shrink_0()
                        .child(div().absolute().right_0().top_0().child(more))
                        .child(div().w(css(260.)).h(css(140.)).flex_shrink_0().when_some(
                            image,
                            |view, image| {
                                view.child(img(image).size_full().object_fit(ObjectFit::Contain))
                            },
                        ))
                        .child(
                            div()
                                .text_size(css(14.))
                                .line_height(css(16.))
                                .child(device_name(device).to_uppercase()),
                        )
                        .child(
                            div()
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .text_color(Colors::muted())
                                .child(format!(
                                    "{} {}",
                                    tr("TEXT_PRODUCT_DEVICE_PROFILE"),
                                    profile.name
                                )),
                        )
                        .child(
                            div()
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .text_color(Colors::muted())
                                .child(format!(
                                    "{} {}{}",
                                    tr("TEXT_PRODUCT_DEVICE_ASSIGNED_KEY"),
                                    if binding.hypershift {
                                        format!("{} + ", tr("TEXT_HYPERSHIFT"))
                                    } else {
                                        String::new()
                                    },
                                    input.label()
                                )),
                        )
                        .into_any_element(),
                )
            })
            .collect();
        v_flex()
            .w_full()
            .when(no_devices, |view| {
                view.child(
                    h_flex()
                        .justify_center()
                        .gap(css(10.))
                        .mt(css(22.))
                        .child(img("synapse/macro/warning.svg").w(css(20.)).h(css(17.)))
                        .child(tr("TEXT_MARCRO_WARING")),
                )
            })
            .child(
                div()
                    .w_full()
                    .mb(css(23.))
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .when(no_devices, |view| view.opacity(0.3))
                    .children(cards)
                    .child(
                        BaseButton::new("macro-assign-device")
                            .disabled(no_devices)
                            .relative()
                            .top(css(13.))
                            .w(css(290.))
                            .h(css(230.))
                            .m(css(15.))
                            .px(css(20.))
                            .py(css(8.))
                            .bg(Colors::surface())
                            .border_2()
                            .border_dashed()
                            .border_color(Colors::border())
                            .rounded(css(5.))
                            .flex()
                            .flex_col()
                            .justify_start()
                            .items_center()
                            .text_center()
                            .hover(|style| style.border_color(Colors::accent()))
                            .focus_visible(|style| style.border_color(Colors::accent()))
                            .child(
                                div()
                                    .h(css(140.))
                                    .w(css(250.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(img("synapse/macro/add.svg").size(css(40.))),
                            )
                            .child(tr("TEXT_ASSIGN_MACRO_TO_DEVICES"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_bindings(window, cx)),
                            ),
                    ),
            )
            .when(!self.local_bindings.is_empty(), |view| {
                view.child(
                    div()
                        .text_center()
                        .text_size(css(12.))
                        .text_color(Colors::muted())
                        .child(session_note()),
                )
            })
            .into_any_element()
    }
}

pub(super) struct BindingDialog {
    open: bool,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    devices: Vec<Entity<ProductWorkspace>>,
    selected: Option<Entity<ProductWorkspace>>,
    selected_subscription: Option<Subscription>,
    profile: Entity<SelectState<Vec<Choice>>>,
    layout: Option<MacroInputLayout>,
    input: Option<String>,
    hovered: Option<String>,
    hypershift: bool,
    macro_id: u64,
    macro_name: String,
    bindings: Vec<LocalBinding>,
    playback: Entity<SelectState<Vec<Choice>>>,
    repeat_editor: Entity<Stepper>,
    repeat_count: u32,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<BindingEvent> for BindingDialog {}

impl BindingDialog {
    fn new(
        macro_id: u64,
        macro_name: String,
        devices: Vec<Entity<ProductWorkspace>>,
        bindings: Vec<LocalBinding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let profile = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let subscription = cx.subscribe(&profile, |this: &mut Self, _, event, cx| {
            if matches!(event, SelectEvent::Confirm(_)) {
                this.input = None;
                this.hovered = None;
                cx.notify();
            }
        });
        let playback = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let repeat_editor = cx.new(|cx| {
            Stepper::new(
                "macro-binding-repeat",
                2.,
                (1., 99., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_modes_area()
        });
        let subscriptions = vec![
            subscription,
            cx.observe(&playback, |_, _, cx| cx.notify()),
            cx.subscribe(
                &repeat_editor,
                |this: &mut Self, _, event: &StepperEvent, cx| {
                    this.repeat_count = event.value as u32;
                    cx.notify();
                },
            ),
        ];
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            open: true,
            focus,
            return_focus,
            devices,
            selected: None,
            selected_subscription: None,
            profile,
            layout: None,
            input: None,
            hovered: None,
            hypershift: false,
            macro_id,
            macro_name,
            bindings,
            playback,
            repeat_editor,
            repeat_count: 2,
            _subscriptions: subscriptions,
        }
    }
    pub(super) fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.input = None;
        self.selected_subscription = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.selected_subscription = None;
        self.layout = None;
        self.input = None;
        self.hovered = None;
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn set_devices(&mut self, devices: Vec<Entity<ProductWorkspace>>, cx: &mut Context<Self>) {
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| !devices.contains(selected))
        {
            self.selected = None;
            self.selected_subscription = None;
            self.layout = None;
            self.input = None;
            self.hovered = None;
        }
        self.devices = devices;
        cx.notify();
    }
    fn choose_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace) {
            return;
        }
        self.layout = macro_inputs::for_device(workspace.read(cx).device(cx));
        if self.layout.is_none() {
            return;
        }
        self.selected_subscription =
            Some(cx.observe_in(&workspace, window, |this, _, window, cx| {
                this.sync_profile(window, cx);
            }));
        self.selected = Some(workspace);
        self.profile.update(cx, |state, cx| {
            state.set_items(Vec::new(), window, cx);
            state.set_selected_index(None, window, cx);
        });
        self.input = None;
        self.hovered = None;
        self.hypershift = false;
        self.sync_profile(window, cx);
        self.focus.focus(window, cx);
    }
    fn sync_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = &self.selected else {
            return;
        };
        let device = workspace.read(cx).device(cx);
        let options = profile_choices(device);
        let old = self.profile.read(cx).selected_value().cloned();
        let selected = old
            .clone()
            .filter(|id| options.iter().any(|entry| entry.value() == id))
            .or_else(|| {
                options
                    .iter()
                    .find(|entry| entry.value() == &device.active_profile)
                    .map(|entry| entry.value().clone())
            })
            .or_else(|| options.first().map(|entry| entry.value().clone()));
        self.layout = macro_inputs::for_device(device);
        self.profile.update(cx, |state, cx| {
            state.set_items(options, window, cx);
            if let Some(selected) = &selected {
                state.set_selected_value(selected, window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
        if old != selected
            || self.input.as_ref().is_some_and(|id| {
                !self.layout.as_ref().is_some_and(|layout| {
                    layout
                        .inputs()
                        .iter()
                        .any(|input| input.id() == id && input.enabled())
                })
            })
        {
            self.input = None;
            self.hovered = None;
        }
        cx.notify();
    }
    pub(super) fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_profile(window, cx);
        let selected = self.playback.read(cx).selected_value().cloned();
        let options = playback::choices(self.input.as_deref().unwrap_or_default());
        self.playback.update(cx, |state, cx| {
            state.set_items(options, window, cx);
            if let Some(selected) = selected {
                state.set_selected_value(&selected, window, cx);
            }
        });
        cx.notify();
    }
    fn draft(&self, cx: &App) -> Option<LocalBinding> {
        let workspace = self.selected.as_ref()?;
        if !self.devices.contains(workspace) {
            return None;
        }
        let profile = self.profile.read(cx).selected_value()?;
        let device = workspace.read(cx).device(cx);
        if !device.profiles.iter().any(|entry| &entry.id == profile) {
            return None;
        }
        let id = self.input.as_ref()?;
        // Validate the live catalog, not only the input captured when opening.
        let layout = macro_inputs::for_device(device)?;
        if !layout
            .inputs()
            .iter()
            .any(|input| input.id() == id && input.enabled())
        {
            return None;
        }
        let playback = self.playback.read(cx).selected_value()?;
        if !playback::choices(id)
            .iter()
            .any(|choice| choice.value() == playback)
        {
            return None;
        }
        Some(LocalBinding {
            macro_id: self.macro_id,
            device: workspace.read(cx).identity(cx),
            profile: profile.clone(),
            input: id.clone(),
            hypershift: self.hypershift,
            playback: playback.clone(),
            repeat_count: if playback == "NTimes" {
                self.repeat_count.clamp(1, 99)
            } else {
                2
            },
        })
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(binding) = self.draft(cx) {
            cx.emit(BindingEvent(binding));
            self.input = None;
            self.focus.focus(window, cx);
            cx.notify();
        }
    }
    fn choose_input(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.layout.as_ref().is_some_and(|layout| {
            layout
                .inputs()
                .iter()
                .any(|input| input.id() == id && input.enabled())
        }) {
            self.input = Some(id.to_string());
            let previous = self.selected.as_ref().and_then(|workspace| {
                self.bindings.iter().find(|binding| {
                    binding.device == workspace.read(cx).identity(cx)
                        && self.profile.read(cx).selected_value() == Some(&binding.profile)
                        && binding.input == id
                        && binding.hypershift == self.hypershift
                })
            });
            let selected = previous
                .map(|binding| binding.playback.clone())
                .unwrap_or_else(|| "Once".into());
            self.repeat_count = previous.map(|binding| binding.repeat_count).unwrap_or(2);
            let options = playback::choices(id);
            let selected = if options.iter().any(|choice| choice.value() == &selected) {
                selected
            } else {
                "Once".into()
            };
            self.playback.update(cx, |state, cx| {
                state.set_items(options, window, cx);
                state.set_selected_value(&selected, window, cx);
            });
            self.repeat_editor.update(cx, |state, cx| {
                state.sync_value(self.repeat_count as f64, 1., false, window, cx)
            });
            cx.notify();
        }
    }
    fn assigned(&self, id: &str, cx: &App) -> bool {
        self.selected.as_ref().is_some_and(|workspace| {
            self.bindings.iter().any(|binding| {
                binding.macro_id == self.macro_id
                    && binding.device == workspace.read(cx).identity(cx)
                    && self.profile.read(cx).selected_value() == Some(&binding.profile)
                    && binding.input == id
                    && binding.hypershift == self.hypershift
            })
        })
    }

    fn chooser(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w_full()
            .mb(css(23.))
            .flex()
            .flex_wrap()
            .items_start()
            .children(self.devices.iter().filter_map(|workspace| {
                let device = workspace.read(cx).device(cx);
                if !eligible(device) {
                    return None;
                }
                let enabled =
                    !device.profiles.is_empty() && macro_inputs::for_device(device).is_some();
                let image = resources::dashboard_image(
                    device.product_id,
                    device.edition_id,
                    device.layout_id,
                );
                let target = workspace.clone();
                Some(
                    BaseButton::new(("macro-device", workspace.entity_id()))
                        .disabled(!enabled)
                        .relative()
                        .top(css(13.))
                        .w(css(300.))
                        .h(css(200.))
                        .m(css(15.))
                        .px(css(20.))
                        .py(css(8.))
                        .bg(Colors::card())
                        .rounded(css(5.))
                        .flex()
                        .flex_col()
                        .justify_start()
                        .items_center()
                        .text_center()
                        .hover(|style| style.text_color(Colors::accent()))
                        .focus_visible(|style| style.border_1().border_color(Colors::accent()))
                        .child(div().w(css(260.)).h(css(140.)).flex_shrink_0().when_some(
                            image,
                            |view, image| {
                                view.child(img(image).size_full().object_fit(ObjectFit::Contain))
                            },
                        ))
                        .child(
                            div()
                                .text_size(css(14.))
                                .line_height(css(16.))
                                .child(device_name(device).to_uppercase()),
                        )
                        .when(!enabled, |button| {
                            button.child(
                                div().text_size(css(12.)).text_color(Colors::muted()).child(
                                    local_text(
                                        "此设备的按键视图尚不可用",
                                        "Input view unavailable for this device",
                                    ),
                                ),
                            )
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.choose_device(target.clone(), window, cx)
                        }))
                        .into_any_element(),
                )
            }))
            .into_any_element()
    }

    fn input_button(&self, input: &MacroInput, cx: &mut Context<Self>) -> BaseButton {
        let id = input.id().to_string();
        let hover_id = id.clone();
        let selected = self.input.as_deref() == Some(input.id());
        let assigned = self.assigned(input.id(), cx);
        BaseButton::new(SharedString::from(format!("macro-input-{id}")))
            .accessibility_label(input.label().to_string())
            .selected(selected)
            .disabled(!input.enabled())
            .px(css(10.))
            .py(css(5.))
            .text_size(css(14.))
            .line_height(css(20.))
            .border_1()
            .border_color(if selected {
                self.layer_color()
            } else {
                Colors::border()
            })
            .rounded(css(3.))
            .bg(Colors::card())
            .text_color(if selected || assigned {
                self.layer_color()
            } else {
                Colors::text()
            })
            .hover(|style| style.border_color(self.layer_color()))
            .focus_visible(|style| style.border_color(self.layer_color()))
            .styles(|style| style.disabled(|style| style.opacity(0.3)))
            .child(if assigned {
                self.macro_name.clone()
            } else {
                input.label().to_string()
            })
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                    this.hovered = Some(hover_id.clone());
                } else if this.hovered.as_ref() == Some(&hover_id) {
                    this.hovered = None;
                }
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.choose_input(&id, window, cx)))
    }
    fn layer_color(&self) -> Hsla {
        if self.hypershift {
            crate::ui::theme::DrawerColors::new().hypershift()
        } else {
            Colors::accent()
        }
    }

    fn product(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(workspace) = &self.selected else {
            return div().into_any_element();
        };
        let device = workspace.read(cx).snapshot(cx);
        let Some(layout) = &self.layout else {
            return div().into_any_element();
        };
        v_flex()
            .w_full()
            .relative()
            .pt(css(10.))
            .child(
                h_flex()
                    .w_full()
                    .h(css(36.))
                    .justify_end()
                    .gap(css(10.))
                    .child(i18n::t("PROFILE"))
                    .child(
                        surface::select(&self.profile)
                            .items(profile_choices(&device))
                            .accessibility_label(i18n::t("PROFILE"))
                            .w(css(230.)),
                    ),
            )
            .child(if layout.mouse_diagram() {
                self.mouse_diagram(layout, &device, cx)
            } else {
                self.keyboard_or_catalog(layout, cx)
            })
            .child(
                h_flex().justify_center().my(css(10.)).child(
                    h_flex()
                        .h(css(36.))
                        .p(css(5.))
                        .gap(css(5.))
                        .bg(Colors::card())
                        .border_1()
                        .border_color(Colors::border())
                        .rounded(css(18.))
                        .children([false, true].into_iter().map(|hyper| {
                            let selected = self.hypershift == hyper;
                            let color = if hyper {
                                crate::ui::theme::DrawerColors::new().hypershift()
                            } else {
                                Colors::accent()
                            };
                            BaseButton::new(if hyper {
                                "macro-layer-hyper"
                            } else {
                                "macro-layer-standard"
                            })
                            .selected(selected)
                            .h(css(24.))
                            .px(css(10.))
                            .py_0()
                            .rounded(css(12.))
                            .bg(if selected { color } else { Colors::card() })
                            .text_color(if selected {
                                Colors::card()
                            } else {
                                Colors::text()
                            })
                            .focus_visible(|style| style.border_1().border_color(color))
                            .child(i18n::t(if hyper { "HYPERSHIFT" } else { "STANDARD" }))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.hypershift = hyper;
                                    this.input = None;
                                    this.hovered = None;
                                    cx.notify();
                                },
                            ))
                        })),
                ),
            )
            .when(self.input.is_some(), |view| {
                view.child(self.binding_editor(cx))
            })
            .child(
                div()
                    .mt(css(10.))
                    .text_center()
                    .text_size(css(12.))
                    .text_color(Colors::muted())
                    .child(session_note()),
            )
            .into_any_element()
    }

    fn keyboard_or_catalog(&self, layout: &MacroInputLayout, cx: &mut Context<Self>) -> AnyElement {
        let [width, height] = layout.viewbox();
        let has_shapes = layout.inputs().iter().any(|input| input.shape().is_some());
        v_flex()
            .w_full()
            .items_center()
            .child(
                div()
                    .relative()
                    .w(css(width))
                    .h(css(height))
                    .flex_shrink_0()
                    .when_some(layout.image(), |view, path| {
                        view.child(
                            img(SharedString::from(path.to_string()))
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        )
                    })
                    .when(has_shapes, |view| {
                        view.children(layout.inputs().iter().filter_map(|input| {
                            let key = input.shape()?;
                            let bounds = input.bounds()?;
                            let id = input.id().to_string();
                            let hover_id = id.clone();
                            let assigned = self.assigned(&id, cx);
                            Some(
                                div()
                                    .absolute()
                                    .left(css(bounds[0]))
                                    .top(css(bounds[1]))
                                    .w(css(bounds[2]))
                                    .h(css(bounds[3]))
                                    .child(
                                        KeyRegion::new(
                                            key,
                                            self.layer_color().opacity(if assigned {
                                                0.4
                                            } else {
                                                0.
                                            }),
                                            self.layer_color(),
                                            self.input.as_ref() == Some(&id),
                                            self.hovered.as_ref() == Some(&id),
                                            cx.listener(move |this, _, window, cx| {
                                                this.choose_input(&id, window, cx)
                                            }),
                                            cx.listener(move |this, hovered: &bool, _, cx| {
                                                if *hovered {
                                                    this.hovered = Some(hover_id.clone());
                                                } else if this.hovered.as_ref() == Some(&hover_id) {
                                                    this.hovered = None;
                                                }
                                                cx.notify();
                                            }),
                                        )
                                        .disabled(!input.enabled()),
                                    ),
                            )
                        }))
                    }),
            )
            // Catalog-only products retain their real input IDs; no invented diagram.
            .when(
                layout.inputs().iter().any(|input| input.shape().is_none()),
                |view| {
                    view.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(css(10.))
                            .justify_center()
                            .children(
                                layout
                                    .inputs()
                                    .iter()
                                    .filter(|input| input.shape().is_none())
                                    .map(|input| self.input_button(input, cx)),
                            ),
                    )
                },
            )
            .into_any_element()
    }

    fn binding_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let input = self.layout.as_ref().and_then(|layout| {
            layout
                .inputs()
                .iter()
                .find(|input| Some(input.id()) == self.input.as_deref())
        });
        let device = self
            .selected
            .as_ref()
            .map(|workspace| workspace.read(cx).device(cx));
        let name = device.map(device_name).unwrap_or_default();
        let profile = device
            .and_then(|device| {
                device
                    .profiles
                    .iter()
                    .find(|profile| self.profile.read(cx).selected_value() == Some(&profile.id))
            })
            .map(|profile| profile.name.clone())
            .unwrap_or_default();
        let display_name = |name: String| {
            div()
                .w_full()
                .h(css(27.))
                .px(css(5.))
                .py(css(4.))
                .border_1()
                .border_color(Colors::border())
                .mb(css(10.))
                .truncate()
                .child(name)
        };
        // HM overrides the product editor's right placement to 50% + 220px.
        v_flex()
            .absolute()
            .left(relative(0.5))
            .ml(css(220.))
            .top(css(66.))
            .w(css(292.))
            .min_h(css(370.))
            .p(css(20.))
            .bg(Colors::card())
            .border_1()
            .border_color(Colors::border())
            .rounded_b(css(5.))
            .occlude()
            .child(
                div()
                    .absolute()
                    .left(css(-1.))
                    .top(css(-36.))
                    .w(css(292.))
                    .h(css(36.))
                    .bg(Colors::surface())
                    .border_1()
                    .border_color(Colors::border())
                    .rounded_t(css(5.))
                    .text_color(Colors::muted())
                    .text_center()
                    .pt(css(10.))
                    .pb(css(9.))
                    .child(
                        input
                            .map(|input| input.label().to_string())
                            .unwrap_or_default(),
                    ),
            )
            .child(display_name(self.macro_name.clone()))
            .child(div().mb(css(5.)).child(i18n::t("DEVICE")))
            .child(display_name(name))
            .child(div().mb(css(5.)).child(i18n::t("PROFILE_LOWERCASE")))
            .child(display_name(profile))
            .child(div().mb(css(5.)).child(i18n::t("PLAYBACK_OPTION")))
            .child(
                surface::select(&self.playback)
                    .items(playback::choices(self.input.as_deref().unwrap_or_default()))
                    .accessibility_label(i18n::t("PLAYBACK_OPTION"))
                    .w_full(),
            )
            .when(
                self.playback
                    .read(cx)
                    .selected_value()
                    .is_some_and(|id| id == "NTimes"),
                |view| {
                    view.child(
                        div()
                            .mt(css(10.))
                            .mb(css(5.))
                            .child(i18n::t("NUMBER_OF_TIMES")),
                    )
                    .child(self.repeat_editor.clone())
                },
            )
            .child(
                h_flex()
                    .mt(css(20.))
                    .justify_end()
                    .gap(css(10.))
                    .child(
                        BaseButton::new("macro-binding-cancel")
                            .px(css(10.))
                            .h(css(27.))
                            .border_1()
                            .border_color(Colors::border())
                            .child(i18n::t("CANCEL"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.input = None;
                                this.focus.focus(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        BaseButton::new("macro-binding-save")
                            .disabled(self.draft(cx).is_none())
                            .px(css(10.))
                            .h(css(27.))
                            .bg(Colors::accent())
                            .text_color(Colors::card())
                            .child(i18n::t("SAVE"))
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for BindingDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let opacity = Presence::new(
            (
                ElementId::from(("macro-bindings", cx.entity_id())),
                "opacity",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
        let progress = Presence::new(
            (
                ElementId::from(("macro-bindings", cx.entity_id())),
                "position",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
        let viewport = window.viewport_size();
        let unit = window.rem_size() / 16.;
        let width = if f32::from(viewport.width / unit) <= 1028. {
            viewport.width
        } else {
            unit * 1020.
        };
        let top = viewport.height + (unit * 106. - viewport.height) * progress;
        let title = self
            .selected
            .as_ref()
            .map(|workspace| device_name(workspace.read(cx).device(cx)))
            .unwrap_or_else(|| tr("TEXT_SELECT_A_DEVICE"));
        let confirm_owner = cx.entity().downgrade();
        let cancel_owner = cx.entity().downgrade();
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(move |_, window, cx| {
                let _ = confirm_owner.update(cx, |this, cx| this.save(window, cx));
                false
            })
            .on_cancel(move |_, window, cx| {
                cancel_owner
                    .update(cx, |this, cx| {
                        if this.input.take().is_some() {
                            this.focus.focus(window, cx);
                            cx.notify();
                            false
                        } else {
                            true
                        }
                    })
                    .unwrap_or(true)
            })
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(unit * 86.)
                    .bottom_0()
                    .opacity(opacity)
                    .bg(Colors::scrim())
                    .child(
                        img("synapse/profiles-glow.svg")
                            .absolute()
                            .bottom_0()
                            .left((viewport.width - unit * 960.) / 2.)
                            .w(unit * 960.)
                            .h(unit * 500.),
                    ),
            )
            .popup(
                v_flex()
                    .id("macro-binding-dialog")
                    .absolute()
                    .left((viewport.width - width) / 2.)
                    .top(top)
                    .w(width)
                    .h((viewport.height - top).max(px(0.)))
                    .bg(Colors::surface())
                    .rounded_t(css(5.))
                    .occlude()
                    .font_family("Roboto")
                    .text_color(Colors::text())
                    .text_size(css(14.))
                    .child(
                        h_flex()
                            .relative()
                            .w_full()
                            .h(css(36.))
                            .flex_shrink_0()
                            .justify_center()
                            .pt(css(9.))
                            .pb(css(8.))
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .line_height(css(19.))
                            .text_color(Colors::muted())
                            .border_b_1()
                            .border_color(Colors::border())
                            .child(title.to_uppercase())
                            .when(self.selected.is_some(), |header| {
                                header.child(
                                    BaseButton::new("macro-binding-back")
                                        .accessibility_label(i18n::t("BACK"))
                                        .absolute()
                                        .left_0()
                                        .top_0()
                                        .size(css(36.))
                                        .p_0()
                                        .hover(|style| style.bg(Colors::hover()))
                                        .active(|style| style.bg(Colors::pressed()))
                                        .child(img("synapse/profiles-back.svg").size(css(20.)))
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.back(window, cx)
                                            }),
                                        ),
                                )
                            })
                            .child(
                                BaseButton::new("macro-binding-close")
                                    .accessibility_label(i18n::t("CLOSE"))
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .size(css(36.))
                                    .p_0()
                                    .hover(|style| style.bg(Colors::hover()))
                                    .active(|style| style.bg(Colors::pressed()))
                                    .child(img("synapse/macro/binding-close.svg").size(css(20.)))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.close(window, cx)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("macro-binding-body")
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .scrollable_both()
                            .child(div().px(css(15.)).pb(css(15.)).child(
                                if self.selected.is_some() {
                                    self.product(cx)
                                } else {
                                    self.chooser(cx)
                                },
                            )),
                    ),
            )
            .into_any_element()
    }
}

mod diagram;
mod playback;
