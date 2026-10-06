//! Independent local Studio session. File saves never invoke a hardware service.
use super::AppShell;
use crate::features::chroma_studio::{ChromaStudio, ChromaStudioEvent, ChromaStudioSnapshot};
use gpui_kit::{component::*, *};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftFile {
    version: u32,
    studio: ChromaStudioSnapshot,
}
fn path() -> PathBuf {
    crate::store::store_path().with_file_name("chroma-studio-draft.json")
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
fn write(snapshot: ChromaStudioSnapshot, previous: Option<Vec<u8>>) -> anyhow::Result<Vec<u8>> {
    use std::io::Write;
    snapshot.validate()?;
    anyhow::ensure!(
        read_bytes()? == previous,
        "Studio draft changed on disk; reopen the application before saving."
    );
    let bytes = serde_json::to_vec_pretty(&DraftFile {
        version: 1,
        studio: snapshot,
    })?;
    let path = path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    anyhow::ensure!(
        read_bytes()? == previous,
        "Studio draft changed during save."
    );
    if let Some(previous) = previous {
        std::fs::write(path.with_extension("json.bak"), previous)?;
    }
    std::fs::rename(temporary, path)?;
    Ok(bytes)
}

/// The shell retains this entity after its window closes, preserving unsaved edits.
pub(super) struct StudioSession {
    page: Entity<ChromaStudio>,
    disk: Option<Vec<u8>>,
    error: Option<String>,
    save_task: Option<Task<()>>,
    pending: Option<ChromaStudioSnapshot>,
    _subscription: Subscription,
}
pub(super) struct StudioSaveFinished(pub(super) bool);
impl EventEmitter<StudioSaveFinished> for StudioSession {}
impl StudioSession {
    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.page.update(cx, |page, cx| page.focus(window, cx));
    }
    pub(super) fn save_pending(&self) -> bool {
        self.save_task.is_some() || self.pending.is_some()
    }
    fn new(cx: &mut Context<Self>) -> Self {
        let page = cx.new(ChromaStudio::new);
        let (disk, error) = match read_bytes() {
            Ok(Some(bytes)) => match decode(&bytes) {
                Ok(snapshot) => {
                    page.update(cx, |page, cx| page.restore(snapshot, cx));
                    (Some(bytes), None)
                }
                Err(error) => (None, Some(error.to_string())),
            },
            Ok(None) => (None, None),
            Err(error) => (None, Some(error.to_string())),
        };
        let subscription = cx.subscribe(&page, |this, _, event, cx| {
            let ChromaStudioEvent::Save(snapshot) = event;
            this.request_save(snapshot.clone(), cx);
        });
        Self {
            page,
            disk,
            error,
            save_task: None,
            pending: None,
            _subscription: subscription,
        }
    }
    fn request_save(&mut self, snapshot: ChromaStudioSnapshot, cx: &mut Context<Self>) {
        self.page
            .update(cx, |page, cx| page.set_save_pending(true, cx));
        if self.save_task.is_some() {
            self.pending = Some(snapshot);
            return;
        }
        let previous = self.disk.clone();
        let saved = snapshot.clone();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { write(snapshot, previous) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.save_task = None;
                let succeeded = result.is_ok();
                match result {
                    Ok(bytes) => {
                        this.disk = Some(bytes);
                        this.error = None;
                        this.page.update(cx, |page, cx| page.saved(saved, cx));
                        if let Some(next) = this.pending.take() {
                            this.request_save(next, cx);
                        }
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        this.pending = None;
                    }
                }
                let pending = this.save_pending();
                this.page
                    .update(cx, |page, cx| page.set_save_pending(pending, cx));
                cx.emit(StudioSaveFinished(succeeded));
                cx.notify();
            });
        }));
    }
}
impl Render for StudioSession {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut view = v_flex().size_full().bg(cx.theme().background);
        if let Some(error) = &self.error {
            view = view.child(
                div()
                    .p_2()
                    .text_color(cx.theme().danger)
                    .child(format!("Local draft: {error}")),
            );
        }
        view.child(div().flex_1().min_h_0().child(self.page.clone()))
    }
}
impl AppShell {
    pub(super) fn open_chroma_studio(&mut self, cx: &mut Context<Self>) {
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
        self.open_chroma_window(cx);
        if let Some(host) = &self.chroma_host {
            host.update(cx, |host, cx| host.open_studio(session, cx));
        }
    }
    pub(super) fn studio_save_pending(&self, cx: &App) -> bool {
        self.chroma_studio
            .as_ref()
            .is_some_and(|session| session.read(cx).save_pending())
    }
}
