//! Profile-file commands enter the retained product owner, never a page copy.
use super::*;
use razer_model::profile_transfer::{SourceExportSelection, SourceProfileImport};

impl ProductWorkspace {
    pub fn source_profile_macros(&self, cx: &App) -> Vec<serde_json::Value> {
        match &self.body {
            Body::Source(body) => body.read(cx).source_profile_macros().to_vec(),
            Body::Existing(_) => Vec::new(),
        }
    }

    pub fn import_global_profiles(
        &mut self,
        rows: &[serde_json::Value],
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match &self.body {
            Body::Source(body) => {
                body.update(cx, |body, cx| body.request_global_profile_import(rows, cx))
            }
            Body::Existing(_) => {
                Err("This profile owner requires its local profile importer".into())
            }
        }
    }

    pub fn export_global_profiles(
        &mut self,
        selection: &[SourceExportSelection],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.export_global_profiles(selection, window, cx)
            }),
            Body::Existing(_) => {
                Err("This profile owner requires its local profile exporter".into())
            }
        }
    }

    pub fn finish_source_profile_import(
        &mut self,
        request: &SourceProfileImport,
        receipt: Result<Device, String>,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.finish_source_profile_import(request, receipt, cx)
            }),
            Body::Existing(_) => Ok(false),
        }
    }

    pub fn refresh_transferred_profiles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.refresh_profile_choices(window, cx));
        }
    }
}
