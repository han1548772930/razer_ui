//! One physical mapping session serves the current source-selected logical members.
//! Saved documents, service publication receipts and device readings stay separate.
use super::*;
use razer_discovery::receiver::mapping::{self, Document, Plan};
use razer_pages::features::ReceiverMappingPublication;
use razer_service::mapping::device::{Reply, Session};
use std::{collections::BTreeMap, sync::mpsc, time::Duration};
use uuid::Uuid;

use super::product_mapping::MappingTargetOwner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    logical: u64,
    physical: Option<u64>,
}

#[derive(Default)]
struct Publications {
    revision: u64,
    pending: BTreeMap<u32, Pending>,
}

impl Publications {
    fn request(&mut self, product: u32, revision: u64) {
        self.pending.insert(
            product,
            Pending {
                logical: revision,
                physical: None,
            },
        );
    }

    fn submit(&mut self) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        for pending in self.pending.values_mut() {
            pending.physical = None;
        }
        self.revision
    }

    fn projected(&mut self, revision: u64, products: &[u32]) {
        for (&product, pending) in &mut self.pending {
            pending.physical = products.contains(&product).then_some(revision);
        }
    }

    fn complete(&mut self, revision: u64) -> Vec<(u32, u64)> {
        let completed = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.physical == Some(revision))
            .map(|(&product, pending)| (product, pending.logical))
            .collect::<Vec<_>>();
        for (product, _) in &completed {
            self.pending.remove(product);
        }
        completed
    }

    fn fail_all(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(|(product, pending)| (product, pending.logical))
            .collect()
    }
}

struct Group {
    plan: Plan,
    members: BTreeMap<u32, MappingTargetOwner>,
    serials: BTreeMap<u32, Option<String>>,
    documents: BTreeMap<u32, Document>,
    publications: Publications,
    projection: Option<Task<()>>,
    session: Option<Session>,
    replies: mpsc::Receiver<Reply>,
    current: bool,
    draining: bool,
    failure: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    groups: BTreeMap<Uuid, Group>,
    pump: Option<Task<()>>,
}

fn workspace_for(
    members: &BTreeMap<u32, MappingTargetOwner>,
    product: u32,
    devices: &[Entity<ProductWorkspace>],
    cx: &App,
) -> Option<Entity<ProductWorkspace>> {
    let owner = members.get(&product)?;
    devices
        .iter()
        .find(|workspace| owner.matches(workspace, cx))
        .cloned()
}

fn acknowledge(
    group: &Group,
    owner: Uuid,
    requests: Vec<(u32, u64)>,
    result: Result<(), String>,
    devices: &[Entity<ProductWorkspace>],
    cx: &mut App,
) {
    for (product, revision) in requests {
        if let Some(workspace) = workspace_for(&group.members, product, devices, cx) {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_shared_mouse_mapping_publication(
                    owner,
                    revision,
                    result.clone(),
                    cx,
                )
            });
        }
    }
}

impl AppShell {
    pub(super) fn sync_receiver_mapping(&mut self, cx: &mut Context<Self>) {
        if self.close_requested {
            return;
        }
        let plans = match mapping::plans(
            &self.device_observations,
            self.local_pairing_records.as_ref(),
        ) {
            Ok(plans) => plans,
            Err(error) => {
                log::warn!("[receiver mapping selection] {error:#}");
                let current = self
                    .receiver_mapping
                    .groups
                    .iter()
                    .filter_map(|(owner, group)| group.current.then_some(*owner))
                    .collect::<Vec<_>>();
                for owner in current {
                    if let Some(group) = self.receiver_mapping.groups.get_mut(&owner) {
                        group.failure = Some(format!("Receiver mapping selection: {error:#}"));
                    }
                    self.retire_receiver_mapping(owner, cx);
                }
                return;
            }
        };
        let selected = plans
            .into_iter()
            .filter_map(|plan| {
                let mut members = BTreeMap::new();
                for member in plan.target().members() {
                    let candidates = self
                        .devices
                        .iter()
                        .filter(|workspace| {
                            let device = workspace.read(cx).device(cx);
                            device.product_id == member.product_id()
                                && device
                                    .device_container_id
                                    .eq_ignore_ascii_case(plan.target().device_container_id())
                                && self.device_observations.iter().any(|observed| {
                                    observed.matches(device)
                                        && observed.physical_product_id()
                                            == plan.target().physical_product_id()
                                        && Some(observed.connection())
                                            == device.dashboard.connection_observation
                                })
                        })
                        .collect::<Vec<_>>();
                    let [workspace] = candidates.as_slice() else {
                        return None;
                    };
                    members.insert(
                        member.product_id(),
                        MappingTargetOwner::capture(workspace, cx),
                    );
                }
                Some((plan, members))
            })
            .collect::<Vec<_>>();
        let retired = self
            .receiver_mapping
            .groups
            .iter()
            .filter_map(|(id, group)| {
                (group.current
                    && !selected
                        .iter()
                        .any(|(plan, members)| group.plan == *plan && group.members == *members))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in retired {
            self.retire_receiver_mapping(id, cx);
        }
        for (plan, members) in selected {
            let existing = self
                .receiver_mapping
                .groups
                .iter()
                .find(|(_, group)| group.current && group.plan == plan && group.members == members)
                .map(|(id, _)| *id);
            let id = if let Some(id) = existing {
                id
            } else {
                let (session, replies) = match Session::start_receiver(plan.target().clone()) {
                    Ok(started) => started,
                    Err(error) => {
                        log::warn!("[receiver mapping start] {error:#}");
                        continue;
                    }
                };
                let id = Uuid::new_v4();
                self.receiver_mapping.groups.insert(
                    id,
                    Group {
                        plan,
                        members,
                        serials: BTreeMap::new(),
                        documents: BTreeMap::new(),
                        publications: Publications::default(),
                        projection: None,
                        session: Some(session),
                        replies,
                        current: true,
                        draining: false,
                        failure: None,
                    },
                );
                id
            };
            let group = self
                .receiver_mapping
                .groups
                .get_mut(&id)
                .expect("selected group exists");
            let physical = group.plan.target().physical_product_id();
            let refresh = group
                .members
                .keys()
                .filter_map(|&product| {
                    let workspace = workspace_for(&group.members, product, &self.devices, cx)?;
                    let serial = workspace.read(cx).receiver_mapping_serial(cx);
                    if group.serials.get(&product) == Some(&serial) {
                        return None;
                    }
                    group.serials.insert(product, serial);
                    Some(workspace)
                })
                .collect::<Vec<_>>();
            for workspace in refresh {
                workspace.update(cx, |workspace, cx| {
                    workspace.configure_shared_mouse_mapping(id, physical, cx)
                });
            }
        }
        self.ensure_receiver_mapping_pump(cx);
    }

    pub(super) fn publish_receiver_mapping(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &ReceiverMappingPublication,
        cx: &mut Context<Self>,
    ) {
        let owner = request.owner();
        if !self.devices.contains(workspace) {
            return;
        }
        let Some(group) = self.receiver_mapping.groups.get_mut(&owner) else {
            return;
        };
        if !group.current
            || !group
                .members
                .get(&request.product_id())
                .is_some_and(|binding| binding.matches(workspace, cx))
        {
            return;
        }
        if group.draining {
            acknowledge(
                group,
                owner,
                vec![(request.product_id(), request.revision())],
                Err("Receiver mapping publication arrived after shutdown admission closed".into()),
                &self.devices,
                cx,
            );
            return;
        }
        group.documents.insert(
            request.product_id(),
            Document::new(
                request.document().clone(),
                workspace.read(cx).receiver_mapping_serial(cx),
            ),
        );
        group
            .publications
            .request(request.product_id(), request.revision());
        let revision = group.publications.submit();
        let plan = group.plan.clone();
        let documents = group.documents.clone();
        group.projection = Some(cx.spawn(async move |shell, cx| {
            let result = cx
                .background_spawn(async move { plan.project(&documents) })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                let Some(group) = shell.receiver_mapping.groups.get_mut(&owner) else {
                    return;
                };
                if !group.current || group.draining || group.publications.revision != revision {
                    return;
                }
                if group.members.values().any(|member| {
                    !shell
                        .devices
                        .iter()
                        .any(|workspace| member.matches(workspace, cx))
                }) {
                    shell.retire_receiver_mapping(owner, cx);
                    return;
                }
                group.projection = None;
                let result = result.and_then(|projection| {
                    let Some(projection) = projection else {
                        return Ok(());
                    };
                    group
                        .publications
                        .projected(revision, projection.products());
                    let session = group.session.as_ref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "{}",
                            group
                                .failure
                                .as_deref()
                                .unwrap_or("Receiver mapping session is stopped")
                        )
                    })?;
                    session
                        .publish(revision, projection.into_document(), None)
                        .map_err(anyhow::Error::msg)
                });
                if let Err(error) = result {
                    let pending = group.publications.fail_all();
                    acknowledge(
                        group,
                        owner,
                        pending,
                        Err(format!("{error:#}")),
                        &shell.devices,
                        cx,
                    );
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(true, cx);
                }
            });
        }));
    }

    fn retire_receiver_mapping(&mut self, owner: Uuid, cx: &mut Context<Self>) {
        let Some(group) = self.receiver_mapping.groups.get_mut(&owner) else {
            return;
        };
        group.current = false;
        group.projection = None;
        group.session = None;
        let pending = group.publications.fail_all();
        acknowledge(
            group,
            owner,
            pending,
            Err(group
                .failure
                .clone()
                .unwrap_or_else(|| "Receiver mapping connection changed".into())),
            &self.devices,
            cx,
        );
        for product in group.members.keys() {
            if let Some(workspace) = workspace_for(&group.members, *product, &self.devices, cx) {
                workspace.update(cx, |workspace, cx| {
                    workspace.release_shared_mouse_mapping(owner, group.failure.clone(), cx)
                });
            }
        }
    }

    pub(super) fn retain_receiver_mapping_connections(
        &mut self,
        retained: &[Entity<ProductWorkspace>],
        cx: &mut Context<Self>,
    ) {
        let retired = self
            .receiver_mapping
            .groups
            .iter()
            .filter_map(|(id, group)| {
                (group.current
                    && group.members.values().any(|member| {
                        !retained
                            .iter()
                            .any(|workspace| member.matches(workspace, cx))
                    }))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in retired {
            self.retire_receiver_mapping(id, cx);
        }
    }

    pub(super) fn finish_receiver_mapping_exit(&mut self) -> bool {
        for group in self
            .receiver_mapping
            .groups
            .values_mut()
            .filter(|group| group.current && !group.draining)
        {
            if group.projection.is_some() {
                continue;
            }
            group.draining = true;
            if let Some(session) = &group.session {
                if let Err(error) = session.request_stop_after_pending() {
                    group.failure = Some(error);
                    group.session = None;
                }
            }
        }
        !self.receiver_mapping.groups.is_empty()
    }

    pub(super) fn resume_receiver_mapping(&mut self, cx: &mut Context<Self>) {
        for group in self
            .receiver_mapping
            .groups
            .values_mut()
            .filter(|group| group.current)
        {
            group.serials.clear();
        }
        self.sync_receiver_mapping(cx);
    }

    pub(super) fn install_receiver_mapping_cleanup(&mut self) {
        self.register_exit_cleanup(|shell, cx| {
            shell.receiver_mapping.pump = None;
            let groups = std::mem::take(&mut shell.receiver_mapping.groups);
            let replies = groups.into_values().map(|mut group| {
                group.projection = None;
                group.session = None;
                group.replies
            }).collect::<Vec<_>>();
            let cleanup = cx.background_spawn(async move {
                for replies in replies {
                    loop {
                        match replies.recv() {
                            Ok(Reply::Stopped(result)) => {
                                if let Err(error) = result { log::warn!("[receiver mapping cleanup] {error}"); }
                                break;
                            }
                            Ok(Reply::Failed(error)) => log::warn!("[receiver mapping cleanup] {error}"),
                            Ok(_) => {},
                            Err(_) => {
                                log::warn!("[receiver mapping cleanup] worker closed without a stop receipt");
                                break;
                            }
                        }
                    }
                }
            });
            async move { cleanup.await }
        });
    }

    fn ensure_receiver_mapping_pump(&mut self, cx: &mut Context<Self>) {
        if self.receiver_mapping.pump.is_some() || self.receiver_mapping.groups.is_empty() {
            return;
        }
        self.receiver_mapping.pump = Some(cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if !shell
                    .update(cx, |shell, cx| shell.poll_receiver_mapping(cx))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }

    fn poll_receiver_mapping(&mut self, cx: &mut Context<Self>) -> bool {
        let mut completed = Vec::new();
        let mut resume = false;
        for (&owner, group) in &mut self.receiver_mapping.groups {
            loop {
                let reply = match group.replies.try_recv() {
                    Ok(reply) => reply,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => Reply::Stopped(Err(
                        "Receiver mapping worker closed without a stop receipt".into(),
                    )),
                };
                match reply {
                    Reply::Ready => {}
                    Reply::Published {
                        revision, result, ..
                    } => {
                        let pending = group.publications.complete(revision);
                        acknowledge(group, owner, pending, result, &self.devices, cx);
                    }
                    Reply::Events(events) if group.current && !group.draining => {
                        for event in events {
                            match group.plan.event_targets(&event) {
                                Ok(products) => {
                                    for product in products {
                                        if let Some(workspace) = workspace_for(
                                            &group.members,
                                            product,
                                            &self.devices,
                                            cx,
                                        ) {
                                            workspace.update(cx, |workspace, cx| {
                                                workspace.observe_shared_mouse_mapping_event(
                                                    owner,
                                                    event.clone(),
                                                    cx,
                                                )
                                            });
                                        }
                                    }
                                }
                                Err(error) => log::warn!("[receiver mapping callback] {error:#}"),
                            }
                        }
                    }
                    Reply::Events(_) => {}
                    Reply::Failed(error) => {
                        group.failure = Some(error.clone());
                        let pending = group.publications.fail_all();
                        acknowledge(group, owner, pending, Err(error.clone()), &self.devices, cx);
                        log::warn!("[receiver mapping] {error}");
                    }
                    Reply::Stopped(result) => {
                        resume |= group.current && group.draining && !self.close_requested;
                        group.session = None;
                        group.projection = None;
                        let error = result.err().or_else(|| group.failure.clone());
                        let pending = group.publications.fail_all();
                        acknowledge(
                            group,
                            owner,
                            pending,
                            Err(error.clone().unwrap_or_else(|| {
                                "Receiver mapping stopped before publication".into()
                            })),
                            &self.devices,
                            cx,
                        );
                        for product in group.members.keys() {
                            if let Some(workspace) =
                                workspace_for(&group.members, *product, &self.devices, cx)
                            {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.release_shared_mouse_mapping(owner, error.clone(), cx)
                                });
                            }
                        }
                        if let Some(error) = error {
                            log::warn!("[receiver mapping cleanup] {error}");
                        }
                        completed.push(owner);
                        break;
                    }
                }
            }
        }
        for owner in completed {
            self.receiver_mapping.groups.remove(&owner);
        }
        if self.receiver_mapping.groups.is_empty() {
            self.receiver_mapping.pump = None;
            if self.close_requested && self.save_task.is_none() {
                self.continue_save_queue(true, cx);
            } else if resume {
                self.sync_receiver_mapping(cx);
            }
            false
        } else {
            if resume {
                self.sync_receiver_mapping(cx);
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Publications;

    #[test]
    fn newer_physical_document_owns_all_pending_logical_confirmations() {
        let mut publications = Publications::default();
        publications.request(158, 7);
        let first = publications.submit();
        publications.projected(first, &[158]);
        publications.request(190, 3);
        let second = publications.submit();
        publications.projected(second, &[158, 190]);
        assert!(publications.complete(first).is_empty());
        assert_eq!(publications.complete(second), [(158, 7), (190, 3)]);
        assert!(publications.complete(second).is_empty());
    }

    #[test]
    fn superseded_logical_revision_cannot_acknowledge_a_new_save() {
        let mut publications = Publications::default();
        publications.request(190, 8);
        let first = publications.submit();
        publications.projected(first, &[190]);
        publications.request(190, 9);
        let second = publications.submit();
        publications.projected(second, &[190]);
        assert!(publications.complete(first).is_empty());
        assert_eq!(publications.complete(second), [(190, 9)]);
    }

    #[test]
    fn failure_resolves_every_unconfirmed_logical_request() {
        let mut publications = Publications::default();
        publications.request(158, 1);
        publications.request(190, 2);
        publications.submit();
        assert_eq!(publications.fail_all(), [(158, 1), (190, 2)]);
        assert!(publications.fail_all().is_empty());
    }

    #[test]
    fn omitted_peer_keeps_its_save_unconfirmed_until_it_is_in_a_publication() {
        let mut publications = Publications::default();
        publications.request(158, 1);
        publications.request(190, 2);
        let master_only = publications.submit();
        publications.projected(master_only, &[158]);
        assert_eq!(publications.complete(master_only), [(158, 1)]);
        let combined = publications.submit();
        publications.projected(combined, &[158, 190]);
        assert_eq!(publications.complete(combined), [(190, 2)]);
    }
}
