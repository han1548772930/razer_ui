//! ECMAScript string behavior used by current source consumers.

use std::cmp::Ordering;

/// Number children use ECMAScript String(number), including non-finite values.
pub fn number_text(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else {
        crate::ecmascript_number::to_string(value).expect("finite ECMAScript number")
    }
}

/// Current AM/W$ probes the base and suffixes 1..99, then returns suffix 100
/// even if it collides. `Keymap` starts at suffix 1. The caller supplies the
/// source's strict string-name equality without flattening locale objects.
pub fn unique_profile_name(base: &str, mut exists: impl FnMut(&str) -> bool) -> String {
    let mut candidate = base.to_owned();
    for suffix in 1..=100 {
        if base == "Keymap" {
            candidate = format!("{base} {suffix}");
        }
        if !exists(&candidate) {
            return candidate;
        }
        candidate = format!("{base} {suffix}");
    }
    candidate
}

/// The current profile helper compares individual UTF-16 code units, grouping
/// its explicit ASCII punctuation before digits and the remaining characters.
/// Its `localeCompare` receives no locale argument; the host supplies one.
pub struct ProfileNameOrder {
    collator: icu_collator::CollatorBorrowed<'static>,
}

impl ProfileNameOrder {
    pub fn try_new(locale: &str) -> anyhow::Result<Self> {
        let locale: icu_locale_core::Locale = locale
            .parse()
            .map_err(|error| anyhow::anyhow!("Invalid host collation locale: {error}"))?;
        let collator =
            icu_collator::CollatorBorrowed::try_new(locale.into(), Default::default())
                .map_err(|error| anyhow::anyhow!("Cannot initialize host collation: {error}"))?;
        Ok(Self { collator })
    }

    pub fn compare(&self, left: &str, right: &str) -> Ordering {
        let mut left_units = left.encode_utf16();
        let mut right_units = right.encode_utf16();
        loop {
            match (left_units.next(), right_units.next()) {
                (Some(left), Some(right)) => {
                    let rank = profile_character_rank(left).cmp(&profile_character_rank(right));
                    if rank != Ordering::Equal {
                        return rank;
                    }
                    if left != right {
                        // Equal collation at the first different unit is a
                        // stable-sort tie; the source does not inspect suffixes.
                        return self.compare_unit(left, right);
                    }
                }
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
            }
        }
    }

    fn compare_unit(&self, left: u16, right: u16) -> Ordering {
        let surrogate = |unit| (0xd800..=0xdfff).contains(&unit);
        match (surrogate(left), surrogate(right)) {
            (true, true) => left.cmp(&right),
            (true, false) | (false, true) if left == 0xd7ff || right == 0xd7ff => left.cmp(&right),
            (left_surrogate, right_surrogate) => {
                // ICU's UTF-16 collation assigns isolated surrogates implicit
                // weights in code-point order. ICU4X replaces them with FFFD;
                // D7FF is the adjacent unassigned weight in the same group.
                // Other scalar weights remain on the same side of this group.
                let left = if left_surrogate { 0xd7ff } else { left };
                let right = if right_surrogate { 0xd7ff } else { right };
                self.collator.compare_utf16(&[left], &[right])
            }
        }
    }
}

fn profile_character_rank(unit: u16) -> u8 {
    if unit <= 0x7f && b"!@#$%^&*()_+-=[]{};':\"\\|,.<>/?~`".contains(&(unit as u8)) {
        1
    } else if (u16::from(b'0')..=u16::from(b'9')).contains(&unit) {
        2
    } else {
        3
    }
}

/// ECMAScript WhiteSpace and LineTerminator code points. FEFF participates;
/// NEL, the Mongolian vowel separator and the zero width space do not.
pub const fn is_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'..='\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// `String.prototype.trim` for UTF-8 input, retaining interior characters.
pub fn trim(value: &str) -> &str {
    value.trim_matches(is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::{ProfileNameOrder, is_whitespace, number_text, trim, unique_profile_name};
    use std::cmp::Ordering;

    #[test]
    fn number_children_keep_source_integer_exponent_and_nonfinite_spelling() {
        for (value, expected) in [
            (400., "400"),
            (0., "0"),
            (-0., "0"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ] {
            assert_eq!(number_text(value), expected);
        }
    }

    #[test]
    fn profile_name_collision_probes_stop_at_the_source_limit() {
        for base in ["Mouse", "Keymap"] {
            let mut probes = Vec::new();
            let name = unique_profile_name(base, |candidate| {
                probes.push(candidate.to_owned());
                true
            });
            assert_eq!(name, format!("{base} 100"));
            assert_eq!(probes.len(), 100);
            if base == "Keymap" {
                assert_eq!(probes[0], "Keymap 1");
                assert_eq!(probes[99], "Keymap 100");
            } else {
                assert_eq!(probes[0], "Mouse");
                assert_eq!(probes[99], "Mouse 99");
            }
        }
    }

    #[test]
    fn profile_names_keep_source_character_groups_and_lexical_digits() {
        let order = ProfileNameOrder::try_new("en-US").unwrap();
        let mut names = ["z", "2", "10", "#", "1", "!", "A", "a", ""];
        names.sort_by(|left, right| order.compare(left, right));
        assert_eq!(names, ["", "!", "#", "1", "10", "2", "a", "A", "z"]);
        assert_eq!(order.compare("9", " space"), Ordering::Less);
        assert_eq!(order.compare("name", "name 1"), Ordering::Less);
    }

    #[test]
    fn profile_collation_uses_the_host_locale_and_first_different_utf16_unit() {
        let english = ProfileNameOrder::try_new("en-US").unwrap();
        let swedish = ProfileNameOrder::try_new("sv-SE").unwrap();
        assert_eq!(english.compare("ä", "z"), Ordering::Less);
        assert_eq!(swedish.compare("ä", "z"), Ordering::Greater);
        assert_eq!(english.compare("\u{0340}z", "\u{0300}a"), Ordering::Equal);
        assert_eq!(english.compare("😀z", "😃a"), Ordering::Less);
        assert_eq!(english.compare("🧑", "😀"), Ordering::Greater);
        assert!(ProfileNameOrder::try_new("not a locale").is_err());
    }

    // WhiteSpace and LineTerminator are listed independently of the predicate
    // ranges to retain their complete language-defined membership.
    const SOURCE_WHITESPACE: [char; 25] = [
        '\u{0009}', '\u{000b}', '\u{000c}', '\u{0020}', '\u{00a0}', '\u{feff}', '\u{1680}',
        '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}',
        '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{202f}', '\u{205f}', '\u{3000}',
        '\u{000a}', '\u{000d}', '\u{2028}', '\u{2029}',
    ];

    #[test]
    fn all_source_whitespace_and_line_terminators_are_trimmed_at_both_ends() {
        for character in SOURCE_WHITESPACE {
            assert!(is_whitespace(character), "U+{:04X}", character as u32);
            let value = format!("{character}配置 7{character}");
            assert_eq!(trim(&value), "配置 7", "U+{:04X}", character as u32);
        }
        let whitespace: String = SOURCE_WHITESPACE.into_iter().collect();
        assert_eq!(trim(&format!("{whitespace}name{whitespace}")), "name");
    }

    #[test]
    fn nel_mongolian_separator_and_zero_width_space_remain_at_the_boundary() {
        for character in [
            '\u{0085}', '\u{180e}', '\u{200b}', '\u{0000}', '\u{001c}', '\u{007f}', '\u{3001}',
        ] {
            assert!(!is_whitespace(character), "U+{:04X}", character as u32);
            let value = format!("{character} name {character}");
            assert_eq!(trim(&value), value, "U+{:04X}", character as u32);
            assert_eq!(trim(&format!("\t{value}\u{feff}")), value);
        }
        assert_eq!(trim("\u{feff}42\u{feff}"), "42");
        assert_eq!(trim("\u{0085}42\u{0085}"), "\u{0085}42\u{0085}");
    }

    #[test]
    fn trimming_preserves_interior_whitespace_and_unicode() {
        let interior = "键\u{feff}\t\u{00a0}盘\r\n\u{2028}1\u{0085}2\u{180e}3\u{200b}4";
        assert_eq!(trim(&format!("\u{feff}\t{interior}\u{3000}\r")), interior);
        assert_eq!(trim(interior), interior);
        assert_eq!(
            trim("\t\u{0085}\u{feff}42\u{feff}\u{0085} "),
            "\u{0085}\u{feff}42\u{feff}\u{0085}"
        );
    }

    #[test]
    fn empty_and_whitespace_only_strings_have_empty_trimmed_content() {
        assert_eq!(trim(""), "");
        let whitespace: String = SOURCE_WHITESPACE.into_iter().collect();
        assert_eq!(trim(&whitespace), "");
        assert_eq!(trim("\r\n\u{feff}\u{3000}"), "");
    }
}
