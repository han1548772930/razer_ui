//! The OBM event consumer commits documents before their index. Both writes
//! use the existing background save queue and retain the connection owner.
use super::*;
use razer_model::macro_library::onboard_import::{Import, Phase};

pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
    import: Import,
    completion: Option<async_channel::Sender<Result<(), String>>>,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    fn current(&self, devices: &[Entity<ProductWorkspace>], cx: &App) -> bool {
        devices.contains(&self.workspace)
            && self.workspace.read(cx).identity(cx) == self.identity
            && self.workspace.read(cx).connection_revision() == self.connection_revision
            && self
                .completion
                .as_ref()
                .is_some_and(|sender| !sender.is_closed())
    }
    fn complete(&mut self, result: Result<(), String>) {
        if let Some(sender) = self.completion.take() {
            let _ = sender.try_send(result);
        }
    }
}
impl Drop for LocalSave {
    fn drop(&mut self) {
        self.complete(Err(
            "Onboard macro import was canceled before completion".into()
        ));
    }
}

impl AppShell {
    pub(super) fn persist_keyboard_macro_import(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        documents: &[razer_device::onboard_macro::import::Document],
        completion: async_channel::Sender<Result<(), String>>,
        cx: &mut Context<Self>,
    ) {
        let prepared = (|| {
            let values = documents
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            Import::prepare(&self.macro_library.read(cx).snapshot(), &values)
        })();
        let import = match prepared {
            Ok(import) => import,
            Err(error) => {
                let _ = completion.try_send(Err(error));
                return;
            }
        };
        let mut save = LocalSave {
            identity: workspace.read(cx).identity(cx),
            connection_revision: workspace.read(cx).connection_revision(),
            workspace,
            import,
            completion: Some(completion),
        };
        if self.close_requested || !save.current(&self.devices, cx) {
            save.complete(Err(
                "Onboard macro import connection is no longer current".into()
            ));
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|owner| owner.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            self.main_window,
            file,
            Some(ProductSave::KeyboardMacroImport(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_keyboard_macro_import(
        &self,
        prepared: &mut PreparedSave,
        cx: &App,
    ) -> Result<(), String> {
        let Some(ProductSave::KeyboardMacroImport(save)) = &prepared.product else {
            return Ok(());
        };
        if !save.current(&self.devices, cx)
            || !save.import.matches(&self.macro_library.read(cx).snapshot())
        {
            return Err("Onboard macro import conflicts with a newer library or connection".into());
        }
        prepared.file.macros = save.import.candidate().clone();
        Ok(())
    }

    pub(super) fn finish_keyboard_macro_import(
        &mut self,
        mut save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if !succeeded {
            save.complete(Err("Onboard macro import was not saved locally".into()));
            return;
        }
        if !save.current(&self.devices, cx) {
            save.complete(Err(
                "Onboard macro import reached disk after its connection ended".into(),
            ));
            return;
        }
        if let Err(error) = self.macro_library.update(cx, |library, cx| {
            library.publish_onboard_import(&save.import, cx)
        }) {
            self.storage_error = Some(error.clone());
            save.complete(Err(error));
            return;
        }
        if save.import.phase() == Phase::Index {
            save.complete(Ok(()));
            return;
        }
        let index = save
            .import
            .prepare_index(&self.macro_library.read(cx).snapshot(), || {
                let elapsed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| error.to_string())?;
                u64::try_from(elapsed.as_millis()).map_err(|error| error.to_string())
            });
        match index {
            Err(error) => save.complete(Err(error)),
            Ok(index) => {
                save.import = index;
                let file = self.workspace_file(
                    self.devices
                        .iter()
                        .map(|owner| owner.read(cx).saved_snapshot(cx))
                        .collect(),
                    cx,
                );
                self.pending_saves.push_front(self.prepare_save(
                    self.main_window,
                    file,
                    Some(ProductSave::KeyboardMacroImport(save)),
                    cx,
                ));
            }
        }
    }
}
