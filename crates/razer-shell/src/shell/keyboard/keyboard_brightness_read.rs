//! Actual brightness observation, independent of the local draft and source init setter.
use super::keyboard_brightness_runtime::Scope;
use super::*;
use razer_discovery::direct;

impl AppShell {
    pub(super) fn read_keyboard_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        generation: u64,
        background: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace
            .read(cx)
            .keyboard_brightness_read_matches(generation, cx)
        {
            return;
        }
        let device = workspace.read(cx).device(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(device) && direct::keyboard_brightness_available(route))
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness_read(
                    generation,
                    None,
                    Some("没有唯一的在线键盘连接，未读取亮度".into()),
                    false,
                    cx,
                )
            });
            return;
        };
        let route = (**route).clone();
        let canceled = workspace
            .read(cx)
            .keyboard_brightness_cancellation_guard(generation, None, cx);
        if self.close_requested || !self.devices.contains(&workspace) || canceled.is_none() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness_read(
                    generation,
                    None,
                    Some("键盘亮度请求范围已关闭".into()),
                    false,
                    cx,
                )
            });
            return;
        }
        let canceled = canceled.expect("Current request has its cancellation owner");
        let scope = Scope::new(&workspace, route, cx);
        let completion_scope = scope.clone();
        let completion_signal = canceled.clone();
        let window_handle = window.window_handle();
        let workspace = workspace.downgrade();
        self.start_keyboard_brightness_operation(
            scope,
            if background {
                super::keyboard_brightness_runtime::Operation::Reconcile
            } else {
                super::keyboard_brightness_runtime::Operation::Read
            },
            canceled,
            move |this, result, cx| {
                let Some(workspace) = workspace.upgrade() else {
                    return;
                };
                if !workspace
                    .read(cx)
                    .keyboard_brightness_read_matches(generation, cx)
                {
                    return;
                }
                let scope_current = completion_scope.current(this, &completion_signal, cx);
                if scope_current && let Some(metadata) = result.metadata() {
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_keyboard_metadata(Some(metadata.clone()), cx)
                    });
                    let candidate = this
                        .device_observations
                        .iter()
                        .find(|route| route.matches(workspace.read(cx).device(cx)))
                        .cloned();
                    if let Some(route) = candidate {
                        let owner = cx.entity().downgrade();
                        let workspace = workspace.clone();
                        let deferred_scope = completion_scope.clone();
                        let deferred_signal = completion_signal.clone();
                        cx.defer(move |cx| {
                            let _ = window_handle.update(cx, |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    if !deferred_scope.current(this, &deferred_signal, cx) {
                                        return;
                                    }
                                    this.read_keyboard_local_profiles(
                                        vec![(workspace, route)],
                                        window,
                                        cx,
                                    );
                                });
                            });
                        });
                    }
                }
                // The feature retains the actual reading independently of any
                // newer preview and queued write from this connection.
                let observed = result.observed();
                let error = result.error();
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_brightness_read(
                        generation,
                        observed,
                        error,
                        scope_current,
                        cx,
                    )
                });
                // Acceptance and worker cleanup do not depend on an OS window.
                let _ = window_handle.update(cx, |_, window, cx| {
                    workspace.update(cx, |workspace, cx| {
                        workspace.sync_keyboard_brightness_controls(window, cx)
                    });
                });
                if scope_current {
                    if let Some(message) = workspace.read(cx).keyboard_brightness_diagnostic(cx) {
                        this.status = message;
                        cx.notify();
                    }
                }
            },
            cx,
        );
    }
}
