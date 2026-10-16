//! Current Noa sound persistence and guarded physical submission.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{AudioProductWorkspace, LeviathanRequest};
use std::sync::{Mutex, OnceLock};

static QUEUE: OnceLock<Mutex<()>> = OnceLock::new();
#[path="leviathan_events.rs"]
mod events;
pub(super) struct LeviathanSave {
    workspace: Entity<ProductWorkspace>,
    page: Entity<AudioProductWorkspace>,
    request: LeviathanRequest,
    identity: String,
    profile_id: String,
}
impl AppShell {
    pub(super) fn request_leviathan(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: LeviathanRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if !page.read(cx).leviathan_request_current(&request) {
            return;
        }
        if !request.persists() {
            self.submit_leviathan(workspace, page, request, cx);
            return;
        }
        if let Some(error) = self.storage_error.clone() {
            self.status = format!("声音设置本地保存失败：{error}");
            page.update(cx, |page, cx| {
                page.finish_leviathan_task(request.generation, Err(error), cx)
            });
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let profile_id = workspace.read(cx).device(cx).active_profile.clone();
        let file = store::WorkspaceFile::new(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            self.tracking_intro_seen,
        )
        .with_shortcuts(self.shortcuts.read(cx).saved_snapshot())
        .with_preferences(self.settings.read(cx).snapshot())
        .with_custom_colors(cx.global::<CustomColors>().colors())
        .with_host_tab_order(self.host_tabs.order())
        .with_dashboard(self.dashboard_state.read(cx).snapshot())
        .with_module_services(self.module_catalog.read(cx).service_snapshot())
        .with_macros(self.macro_library.read(cx).snapshot());
        self.pending_saves.push_back(PreparedSave {
            window: window.window_handle(),
            file,
            mouse_polling: None,
            mouse_dpi_stages: None,
            mouse_dpi: None,
            mouse_current_dpi: None,
            mouse_current_power: None,
            controller_trigger: None,
            seiren_eq: None,
            mouse_profile: None,
            keyboard_actuation: None,
            leviathan: Some(LeviathanSave {
                workspace,
                page,
                request,
                identity,
                profile_id,
            }),
        });
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_leviathan_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(sound) = &save.leviathan else {
            return Ok(());
        };
        let (field, value) = sound
            .request
            .profile_field()
            .context("Noa profile field is absent")?;
        let mut found = false;
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut device = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == sound.identity {
                    let profile = device
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == sound.profile_id)
                        .context("Noa source profile was removed")?;
                    let settings = profile
                        .source_settings
                        .get_or_insert_with(|| serde_json::json!({}));
                    if settings.get("profile").is_none() {
                        settings["profile"] = serde_json::json!({});
                    }
                    ensure!(
                        settings["profile"].is_object(),
                        "Noa source profile settings differ"
                    );
                    settings["profile"][field] = value.clone();
                    let document=device.source_device_settings.get_or_insert_with(||serde_json::json!({}));
                    ensure!(document.is_object(),"Noa source device document differs");
                    let version=razer_device::mapping_submission::next_source_version(document.get("version").and_then(serde_json::Value::as_u64))?;
                    document["version"]=serde_json::json!(version);
                    found = true;
                }
                Ok(device)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        ensure!(found, "Noa local profile owner disappeared");
        Ok(())
    }
    pub(super) fn finish_leviathan_save(
        &mut self,
        save: LeviathanSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if succeeded&&!save.request.writes_device(){
            save.page.update(cx,|page,cx|page.finish_leviathan_local(save.request.generation,cx));return;
        }
        if !succeeded
            || self.close_requested
            || !save.page.read(cx).leviathan_request_current(&save.request)
        {
            save.page.update(cx, |page, cx| {
                page.finish_leviathan_task(
                    save.request.generation,
                    Err(if succeeded {
                        "声音设置已保存到本机，提交范围已取消"
                    } else {
                        "声音设置本地保存失败，未写设备"
                    }
                    .into()),
                    cx,
                )
            });
            return;
        }
        self.submit_leviathan(save.workspace, save.page, save.request, cx);
    }
    fn submit_leviathan(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        page: Entity<AudioProductWorkspace>,
        request: LeviathanRequest,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || !page.read(cx).leviathan_request_current(&request) {
            return;
        }
        let device = workspace.read(cx).device(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|observed| {
                observed.matches(device)
                    && observed.product_id() == 1352
                    && observed.physical_product_id() == 1352
                    && observed.peer_product_id().is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            let error = "没有唯一的 Leviathan V2 Pro 物理设备观察，未确认设备读写".to_owned();
            self.status = error.clone();
            page.update(cx, |page, cx| {
                page.finish_leviathan_task(request.generation, Err(error), cx)
            });
            cx.notify();
            return;
        }
        let observed = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let profile_id = device.active_profile.clone();
        if request.operation.command().is_read() {
            self.start_leviathan_monitor(workspace.clone(),page.clone(),observed.clone(),request.guard(),cx);
        }
        cx.spawn(async move |this,cx| {
            let guard=request.clone();
            let selected=observed.clone();
            let result=cx.background_spawn(async move {
                let _serial=QUEUE.get_or_init(||Mutex::new(())).lock().map_err(|_|anyhow::anyhow!("Noa request queue is poisoned"))?;
                ensure!(guard.is_valid(),"Noa scope canceled before service creation");
                let mut client=ServiceClient::spawn()?;client.set_request_guard(guard.guard());
                let result=(|| {
                    let route=razer_discovery::direct::leviathan::resolve(&mut client,&selected)?;
                    ensure!(guard.is_valid(),"Noa scope canceled before submission");
                    route.leviathan(&mut client,guard.operation.command())
                })();
                if !client.is_stopped(){if let Err(error)=client.request(ServiceRequest::Shutdown){eprintln!("Noa worker cleanup: {error:#}");}}
                result
            }).await.map_err(|error|format!("{error:#}"));
            let _=this.update(cx,|this,cx| {
                let current=this.devices.contains(&workspace)&&workspace.read(cx).identity(cx)==identity
                    &&workspace.read(cx).device(cx).active_profile==profile_id
                    &&this.device_observations.iter().any(|current|current.matches(workspace.read(cx).device(cx))
                        &&current.physical_product_id()==observed.physical_product_id()
                        &&current.peer_product_id()==observed.peer_product_id()
                        &&current.hid_node()==observed.hid_node()&&current.transport()==observed.transport());
                if let Err(error)=&result {this.status=format!("声音设备操作未确认：{error}");}
                let result=if current {result}else{Err("Noa device/profile scope changed; an already submitted write may have reached the device".into())};
                page.update(cx,|page,cx|page.finish_leviathan_task(request.generation,result,cx));
                cx.notify();
            });
        }).detach();
    }
}
