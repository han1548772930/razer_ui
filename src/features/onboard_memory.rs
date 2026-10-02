//! 653: fR → LR / UR / gR. Board readback is independent of local Profile drafts.
//! Slot IDs are 2–5; the white row is the current hybrid-memory Profile.
use super::*;
use crate::{i18n, ui::theme::OnboardMemoryColors};
use gpui_kit::component::{spinner::Spinner, tooltip::Tooltip};
use serde::Deserialize;
use std::{collections::BTreeSet, rc::Rc};

const SLOTS: [u8; 4] = [2, 3, 4, 5];

#[cfg(test)]
#[path = "onboard_memory_tests.rs"]
mod tests;

#[derive(Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BoardProfile {
    guid: String,
    name: String,
    slot_id: u8,
    locked: bool,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BoardMacro {
    guid: String,
    name: String,
    memory: f64,
    save_status: String,
    /// Filled by the service adapter from the original macro-to-Profile relation.
    profile_names: Vec<String>,
}

/// The renderer consumes the original OBM reducer fields. Missing memory is
/// unknown, never the original reducer's speculative 100% initialization value.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BoardSnapshot {
    init_obm_completed: bool,
    profiles: Vec<BoardProfile>,
    saving_profiles: BTreeSet<u8>,
    removing_profiles: BTreeSet<u8>,
    error_saving_profiles: BTreeSet<u8>,
    macros: Vec<BoardMacro>,
    failed_macros: Vec<BoardMacro>,
    available_memory: Option<f64>,
    mapping_conflict: bool,
}

/// Requests are intents only. No selection writes a local Profile or pretends
/// that firmware accepted it; completion must arrive as a fresh board snapshot.
#[derive(Clone)]
pub(super) enum BoardRequest {
    Assign { slot: u8, profile: Profile },
    Remove { slot: u8 },
    ResolveConflict { use_keyboard: bool },
}
type RequestHandler = Rc<dyn Fn(BoardRequest, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(super) struct OnboardMemoryControl {
    workspace: WeakEntity<DeviceWorkspace>,
}

impl OnboardMemoryControl {
    pub(super) fn new(workspace: WeakEntity<DeviceWorkspace>) -> Self {
        Self { workspace }
    }
}

impl RenderOnce for OnboardMemoryControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The board snapshot and its conflict prompt outlive the menu. The
        // original gR portal remains mounted even when fR.menuOpen is false.
        window.use_keyed_state(
            ("onboard-memory-control", self.workspace.entity_id()),
            cx,
            |window, cx| OnboardMemoryPanel::new(self.workspace, window, cx),
        )
    }
}

struct OnboardMemoryPanel {
    workspace: WeakEntity<DeviceWorkspace>,
    device: String,
    popup: Option<WeakEntity<PopoverState>>,
    readback: Option<BoardSnapshot>,
    request: Option<RequestHandler>,
    selectors: [Entity<SelectState<Vec<Choice>>>; 4],
    pending: BTreeSet<u8>,
    error_dismissed: bool,
    conflict_pending: bool,
    trigger_focus: FocusHandle,
    conflict_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    preview: bool,
    preview_generation: u64,
    readback_generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl OnboardMemoryPanel {
    fn new(
        workspace: WeakEntity<DeviceWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let device = workspace
            .upgrade()
            .map(|view| view.read(cx).identity())
            .unwrap_or_default();
        let selectors = std::array::from_fn(|_| {
            cx.new(|cx| {
                SelectState::new(
                    vec![Choice::new("unread", "未读取")],
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            })
        });
        let mut subscriptions = Vec::new();
        for (index, selector) in selectors.iter().enumerate() {
            subscriptions.push(cx.observe(selector, |_, _, cx| cx.notify()));
            subscriptions.push(cx.subscribe_in(
                selector,
                window,
                move |this, _, event, window, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        this.assign(SLOTS[index], value, window, cx);
                    }
                },
            ));
        }
        if let Some(view) = workspace.upgrade() {
            subscriptions.push(cx.observe(&view, |_, _, cx| cx.notify()));
        }
        Self {
            workspace,
            device,
            popup: None,
            readback: None,
            request: None,
            selectors,
            pending: BTreeSet::new(),
            error_dismissed: false,
            conflict_pending: false,
            trigger_focus: cx.focus_handle(),
            conflict_focus: cx.focus_handle(),
            return_focus: None,
            preview: false,
            preview_generation: 0,
            readback_generation: 0,
            _subscriptions: subscriptions,
        }
    }

    /// Reserved for the device transport: callers must supply matching device
    /// identity and actual readback. The current local workspace has no adapter.
    fn receive(
        &mut self,
        device: &str,
        value: serde_json::Value,
        request: Option<RequestHandler>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if device != self.device || self.current(cx).is_none() {
            return Err("板载配置响应不属于当前设备。".into());
        }
        let snapshot: BoardSnapshot =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        let mut occupied_slots = BTreeSet::new();
        if snapshot.profiles.iter().any(|profile| {
            !SLOTS.contains(&profile.slot_id) || !occupied_slots.insert(profile.slot_id)
        }) {
            return Err("板载配置包含重复或不受支持的槽位。".into());
        }
        let conflict = snapshot.mapping_conflict;
        self.readback_generation = self.readback_generation.wrapping_add(1);
        self.readback = Some(snapshot);
        self.request = request;
        self.pending.clear();
        self.error_dismissed = false;
        self.conflict_pending = false;
        self.sync_selectors(window, cx);
        if conflict && self.return_focus.is_none() {
            self.return_focus = window.focused(cx);
            self.conflict_focus.focus(window, cx);
        } else if !conflict {
            self.restore_conflict_focus(window, cx);
        }
        cx.notify();
        Ok(())
    }

    fn current(&self, cx: &App) -> Option<Entity<DeviceWorkspace>> {
        self.workspace
            .upgrade()
            .filter(|view| view.read(cx).identity() == self.device)
    }

    fn choices(&self, cx: &App) -> Vec<Choice> {
        let Some(view) = self.current(cx) else {
            return Vec::new();
        };
        let workspace = view.read(cx);
        let mut profiles = workspace.device.profiles.iter().collect::<Vec<_>>();
        profiles.sort_by_key(|profile| profile.id != workspace.device.active_profile);
        let mut choices = vec![Choice::new("NONE", i18n::t("NONE"))];
        choices.extend(
            profiles
                .into_iter()
                .map(|profile| Choice::new(&profile.guid, profile.name.clone())),
        );
        choices
    }

    fn sync_selectors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (index, slot) in SLOTS.into_iter().enumerate() {
            let mut choices = self.choices(cx);
            let assigned = self.readback.as_ref().and_then(|snapshot| {
                snapshot
                    .profiles
                    .iter()
                    .find(|profile| profile.slot_id == slot)
            });
            let value = assigned
                .map_or("NONE", |profile| profile.guid.as_str())
                .to_string();
            if let Some(profile) =
                assigned.filter(|profile| !choices.iter().any(|choice| choice.id() == profile.guid))
            {
                choices.push(Choice::new(&profile.guid, profile.name.clone()));
            }
            self.selectors[index].update(cx, |state, cx| {
                state.set_items(choices, window, cx);
                state.set_selected_value(&value, window, cx);
            });
        }
    }

    fn slot_busy(&self, slot: u8) -> bool {
        self.pending.contains(&slot)
            || self.readback.as_ref().is_some_and(|snapshot| {
                snapshot.saving_profiles.contains(&slot)
                    || snapshot.removing_profiles.contains(&slot)
            })
    }

    fn can_assign(&self, slot: u8, cx: &App) -> bool {
        self.current(cx).is_some()
            && self.request.is_some()
            && self.readback.as_ref().is_some_and(|snapshot| {
                snapshot.init_obm_completed
                    && !snapshot.mapping_conflict
                    && !snapshot
                        .profiles
                        .iter()
                        .any(|profile| profile.slot_id == slot && profile.locked)
            })
            && !self.slot_busy(slot)
    }

    fn assign(&mut self, slot: u8, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_assign(slot, cx) {
            if self.readback.is_some() {
                self.sync_selectors(window, cx);
            }
            return;
        }
        let assigned = self.readback.as_ref().and_then(|snapshot| {
            snapshot
                .profiles
                .iter()
                .find(|profile| profile.slot_id == slot)
        });
        if assigned
            .map(|profile| profile.guid.as_str())
            .unwrap_or("NONE")
            == value
        {
            return;
        }
        let request = if value == "NONE" {
            BoardRequest::Remove { slot }
        } else {
            let Some(profile) = self.current(cx).and_then(|view| {
                view.read(cx)
                    .device
                    .profiles
                    .iter()
                    .find(|profile| profile.guid == value)
                    .cloned()
            }) else {
                self.sync_selectors(window, cx);
                return;
            };
            BoardRequest::Assign { slot, profile }
        };
        self.pending.insert(slot);
        self.queue_request(request, window, cx);
        cx.notify();
    }

    fn queue_request(&self, request: BoardRequest, window: &mut Window, cx: &mut Context<Self>) {
        let Some(handler) = self.request.clone() else {
            return;
        };
        let owner = cx.entity().downgrade();
        let generation = self.readback_generation;
        // Release Select/panel borrows and discard intents based on readback
        // that was replaced before dispatch, including an ended preview.
        window.defer(cx, move |window, cx| {
            let current = owner.upgrade().is_some_and(|owner| {
                let panel = owner.read(cx);
                panel.readback_generation == generation && panel.current(cx).is_some()
            });
            if current {
                handler(request, window, cx);
            }
        });
    }

    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(popup) = &self.popup {
            _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
        }
    }

    fn preview_allowed(&self, cx: &App) -> bool {
        self.current(cx)
            .is_some_and(|view| view.read(cx).device.serial_number.starts_with("PREVIEW-"))
    }

    fn show_preview(&mut self, scenario: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview_allowed(cx) {
            return;
        }
        self.preview_generation = self.preview_generation.wrapping_add(1);
        if scenario == "unknown" {
            self.readback_generation = self.readback_generation.wrapping_add(1);
            self.preview = false;
            self.readback = None;
            self.request = None;
            self.pending.clear();
            self.conflict_pending = false;
            self.restore_conflict_focus(window, cx);
            for selector in &self.selectors {
                selector.update(cx, |state, cx| {
                    state.set_items(vec![Choice::new("unread", "未读取")], window, cx);
                    state.set_selected_value(&"unread".to_string(), window, cx);
                });
            }
            cx.notify();
            return;
        }
        let Some(view) = self.current(cx) else {
            return;
        };
        let Some(profile) = view.read(cx).device.active_profile_obj().cloned() else {
            return;
        };
        self.preview = true;
        let mut snapshot = serde_json::json!({
            "initObmCompleted": true,
            "profiles": [{"slotId":2,"guid":profile.guid,"name":profile.name}],
            "availableMemory": 100.0
        });
        match scenario {
            "syncing" => {
                snapshot["savingProfiles"] = serde_json::json!([2]);
                snapshot["removingProfiles"] = serde_json::json!([3]);
            }
            "locked" => {
                snapshot["profiles"][0]["locked"] = true.into();
            }
            "failure" => {
                snapshot["errorSavingProfiles"] = serde_json::json!([2, 3]);
            }
            "memory" => {
                snapshot["availableMemory"] = 24.75.into();
                snapshot["macros"] = serde_json::json!([
                    {"guid":"preview-combo","name":"组合操作（示例）","memory":75.25,"profileNames":[profile.name]},
                    {"guid":"preview-full","name":"空间不足（示例）","memory":30.0,"saveStatus":"noSpace","profileNames":[profile.name]}
                ]);
                snapshot["failedMacros"] = serde_json::json!([{ "guid":"preview-failed", "name":"传输失败（示例）", "profileNames":[profile.name]}]);
            }
            "conflict" => {
                snapshot["mappingConflict"] = true.into();
            }
            _ => {}
        }
        let weak = cx.entity().downgrade();
        let generation = self.preview_generation;
        let handler = Rc::new(move |request, window: &mut Window, cx: &mut App| {
            _ = weak.update(cx, |this, cx| {
                if this.preview_generation == generation {
                    this.preview_request(request, window, cx);
                }
            });
        });
        let device = self.device.clone();
        _ = self.receive(&device, snapshot, Some(handler), window, cx);
    }

    fn preview_request(
        &mut self,
        request: BoardRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.preview || !self.preview_allowed(cx) {
            return;
        }
        let Some(snapshot) = &mut self.readback else {
            return;
        };
        match request {
            BoardRequest::Assign { slot, profile } => {
                snapshot.profiles.retain(|profile| profile.slot_id != slot);
                snapshot.profiles.push(BoardProfile {
                    slot_id: slot,
                    guid: profile.guid,
                    name: profile.name,
                    locked: false,
                });
            }
            BoardRequest::Remove { slot } => {
                snapshot.profiles.retain(|profile| profile.slot_id != slot);
            }
            BoardRequest::ResolveConflict { use_keyboard } => {
                // The two preview choices resolve only this example's dialog.
                // Neither source wins over a real local or device configuration.
                let _ = use_keyboard;
                snapshot.mapping_conflict = false;
                self.conflict_pending = false;
                self.restore_conflict_focus(window, cx);
            }
        }
        self.pending.clear();
        self.sync_selectors(window, cx);
        cx.notify();
    }

    fn restore_conflict_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = self.return_focus.take() {
            if self
                .popup
                .as_ref()
                .and_then(|popup| popup.upgrade())
                .is_some_and(|popup| popup.read(cx).is_open())
            {
                focus.focus(window, cx);
            } else {
                self.trigger_focus.focus(window, cx);
            }
        }
    }

    fn resolve_conflict(
        &mut self,
        use_keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.conflict_pending || self.current(cx).is_none() {
            return;
        }
        if self.request.is_some() {
            self.conflict_pending = true;
            self.queue_request(BoardRequest::ResolveConflict { use_keyboard }, window, cx);
            cx.notify();
        }
    }

    fn conflict(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let width = (window.rem_size() * (600. / 16.))
            .min((window.viewport_size().width - window.rem_size() * (100. / 16.)).max(px(0.)));
        let panel = v_flex()
            .occlude()
            .w(width)
            .p(surface::css(20.))
            .px(surface::css(30.))
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().warning)
            .rounded(surface::css(5.))
            .text_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.8))
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .mb(surface::css(20.))
                    .text_color(cx.theme().warning)
                    .text_size(surface::css(16.))
                    .child(img("synapse/onboard-warning.svg").size(surface::css(25.)))
                    .child(i18n::t("KEYBOARD_ADJUSTMENTS")),
            )
            .child(
                div()
                    .mb(surface::css(30.))
                    .child(i18n::t("KEYBOARD_ADJUSTMENTS_DESC")),
            )
            .child(
                h_flex().justify_center().gap(surface::css(10.)).children(
                    [
                        ("onboard-use-keyboard", "KEYBOARD", true),
                        ("onboard-use-synapse", "SYNAPSE", false),
                    ]
                    .map(|(id, label, use_keyboard)| {
                        BaseButton::new(id)
                            .accessibility_label(i18n::t(label))
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(surface::css(27.))
                            .w(surface::css(100.))
                            .p_0()
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(cx.theme().title_bar)
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .bg(if use_keyboard {
                                OnboardMemoryColors::secondary()
                            } else {
                                cx.theme().primary
                            })
                            .text_color(if use_keyboard {
                                OnboardMemoryColors::white()
                            } else {
                                cx.theme().title_bar
                            })
                            .disabled(self.request.is_none() || self.conflict_pending)
                            .when(self.request.is_some() && !self.conflict_pending, |button| {
                                button.hover(|button| button.opacity(0.8))
                            })
                            .when(self.request.is_none() || self.conflict_pending, |button| {
                                button.opacity(0.3)
                            })
                            .child(i18n::t(label))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.resolve_conflict(use_keyboard, window, cx)
                            }))
                    }),
                ),
            );
        let owner = cx.entity().downgrade();
        gpui_kit::base::Dialog::new(cx)
            .layer(1, true)
            .focus_handle(self.conflict_focus.clone())
            .close_on_backdrop_press(false)
            .on_cancel(|_, _, _| false)
            .on_ok(move |_, window, cx| {
                _ = owner.update(cx, |this, cx| this.resolve_conflict(false, window, cx));
                false
            })
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().title_bar.opacity(0.5)),
            )
            .popup(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height * 0.5)
                    .flex()
                    .items_end()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}

fn help(id: impl Into<ElementId>, text: String, width: f32, cx: &App) -> impl IntoElement {
    BaseButton::new(id)
        .size(surface::css(14.))
        .flex_shrink_0()
        .p_0()
        .rounded_full()
        .bg(cx.theme().secondary)
        .accessibility_label(text.clone())
        .hover(|button| button.bg(cx.theme().foreground.opacity(0.3)))
        .tooltip(move |window, cx| source_tooltip(text.clone(), width, cx).build(window, cx))
        .child(img("synapse/onboard-help.svg").size_full())
}

fn slot_name(slot: u8) -> &'static str {
    match slot {
        2 => "red",
        3 => "green",
        4 => "blue",
        5 => "cyan",
        _ => "",
    }
}

fn render_macro(entry: &BoardMacro, failed: bool, cx: &App) -> impl IntoElement {
    let full = failed || entry.save_status == "noSpace";
    let danger = ProfileAlertColors::new().danger();
    v_flex()
        .id(SharedString::from(format!(
            "onboard-macro-{}-{failed}",
            entry.guid
        )))
        .relative()
        .px(surface::css(20.))
        .py(surface::css(10.))
        .child(
            h_flex()
                .h(surface::css(17.))
                .mb(surface::css(10.))
                .justify_between()
                .text_color(if full { danger } else { cx.theme().foreground })
                .child(div().flex_1().truncate().child(entry.name.clone()))
                .child(format!("{}%", if failed { 0. } else { entry.memory })),
        )
        .when(full, |view| {
            view.child(
                img("synapse/onboard-no-space.svg")
                    .absolute()
                    .right(surface::css(50.))
                    .top(surface::css(8.))
                    .size(surface::css(20.)),
            )
        })
        .children(entry.profile_names.iter().map(|name| {
            div()
                .border_l_1()
                .border_color(if full { danger } else { cx.theme().primary })
                .pl(surface::css(8.))
                .text_color(crate::ui::theme::DrawerColors::new().muted())
                .child(name.clone())
        }))
}

fn profile_icon(slot: Option<u8>, warning: bool, cx: &App) -> AnyElement {
    div()
        .id(("onboard-profile-icon", usize::from(slot.unwrap_or(0))))
        .relative()
        .size(surface::css(20.))
        .flex_shrink_0()
        .child(img("synapse/profile-obm.svg").size_full())
        .when_some(slot, |view, slot| {
            view.child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .size(surface::css(8.))
                    .rounded_full()
                    .bg(OnboardMemoryColors::slot(slot)),
            )
        })
        .when(warning, |icon| {
            icon.child(
                img("synapse/onboard-error.svg")
                    .absolute()
                    .inset_0()
                    .size_full(),
            )
            .tooltip(|window, cx| {
                source_tooltip(i18n::t("OBM_PROFILE_PANEL_TIP_1"), 300., cx).build(window, cx)
            })
        })
        .text_color(cx.theme().foreground)
        .into_any_element()
}

fn source_tooltip(text: String, width: f32, cx: &App) -> Tooltip {
    Tooltip::new(text)
        .w(surface::css(width))
        .m_0()
        .px(surface::css(10.))
        .py(surface::css(8.))
        .rounded_none()
        .shadow_none()
        .bg(cx.theme().title_bar)
        .text_color(cx.theme().foreground)
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
}

impl OnboardMemoryPanel {
    fn render_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let hybrid_warning = self
            .current(cx)
            .is_some_and(|view| view.read(cx).settings().bindings.contains_key("DKM_SB_01"));
        let profile_name = self
            .current(cx)
            .and_then(|view| {
                view.read(cx)
                    .device
                    .active_profile_obj()
                    .map(|profile| profile.name.clone())
            })
            .unwrap_or_default();
        let mut rows = v_flex()
            .id("onboard-slot-list")
            .min_h(surface::css(239.))
            .max_h((window.viewport_size().height - window.rem_size() * (276. / 16.)).max(px(100.)))
            .overflow_y_scrollbar()
            .child(
                h_flex()
                    .h(surface::css(50.))
                    .flex_shrink_0()
                    .px(surface::css(20.))
                    .gap(surface::css(10.))
                    .child(profile_icon(None, hybrid_warning, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(cx.theme().foreground.opacity(0.3))
                            .child(profile_name),
                    )
                    .child(help(
                        "onboard-hybrid-help",
                        i18n::t("OBM_PROFILE_PANEL_TIP_2"),
                        290.,
                        cx,
                    )),
            );
        for (index, slot) in SLOTS.into_iter().enumerate() {
            let assigned = self.readback.as_ref().and_then(|snapshot| {
                snapshot
                    .profiles
                    .iter()
                    .find(|profile| profile.slot_id == slot)
            });
            let mut row = h_flex()
                .id(("onboard-slot", slot as usize))
                .h(surface::css(50.))
                .flex_shrink_0()
                .px(surface::css(20.))
                .gap(surface::css(10.))
                .child(profile_icon(Some(slot), false, cx));
            if let Some(profile) = assigned.filter(|profile| profile.locked) {
                row = row
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(profile.name.clone()),
                    )
                    .child(
                        BaseButton::new(("onboard-slot-lock", slot as usize))
                            .size(surface::css(20.))
                            .p_0()
                            .accessibility_label("查看锁定配置的恢复说明")
                            .tooltip(|window, cx| {
                                source_tooltip(
                                    format!(
                                        "{}\n\n{}",
                                        i18n::t("OBM_LOCK_ICON_TOOLTIP_V1"),
                                        i18n::t("OBM_LOCK_ICON_TOOLTIP_V2")
                                    ),
                                    305.,
                                    cx,
                                )
                                .build(window, cx)
                            })
                            .child(img("synapse/onboard-lock.svg").size_full())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close(window, cx);
                                if let Some(workspace) = this.current(cx) {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.continue_with(
                                            Continue::Page(crate::nav::Tab::Help),
                                            window,
                                            cx,
                                        )
                                    });
                                }
                            })),
                    );
            } else {
                let mut choices = if self.readback.is_none() {
                    vec![Choice::new("unread", "未读取")]
                } else {
                    self.choices(cx)
                };
                if let Some(profile) = assigned
                    .filter(|profile| !choices.iter().any(|choice| choice.id() == profile.guid))
                {
                    choices.push(Choice::new(&profile.guid, profile.name.clone()));
                }
                row = row.child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .child(
                            surface::select(&self.selectors[index])
                                .id(("onboard-slot-select", slot as usize))
                                .accessibility_label(format!("板载槽位 {}", slot - 1))
                                .items(choices)
                                .w_full()
                                .disabled(!self.can_assign(slot, cx)),
                        )
                        .when(self.slot_busy(slot), |view| {
                            view.child(
                                div()
                                    .absolute()
                                    .right(surface::css(30.))
                                    .top(surface::css(5.))
                                    .child(Spinner::new().small()),
                            )
                        }),
                );
            }
            rows = rows.child(row);
        }
        if self.readback.is_none() {
            rows = rows.child(
                div()
                    .px(surface::css(20.))
                    .pb(surface::css(10.))
                    .text_color(cx.theme().muted_foreground)
                    .child("板载配置尚未读取，连接设备服务后可分配配置文件。"),
            );
        }
        if let Some(snapshot) = self
            .readback
            .as_ref()
            .filter(|snapshot| !snapshot.error_saving_profiles.is_empty() && !self.error_dismissed)
        {
            let error = if snapshot.error_saving_profiles.len() == 1 {
                let slot = *snapshot.error_saving_profiles.iter().next().unwrap();
                let template = i18n::t("OBM_PROFILE_PANEL_ERROR_ONE_SLOT");
                let name = slot_name(slot);
                let offset = template.find("{{slot}}");
                StyledText::new(template.replace("{{slot}}", name)).with_highlights(
                    offset.into_iter().map(|start| {
                        (
                            start..start + name.len(),
                            HighlightStyle {
                                // LR uses CSS red even for the green/blue/cyan slot name.
                                color: Some(OnboardMemoryColors::slot(2)),
                                ..Default::default()
                            },
                        )
                    }),
                )
            } else {
                StyledText::new(i18n::t("OBM_PROFILE_PANEL_ERROR_MORE_SLOT"))
            };
            rows = rows.child(
                v_flex()
                    .px(surface::css(20.))
                    .pb(surface::css(10.))
                    .gap(surface::css(10.))
                    .child(
                        h_flex()
                            .items_start()
                            .gap(surface::css(10.))
                            .child(
                                img("synapse/onboard-transfer-warning.svg").size(surface::css(20.)),
                            )
                            .child(div().flex_1().child(error)),
                    )
                    .child(
                        BaseButton::new("onboard-dismiss-error")
                            .p_0()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .text_color(cx.theme().muted_foreground)
                            .underline()
                            .hover(|button| button.text_color(cx.theme().primary))
                            .active(|button| button.opacity(0.7))
                            .child(i18n::t("DISMISS"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.error_dismissed = true;
                                cx.notify();
                            })),
                    ),
            );
        }
        let mut panel = v_flex()
            .id("profile-obm-content")
            .test_support()
            .w(surface::css(270.))
            .relative()
            .pt(surface::css(36.))
            .min_h(surface::css(260.))
            .max_h((window.viewport_size().height - window.rem_size() * (150. / 16.)).max(px(200.)))
            .bg(cx.theme().group_box)
            .rounded(surface::css(5.))
            .border_1()
            .border_t_0()
            .border_color(cx.theme().border)
            .shadow(vec![BoxShadow {
                color: cx.theme().title_bar.opacity(0.7),
                offset: point(px(0.), px(0.)),
                blur_radius: window.rem_size() * (20. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                h_flex()
                    .absolute()
                    .top_0()
                    .left(surface::css(-1.))
                    .h(surface::css(36.))
                    .w(surface::css(270.))
                    .flex_shrink_0()
                    .justify_center()
                    .gap(surface::css(5.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_t(surface::css(5.))
                    .bg(cx.theme().sidebar)
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::t("ON_BOARD_MEMORY"))
                    .child(help(
                        "onboard-memory-help",
                        i18n::t("ON_BOARD_MEMORY_TOOLTIP"),
                        270.,
                        cx,
                    ))
                    .child(
                        BaseButton::new("onboard-memory-close")
                            .absolute()
                            .right_0()
                            .top_0()
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .accessibility_label("关闭板载内存")
                            .hover(|button| button.bg(cx.theme().foreground.opacity(0.1)))
                            .active(|button| button.bg(cx.theme().title_bar.opacity(0.1)))
                            .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .child(rows);
        let is_ble = self
            .current(cx)
            .is_some_and(|view| view.read(cx).device.use_ble);
        if !is_ble {
            if let Some(snapshot) = &self.readback {
                if !snapshot.macros.is_empty() {
                    panel = panel.child(
                        v_flex()
                            .min_h_0()
                            .flex_shrink_1()
                            .bg(cx.theme().sidebar)
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(
                                h_flex()
                                    .h(surface::css(40.))
                                    .flex_shrink_0()
                                    .px(surface::css(20.))
                                    .gap(surface::css(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(img("synapse/onboard-macro.svg").size(surface::css(20.)))
                                    .child(i18n::t("MACROS")),
                            )
                            .child(
                                v_flex()
                                    .id("onboard-macro-list")
                                    .min_h_0()
                                    .overflow_y_scrollbar()
                                    .children(
                                        snapshot
                                            .macros
                                            .iter()
                                            .map(|entry| (entry, false))
                                            .chain(
                                                snapshot
                                                    .failed_macros
                                                    .iter()
                                                    .map(|entry| (entry, true)),
                                            )
                                            .map(|(entry, failed)| render_macro(entry, failed, cx)),
                                    ),
                            ),
                    );
                }
            }
        }
        let available = self
            .readback
            .as_ref()
            .and_then(|snapshot| snapshot.available_memory)
            .filter(|value| value.is_finite())
            .map(|value| format!("{}%", (value.clamp(0., 100.) * 100.).round() / 100.));
        panel = panel.child(
            h_flex()
                .h(surface::css(30.))
                .flex_shrink_0()
                .px(surface::css(20.))
                .justify_between()
                .bg(cx.theme().sidebar)
                .rounded_b(surface::css(4.))
                .border_t_1()
                .border_color(cx.theme().border)
                .child(i18n::t("AVAILABLE"))
                .child(if is_ble {
                    BaseButton::new("onboard-ble-memory-help")
                        .size(surface::css(20.))
                        .p_0()
                        .accessibility_label(i18n::t(
                            "SWITCH_TO_WIRED_OR_HYPERSPEED_TO_VIEW_AVAILABLE_SPACE",
                        ))
                        .tooltip(|window, cx| {
                            source_tooltip(
                                i18n::t("SWITCH_TO_WIRED_OR_HYPERSPEED_TO_VIEW_AVAILABLE_SPACE"),
                                210.,
                                cx,
                            )
                            .build(window, cx)
                        })
                        .child(img("synapse/onboard-info.svg").size_full())
                        .into_any_element()
                } else {
                    div()
                        .child(available.unwrap_or_else(|| "未读取".into()))
                        .into_any_element()
                }),
        );
        let preview_allowed = self.preview_allowed(cx);
        div()
            .id("onboard-menu-content")
            .child(panel)
            .when(preview_allowed, |view| {
                view.child(
                    v_flex()
                        .w(surface::css(270.))
                        .p(surface::css(10.))
                        .gap(surface::css(8.))
                        .bg(cx.theme().sidebar)
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(surface::css(12.))
                                .text_color(cx.theme().warning)
                                .child(if self.preview {
                                    "界面预览 · 示例状态，不写入设备或本地配置"
                                } else {
                                    "界面预览 · 仅此预览设备可用"
                                }),
                        )
                        .child(
                            h_flex().flex_wrap().gap(surface::css(5.)).children(
                                [
                                    ("ready", "分配槽位"),
                                    ("syncing", "正在同步"),
                                    ("locked", "锁定槽位"),
                                    ("failure", "传输失败"),
                                    ("memory", "宏与内存"),
                                    ("conflict", "映射冲突"),
                                    ("unknown", "结束预览"),
                                ]
                                .map(|(scenario, label)| {
                                    Button::new(SharedString::from(format!(
                                        "onboard-preview-{scenario}"
                                    )))
                                    .label(label)
                                    .xsmall()
                                    .h(surface::css(24.))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.show_preview(scenario, window, cx)
                                    }))
                                }),
                            ),
                        ),
                )
            })
            .map(|view| {
                if preview_allowed {
                    view.max_h(
                        (window.viewport_size().height - window.rem_size() * (40. / 16.))
                            .max(px(200.)),
                    )
                    .overflow_y_scrollbar()
                    .into_any_element()
                } else {
                    view.into_any_element()
                }
            })
    }
}

impl Render for OnboardMemoryPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = cx.entity().downgrade();
        let trigger_focus = self.trigger_focus.clone();
        // Component Select gives its descendant List focus while open. Let
        // that topmost menu own outside dismissal, matching LR.keepUIon on a
        // selection; an option may lie beyond the OBM panel's physical bounds.
        let selecting = self.selectors.iter().any(|selector| {
            let focus = selector.focus_handle(cx);
            focus.contains_focused(window, cx) && !focus.is_focused(window)
        });
        let active_guid = self.current(cx).and_then(|view| {
            view.read(cx)
                .device
                .active_profile_obj()
                .map(|profile| profile.guid.clone())
        });
        let selected_slot = active_guid.and_then(|guid| {
            self.readback
                .as_ref()?
                .profiles
                .iter()
                .filter(|profile| profile.guid == guid)
                .map(|profile| profile.slot_id)
                .min()
        });
        let saving = self
            .readback
            .as_ref()
            .is_some_and(|snapshot| !snapshot.saving_profiles.is_empty());
        let error = self
            .readback
            .as_ref()
            .is_some_and(|snapshot| !snapshot.error_saving_profiles.is_empty());
        div()
            .flex_shrink_0()
            .child(
                Popover::new("profile-obm-popover")
                    .overlay_closable(!selecting)
                    .anchor(Anchor::TopCenter)
                    .offset(window.rem_size() * (2. / 16.))
                    .trigger_with(move |open, _, cx| {
                        BaseButton::new("profile-obm")
                            .accessibility_label(i18n::t("ON_BOARD_PROFILES"))
                            .track_focus(&trigger_focus)
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(surface::css(26.))
                            .border_1()
                            .border_color(if open {
                                cx.theme().primary
                            } else {
                                cx.theme().transparent
                            })
                            .hover(|button| button.border_color(cx.theme().border))
                            .tooltip(|window, cx| {
                                Tooltip::new(i18n::t("ON_BOARD_PROFILES")).build(window, cx)
                            })
                            .child(img("synapse/profile-obm.svg").size(surface::css(20.)))
                            .when(saving, |button| {
                                button.child(
                                    img("synapse/onboard-overlay-sync.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full(),
                                )
                            })
                            .when(error, |button| {
                                button.child(
                                    img("synapse/onboard-overlay-error.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full(),
                                )
                            })
                            .when_some(selected_slot, |button, slot| {
                                button.child(
                                    div()
                                        .absolute()
                                        .right(surface::css(1.))
                                        .top(surface::css(1.))
                                        .size(surface::css(9.))
                                        .rounded_full()
                                        .bg(OnboardMemoryColors::trigger_slot(slot)),
                                )
                            })
                            .into_any_element()
                    })
                    .content(move |_, window, cx| {
                        let popup = cx.entity().downgrade();
                        let content = panel
                            .update(cx, |panel, cx| {
                                panel.popup = Some(popup);
                                panel.render_panel(window, cx)
                            })
                            .ok();
                        // 270px panel starts at -121 from its 26px trigger.
                        div()
                            .w(surface::css(272.))
                            .pl(surface::css(2.))
                            .children(content)
                    }),
            )
            .when(
                self.readback
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.mapping_conflict),
                |view| view.child(self.conflict(window, cx)),
            )
    }
}
