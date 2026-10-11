//! Source helper 21107/1107 connected to retained host tab and window owners.
use super::*;
use razer_model::host_window_focus::{active_tab_changed, query_focus, selected_focus};

impl AppShell {
    pub(super) fn observe_controller_calibration_tab(
        &mut self,
        next: &Location,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = host_tabs::HostTabs::frame_name(next);
        for workspace in &self.devices {
            let Some(generation) = workspace.read(cx).trigger_calibration_generation(cx) else {
                continue;
            };
            let frame =
                host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
            let active = active_tab_changed(&frame, &selected);
            workspace.update(cx, |workspace, cx| {
                workspace.observe_trigger_calibration_focus(generation, active, window, cx)
            });
        }
    }
    pub(super) fn initialize_controller_calibration_focus(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A retry changes the calibration operation, not the mounted helper.
        if !workspace.read(cx).trigger_calibration_focus_pending(cx) {
            return;
        }
        let Some(generation) = workspace.read(cx).trigger_calibration_generation(cx) else {
            return;
        };
        let frame =
            host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
        let selected = host_tabs::HostTabs::frame_name(&self.location);
        let status = match host_tabs::host_window::status(window) {
            Ok(status) => status,
            Err(error) => {
                eprintln!("[calibration] initial window status unavailable: {error:#}");
                return;
            }
        };
        let active = query_focus("synapse", Some(&frame), status, Some(&selected));
        workspace.update(cx, |workspace, cx| {
            workspace.observe_trigger_calibration_focus(generation, active, window, cx)
        });
    }

    pub(super) fn observe_controller_calibration_window_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = host_tabs::HostTabs::frame_name(&self.location);
        // common.js sends focus(true/false) on BrowserWindow focus/blur.
        // The true branch only queries getFocusTab; false re-queries status.
        let focused = window.is_window_active();
        let status = if focused {
            None
        } else {
            match host_tabs::host_window::status(window) {
                Ok(status) => status,
                Err(error) => {
                    // A failed query does not produce a source observation.
                    eprintln!("[calibration] window status unavailable: {error:#}");
                    return;
                }
            }
        };
        for workspace in &self.devices {
            let Some(generation) = workspace.read(cx).trigger_calibration_generation(cx) else {
                continue;
            };
            let frame =
                host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
            let active = if focused {
                selected_focus("synapse", &frame, Some(&selected))
            } else {
                query_focus("synapse", Some(&frame), status, Some(&selected))
            };
            workspace.update(cx, |workspace, cx| {
                workspace.observe_trigger_calibration_focus(generation, active, window, cx)
            });
        }
    }
}
