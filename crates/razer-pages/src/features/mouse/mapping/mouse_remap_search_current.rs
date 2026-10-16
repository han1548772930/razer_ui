//! Actual 199/208 search component: original English dictionary, mouse-only
//! suggestions and retained searchKey even after Back, Clear and selection.
use super::*;

#[derive(Deserialize)]
struct OptionRow {
    content: String,
}
#[derive(Deserialize)]
struct Group {
    #[serde(rename = "functionName")]
    function: String,
    #[serde(rename = "functionID")]
    label: String,
    #[serde(rename = "type")]
    options: Vec<OptionRow>,
}
#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    search_label: String,
    groups: Vec<Group>,
}
fn policy(pid: u32) -> Option<&'static Policy> {
    static DATA: OnceLock<Vec<Policy>> = OnceLock::new();
    DATA.get_or_init(|| read_mouse_remap_search_current_catalog())
        .iter()
        .find(|p| p.product_id == pid)
}
#[derive(Default)]
pub(super) struct State {
    open: bool,
    suggestions: bool,
    text: String,
    pub(super) search_key: String,
    input: Option<Entity<InputState>>,
    subscription: Option<Subscription>,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    options_bounds: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
    search_hover: bool,
    clear_hover: bool,
}
fn filter<'a>(policy: &'a Policy, query: &str) -> Vec<(usize, Vec<(usize, &'a OptionRow)>)> {
    policy
        .groups
        .iter()
        .enumerate()
        .filter_map(|(group_index, group)| {
            let group_matches = group.label.to_lowercase().contains(query);
            let options = group
                .options
                .iter()
                .enumerate()
                .filter(|(_, row)| group_matches || row.content.to_lowercase().contains(query))
                .collect::<Vec<_>>();
            (!options.is_empty() || group_matches).then_some((group_index, options))
        })
        .collect()
}
impl MouseProductWorkspace {
    fn open_current_remap_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if policy(self.spec.product_id).is_none() {
            return;
        }
        if self.mapping_menu_current_state.remap.input.is_none() {
            let value = self.mapping_menu_current_state.remap.text.clone();
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .placeholder("Search")
            });
            let subscription = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let raw = input.read(cx).value().to_string();
                let lower = raw.to_lowercase();
                if raw != lower {
                    input.update(cx, |input, cx| input.set_value(lower.clone(), window, cx));
                }
                let state = &mut this.mapping_menu_current_state.remap;
                state.text = lower.clone();
                state.search_key = lower;
                state.suggestions = true;
                cx.notify();
            });
            let state = &mut self.mapping_menu_current_state.remap;
            state.input = Some(input);
            state.subscription = Some(subscription);
        }
        self.mapping_menu_current_state.remap.open = true;
        if let Some(input) = &self.mapping_menu_current_state.remap.input {
            window.focus(&input.focus_handle(cx), cx);
        }
        cx.notify();
    }
    fn close_current_remap_search_outside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.mapping_menu_current_state.remap;
        state.open = false;
        state.text.clear();
        state.search_key.clear();
        if let Some(input) = &state.input {
            // gpui-base 0.7.1 set_value suppresses Change; this deliberately
            // leaves source filter flags untouched, unlike user typing.
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        cx.notify();
    }
    fn select_current_remap_suggestion(
        &mut self,
        function: &str,
        index: usize,
        label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.mapping_menu_current_state.remap;
        state.text = label.into();
        state.suggestions = false;
        if let Some(input) = &state.input {
            input.update(cx, |input, cx| input.set_value(label, window, cx));
        }
        // The parent owns value-zero truthiness, retained searchValue, child
        // initialization and enableSave; searchKey is deliberately retained.
        self.apply_current_mouse_remap_search(function, index, window, cx);
        cx.notify();
    }
    pub(in super::super) fn current_mouse_remap_search(
        &self,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_menu_current_state.remap;
        let anchor = state.anchor.clone();
        let options_bounds = state.options_bounds.clone();
        let mut search = div()
            .id("mouse-current-remap-search")
            .absolute()
            .left_0()
            .top_0()
            .w(surface::css(if state.open { 284. } else { 36. }))
            .h(surface::css(if state.open { 32. } else { 36. }))
            .on_prepaint(move |bounds, _, _| anchor.set(bounds))
            .on_mouse_down_out(
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    // The mounted source's document mousedown listener also
                    // clears retained text after Back has collapsed the input.
                    if event.button == MouseButton::Left
                        && !options_bounds.get().contains(&event.position)
                    {
                        this.close_current_remap_search_outside(window, cx);
                    }
                }),
            );
        if state.open {
            let input = state.input.as_ref()?;
            search = search
                .child(
                    Input::new(input)
                        .appearance(false)
                        .w(surface::css(241.))
                        .h(surface::css(32.))
                        .p_0()
                        .pl(surface::css(47.))
                        .rounded(surface::css(5.))
                        .border_0()
                        .bg(Colors::selected_surface())
                        .text_color(Colors::text())
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.)),
                )
                .child(
                    BaseButton::new("mouse-current-remap-back")
                        .absolute()
                        .left(surface::css(10.))
                        .top(surface::css(7.04))
                        .size(surface::css(36.))
                        .p_0()
                        .child(
                            img("synapse/profiles-back.svg")
                                .absolute()
                                .left_0()
                                .top_0()
                                .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mapping_menu_current_state.remap.open = false;
                            cx.notify();
                        })),
                );
            if !state.text.is_empty() {
                search = search.child(
                    BaseButton::new("mouse-current-remap-clear")
                        .absolute()
                        .right_0()
                        .top_0()
                        .size(surface::css(36.))
                        .p_0()
                        .on_hover(cx.listener(|this, hovered, _, cx| {
                            this.mapping_menu_current_state.remap.clear_hover = *hovered;
                            cx.notify();
                        }))
                        .child(
                            img(if state.clear_hover {
                                "synapse/profiles-clear-hover.svg"
                            } else {
                                "synapse/profiles-clear.svg"
                            })
                            .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            let state = &mut this.mapping_menu_current_state.remap;
                            state.text.clear();
                            if let Some(input) = &state.input {
                                input.update(cx, |input, cx| input.set_value("", window, cx));
                            }
                            cx.notify();
                        })),
                );
            }
            if state.suggestions && !state.text.is_empty() {
                let matches = filter(policy, &state.search_key);
                let observed = state.options_bounds.clone();
                let mut options = v_flex()
                    .id("mouse-current-remap-suggestions")
                    .w(surface::css(291.))
                    .max_h(surface::css(450.))
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(Colors::line())
                    .bg(razer_widgets::theme::TooltipColors::background())
                    .text_color(Colors::text())
                    .overflow_y_scroll()
                    .track_scroll(&state.scroll)
                    .occlude()
                    .on_prepaint(move |bounds, _, _| observed.set(bounds));
                if matches.is_empty() {
                    options = options.border_color(Colors::selected()).child(
                        div()
                            .text_size(surface::css(14.))
                            .italic()
                            .child("No suggestions available."),
                    );
                }
                for (group_index, rows) in matches {
                    let group = &policy.groups[group_index];
                    let function = group.function.clone();
                    let label = group.label.clone();
                    options = options.child(
                        BaseButton::new(SharedString::from(format!(
                            "mouse-current-remap-group-{group_index}"
                        )))
                        .flex_shrink_0()
                        .ml(surface::css(25.))
                        .p(surface::css(10.))
                        .justify_start()
                        .text_size(surface::css(13.))
                        .line_height(surface::css(17.))
                        .hover(|style| style.bg(MenuColors::search_option_hover()))
                        .child(label.clone())
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.select_current_remap_suggestion(
                                    &function, 0, &label, window, cx,
                                )
                            },
                        )),
                    );
                    for (index, row) in rows {
                        let function = group.function.clone();
                        let label = row.content.clone();
                        options = options.child(
                            BaseButton::new(SharedString::from(format!(
                                "mouse-current-remap-{group_index}-{index}"
                            )))
                            .flex_shrink_0()
                            .ml(surface::css(35.))
                            .p(surface::css(5.))
                            .justify_start()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .hover(|style| style.bg(MenuColors::search_option_hover()))
                            .child(label.clone())
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.select_current_remap_suggestion(
                                        &function, index, &label, window, cx,
                                    )
                                },
                            )),
                        );
                    }
                }
                search = search.child(
                    deferred(Suggestions {
                        content: options.into_any_element(),
                        anchor: state.anchor.clone(),
                    })
                    .with_priority(106),
                );
            } else {
                state.options_bounds.set(Bounds::default());
            }
        } else {
            state.options_bounds.set(Bounds::default());
            search = search
                .child(
                    BaseButton::new("mouse-current-remap-open")
                        .size(surface::css(36.))
                        .p_0()
                        .on_hover(cx.listener(|this, hovered, _, cx| {
                            this.mapping_menu_current_state.remap.search_hover = *hovered;
                            cx.notify();
                        }))
                        .child(
                            img(if state.search_hover {
                                "synapse/profiles-search-hover.svg"
                            } else {
                                "synapse/profiles-search.svg"
                            })
                            .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_current_remap_search(window, cx)
                        })),
                )
                .when(state.search_hover, |search| {
                    search.child(
                        div()
                            .absolute()
                            .left(surface::css(15.))
                            .top(surface::css(30.))
                            .px(surface::css(10.))
                            .py(surface::css(8.))
                            .border_1()
                            .border_color(Colors::line())
                            .bg(razer_widgets::theme::TooltipColors::background())
                            .text_size(surface::css(14.))
                            .whitespace_nowrap()
                            .child(t(&policy.search_label)),
                    )
                });
        }
        Some(search.into_any_element())
    }
}
struct Suggestions {
    content: AnyElement,
    anchor: Rc<Cell<Bounds<Pixels>>>,
}
impl IntoElement for Suggestions {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Suggestions {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let anchor = self.anchor.get();
        window.with_element_offset(
            point(anchor.left(), anchor.bottom()) - bounds.origin,
            |window| self.content.prepaint(window, cx),
        );
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}

// Generated catalog reader: razer-pages/mouse_remap_search_current_data.
fn read_mouse_remap_search_current_catalog() -> Vec<Policy> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Policy>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Policy> {
        let value = match r.tag()? {
            0 => Policy {
                product_id: r.child(decode_2)?,
                search_label: r.child(decode_3)?,
                groups: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Group>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Group> {
        let value = match r.tag()? {
            0 => Group {
                function: r.child(decode_3)?,
                label: r.child(decode_3)?,
                options: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<OptionRow>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<OptionRow> {
        let value = match r.tag()? {
            0 => OptionRow {
                content: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_REMAP_SEARCH_CURRENT_DATA,
        0,
        "7a6acbe89ba59caff2ff124a57c0fa71e00efe5c581fb89b323d510a6ef9ba15",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_mouse_remap_search_current_data_decodes() {
    let _ = read_mouse_remap_search_current_catalog();
}
// End generated catalog reader: razer-pages/mouse_remap_search_current_data.
