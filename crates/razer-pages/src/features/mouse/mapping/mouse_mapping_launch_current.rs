//! Per-product mounted MapLaunch drafts. Current source evidence is under
//! docs/re/mouse-launch-current/. Native submission remains a separate gap.
use super::*;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::radio::Radio;

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    program_label: String,
    folder_key: String,
    initial_requires_synapse: bool,
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| read_mouse_mapping_launch_current_catalog())
        .iter()
        .find(|p| p.product_id == product_id)
}

#[derive(Default)]
pub(super) struct State {
    mode: Option<bool>, // Some(true): local program; None: unselected.
    path: String,
    original_group: Option<Value>,
    path_input: Option<Entity<InputState>>,
    picker_task: Option<Task<()>>,
}

impl State {
    pub(super) fn cancel_picker(&mut self) {
        self.picker_task = None;
    }
}

impl MouseProductWorkspace {
    pub(super) fn initialize_current_mouse_launch(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(policy) = policy(self.spec.product_id) else {
            return;
        };
        let state = &mut self.mapping_current_state;
        state.basic_requires_synapse = policy.initial_requires_synapse;
        let original = button.get("assignmentGroup").filter(|g| !g.is_null());
        let mut controls = State {
            original_group: original.cloned(),
            ..Default::default()
        };
        // An unrelated assignment does not seed either field. The source
        // constructor starts with both controls disabled until a radio is used.
        if button["assignment"] == "LAUNCH_PROGRAM"
            && let Some(group) = original
        {
            if let Some(path) = group.get("path") {
                controls.mode = Some(true);
                controls.path = path.as_str().unwrap_or("").to_owned();
            }
            state.can_save = false;
            state.changed = false;
            state.basic_requires_synapse = true;
        }
        let path = cx.new(|cx| InputState::new(window, cx).default_value(controls.path.clone()));
        controls.path_input = Some(path);
        state.launch = controls;
    }

    fn change_current_mouse_launch_mode(&mut self, program: bool, cx: &mut Context<Self>) {
        if !program {
            return;
        }
        let state = &mut self.mapping_current_state;
        if state.launch.mode == Some(program) {
            return; // componentDidUpdate only recomputes when launch changes.
        }
        state.launch.mode = Some(program);
        let (field, value) = ("path", &state.launch.path);
        let changed = !value.is_empty()
            && state
                .launch
                .original_group
                .as_ref()
                .is_none_or(|group| group[field] != *value);
        state.can_save = changed;
        state.changed = changed;
        state.basic_requires_synapse = true;
        cx.notify();
    }

    fn browse_current_mouse_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mapping_current_state.launch.mode != Some(true) {
            return;
        }
        let Some(path_input) = self.mapping_current_state.launch.path_input.clone() else {
            return;
        };
        let identity = path_input.entity_id();
        let generation = self.draft_generation;
        // Current MapLaunch.openDialog -> host showFileOpenDialog passes
        // an executable filter into the native picker. A cancelled dialog does
        // not mutate the local draft, save gate, or original assignment.
        let native_owner = match raw_window_handle::HasWindowHandle::window_handle(window)
            .map(|handle| handle.as_raw())
        {
            Ok(raw_window_handle::RawWindowHandle::Win32(handle)) => handle.hwnd.get(),
            other => {
                eprintln!("[mouse-launch] native file dialog owner unavailable: {other:?}");
                return;
            }
        };
        let picker = match razer_platform::file_dialog::OpenFileDialog::start(
            native_owner,
            "Open",
            &[".exe"],
            false,
        ) {
            Ok(picker) => picker,
            Err(error) => {
                eprintln!("[mouse-launch] cannot open file dialog: {error:#}");
                return;
            }
        };
        let task = cx.spawn_in(window, async move |view, cx| {
            let paths = match picker.result().await {
                Ok(Some(paths)) => paths,
                Ok(None) => return,
                Err(error) => {
                    eprintln!("[mouse-launch] file dialog failed: {error:#}");
                    return;
                }
            };
            if let Some(path) = paths.first() {
                let value = path.to_string_lossy().into_owned();
                let _ = view.update_in(cx, |this, window, cx| {
                    let state = &mut this.mapping_current_state;
                    if !this.active
                        || this.page != "TAB_CUSTOMIZE"
                        || this.draft_generation != generation
                        || state.function != "LAUNCH_PROGRAM"
                        || !state
                            .launch
                            .path_input
                            .as_ref()
                            .is_some_and(|input| input.entity_id() == identity)
                    {
                        return;
                    }
                    state.launch.path = value.clone();
                    path_input.update(cx, |input, cx| input.set_value(value, window, cx));
                    // The file-dialog callback enables saving even when the
                    // selected path equals the original. Cancel leaves state.
                    state.can_save = true;
                    state.changed = true;
                    state.basic_requires_synapse = true;
                    cx.notify();
                });
            }
        });
        self.mapping_current_state.launch.picker_task = Some(task);
    }

    pub(super) fn current_mouse_launch_payload(&self, button: &Value) -> Option<Vec<Value>> {
        policy(self.spec.product_id)?;
        let state = &self.mapping_current_state.launch;
        if state.mode != Some(true) || state.path.is_empty() {
            return None;
        }
        let group = json!({"path":state.path});
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],
            "isHyperShift":self.hypershift,"outputType":"launchGroup","launchGroup":group}),
        ])
    }

    pub(super) fn current_mouse_launch_body(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_current_state.launch;
        let path = state.path_input.as_ref()?;
        let program = state.mode == Some(true);
        Some(
            v_flex()
                .child(
                    Radio::new("mouse-current-launch-program")
                        .label(t(&policy.program_label))
                        .checked(program)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.change_current_mouse_launch_mode(true, cx)
                        })),
                )
                .child(div().h(surface::css(17.)))
                .child(
                    h_flex()
                        .ml(surface::css(30.))
                        .mb(surface::css(20.))
                        .child(
                            Input::new(path)
                                .readonly(true)
                                .disabled(!program)
                                .w(surface::css(178.))
                                .h(surface::css(29.)),
                        )
                        .child(
                            BaseButton::new("mouse-current-launch-folder")
                                .disabled(!program)
                                .ml(surface::css(-25.))
                                .mt(surface::css(5.))
                                .size(surface::css(20.))
                                .p_0()
                                .child(
                                    razer_assets::resource_svg()
                                        .path(policy.folder_key.clone())
                                        .size_full(),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.browse_current_mouse_launch(window, cx)
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}

// Generated catalog reader: razer-pages/mouse_mapping_launch_current_data.
fn read_mouse_mapping_launch_current_catalog() -> Vec<Policy> {
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
                program_label: r.child(decode_3)?,
                folder_key: r.child(decode_3)?,
                initial_requires_synapse: r.child(decode_4)?,
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
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_MAPPING_LAUNCH_CURRENT_DATA,
        0,
        "213c29aaf268eaa6170bf75d7c002b39a0cb86a6c9129f7900eb5aea451b2978",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_mapping_launch_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_mapping_launch_current_data_decodes() {
        let _ = super::read_mouse_mapping_launch_current_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_mapping_launch_current_data.
