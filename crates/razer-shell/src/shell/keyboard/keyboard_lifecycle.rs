//! Current PID 756 beforeunload/screenlocked/systemsuspend cancellation path.
//! Ordinary blur is distinct; suspend is recovered from current SysUtils in IDA.
use super::*;
use razer_platform::session_changes::{SessionChange, SessionMonitor};

pub(super) struct Owner {
    _monitor: SessionMonitor,
    _pump: Task<()>,
}

impl AppShell {
    pub(super) fn install_keyboard_lifecycle(&mut self, cx: &mut Context<Self>) {
        match SessionMonitor::start() {
            Ok((monitor, events)) => {
                let pump = cx.spawn(async move |shell, cx| {
                    while let Ok(event) = events.recv().await {
                        match event {
                            SessionChange::Locked | SessionChange::Suspended => {
                                if shell
                                    .update(cx, |shell, cx| {
                                        for workspace in &shell.devices {
                                            workspace.update(cx, |workspace, cx| {
                                                workspace.cancel_keyboard_calibration_v2(cx)
                                            });
                                        }
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            SessionChange::Unlocked | SessionChange::Resumed => {}
                            SessionChange::Failed(error) => {
                                eprintln!(
                                    "[keyboard lifecycle] session observer unavailable: {error}"
                                );
                                break;
                            }
                        }
                    }
                });
                self.keyboard_lifecycle = Some(Owner {
                    _monitor: monitor,
                    _pump: pump,
                });
            }
            Err(error) => eprintln!("[keyboard lifecycle] {error}"),
        }
        self.subscriptions.push(cx.on_app_quit(|shell, cx| {
            // Source beforeunload sends STOP(false) only while calibrating.
            for workspace in &shell.devices {
                workspace.update(cx, |workspace, cx| {
                    workspace.cancel_keyboard_calibration_v2(cx)
                });
            }
            shell.keyboard_lifecycle = None;
            async {}
        }));
    }
}
