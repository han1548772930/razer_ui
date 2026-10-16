//! Current mounted dpiStages reducer initial rows and local active-profile load.
//! Legacy uppercase DPIStages does not enter this lowercase producer branch.
use super::*;

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    initial_rows: Vec<Value>,
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| crate::data::mouse_mapping_stages_current_data!())
        .iter()
        .find(|p| p.product_id == product_id)
}
impl MouseProductWorkspace {
    pub(super) fn current_mouse_sensitivity_stages(&self) -> Option<&[Value]> {
        // Source loadActiveProfilesSettings only dispatches this lowercase
        // property when it is present. Without it, the reducer keeps its own
        // initial rows; do not normalize uppercase CONFIG data into that path.
        if self.draft.get("dpiStages").is_some() {
            return self
                .draft
                .pointer("/dpiStages/stages")
                .and_then(Value::as_array)
                .map(Vec::as_slice);
        }
        Some(&policy(self.spec.product_id)?.initial_rows)
    }
}
