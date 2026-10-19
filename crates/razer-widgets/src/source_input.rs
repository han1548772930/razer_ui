//! UTF-16 maxlength behavior for the current source's single-line inputs.
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{Context, Entity, SharedString, Window};
use std::ops::Range;

/// Create an input whose interactive value follows HTML's UTF-16 length unit.
/// The caller supplies the maxlength from its current source component.
pub fn input_state(
    max_length: usize,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> InputState {
    InputState::new(window, cx).validate(move |value, _| within_length(value, max_length))
}

/// Load the source prop without applying the interactive HTML maxlength.
/// The framework's ordinary setter also validates programmatic replacements.
pub fn set_value(
    input: &mut InputState,
    value: impl Into<SharedString>,
    max_length: usize,
    window: &mut Window,
    cx: &mut Context<InputState>,
) {
    input.set_validator(|_, _| true, cx);
    input.set_value(value.into(), window, cx);
    input.set_validator(move |value, _| within_length(value, max_length), cx);
}

fn within_length(value: &str, max_length: usize) -> bool {
    value.encode_utf16().count() <= max_length
}

fn insertion_prefix<'a>(
    current: &str,
    selection: Range<usize>,
    inserted: &'a str,
    max_length: usize,
) -> &'a str {
    let retained = current[..selection.start].encode_utf16().count()
        + current[selection.end..].encode_utf16().count();
    let mut available = max_length.saturating_sub(retained);
    let mut end = 0;
    for (offset, ch) in inserted.char_indices() {
        if ch.len_utf16() > available {
            break;
        }
        available -= ch.len_utf16();
        end = offset + ch.len_utf8();
    }
    &inserted[..end]
}

/// Preserve the unselected text when a paste exceeds a source maxlength.
/// Ordinary keyboard editing remains with the input's UTF-16 validator.
pub trait SourceInputExt {
    fn source_max_length(self, state: &Entity<InputState>, max_length: usize) -> Self;
}

impl SourceInputExt for Input {
    fn source_max_length(self, state: &Entity<InputState>, max_length: usize) -> Self {
        let state = state.clone();
        self.on_paste(move |clipboard, window, cx| {
            let Some(text) = clipboard.text().filter(|text| !text.is_empty()) else {
                return false;
            };
            if !state.read(cx).is_editable() {
                return true;
            }
            // The framework removes line breaks from single-line pastes.
            let text = text.replace(['\r', '\n'], "");
            let input = state.read(cx);
            let current = input.value();
            let prefix = insertion_prefix(&current, input.selected_range(), &text, max_length);
            if !prefix.is_empty() {
                state.update(cx, |input, cx| input.replace(prefix.to_owned(), window, cx));
            }
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::insertion_prefix;

    #[test]
    fn paste_truncates_only_inserted_text_and_preserves_selection_suffix() {
        assert_eq!(insertion_prefix("abcd", 2..2, "12345", 6), "12");
        assert_eq!(insertion_prefix("abcd", 1..3, "12345", 4), "12");
        assert_eq!(insertion_prefix("abcd", 0..4, "12345", 4), "1234");
        assert_eq!(insertion_prefix("abcd", 2..2, "12345", 4), "");
        assert_eq!(insertion_prefix("abcd", 2..2, "", 4), "");
    }

    #[test]
    fn paste_counts_utf16_without_splitting_supplementary_characters() {
        assert_eq!(insertion_prefix("甲乙", 3..3, "😀x", 4), "😀");
        assert_eq!(insertion_prefix("甲乙", 3..3, "😀x", 3), "");
        assert_eq!(insertion_prefix("😀x", 0..4, "甲😀", 4), "甲😀");
        assert_eq!(insertion_prefix("abcd", 2..2, "x", 2), "");
    }
}
