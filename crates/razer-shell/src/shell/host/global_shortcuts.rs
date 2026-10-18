//! Composes committed local snapshots with the long-lived mapping service.
use super::*;
use razer_model::{macro_library::MacroLibraryFile, shortcuts::Shortcut};
use razer_service::mapping::shortcuts::{Reply, Session};

#[derive(Clone, PartialEq)]
struct Snapshot {
    shortcuts: Vec<Shortcut>,
    macros: MacroLibraryFile,
}

struct Owner {
    session: Session,
    _pump: Task<()>,
    generation: u64,
    revision: u64,
    last_submitted: Option<Snapshot>,
    stopping: bool,
}

#[derive(Default)]
pub(super) struct State {
    owner: Option<Owner>,
    generation: u64,
    desired_running: bool,
}

impl AppShell {
    pub(super) fn republish_global_shortcuts(&mut self, cx: &mut Context<Self>) {
        if let Some(owner) = &mut self.global_shortcuts.owner {
            owner.last_submitted = None;
        }
        self.publish_global_shortcuts(cx);
    }

    pub(super) fn install_global_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.global_shortcuts.desired_running = true;
        self.start_global_shortcuts(cx);
        self.register_exit_cleanup(|shell, cx| {
            shell.global_shortcuts.desired_running = false;
            let owner = shell.global_shortcuts.owner.take();
            let executor = cx.background_executor().clone();
            async move {
                if let Some(owner) = owner {
                    if let Err(error) = owner.session.request_stop_after_pending() {
                        eprintln!("[global shortcuts] exit drain failed: {error}");
                    }
                    while !owner.session.is_finished() {
                        executor.timer(std::time::Duration::from_millis(25)).await;
                    }
                }
            }
        });
    }

    fn start_global_shortcuts(&mut self, cx: &mut Context<Self>) {
        if self.global_shortcuts.owner.is_some() || !self.global_shortcuts.desired_running {
            return;
        }
        let Some(generation) = self.global_shortcuts.generation.checked_add(1) else {
            self.status = "快捷键后台会话代次已耗尽。".into();
            return;
        };
        let (session, replies) = match Session::start() {
            Ok(owner) => owner,
            Err(error) => {
                self.status = format!("快捷键后台服务启动失败：{error}");
                return;
            }
        };
        self.global_shortcuts.generation = generation;
        let pump = cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(25)).await;
                let mut batch = Vec::new();
                let mut disconnected = false;
                for _ in 0..128 {
                    match replies.try_recv() {
                        Ok(reply) => batch.push(reply),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
                if batch.is_empty() && !disconnected { continue; }
                let finished = shell.update(cx, |shell, cx| {
                    if !shell.global_shortcuts.owner.as_ref().is_some_and(|owner| owner.generation == generation) {
                        return true;
                    }
                    let mut stopped = false;
                    for reply in batch {
                        match reply {
                            Reply::Ready => {},
                            Reply::Published { revision, result } => {
                                let owner = shell.global_shortcuts.owner.as_ref().expect("Current shortcut owner");
                                match result {
                                    Ok(()) if revision == owner.revision && !owner.stopping => {
                                        shell.status = "快捷键映射已由本地引擎接收".into();
                                    },
                                    Err(error) => {
                                        if revision == owner.revision && !owner.stopping {
                                            shell.status = format!("快捷键引擎未接收映射：{error}");
                                            if let Some(owner) = &mut shell.global_shortcuts.owner {
                                                owner.last_submitted = None;
                                            }
                                        } else {
                                            eprintln!("[global shortcuts] revision {revision} failed: {error}");
                                        }
                                    },
                                    Ok(()) => {},
                                }
                            },
                            Reply::Unsupported(_) => {
                                shell.status = "快捷键引擎报告了不支持的映射。".into();
                            },
                            Reply::Failed(error) => {
                                shell.status = format!("快捷键后台服务失败：{error}");
                            },
                            Reply::Stopped(result) => {
                                if let Err(error) = result {
                                    shell.status = format!("快捷键后台清理失败：{error}");
                                }
                                stopped = true;
                            },
                        }
                    }
                    if disconnected && !stopped {
                        shell.status = "快捷键后台回执通道已关闭。".into();
                        stopped = shell.global_shortcuts.owner.as_ref().is_some_and(|owner| owner.session.is_finished());
                    }
                    if stopped {
                        let expected_stop = shell.global_shortcuts.owner.as_ref().is_some_and(|owner| owner.stopping);
                        // The worker releases native state before Stopped. A canceled
                        // exit can now install a distinct successor without overlap.
                        shell.global_shortcuts.owner = None;
                        if expected_stop && shell.global_shortcuts.desired_running {
                            shell.start_global_shortcuts(cx);
                        }
                    }
                    cx.notify();
                    stopped
                });
                if finished.unwrap_or(true) { break; }
            }
        });
        self.global_shortcuts.owner = Some(Owner {
            session,
            _pump: pump,
            generation,
            revision: 0,
            last_submitted: None,
            stopping: false,
        });
        self.publish_global_shortcuts(cx);
    }

    pub(super) fn publish_global_shortcuts(&mut self, cx: &mut Context<Self>) {
        if self.global_shortcuts.owner.is_none() {
            self.start_global_shortcuts(cx);
            return;
        }
        let snapshot = Snapshot {
            shortcuts: self.shortcuts.read(cx).snapshot(),
            macros: self.macro_library.read(cx).snapshot(),
        };
        let Some(owner) = self
            .global_shortcuts
            .owner
            .as_mut()
            .filter(|owner| !owner.stopping)
        else {
            return;
        };
        if owner.last_submitted.as_ref() == Some(&snapshot) {
            return;
        }
        let Some(revision) = owner.revision.checked_add(1) else {
            self.status = "快捷键后台请求序号已耗尽。".into();
            return;
        };
        match owner.session.publish(
            revision,
            snapshot.shortcuts.clone(),
            snapshot.macros.clone(),
        ) {
            Ok(()) => {
                owner.revision = revision;
                owner.last_submitted = Some(snapshot);
            }
            Err(error) => self.status = format!("快捷键后台提交失败：{error}"),
        }
    }

    pub(super) fn stop_global_shortcuts(&mut self) {
        self.global_shortcuts.desired_running = false;
        if let Some(owner) = &mut self.global_shortcuts.owner {
            owner.stopping = true;
            if let Err(error) = owner.session.request_stop_after_pending() {
                self.status = format!("快捷键后台排空失败：{error}");
            }
        }
    }

    pub(super) fn resume_global_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.global_shortcuts.desired_running = true;
        self.start_global_shortcuts(cx);
    }
}
