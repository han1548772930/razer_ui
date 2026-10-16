use super::{OptionSpec, localized_choices};
use gpui_kit::component::select::SelectItem;
#[test]
fn source_locale_choices_keep_values_while_switching_visible_labels() {
    let options: Vec<OptionSpec> = serde_json::from_value(serde_json::json!([
        {"label": "CANCEL", "value": 0},
        {"label": "SAVE", "value": 9}
    ]))
    .unwrap();
    let english = localized_choices(&options, "en");
    for locale in ["zh-CN", "zh-cn", "ZH_CN"] {
        let chinese = localized_choices(&options, locale);
        assert_eq!(chinese[0].title().as_ref(), "取消");
        assert_eq!(chinese[1].title().as_ref(), "保存");
        for (before, after) in english.iter().zip(&chinese) {
            assert_eq!(before.value(), after.value());
            assert_ne!(before.title(), after.title());
        }
    }
    assert_eq!(options[0].value, serde_json::json!(0));
    assert_eq!(options[1].value, serde_json::json!(9));
    assert!(localized_choices(&[], "zh-CN").is_empty());
}
