//! Independent local Studio session. File saves never invoke a hardware service.
use super::AppShell;
use gpui_kit::{component::*, *};
use razer_pages::features::chroma_studio::ChromaStudio;
use razer_pages::features::chroma_studio::ChromaStudioEvent;
use razer_pages::features::chroma_studio::{ChromaStudioSnapshot,StudioProfileCommand,StudioProfileItem};
use razer_storage::chroma_profiles::{self,StudioChange,StudioLibrary};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf,collections::VecDeque,sync::{Arc,atomic::{AtomicBool,Ordering}}};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftFile {
    version: u32,
    studio: ChromaStudioSnapshot,
}
fn path() -> PathBuf {
    razer_storage::store_path().with_file_name("chroma-studio-draft.json")
}
fn read_bytes() -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::read(path()) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn decode(bytes: &[u8]) -> anyhow::Result<ChromaStudioSnapshot> {
    let file: DraftFile = serde_json::from_slice(bytes)?;
    anyhow::ensure!(
        file.version == 1,
        "Unsupported Studio draft version {}",
        file.version
    );
    file.studio.validate()?;
    Ok(file.studio)
}
/// The shell retains this entity after its window closes, preserving unsaved edits.
pub(super) struct StudioSession {
    page: Entity<ChromaStudio>,
    library:StudioLibrary<ChromaStudioSnapshot>,
    default_editor:Option<ChromaStudioSnapshot>,
    error: Option<String>,
    load_task:Option<Task<()>>,
    save_task: Option<Task<()>>,
    pending: VecDeque<ChromaStudioEvent>,
    failed:Option<Mutation>,
    revision:u64,
    watch_task:Option<Task<()>>,
    last_observed:StudioLibrary<ChromaStudioSnapshot>,
    external_selection:Option<String>,
    window:Option<AnyWindowHandle>,
    guard:Arc<AtomicBool>,
    _subscription: Subscription,
}
#[derive(Clone)]
struct Mutation {
    change:StudioChange<ChromaStudioSnapshot>,
    saved:Option<ChromaStudioSnapshot>,
}
pub(super) struct StudioSaveFinished(pub(super) bool);
impl EventEmitter<StudioSaveFinished> for StudioSession {}
impl StudioSession {
    pub(super) fn deactivate(&mut self, cx: &mut App) {
        self.window=None;
        self.page.update(cx, |page, cx| page.deactivate(cx));
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut App) {
        self.window=Some(window.window_handle());
        self.page.update(cx, |page, cx| page.focus(window, cx));
        if let Some(id)=self.external_selection.take() {
            self.page.update(cx,|page,cx|page.select_external_profile(id,window,cx));
        }
    }
    pub(super) fn save_pending(&self) -> bool {
        self.load_task.is_some() || self.save_task.is_some() || (self.failed.is_none()&&!self.pending.is_empty())
    }
    fn new(cx: &mut Context<Self>) -> Self {
        let page = cx.new(ChromaStudio::new);
        let subscription = cx.subscribe(&page, |this, _, event, cx| {
            this.pending.push_back(event.clone());this.start_pending(cx);
        });
        let load_task=cx.spawn(async move |this,cx| {
            let result=cx.background_spawn(async {
                let library=chroma_profiles::load_studio::<ChromaStudioSnapshot>(&chroma_profiles::path(),ChromaStudioSnapshot::validate)?;
                let legacy=if library.profiles().is_empty() {read_bytes()?.as_deref().map(decode).transpose()?}else{None};
                Ok::<_,anyhow::Error>((library,legacy,ChromaStudioSnapshot::default_profile()?))
            }).await;
            let _=this.update(cx,|this,cx| {
                this.load_task=None;
                match result {
                    Ok((library,legacy,default_editor))=>{
                        if let Some(profile)=library.selected() {this.page.update(cx,|page,cx|page.restore(profile.editor().clone(),cx));}
                        else if let Some(legacy)=legacy {this.page.update(cx,|page,cx|page.restore_draft(legacy,cx));}
                        this.last_observed=library.clone();this.library=library;this.default_editor=Some(default_editor);this.error=None;
                    }
                    Err(error)=>this.error=Some(format!("{error:#}")),
                }
                this.publish_profiles(cx);this.start_pending(cx);this.start_watch(cx);cx.notify();
            });
        });
        Self {
            page,
            library:StudioLibrary::default(),
            default_editor:None,
            error:None,
            load_task:Some(load_task),
            save_task: None,
            pending:VecDeque::new(),
            failed:None,revision:0,watch_task:None,last_observed:StudioLibrary::default(),external_selection:None,window:None,
            guard:Arc::new(AtomicBool::new(true)),
            _subscription: subscription,
        }
    }
    fn publish_profiles(&mut self,cx:&mut Context<Self>) {
        let profiles=self.library.profiles().iter().map(|profile|StudioProfileItem {id:profile.id().into(),name:profile.name().into()}).collect();
        let selected=self.library.selected().map(|profile|profile.id().to_owned());
        let pending=self.save_pending();
        self.page.update(cx,|page,cx|{page.set_profiles(profiles,selected,pending,cx);page.set_save_pending(pending,cx);});
    }
    fn start_pending(&mut self,cx:&mut Context<Self>) {
        if self.load_task.is_some()||self.save_task.is_some() {return;}
        let mutation=if let Some(failed)=self.failed.take() {failed}else{
            let Some(event)=self.pending.pop_front() else {self.publish_profiles(cx);return;};
            let saved=match &event {ChromaStudioEvent::Save(snapshot)=>Some(snapshot.clone()),_=>None};
            let restore=matches!(event,ChromaStudioEvent::Profile(StudioProfileCommand::Add {..}|StudioProfileCommand::Select {..}|StudioProfileCommand::Duplicate {..}|StudioProfileCommand::Delete {..}));
            let change=self.prepare(event,cx);
            let result=change.and_then(|change|Ok((self.library.changed(change.clone(),ChromaStudioSnapshot::validate)?,change)));
            let (library,change)=match result {
                Ok(prepared)=>prepared,
                Err(error)=>{self.error=Some(format!("{error:#}"));self.pending.clear();self.publish_profiles(cx);cx.emit(StudioSaveFinished(false));cx.notify();return;}
            };
            // Current 8480 dispatches the profile transition before its local
            // persistence call. A file error is separate and never rolls it back.
            self.library=library;
            if restore {if let Some(profile)=self.library.selected() {self.page.update(cx,|page,cx|page.restore(profile.editor().clone(),cx));}}
            Mutation {change,saved}
        };
        self.revision=self.revision.wrapping_add(1);
        let guard=self.guard.clone();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            let change=mutation.change.clone();
            let result = cx
                .background_spawn(async move {
                    chroma_profiles::update_studio(&chroma_profiles::path(),&razer_storage::source_local::path(),change,&guard,ChromaStudioSnapshot::validate)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.save_task = None;
                let succeeded = result.is_ok();
                match result {
                    Ok(library) => {
                        this.last_observed=library.clone();this.library=library;
                        this.error = None;
                        if let Some(saved)=mutation.saved {this.page.update(cx,|page,cx|page.saved(saved,cx));}
                        this.start_pending(cx);
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        this.failed=Some(mutation);
                    }
                }
                this.publish_profiles(cx);
                cx.emit(StudioSaveFinished(succeeded));
                cx.notify();
            });
        }));
        self.publish_profiles(cx);
    }
    fn prepare(&self,event:ChromaStudioEvent,cx:&App)->anyhow::Result<StudioChange<ChromaStudioSnapshot>> {
        let fresh_id=||uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
        Ok(match event {
            ChromaStudioEvent::Save(editor)=>match self.library.selected() {
                Some(profile)=>StudioChange::Save {id:profile.id().into(),editor},
                None=>StudioChange::Add {id:fresh_id(),name:self.page.read(cx).next_profile_name(),editor},
            },
            ChromaStudioEvent::Profile(command)=>match command {
                StudioProfileCommand::Add {name}=>StudioChange::Add {id:fresh_id(),name,editor:self.default_editor.clone().ok_or_else(||anyhow::anyhow!("Studio source default has not loaded"))?},
                StudioProfileCommand::Select {id}=>StudioChange::Select {id},
                StudioProfileCommand::Rename {id,name}=>StudioChange::Rename {id,name},
                StudioProfileCommand::Duplicate {id,name}=>StudioChange::Duplicate {id,new_id:fresh_id(),name},
                StudioProfileCommand::Delete {id}=>StudioChange::Delete {id},
            }
        })
    }
    fn start_watch(&mut self,cx:&mut Context<Self>) {
        if self.watch_task.is_some() {return;}
        let guard=self.guard.clone();
        self.watch_task=Some(cx.spawn(async move |this,cx| {
            while guard.load(Ordering::Acquire) {
                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                let revision=this.update(cx,|this,_|(!this.save_pending()&&this.failed.is_none()).then_some(this.revision));
                let Ok(Some(revision))=revision else {if revision.is_err() {break;}continue;};
                let observed=cx.background_spawn(async {chroma_profiles::load_studio::<ChromaStudioSnapshot>(&chroma_profiles::path(),ChromaStudioSnapshot::validate)}).await;
                let result=this.update(cx,|this,cx| {
                    if revision!=this.revision||this.save_pending()||this.failed.is_some() {return;}
                    let observed=match observed {Ok(observed)=>observed,Err(error)=>{this.error=Some(format!("{error:#}"));cx.notify();return;}};
                    if observed==this.last_observed {return;}
                    let selected=observed.selected_id().map(str::to_owned);
                    this.last_observed=observed;
                    if selected.as_deref()==this.library.selected_id() {return;}
                    let Some(id)=selected.filter(|id|this.library.profiles().iter().any(|profile|profile.id()==id)) else {return;};
                    this.external_selection=Some(id.clone());
                    if let Some(handle)=this.window {
                        let owner=cx.entity().downgrade();
                        cx.defer(move |cx| {
                            let _=handle.update(cx,|_,window,cx| {
                                let _=owner.update(cx,|this,cx| {
                                    if this.external_selection.as_ref()!=Some(&id)||this.window!=Some(handle) {return;}
                                    this.external_selection=None;
                                    this.page.update(cx,|page,cx|page.select_external_profile(id,window,cx));
                                });
                            });
                        });
                    }
                });
                if result.is_err() {break;}
            }
        }));
    }
}
impl Drop for StudioSession {fn drop(&mut self){self.guard.store(false,Ordering::Release);}}
impl Render for StudioSession {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut view = v_flex().size_full().bg(cx.theme().background);
        if let Some(error) = &self.error {
            view = view.child(
                div()
                    .p_2()
                    .text_color(cx.theme().danger)
                    .child(format!("Studio 本地配置：{error}")),
            );
        }
        view.child(div().flex_1().min_h_0().child(self.page.clone()))
    }
}
impl AppShell {
    pub(super) fn open_chroma_studio(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.try_open_chroma_studio(cx) {
            eprintln!("[chroma studio] {error}");
        }
    }
    pub(super) fn try_open_chroma_studio(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.chroma_studio.is_none() {
            let session = cx.new(StudioSession::new);
            self.subscriptions.push(cx.subscribe(
                &session,
                |this, _, event: &StudioSaveFinished, cx| {
                    if !event.0 {
                        let was_exiting = this.close_requested;
                        this.close_requested = false;
                        if was_exiting {
                            this.open_chroma_studio(cx);
                        }
                    } else if this.close_requested
                        && this.save_task.is_none()
                        && !this.studio_save_pending(cx)
                    {
                        this.continue_save_queue(true, cx);
                    }
                },
            ));
            self.chroma_studio = Some(session);
        }
        let session = self
            .chroma_studio
            .as_ref()
            .expect("created Studio session")
            .clone();
        self.try_open_chroma_window(cx)?;
        let host = self
            .chroma_host
            .as_ref()
            .ok_or_else(|| "Chroma host was not created".to_owned())?;
        host.update(cx, |host, cx| host.open_studio(session, cx));
        Ok(())
    }
    pub(super) fn studio_save_pending(&self, cx: &App) -> bool {
        self.chroma_studio
            .as_ref()
            .is_some_and(|session| session.read(cx).save_pending())
    }
}
