//! Keyboard page ownership inside a shared physical receiver session.
use super::*;
use crate::features::ReceiverMappingPublication;

#[derive(Clone, Copy)]
pub(super) struct Owner {
    id: uuid::Uuid,
    physical_product: u32,
    generation: u64,
}

#[cfg(test)]
mod tests {
    use super::{KeyboardProductWorkspace, ReceiverMappingPublication};
    use serde_json::json;
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, px, size};

    #[gpui_kit::test]
    fn keyboard_receiver_saved_publications_keep_one_owner_and_reject_superseded_work(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        for product in razer_device::keyboard_receiver_mapping::PRODUCTS {
            let mut page = None;
            let window = cx.open_window(size(px(1100.), px(800.)), |window, cx| {
                let body = cx.new(|cx| KeyboardProductWorkspace::new(product, false, window, cx));
                page = Some(body.clone());
                Root::new(body, window, cx)
            });
            let page = page.unwrap();
            let events = cx.new(|_| Vec::<ReceiverMappingPublication>::new());
            let _subscription = events.update(cx, |_, cx| {
                cx.subscribe(&page, |events, _, event: &ReceiverMappingPublication, _| {
                    events.push(event.clone())
                })
            });
            let owner = uuid::Uuid::new_v4();
            let successor = uuid::Uuid::new_v4();
            cx.update_window(window.into(), |_, window, cx| {
                page.update(cx, |page, cx| {
                    let mut draft = page.snapshot();
                    draft["guid"] = "saved-profile".into();
                    page.restore(Some(&draft), window, cx);
                    page.configure_shared_keyboard_mapping(owner, 197, "receiver".into(), 4, cx);
                    page.publish_shared_keyboard_mapping(
                        json!({"productId":product,"activeProfile":"saved-profile","version":1}),
                        cx,
                    );
                });
            })
            .unwrap();
            cx.run_until_parked();
            let first = events.read_with(cx, |events, _| {
                assert_eq!(events.len(), 1);
                events[0].clone()
            });
            page.update(cx, |page, cx| {
                assert!(page.legacy_mapping.owner.is_none());
                assert!(page.legacy_mapping.published.is_none());
                page.observe_running_games(Some(1), cx);
                page.publish_shared_keyboard_mapping(
                    json!({"productId":product,"activeProfile":"saved-profile","version":2}),
                    cx,
                );
                assert!(!page.observe_shared_keyboard_publication(
                    owner,
                    first.revision(),
                    Ok(()),
                    cx
                ));
            });
            cx.run_until_parked();
            let second = events.read_with(cx, |events, _| {
                assert_eq!(
                    events.len(),
                    2,
                    "runtime observation belongs to the shared projection owner"
                );
                events[1].clone()
            });
            page.update(cx, |page, cx| {
                assert!(page.observe_shared_keyboard_publication(
                    owner,
                    second.revision(),
                    Err("publication failed".into()),
                    cx
                ));
                assert!(!page.legacy_mapping.ready);
                assert!(page.legacy_mapping.published.is_none());
                assert!(!page.observe_shared_keyboard_publication(
                    owner,
                    second.revision(),
                    Ok(()),
                    cx
                ));
                page.publish_shared_keyboard_mapping(
                    json!({"productId":product,"activeProfile":"saved-profile","version":3}),
                    cx,
                );
            });
            cx.run_until_parked();
            let retry = events.read_with(cx, |events, _| {
                assert_eq!(events.len(), 3);
                events[2].clone()
            });
            page.update(cx, |page, cx| {
                assert!(page.observe_shared_keyboard_publication(
                    owner,
                    retry.revision(),
                    Ok(()),
                    cx
                ));
                assert!(page.legacy_mapping.ready);
                assert!(page.observe_shared_keyboard_event(
                    owner,
                    json!({"jsonEvent":{"type":"ignored"}}),
                    cx
                ));
                page.configure_shared_keyboard_mapping(successor, 197, "receiver".into(), 5, cx);
                page.release_shared_keyboard_mapping(owner, Some("old cleanup".into()), cx);
                assert!(page.shared_keyboard_mapping_matches("receiver", 5));
                assert!(!page.observe_shared_keyboard_publication(
                    owner,
                    second.revision(),
                    Ok(()),
                    cx
                ));
                assert!(!page.observe_shared_keyboard_event(
                    owner,
                    json!({"jsonEvent":{"type":"ignored"}}),
                    cx
                ));
                page.finish_mapping_service();
                assert!(!page.shared_keyboard_mapping_matches("receiver", 5));
                assert!(!page.observe_shared_keyboard_event(
                    successor,
                    json!({"jsonEvent":{"type":"ignored"}}),
                    cx
                ));
            });
        }
    }
}

impl EventEmitter<ReceiverMappingPublication> for KeyboardProductWorkspace {}

impl KeyboardProductWorkspace {
    pub fn shared_keyboard_owner(&self)->Option<uuid::Uuid> {
        self.legacy_mapping.shared_owner.map(|owner|owner.id)
    }
    pub fn shared_keyboard_mapping_matches(&self, container: &str, generation: u64) -> bool {
        self.legacy_mapping
            .shared_owner
            .is_some_and(|owner| owner.generation == generation)
            && self.legacy_mapping.container.as_deref() == Some(container)
            && !self.legacy_mapping.stopping
    }

    pub fn configure_shared_keyboard_mapping(
        &mut self,
        id: uuid::Uuid,
        physical_product: u32,
        container: String,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        if !razer_device::keyboard_receiver_mapping::supports(self.spec.product_id) {
            return;
        }
        if self.shared_keyboard_mapping_matches(&container, generation)
            && self
                .legacy_mapping
                .shared_owner
                .is_some_and(|owner| owner.id == id && owner.physical_product == physical_product)
        {
            return;
        }
        self.stop_shared_keyboard_mapping();
        self.legacy_mapping.container = Some(container);
        self.legacy_mapping.callback_generation = generation;
        self.legacy_mapping.stopping = false;
        self.legacy_mapping.shared_owner = Some(Owner {
            id,
            physical_product,
            generation,
        });
        self.legacy_mapping.poll = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let running = view
                    .update(cx, |this, cx| {
                        if this.legacy_mapping.stopping
                            || !this.legacy_mapping.shared_owner.is_some_and(|owner| {
                                owner.id == id && owner.generation == generation
                            })
                        {
                            return false;
                        }
                        this.repeat_legacy_brightness(cx);
                        true
                    })
                    .unwrap_or(false);
                if !running {
                    break;
                }
            }
        }));
    }

    pub(super) fn stop_shared_keyboard_mapping(&mut self) {
        if self.legacy_mapping.shared_owner.take().is_none() {
            return;
        }
        self.legacy_mapping.poll = None;
        self.legacy_mapping.ready = false;
        self.legacy_mapping.published = None;
        self.legacy_mapping.shared_completed = None;
        self.legacy_mapping.document = None;
        self.legacy_mapping.held_brightness.clear();
        self.legacy_mapping.error = None;
    }

    pub fn publish_shared_keyboard_mapping(&mut self, document: Value, cx: &mut Context<Self>) {
        let Some(owner) = self.legacy_mapping.shared_owner else {
            return;
        };
        if self.legacy_mapping.stopping
            || owner.generation != self.legacy_mapping.callback_generation
        {
            return;
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity {
            product_id: u32,
            active_profile: String,
        }
        let identity = match serde_json::from_value::<Identity>(document.clone()) {
            Ok(identity)
                if identity.product_id == self.spec.product_id
                    && !identity.active_profile.is_empty() =>
            {
                identity
            }
            _ => {
                self.mapping_failure("已保存键盘映射的产品或配置身份无效".into(), cx);
                return;
            }
        };
        if self.draft["guid"].as_str() != Some(identity.active_profile.as_str()) {
            return;
        }
        if self.legacy_mapping.document.as_ref() == Some(&document)
            && self.legacy_mapping.published == Some(self.legacy_mapping.revision)
        {
            return;
        }
        self.legacy_mapping.revision = self.legacy_mapping.revision.wrapping_add(1);
        self.legacy_mapping.shared_completed = None;
        self.legacy_mapping.document = Some(document.clone());
        self.legacy_mapping.held_brightness.clear();
        self.legacy_mapping.error = None;
        cx.emit(ReceiverMappingPublication::new(
            owner.id,
            self.spec.product_id,
            self.legacy_mapping.revision,
            document,
        ));
        cx.notify();
    }

    pub fn observe_shared_keyboard_publication(
        &mut self,
        owner: uuid::Uuid,
        revision: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.legacy_mapping.shared_owner.is_some_and(|current| {
            current.id == owner && current.generation == self.legacy_mapping.callback_generation
        }) || self.legacy_mapping.stopping
            || revision != self.legacy_mapping.revision
            || self.legacy_mapping.shared_completed == Some(revision)
        {
            return false;
        }
        self.legacy_mapping.shared_completed = Some(revision);
        match result {
            Ok(()) => {
                self.legacy_mapping.published = Some(revision);
                self.legacy_mapping.ready = true;
                self.legacy_mapping.error = None;
            }
            Err(error) => {
                self.legacy_mapping.ready = false;
                self.mapping_failure(error, cx);
            }
        }
        cx.notify();
        true
    }

    fn shared_keyboard_events_ready(&self) -> bool {
        self.legacy_mapping
            .shared_owner
            .is_some_and(|owner| owner.generation == self.legacy_mapping.callback_generation)
            && !self.legacy_mapping.stopping
            && self.legacy_mapping.published == Some(self.legacy_mapping.revision)
            && self
                .legacy_mapping
                .document
                .as_ref()
                .is_some_and(|document| document["activeProfile"] == self.draft["guid"])
    }

    pub fn observe_shared_keyboard_event(
        &mut self,
        owner: uuid::Uuid,
        event: Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .legacy_mapping
            .shared_owner
            .is_some_and(|current| current.id == owner)
            || !self.shared_keyboard_events_ready()
        {
            return false;
        }
        self.legacy_mapping_event(event, cx);
        true
    }

    pub fn release_shared_keyboard_mapping(
        &mut self,
        owner: uuid::Uuid,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self
            .legacy_mapping
            .shared_owner
            .is_some_and(|current| current.id == owner)
        {
            self.stop_shared_keyboard_mapping();
            if let Some(error) = error {
                self.mapping_failure(error, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn consume_shared_keyboard_interdevice(
        &mut self,
        mut event: Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.shared_keyboard_events_ready() {
            return false;
        }
        event["type"] = "unsupportedmapping".into();
        self.legacy_mapping_event(json!({"jsonEvent":event}), cx);
        true
    }
}
