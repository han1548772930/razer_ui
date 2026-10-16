//! Per-product windowClick class rules. Concrete GPUI callers supply the
//! original target/direct-parent classes and ancestor className strings.
use super::*;

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    whitelist: Vec<String>,
    raw_option_exemption: bool,
}
fn policy(pid: u32) -> Option<&'static Policy> {
    static DATA: OnceLock<Vec<Policy>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("mouse_window_click_current_data.json"))
            .expect("actual mounted mouse windowClick policies")
    })
    .iter()
    .find(|p| p.product_id == pid)
}
fn exempt(policy: &Policy, target: &[&str], parent: &[&str], ancestors: &[&str]) -> bool {
    policy.raw_option_exemption
        && ancestors
            .iter()
            .any(|classes| classes.contains("raw-text") && classes.contains("option"))
        || policy
            .whitelist
            .iter()
            .any(|class| target.contains(&class.as_str()) || parent.contains(&class.as_str()))
}
impl MouseProductWorkspace {
    /// The ordinary profile trigger, tab and history click bridges call this
    /// after their local handler. Background body/main-container stays exempt.
    /// Two-tap, sensitivity-stage and icon-refresh callers still require their
    /// source-specific mounted action paths; these three callers are none of them.
    pub(crate) fn current_mapping_document_click(
        &mut self,
        target: &[&str],
        parent: &[&str],
        ancestors: &[&str],
        inside_mapping: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id == 190 {
            return self.mapping_190_document_click(
                target,
                parent,
                ancestors,
                inside_mapping,
                window,
                cx,
            );
        }
        if self.mapping_input.is_none()
            || inside_mapping
            || self.current_mapping_confirmation_pending()
        {
            return false;
        }
        let Some(policy) = policy(self.spec.product_id) else {
            return false;
        };
        if exempt(policy, target, parent, ancestors) {
            return false;
        }
        self.request_current_mapping_close(window, cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn actual_162_window_click_has_distinct_exemptions() {
        let modern = policy(70).unwrap();
        let babel = policy(162).unwrap();
        assert!(exempt(modern, &["config-ctx"], &[], &[]));
        assert!(!exempt(babel, &["config-ctx"], &[], &[]));
        assert!(exempt(modern, &[], &[], &["s3 raw-text option selected"]));
        assert!(!exempt(babel, &[], &[], &["s3 raw-text option selected"]));
        // Whitelist is exact classList membership; raw option uses substring
        // tests on a single ancestor's complete className, as the source does.
        assert!(!exempt(modern, &["prefix-config-btn"], &[], &[]));
        assert!(!exempt(modern, &[], &[], &["raw-text", "option"]));
        assert!(exempt(babel, &[], &["body-wrapper"], &[]));
    }
}
