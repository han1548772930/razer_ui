//! Current PID 756 beforeunload/screenlocked/systemsuspend cancellation path.
//! Ordinary blur is distinct; suspend is recovered from current SysUtils in IDA.
use super::*;
use razer_platform::session_changes::{SessionChange, SessionMonitor};

pub(super) struct Owner {
    monitor: SessionMonitor,
    pump: Task<()>,
}
impl Owner {
    fn retire(self) -> SessionMonitor {
        let Self { monitor, pump } = self;
        drop(pump);
        monitor
    }
}

impl AppShell {
    pub(super) fn install_keyboard_lifecycle(&mut self, cx: &mut Context<Self>) {
        self.start_keyboard_lifecycle(cx);
        self.register_exit_resume(|shell, cx| shell.start_keyboard_lifecycle(cx));
        self.register_exit_cleanup(|shell, cx| {
            // Source beforeunload sends STOP(false) only while calibrating.
            for workspace in &shell.devices {
                workspace.update(cx, |workspace, cx| {
                    workspace.cancel_keyboard_calibration_v2(cx)
                });
            }
            let monitor = shell.keyboard_lifecycle.take().map(Owner::retire);
            let cleanup = cx.background_executor().spawn(async move {
                if let Some(monitor) = monitor {
                    if let Err(error) = monitor.shutdown() {
                        eprintln!("[keyboard lifecycle] session observer cleanup failed: {error}");
                    }
                }
            });
            async move { cleanup.await }
        });
    }

    fn start_keyboard_lifecycle(&mut self, cx: &mut Context<Self>) {
        if self.keyboard_lifecycle.is_some() {
            return;
        }
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
                self.keyboard_lifecycle = Some(Owner { monitor, pump });
            }
            Err(error) => eprintln!("[keyboard lifecycle] {error}"),
        }
    }
}
