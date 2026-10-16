use super::{MODULES, ModulePage};

#[test]
fn implemented_dashboard_modules_keep_their_named_host_tab_destinations() {
    let page_for = |id| {
        MODULES
            .iter()
            .find(|module| module.id == id)
            .and_then(|module| module.native_page)
    };

    assert_eq!(page_for("alexa"), None);
    assert_eq!(page_for("macro"), Some(ModulePage::Macro));
    assert_eq!(page_for("linked-games"), Some(ModulePage::Profiles));
    assert_eq!(page_for("armory"), None);
    assert_eq!(page_for("feedback"), None);
}
