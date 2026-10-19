//! Current 9286:Y/T/w/I layer semantics; see chroma-studio-layers-source.json.
use super::*;

/// Current Studio profile and layer callbacks trim before exact-name checks.
pub(super) fn renamed_title<'a, 'b>(
    value: &'a str,
    titles: impl Iterator<Item = &'b str>,
) -> Option<&'a str> {
    let title = razer_device::source_string::trim(value);
    (!title.is_empty() && !titles.into_iter().any(|existing| existing == title)).then_some(title)
}

pub(super) fn renamed_layer_title<'a>(
    value: &'a str,
    group: bool,
    layers: &[Layer],
) -> Option<&'a str> {
    renamed_title(
        value,
        layers
            .iter()
            .filter(|layer| layer.group == group)
            .map(|layer| layer.title.as_str()),
    )
}

/// Source Y checks siblings of the same type and restarts at suffix 1.
pub(super) fn unique_title(title: Option<&str>, group: bool, layers: &[Layer]) -> String {
    let available = |candidate: &str| {
        !layers
            .iter()
            .any(|layer| layer.group == group && layer.title == candidate)
    };
    if let Some(title) = title.filter(|title| !title.is_empty()) {
        if available(title) {
            return title.to_owned();
        }
        let base = suffix_base(title);
        for number in 1_u64.. {
            let candidate = format!("{base} ({number})");
            if available(&candidate) {
                return candidate;
            }
        }
    } else {
        for number in 1_u64.. {
            let candidate = label("TEXT_NEW_GROUP").replace("{{num}}", &number.to_string());
            if available(&candidate) {
                return candidate;
            }
        }
    }
    unreachable!("finite layer inventory")
}

pub(super) fn suffix_base(title: &str) -> &str {
    if let Some(open) = title.rfind('(') {
        if open > 0 && title.as_bytes()[open - 1] == b' ' && title.ends_with(')') {
            let number = title[open + 1..title.len() - 1].trim();
            // JS isNaN accepts empty text, decimal/exponent and base-prefixed numbers.
            let numeric = number.is_empty()
                || matches!(number, "Infinity" | "+Infinity" | "-Infinity")
                || (number
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
                    && number.parse::<f64>().is_ok())
                || ["0x", "0X", "0b", "0B", "0o", "0O"].iter().any(|prefix| {
                    number.strip_prefix(prefix).is_some_and(|digits| {
                        let radix = match prefix.as_bytes()[1] {
                            b'x' | b'X' => 16,
                            b'b' | b'B' => 2,
                            _ => 8,
                        };
                        !digits.is_empty() && digits.chars().all(|digit| digit.is_digit(radix))
                    })
                });
            if numeric {
                return &title[..open - 1];
            }
        }
    }
    title
}

impl ChromaStudio {
    pub(super) fn can_remove_layer(&self, id: u64) -> bool {
        self.document
            .layers
            .iter()
            .any(|layer| !layer.group && layer.id != id)
    }

    pub(super) fn first_visible_layer(&self) -> Option<u64> {
        self.document
            .layers
            .iter()
            .find(|layer| !layer.group && layer.visible)
            .or_else(|| self.document.layers.first().filter(|layer| !layer.group))
            .map(|layer| layer.id)
    }

    pub(super) fn change_layer_effect(&mut self, id: u64, name: &str, cx: &mut Context<Self>) {
        if matches!(name, "reactive" | "ripple") {
            return;
        }
        let Some(effect) = source().effects.iter().find(|effect| effect.name == name) else {
            return;
        };
        let Some(index) = self
            .document
            .layers
            .iter()
            .position(|layer| layer.id == id && !layer.group && layer.name != name)
        else {
            return;
        };
        self.checkpoint();
        let layer = &mut self.document.layers[index];
        layer.name = effect.name.clone();
        layer.title = effect.label.clone();
        layer.value = effect.value;
        layer.params = effect.params.clone();
        layer.paint_params = effect.paint_params.clone();
        cx.notify();
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn studio_titles_use_source_trim_exact_names_and_no_submit_truncation() {
        let titles = ["Exact", "Old"];
        for (value, expected) in [
            ("\u{feff} Fresh \u{feff}", Some("Fresh")),
            ("\u{feff}\u{2028}\u{00a0}", None),
            ("\u{feff}Exact\u{feff}", None),
            (" exact ", Some("exact")),
            ("\u{0085}", Some("\u{0085}")),
        ] {
            assert_eq!(renamed_title(value, titles.into_iter()), expected);
        }
        let existing = "😀".repeat(17);
        assert_eq!(
            renamed_title(&existing, [existing.as_str()].into_iter()),
            None
        );
        assert_eq!(
            renamed_title(&existing, titles.into_iter()),
            Some(existing.as_str())
        );
    }

    #[::core::prelude::v1::test]
    fn studio_layer_rename_checks_layer_and_group_inventories_independently() {
        let layer = |id, title: &str, group| Layer {
            id,
            name: String::new(),
            title: title.into(),
            value: 0,
            params: Value::Null,
            params2: Value::Null,
            paint_params: Value::Null,
            visible: true,
            group,
            extensions: Default::default(),
        };
        let layers = [layer(1, "Layer", false), layer(2, "Group", true)];
        assert_eq!(renamed_layer_title(" Group ", true, &layers), None);
        assert_eq!(renamed_layer_title(" Layer ", false, &layers), None);
        assert_eq!(
            renamed_layer_title(" Group ", false, &layers),
            Some("Group")
        );
        assert_eq!(renamed_layer_title(" Layer ", true, &layers), Some("Layer"));
        assert_eq!(renamed_layer_title(" group ", true, &layers), Some("group"));
    }
}
