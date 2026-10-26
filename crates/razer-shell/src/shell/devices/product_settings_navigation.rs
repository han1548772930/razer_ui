//! Product SDK launchDisplayWindow(policy=3, autoFocus), current settings URLs.
//! Settings are named tabs in the caller's host, never the tray's OS window.
use super::{AppShell, Location, chroma_window::ChromaWindow, profile_migration, release_notes};
use gpui_kit::*;
use razer_app_pages::chroma_settings::{ChromaSettings, SettingsAction};

impl Location {
    pub(super) fn synapse_settings_mount_change(&self, next: &Self) -> Option<bool> {
        let owns_settings = |location: &Self| {
            matches!(
                location,
                Self::Main(razer_pages::nav::Tab::Setting) | Self::ProductSettings(false)
            )
        };
        let mounted = owns_settings(next);
        (owns_settings(self) != mounted).then_some(mounted)
    }
}

#[derive(Default)]
pub(super) struct ProductSettingsState {
    chroma: Option<Entity<ChromaSettings>>,
    subscription: Option<Subscription>,
    observer: Option<Subscription>,
    migration: Option<Entity<profile_migration::MigrationPage>>,
}
impl AppShell {
    pub(super) fn open_product_settings(
        &mut self,
        chroma: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if window.window_handle() != self.main_window {
            let host = self
                .chroma_host
                .clone()
                .ok_or_else(|| "The requesting settings host is no longer available".to_owned())?;
            if !chroma || !host.read(cx).owns_window(window.window_handle()) {
                return Err("The requesting settings host does not own this settings route".into());
            }
            // Reuse the existing Chroma host and its settings/migration tab manager.
            let locale = self.settings.clone();
            host.update(cx, |host: &mut ChromaWindow, cx| {
                host.open_settings_with_locale(locale, window, cx)
            });
            window.activate_window();
            return Ok(());
        }
        self.prepare_product_settings(chroma, window, cx);
        self.navigate(Location::ProductSettings(chroma), window, cx);
        window.activate_window();
        if self.location == Location::ProductSettings(chroma) {
            self.focus_product_settings(chroma, window, cx);
        }
        Ok(())
    }
    pub(super) fn prepare_product_settings(
        &mut self,
        chroma: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !chroma || self.product_settings.chroma.is_some() {
            return;
        }
        let locale = self.settings.clone();
        let dashboard = self.chroma_page.clone();
        let page = cx.new(|cx| ChromaSettings::new(locale, dashboard, window, cx));
        let handle = window.window_handle();
        self.product_settings.subscription =
            Some(cx.subscribe(&page, move |this, _, action, cx| {
                match action {
                    SettingsAction::ResetTutorials => this
                        .chroma_page
                        .update(cx, |page, cx| page.reset_tutorials(cx)),
                    SettingsAction::ReleaseNotes => {
                        let owner = cx.entity().downgrade();
                        let Some(settings) = this.product_settings.chroma.clone() else {
                            return;
                        };
                        cx.defer(move |cx| {
                            let _ = handle.update(cx, |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    if this.location != Location::ProductSettings(true)
                                        || this.product_settings.chroma.as_ref() != Some(&settings)
                                    {
                                        return;
                                    }
                                    let notes = release_notes::open_from_storage(
                                        release_notes::NotesApp::Chroma,
                                        &mut this.host_storage,
                                        window,
                                        cx,
                                    );
                                    settings.update(cx, |settings, cx| {
                                        settings.show_release_notes(notes, cx)
                                    });
                                });
                            });
                        });
                    }
                    SettingsAction::OpenMigration => {
                        let owner = cx.entity().downgrade();
                        cx.defer(move |cx| {
                            let _ = handle.update(cx, |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.prepare_product_settings_migration(cx);
                                    this.navigate(Location::ProductSettingsMigration, window, cx);
                                });
                            });
                        });
                    }
                }
            }));
        self.product_settings.observer = Some(cx.observe(&page, |_, _, cx| cx.notify()));
        self.product_settings.chroma = Some(page);
    }
    pub(super) fn focus_product_settings(
        &self,
        chroma: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if chroma && let Some(page) = &self.product_settings.chroma {
            page.update(cx, |page, cx| page.focus(window, cx));
        } else {
            self.host_tabs
                .focus_location(&Location::ProductSettings(chroma), window, cx);
        }
    }
    pub(super) fn product_settings_page(&self, chroma: bool) -> AnyElement {
        if chroma {
            self.product_settings.chroma.as_ref().map_or_else(
                || div().into_any_element(),
                |page| page.clone().into_any_element(),
            )
        } else {
            self.settings.clone().into_any_element()
        }
    }
    pub(super) fn allow_close_product_settings(
        &self,
        chroma: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        !chroma
            || self
                .product_settings
                .chroma
                .as_ref()
                .is_none_or(|page| page.update(cx, |page, cx| page.can_close(cx)))
    }
    pub(super) fn closed_product_settings(&mut self, chroma: bool) {
        if chroma {
            self.product_settings.chroma = None;
            self.product_settings.subscription = None;
            self.product_settings.observer = None;
        }
    }
    pub(super) fn prepare_product_settings_migration(&mut self, cx: &mut Context<Self>) {
        if self.product_settings.migration.is_none() {
            self.product_settings.migration = Some(cx.new(|cx| {
                profile_migration::MigrationPage::new_for(
                    profile_migration::MigrationApp::Chroma,
                    cx,
                )
                .with_host_toolbar()
            }));
        }
    }
    pub(super) fn product_settings_migration_page(&self) -> AnyElement {
        self.product_settings.migration.as_ref().map_or_else(
            || div().into_any_element(),
            |page| page.clone().into_any_element(),
        )
    }
    pub(super) fn refresh_product_settings_migration(&mut self, cx: &mut Context<Self>) {
        self.product_settings.migration = None;
        self.prepare_product_settings_migration(cx);
        cx.notify();
    }
    pub(super) fn closed_product_settings_migration(&mut self) {
        self.product_settings.migration = None;
    }
}
