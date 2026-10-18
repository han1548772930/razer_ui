//! Keyboard brightness intent -> identity-matched direct Feature route -> readback.
use super::keyboard_brightness_runtime::Scope;
use super::*;
use razer_discovery::direct;

impl AppShell {
    pub(super) fn write_keyboard_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        generation: u64,
        percent: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace
            .read(cx)
            .keyboard_brightness_request_current(generation, percent, cx)
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
            self.status = "亮度未发送：没有唯一的在线键盘连接。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness(
                    generation,
                    percent,
                    None,
                    Some("没有唯一的在线键盘连接，未设置亮度".into()),
                    false,
                    cx,
                )
            });
            cx.notify();
            return;
        };
        let route = (**route).clone();
        let canceled = workspace.read(cx).keyboard_brightness_cancellation_guard(
            generation,
            Some(percent),
            cx,
        );
        if self.close_requested || !self.devices.contains(&workspace) || canceled.is_none() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness(
                    generation,
                    percent,
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
        self.status = "正在设置键盘亮度并读取设备确认。".into();
        cx.notify();
        self.start_keyboard_brightness_operation(
            scope,
            Some(percent),
            canceled,
            move |this, result, cx| {
                let Some(workspace) = workspace.upgrade() else {
                    return;
                };
                if !this.devices.contains(&workspace) {
                    workspace.update(cx, |workspace, cx| {
                        workspace.cancel_keyboard_brightness_connection(cx);
                        workspace.finish_keyboard_brightness(
                            generation,
                            percent,
                            None,
                            Some("键盘亮度页面已关闭".into()),
                            false,
                            cx,
                        );
                    });
                    return;
                }
                if !workspace
                    .read(cx)
                    .keyboard_brightness_request_matches(generation, percent, cx)
                {
                    return;
                }
                let scope_current = completion_scope.current(this, &completion_signal, cx)
                    && workspace
                        .read(cx)
                        .keyboard_brightness_request_current(generation, percent, cx);
                let observed = scope_current.then(|| result.observed()).flatten();
                let error = result.error();
                if scope_current
                    || workspace
                        .read(cx)
                        .keyboard_brightness_request_current(generation, percent, cx)
                {
                    this.status = if !scope_current {
                        "键盘连接已变化，尚未确认当前亮度。".into()
                    } else {
                        match (observed, error.as_deref()) {
                            (Some(actual @ 0..=100), None) => {
                                format!("设备已回读确认键盘亮度：{actual}%。")
                            }
                            (Some(actual @ 0..=100), Some(error)) => {
                                format!("设备已确认键盘亮度：{actual}%；{error}")
                            }
                            (Some(raw), error) => {
                                let mut message = format!("设备返回未知亮度：{raw}");
                                if let Some(error) = error {
                                    message.push_str(&format!("；{error}"));
                                }
                                message
                            }
                            (_, Some(error)) => format!("键盘亮度未确认：{error}。"),
                            (None, None) => {
                                unreachable!("Completion without a reading has an error")
                            }
                        }
                    };
                }
                // Completion may emit the queued newest desired setting. Set
                // status first so its new loading feedback is not overwritten.
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_brightness(
                        generation,
                        percent,
                        observed,
                        error,
                        scope_current,
                        cx,
                    )
                });
                let _ = window_handle.update(cx, |_, window, cx| {
                    workspace.update(cx, |workspace, cx| {
                        workspace.sync_keyboard_brightness_controls(window, cx)
                    });
                });
                cx.notify();
            },
            cx,
        );
    }
}
