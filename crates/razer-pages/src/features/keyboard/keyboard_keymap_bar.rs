//! Current KeymapBar for 555/580/747; local state and middleware messages are separate.
use super::*;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{
    IndexPath,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{Select, SelectEvent, SelectState},
};

const ADD_ROW: &str = "source-keymap-add";
#[derive(Clone)]
struct Item {
    guid: String,
    title: SharedString,
    slot: Option<String>,
    disabled: bool,
}
impl Item {
    fn content(&self) -> AnyElement {
        h_flex()
            .items_center()
            .child(
                img(self
                    .slot
                    .as_ref()
                    .map(|slot| format!("synapse/keyboard-keymap-slot-{slot}.png"))
                    .unwrap_or_else(|| "synapse/keyboard-keymap-add.svg".into()))
                .size(surface::css(20.)),
            )
            .child(self.title.clone())
            .into_any_element()
    }
}
impl SearchableListItem for Item {
    type Value = String;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn value(&self) -> &String {
        &self.guid
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
    fn display_title(&self) -> Option<AnyElement> {
        Some(self.content())
    }
}
struct Choices(Vec<Item>);
impl SearchableListDelegate for Choices {
    type Item = Item;
    fn items_count(&self, _: usize) -> usize {
        self.0.len()
    }
    fn item(&self, ix: IndexPath) -> Option<&Item> {
        self.0.get(ix.row)
    }
    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.0
            .iter()
            .position(|item| item.value() == value)
            .map(IndexPath::new)
    }
    fn render_item(
        &self,
        _: IndexPath,
        item: &Item,
        checked: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        Some(
            div()
                .w_full()
                .min_h(surface::css(25.))
                .px(surface::css(5.))
                .py(surface::css(4.))
                .text_size(surface::css(14.))
                .text_color(rgb(0xcccccc))
                .when(item.guid == ADD_ROW, |row| {
                    row.border_t_1().border_color(rgb(0x515151))
                })
                .when(checked, |row| row.bg(rgb(0x333333)))
                .when(item.disabled, |row| row.opacity(0.3))
                .child(item.content())
                .into_any_element(),
        )
    }
}

pub(super) struct State {
    selector: Option<Entity<SelectState<Choices>>>,
    name: Option<Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    renaming: bool,
    visible: bool,
    pending_delete: Option<(Value, String)>,
    delete_delay: Option<Task<()>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            selector: None,
            name: None,
            subscriptions: Vec::new(),
            renaming: false,
            visible: true,
            pending_delete: None,
            delete_delay: None,
        }
    }
}
fn rows(workspace: &KeyboardProductWorkspace) -> Vec<Value> {
    let mut rows = workspace.draft["keymaps"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        a["slot"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["slot"].as_str().unwrap_or_default())
    });
    rows
}
fn slot(rows: &[Value]) -> Option<String> {
    (1..=8)
        .map(|value| value.to_string())
        .find(|slot| !rows.iter().any(|row| row["slot"] == *slot))
}
fn new_name(rows: &[Value]) -> String {
    for n in 1..=100 {
        let name = format!("Keymap {n}");
        if !rows.iter().any(|row| row["name"] == name) {
            return name;
        }
    }
    "Keymap 100".into()
}
fn duplicate_name(rows: &[Value], name: &str) -> String {
    static SUFFIX: OnceLock<regex::Regex> = OnceLock::new();
    let regex =
        SUFFIX.get_or_init(|| regex::Regex::new(r"(.+)\((\d)\)$").expect("current keymap suffix"));
    let base = regex
        .captures(name)
        .and_then(|capture| capture.get(1))
        .map(|part| part.as_str())
        .unwrap_or(name)
        .trim();
    let taken = rows
        .iter()
        .filter_map(|row| regex.captures(row["name"].as_str()?))
        .filter(|capture| base.contains(capture.get(1).unwrap().as_str().trim()))
        .filter_map(|capture| capture.get(2)?.as_str().parse::<usize>().ok())
        .collect::<Vec<_>>();
    for n in 1..=rows.len() {
        if !taken.contains(&n) {
            return format!("{base} ({n})");
        }
    }
    format!("{base} (2)")
}
impl KeyboardProductWorkspace {
    pub(super) fn init_keymap_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.keymap_bar.visible;
        self.keymap_bar = State {
            visible,
            ..Default::default()
        };
        if !matches!(self.spec.product_id, 555 | 580 | 747) {
            return;
        }
        let rows = rows(self);
        let selected = rows
            .iter()
            .position(|row| row["guid"] == self.draft["activeKeymapId"]);
        let show_slot = self.spec.config["DeviceInfo"]["isShowOBMKeymapKeymapSlot"] == true;
        let mut choices = rows
            .iter()
            .filter_map(|row| {
                let name = row["name"].as_str()?;
                let name = if show_slot
                    && (name.trim().is_empty() || name.trim().eq_ignore_ascii_case("default"))
                {
                    t("GAME_PROFILE_DEFAULT")
                } else {
                    name.into()
                };
                Some(Item {
                    guid: row["guid"].as_str()?.into(),
                    title: if show_slot {
                        format!("Keymap {} - {}", row["slot"].as_str()?, name).into()
                    } else {
                        name.into()
                    },
                    slot: row["slot"].as_str().map(str::to_owned),
                    disabled: row["disabled"] == true,
                })
            })
            .collect::<Vec<_>>();
        if rows.len() < 8 {
            choices.push(Item {
                guid: ADD_ROW.into(),
                title: t("ADD").into(),
                slot: None,
                disabled: false,
            });
        }
        let selector = cx.new(|cx| {
            SelectState::new(Choices(choices), selected.map(IndexPath::new), window, cx)
                .searchable(false)
        });
        let name = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let select_subscription =
            cx.subscribe_in(&selector, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(guid)) = event {
                    if guid == ADD_ROW {
                        this.add_keymap(false, window, cx)
                    } else {
                        this.select_keymap(guid, window, cx)
                    }
                }
            });
        let name_subscription = cx.subscribe_in(&name, window, |this, _, event, window, cx| {
            if this.keymap_bar.renaming
                && matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur)
            {
                this.finish_keymap_rename(window, cx)
            }
        });
        self.keymap_bar.selector = Some(selector);
        self.keymap_bar.name = Some(name);
        self.keymap_bar.subscriptions = vec![select_subscription, name_subscription];
    }
    fn keymap_message(&self, kind: &str, payload: Value, cx: &mut Context<Self>) {
        self.request_lighting_command(KeyboardLightingCommand::Middleware(json!({"type":kind,"timerTick":chrono::Utc::now().timestamp_millis(),"payload":payload})),cx);
    }
    fn select_keymap(&mut self, guid: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.customize_controls_disabled() || !rows(self).iter().any(|row| row["guid"] == guid) {
            return;
        }
        self.clear_analog_mapping(cx);
        self.clear_actuation_selection();
        self.draft["activeKeymapId"] = json!(guid);
        self.keymap_message(
            "ON_SET_KEYMAP_PROFILE",
            json!({"keymapGuid":guid,"keymaps":self.draft["keymaps"]}),
            cx,
        );
        self.init_keymap_bar(window, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn add_keymap(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.customize_controls_disabled() {
            return;
        }
        let mut all = rows(self);
        let Some(slot) = slot(&all) else { return };
        let guid = uuid::Uuid::new_v4().to_string();
        let mut keymap = if duplicate {
            let Some(current) = all
                .iter()
                .find(|row| row["guid"] == self.draft["activeKeymapId"])
            else {
                return;
            };
            json!({"guid":guid,"name":duplicate_name(&all,current["name"].as_str().unwrap_or_default()),"slot":slot,"mappings":self.draft.pointer(&self.mapping_path()).cloned().unwrap_or(json!([]))})
        } else {
            let mut row = self.spec.config["DEFAULTKEYMAP"].clone();
            row["guid"] = json!(guid);
            row["slot"] = json!(slot);
            row["name"] = if self.spec.config["DeviceInfo"]["isShowOBMKeymapKeymapSlot"] == true {
                json!("Default")
            } else {
                json!(new_name(&all))
            };
            row
        };
        if duplicate {
            self.keymap_message("ON_DUPLICATE_KEYMAP_PROFILE", json!({"keymap":keymap}), cx);
            // Source reducer retains only these fields while middleware creates
            // the target mapping list. It does not manufacture a loaded copy.
            keymap = json!({"guid":keymap["guid"],"name":keymap["name"],"slot":keymap["slot"]});
        }
        all.push(keymap);
        self.draft["keymaps"] = json!(all);
        self.select_keymap(&guid, window, cx);
    }
    fn begin_keymap_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = rows(self)
            .into_iter()
            .find(|row| row["guid"] == self.draft["activeKeymapId"])
        else {
            return;
        };
        self.keymap_bar.renaming = true;
        if let Some(name) = &self.keymap_bar.name {
            name.update(cx, |name, cx| {
                name.set_value(row["name"].as_str().unwrap_or_default(), window, cx);
                name.focus(window, cx);
                name.select_all(window, cx)
            });
        }
        cx.notify();
    }
    fn finish_keymap_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.keymap_bar.renaming {
            return;
        }
        self.keymap_bar.renaming = false;
        let name = self
            .keymap_bar
            .name
            .as_ref()
            .map(|name| name.read(cx).value().to_string())
            .unwrap_or_default();
        let name = name.trim();
        let mut all = rows(self);
        let Some(ix) = all
            .iter()
            .position(|row| row["guid"] == self.draft["activeKeymapId"])
        else {
            return;
        };
        let default = self.spec.config["DeviceInfo"]["allowRenameKeymapToDefaultName"] == true
            && name.eq_ignore_ascii_case("default");
        let name = if default { "Default" } else { name };
        if !name.is_empty()
            && all[ix]["name"] != name
            && (default || !all.iter().any(|row| row["name"] == name))
        {
            all[ix]["name"] = json!(name);
            self.draft["keymaps"] = json!(all);
            self.keymap_message(
                "ON_RENAME_KEYMAP_PROFILE",
                json!({"keymapGuid":self.draft["activeKeymapId"],"name":name}),
                cx,
            );
            cx.emit(KeyboardProductChanged);
        }
        self.init_keymap_bar(window, cx);
        cx.notify();
    }
    fn delete_keymap(&mut self, expected_guid: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut all = rows(self);
        if all.len() <= 1
            || self.customize_controls_disabled()
            || self.draft["activeKeymapId"] != expected_guid
        {
            return;
        }
        let Some(ix) = all.iter().position(|row| row["guid"] == expected_guid) else {
            return;
        };
        let next = if self.spec.config["DeviceInfo"]["category"] == "KEYPAD" {
            all.get(ix + 1).or_else(|| all.first())
        } else {
            all.get(ix.saturating_sub(1))
        }
        .and_then(|row| row["guid"].as_str())
        .map(str::to_owned);
        all.remove(ix);
        self.draft["keymaps"] = json!(all);
        self.keymap_message(
            "ON_DELETE_KEYMAP_PROFILE",
            json!({"keymapGuid":expected_guid}),
            cx,
        );
        if let Some(next) = next.filter(|next| next != expected_guid) {
            self.select_keymap(&next, window, cx)
        } else {
            self.init_keymap_bar(window, cx);
            cx.emit(KeyboardProductChanged);
            cx.notify();
        }
    }
    fn request_keymap_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let profile = self.draft["guid"].clone();
        self.keymap_bar.delete_delay = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let _ = owner.update(cx, |this, cx| {
                if this.draft["guid"] != profile {
                    return;
                }
                if let Some(guid) = this.draft["activeKeymapId"].as_str().map(str::to_owned) {
                    this.keymap_bar.pending_delete = Some((profile, guid));
                    cx.notify();
                }
            });
        }));
    }
    pub(super) fn update_keymap_bar_page(&mut self, page: &str) {
        if self.page == page {
            return;
        }
        self.keymap_bar.visible = match self.spec.product_id {
            555 => page == "TAB_CUSTOMIZE",
            580 | 747 => page != "HELP",
            _ => false,
        };
    }
    /// Current root nav/profile header slot; visibility retains the original
    /// history-handler behavior rather than deriving it in render.
    pub fn keymap_bar_element(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.keymap_bar.visible {
            return None;
        }
        let selector = self.keymap_bar.selector.as_ref()?;
        let all = rows(self);
        let count = all.len();
        let mut bar = h_flex()
            .relative()
            .gap(surface::css(5.))
            .items_center()
            .h(surface::css(26.));
        bar = bar.child(
            div()
                .id("keyboard-keymap-icon")
                .size(surface::css(20.))
                .border_2()
                .border_color(cx.theme().foreground)
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .tooltip(|window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(t("KEYMAP")).build(window, cx)
                })
                .child(img("synapse/keyboard-keymap-icon.svg").size(surface::css(14.))),
        );
        if self.keymap_bar.renaming {
            if let Some(name) = &self.keymap_bar.name {
                bar = bar.child(
                    div()
                        .id("keyboard-keymap-rename")
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "escape" {
                                this.keymap_bar.renaming = false;
                                this.init_keymap_bar(window, cx);
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }))
                        .w(surface::css(250.))
                        .child(
                            Input::new(name)
                                .ml(surface::css(10.))
                                .w(surface::css(230.))
                                .h(surface::css(27.))
                                .bg(rgb(0x111111))
                                .border_1()
                                .border_color(rgb(0x44d62c))
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .text_color(rgb(0xcccccc)),
                        ),
                );
            }
        } else {
            bar = bar.child(
                Select::new(selector)
                    .disabled(count <= 1 || self.customize_controls_disabled())
                    .menu_max_h(surface::css(227.))
                    .w(surface::css(250.))
                    .h(surface::css(27.)),
            );
        }
        let owner = cx.entity().downgrade();
        let disabled = self.customize_controls_disabled();
        let profile = self.draft["guid"].clone();
        bar = bar.child(
            Button::new("keyboard-keymap-actions")
                .label("\u{2026}")
                .ghost()
                .disabled(disabled)
                .dropdown_menu(move |mut menu, _, _| {
                    for command in ["ADD", "DUPLICATE", "RENAME", "DELETE"] {
                        let owner = owner.clone();
                        let profile = profile.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(command))
                                .disabled(
                                    disabled
                                        || matches!(command, "ADD" | "DUPLICATE") && count == 8
                                        || command == "DELETE" && count == 1,
                                )
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        if this.draft["guid"] != profile {
                                            return;
                                        }
                                        match command {
                                            "ADD" => this.add_keymap(false, window, cx),
                                            "DUPLICATE" => this.add_keymap(true, window, cx),
                                            "RENAME" => this.begin_keymap_rename(window, cx),
                                            "DELETE" => {
                                                this.request_keymap_delete(window, cx);
                                            }
                                            _ => {}
                                        }
                                    });
                                }),
                        );
                    }
                    menu
                }),
        );
        if let Some((profile, guid)) = &self.keymap_bar.pending_delete {
            let profile = profile.clone();
            let guid = guid.clone();
            bar = bar.child(
                razer_assets::deferred_resource(
                    v_flex()
                        .id("keyboard-keymap-delete-alert")
                        .absolute()
                        .top(surface::css(42.))
                        .left(surface::css(0.))
                        .min_w(surface::css(300.))
                        .max_w(surface::css(330.))
                        .p(surface::css(20.))
                        .items_center()
                        .text_center()
                        .bg(rgb(0x111111))
                        .border_1()
                        .border_color(rgb(0xfd4949))
                        .rounded(surface::css(3.))
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .text_color(rgb(0xcccccc))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.keymap_bar.pending_delete = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .mb(surface::css(10.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xfd4949))
                                .child(t("DELETE_KEYMAP_TITLE").to_uppercase()),
                        )
                        .child(div().mb(surface::css(10.)).child(t("DELETE_KEYMAP_MSG")))
                        .child(
                            gpui_kit::base::Button::new("keyboard-keymap-confirm-delete")
                                .accessibility_label(t("DELETE"))
                                .min_w(surface::css(90.))
                                .h(surface::css(27.))
                                .px(surface::css(5.))
                                .bg(rgb(0xfd4949))
                                .border_1()
                                .border_color(rgba(0x0000004d))
                                .text_color(rgb(0x111111))
                                .text_size(surface::css(12.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(t("DELETE"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.keymap_bar.pending_delete = None;
                                    if this.draft["guid"] == profile {
                                        this.delete_keymap(&guid, window, cx);
                                    }
                                    cx.stop_propagation();
                                    cx.notify();
                                })),
                        ),
                )
                .with_priority(104),
            );
        }
        Some(bar.into_any_element())
    }
}
