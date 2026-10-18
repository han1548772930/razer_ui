//! Retained PID158 software mapping publication. A publication receipt belongs
//! to the native engine; it does not acknowledge an onboard profile write.
use super::*;
use razer_service::mapping::device::{Reply, Session};
use std::time::Duration;

#[derive(Default)]
pub(super) struct State {
    owner: Option<Session>,
    pump: Option<Task<()>>,
    container: Option<String>,
    epoch: u64,
    revision: u64,
    requested: Option<MousePollingScope>,
    published: Option<MousePollingScope>,
    document: Option<Value>,
    stopping: bool,
    error: Option<String>,
}

impl State {
    pub(super) fn stop(&mut self) {
        // Dropping Session cancels admission. Its worker retains the native
        // cleanup and per-target lifecycle lock until removal finishes.
        self.owner = None;
        self.pump = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.container = None;
        self.requested = None;
        self.published = None;
        self.document = None;
        self.stopping = false;
    }

    pub(super) fn finish(&mut self) {
        self.stopping = true;
        if let Some(owner) = &self.owner {
            if let Err(error) = owner.request_stop_after_pending() {
                self.error = Some(error);
            }
        }
    }
}

impl MouseProductWorkspace {
    pub fn set_receiver_mapping_connection(&mut self, container: Option<String>) {
        if self.spec.product_id == 158 && self.receiver_mapping.container != container {
            self.receiver_mapping.stop();
            self.receiver_mapping.container = container;
        }
    }

    pub fn publish_receiver_mapping(
        &mut self,
        container: String,
        document: Value,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 158 || self.receiver_mapping.stopping {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        self.set_receiver_mapping_connection(Some(container.clone()));
        if self.receiver_mapping.document.as_ref() == Some(&document)
            && self.receiver_mapping.requested == Some(scope)
            && self.receiver_mapping.owner.is_some()
        {
            return;
        }
        if self.receiver_mapping.owner.is_none() {
            let (owner, replies) = match Session::start(158, container) {
                Ok(value) => value,
                Err(error) => {
                    self.receiver_mapping_failure(format!("{error:#}"));
                    return;
                }
            };
            self.receiver_mapping.owner = Some(owner);
            let epoch = self.receiver_mapping.epoch;
            self.receiver_mapping.pump = Some(cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                    let batch: Vec<_> = replies.try_iter().collect();
                    if !view.update(cx, |this, cx| {
                        if this.receiver_mapping.epoch != epoch {
                            return false;
                        }
                        let mut running = true;
                        for reply in batch {
                            match reply {
                                Reply::Ready => {}
                                Reply::Published { revision, result, .. } => {
                                    if revision != this.receiver_mapping.revision {
                                        continue;
                                    }
                                    match result {
                                        Ok(()) => {
                                            this.receiver_mapping.published = this.receiver_mapping.requested;
                                            this.receiver_mapping.error = None;
                                        }
                                        Err(error) => this.receiver_mapping_failure(error),
                                    }
                                }
                                Reply::Events(events) => {
                                    if !this.receiver_mapping.stopping
                                        && this.receiver_mapping.published.is_some()
                                        && this.receiver_mapping.published == this.mouse_polling_scope(cx)
                                    {
                                        for event in events {
                                            this.receiver_mapping_event(event, cx);
                                        }
                                    }
                                }
                                Reply::Failed(error) => {
                                    this.receiver_mapping_failure(error);
                                }
                                Reply::Stopped(result) => {
                                    this.receiver_mapping.owner = None;
                                    this.receiver_mapping.published = None;
                                    if let Err(error) = result {
                                        this.receiver_mapping_failure(error);
                                    }
                                    running = false;
                                }
                            }
                        }
                        running
                    }).unwrap_or(false) {
                        break;
                    }
                }
            }));
        }
        self.receiver_mapping.revision = self.receiver_mapping.revision.wrapping_add(1);
        self.receiver_mapping.requested = Some(scope);
        self.receiver_mapping.published = None;
        self.receiver_mapping.document = Some(document.clone());
        if let Some(owner) = &self.receiver_mapping.owner {
            if let Err(error) = owner.publish(self.receiver_mapping.revision, document, None) {
                self.receiver_mapping_failure(error);
            }
        }
    }

    fn receiver_mapping_failure(&mut self, error: String) {
        log::warn!(target: "razer_pages::mouse_mapping", "PID158 mapping: {error}");
        self.receiver_mapping.error = Some(error);
        self.receiver_mapping.published = None;
    }
}
