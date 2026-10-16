//! Current mounted MapText factories and independently extracted emoji policies.
use super::*;
use crate::features::mapping_text_editor::{
    CharacterMapRequested, EmojiData, MappingTextEditor, TextChanged,
};
use gpui_kit::component::input::{InputEvent, TextareaState};

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    group: String,
    version_filter: bool,
    data_index: usize,
}
#[derive(Deserialize)]
struct Policies {
    products: Vec<Policy>,
    datasets: Vec<EmojiData>,
}
fn policies() -> &'static Policies {
    static DATA: OnceLock<Policies> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("mouse_mapping_text_current_data.json"))
            .expect("independently audited current mouse text policies")
    })
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    policies()
        .products
        .iter()
        .find(|p| p.product_id == product_id)
}
pub struct MouseCharacterMapRequested;
impl EventEmitter<MouseCharacterMapRequested> for MouseProductWorkspace {}

impl MouseProductWorkspace {
    pub(super) fn initialize_current_mouse_text(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(policy) = policy(self.spec.product_id) else {
            return;
        };
        let value = if button["assignment"] == "TEXT_FUNCTION"
            && button.get("assignmentGroup").is_some()
            && !button["assignmentGroup"].is_null()
        {
            button["assignmentValue"].as_str().unwrap_or("").to_owned()
        } else {
            String::new()
        };
        let input = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx).placeholder(t("ENTER_TEXT"));
            input.set_value(value.clone(), window, cx);
            input
        });
        let editor = cx.new(|cx| {
            MappingTextEditor::new_with_data(
                input.clone(),
                &policies().datasets[policy.data_index],
                policy.version_filter,
                window,
                cx,
            )
        });
        let state = &mut self.mapping_current_state;
        state.text_value = value;
        state.basic_requires_synapse = true;
        state.subscriptions.push(cx.subscribe_in(
            &input,
            window,
            |this, input, event, window, cx| {
                if !matches!(event, InputEvent::Change)
                    || this.mapping_current_state.function != "TEXT_FUNCTION"
                {
                    return;
                }
                let value = input.read(cx).value().to_string();
                let previous = &this.mapping_current_state.text_value;
                let value = if let Some((range, clipped)) =
                    crate::features::shortcuts::limit_shortcut_text(previous, &value)
                {
                    input.update(cx, |input, cx| {
                        let caret = range.start;
                        input.set_selected_range(range, cx);
                        input.replace("", window, cx);
                        input.set_selected_range(caret..caret, cx);
                    });
                    clipped
                } else {
                    value
                };
                let nonempty = !value.is_empty();
                this.mapping_current_state.text_value = value;
                // Original onTextEnter calls enableSave(length>0,length>0,true),
                // including clearing the dirty guard when text becomes empty.
                this.mapping_current_state.can_save = nonempty;
                this.mapping_current_state.changed = nonempty;
                this.mapping_current_state.basic_requires_synapse = true;
                cx.notify();
            },
        ));
        state
            .subscriptions
            .push(cx.subscribe(&editor, |this, _, event: &TextChanged, cx| {
                if this.mapping_current_state.function != "TEXT_FUNCTION" {
                    return;
                }
                this.mapping_current_state.text_value = event.0.clone();
                this.mapping_current_state.can_save = true;
                this.mapping_current_state.changed = true;
                this.mapping_current_state.basic_requires_synapse = true;
                cx.notify();
            }));
        state.subscriptions.push(
            cx.subscribe(&editor, |_, _, _: &CharacterMapRequested, cx| {
                cx.emit(MouseCharacterMapRequested)
            }),
        );
        state.text_editor = Some(editor);
    }
    pub(super) fn current_mouse_text_body(&self) -> Option<AnyElement> {
        Some(
            self.mapping_current_state
                .text_editor
                .as_ref()?
                .clone()
                .into_any_element(),
        )
    }
    pub(super) fn current_mouse_text_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_current_state;
        let mut payload = json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,"outputType":policy.group});
        payload[&policy.group] = json!({"text":state.text_value});
        Some(vec![payload])
    }
}
