//! Current ARGB port editors commit through Enter and document pointer events.

#[derive(Clone, Copy)]
pub(super) enum NameSubmit {
    Enter,
    OutsideClick,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum NameEdit {
    KeepEditing,
    Close,
    Commit,
}

pub(super) fn name_edit(
    product_id: u32,
    previous: &str,
    value: &str,
    submit: NameSubmit,
) -> NameEdit {
    match product_id {
        778 | 3871 | 3884 if value.is_empty() || value == previous => NameEdit::Close,
        778 | 3871 | 3884 => NameEdit::Commit,
        3886 if matches!(submit, NameSubmit::OutsideClick) && value.is_empty() => {
            NameEdit::KeepEditing
        }
        3886 => NameEdit::Commit,
        _ => unreachable!("Port name editors require a registered ARGB product"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_port_editors_close_without_dispatching_empty_or_unchanged_names() {
        for product_id in [778, 3871, 3884] {
            for submit in [NameSubmit::Enter, NameSubmit::OutsideClick] {
                assert_eq!(name_edit(product_id, "Port 1", "", submit), NameEdit::Close);
                assert_eq!(
                    name_edit(product_id, "Port 1", "Port 1", submit),
                    NameEdit::Close
                );
                assert_eq!(
                    name_edit(product_id, "Port 1", "  ", submit),
                    NameEdit::Commit
                );
            }
        }
    }

    #[test]
    fn current_3886_submits_empty_on_enter_but_keeps_it_open_on_outside_click() {
        assert_eq!(
            name_edit(3886, "Port 1", "", NameSubmit::Enter),
            NameEdit::Commit
        );
        assert_eq!(
            name_edit(3886, "Port 1", "", NameSubmit::OutsideClick),
            NameEdit::KeepEditing
        );
        assert_eq!(
            name_edit(3886, "Port 1", "Port 1", NameSubmit::Enter),
            NameEdit::Commit
        );
        assert_eq!(
            name_edit(3886, "Port 1", "  ", NameSubmit::OutsideClick),
            NameEdit::Commit
        );
    }
}
