//! Source Dashboard groups, distinct from the Devices & Modules route.
use super::*;

impl AppShell {
    pub(in crate::shell) fn dashboard(
        &self,
        layout: &MainLayout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let grid = v_flex()
            .id("dashboard")
            .w(surface::css(layout.dashboard_width))
            .flex_shrink_0()
            .mb(surface::css(50.))
            .when(!layout.narrow, |view| view.mx_auto())
            .child(self.dashboard_device_group(layout, cx))
            .child(self.dashboard_module_group(layout, cx));
        v_flex()
            .w_full()
            .when(self.dashboard_state.read(cx).banner_open(), |view| {
                view.child(
                    razer_widgets::app_introduction_banner::AppIntroductionBanner::new(
                        "dashboard-introduction-banner",
                        "",
                        cx.listener(|this, _, _, cx| {
                            this.dashboard_state
                                .update(cx, |state, cx| state.close_banner(cx))
                        }),
                        cx.listener(|this, _, window, cx| {
                            this.navigate(
                                Location::Tour(crate::shell::TourKind::Synapse),
                                window,
                                cx,
                            )
                        }),
                        cx.listener(|this, _, window, cx| {
                            this.navigate(
                                Location::Tour(crate::shell::TourKind::Chroma),
                                window,
                                cx,
                            )
                        }),
                    ),
                )
            })
            .child(grid)
            .into_any_element()
    }

    fn dashboard_card_group(
        &self,
        group: &'static str,
        id: &'static str,
        label: &'static str,
        layout: &MainLayout,
        cards: Vec<DashboardCard>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let collapsed = self.dashboard_state.read(cx).collapsed(group);
        let state = self.dashboard_state.clone();
        v_flex()
            .id(id)
            .w_full()
            .my(surface::css(10.))
            .child(
                dashboard_group_toggle(id, label, collapsed, cx).on_click(move |_, _, cx| {
                    state.update(cx, |state, cx| state.toggle(group, cx));
                }),
            )
            .child(DashboardGroupContent::new(
                id,
                collapsed,
                layout.dashboard_width,
                cards.len(),
                DashboardGrid::new(group, &self.dashboard_state, layout.dashboard_width, cards),
            ))
            .into_any_element()
    }

    fn dashboard_device_group(&self, layout: &MainLayout, cx: &mut Context<Self>) -> AnyElement {
        let snapshot = self.module_catalog.read(cx).service_snapshot();
        let mut cards = self.devices.iter().map(|entity| {
            let workspace = entity.read(cx);
            let device = workspace.device(cx);
            let key = workspace.identity(cx);
            let supported = workspace.has_local_page(cx);
            let name = dashboard_device::name(device).to_owned();
            let restart = device.setup_status == razer_model::model::SetupStatus::RestartRequired;
            let owner = cx.entity().downgrade();
            let retry_owner = owner.clone();
            let firmware_owner = owner.clone();
            let settings_owner = owner.clone();
            let resuscitate = device.dashboard.no_alive_sign == Some(true);
            let mut observed_device = device.clone();
            if let Some(snapshot) = &snapshot {
                observed_device.dashboard.firmware_needs_upgrade = Some(snapshot.firmware_needs_upgrade(
                    device.product_id, &device.serial_number));
            }
            let content = dashboard_device_card::DeviceCard::new(
                format!("open-{key}"), observed_device,
                self.dashboard_state.clone(),
                dashboard_device_card::Actions {
                    retry: std::rc::Rc::new(move |window, cx| {
                        let _ = retry_owner.update(cx, |shell, cx| {
                            // Current BoxItemDevice.reInstall sends a request;
                            // do not fabricate setup-state or install success.
                            shell.status = if i18n::locale().eq_ignore_ascii_case("zh-cn") {
                                if resuscitate { "设备恢复服务未连接，尚未发送恢复请求。" }
                                else { "设备安装服务未连接，尚未发送重试请求。" }
                            } else if resuscitate { "Device recovery service is unavailable. No recovery request was sent." }
                            else { "Device installer service is unavailable. No retry request was sent." }.into();
                            window.push_notification(shell.status.clone(), cx);
                            cx.notify();
                        });
                    }),
                    firmware: std::rc::Rc::new(move |window, cx| {
                        let _ = firmware_owner.update(cx, |shell, cx| shell.navigate(
                            Location::Main(razer_pages::nav::Tab::Modules), window, cx));
                    }),
                    settings: std::rc::Rc::new(move |window, cx| {
                        let _ = settings_owner.update(cx, |shell, cx| shell.navigate(
                            Location::Main(razer_pages::nav::Tab::Setting), window, cx));
                    }),
                },
            )
            .open_without_installation(local_installation_entry(device, supported))
            .local_preview_image(razer_settings::products::local_preview_artwork(device));
            DashboardCard::button(format!("open-{key}"), name, content, move |window, cx| {
                let _ = owner.update(cx, |shell, cx| {
                    if restart {
                        shell.request_restart_notification(window, cx);
                    } else { shell.navigate(Location::Device(key.clone()), window, cx); }
                });
            }).unavailable(!restart && (!supported || !dashboard_device::can_focus(device)))
        }).collect::<Vec<_>>();
        if cards.is_empty() {
            cards.push(DashboardCard::empty(dashboard_empty_devices(cx)));
        }
        self.dashboard_card_group(
            "devices",
            "dashboard-devices",
            "DEVICES_HEADER",
            layout,
            cards,
            cx,
        )
    }

    fn dashboard_module_group(&self, layout: &MainLayout, cx: &mut Context<Self>) -> AnyElement {
        // Current 22431 AVAILABLE_MODULES order. Compiled local pages satisfy
        // availability directly; the user's requested direct-open behavior
        // does not depend on an external installer's installedModules list.
        let wifi_owner = cx.entity().downgrade();
        let tour_owner = wifi_owner.clone();
        let mut cards = [
            (
                "macro",
                "MACRO",
                "synapse/module-macro.svg",
                Location::Macro,
            ),
            (
                "linkedGames",
                "LINKED_GAMES",
                "synapse/module-linked-games.svg",
                Location::Profiles,
            ),
        ]
        .into_iter()
        .map(|(key, label, asset, location)| {
            self.dashboard_module_card(key, label, asset, location, cx)
        })
        .collect::<Vec<_>>();
        cards.push(DashboardCard::button(
            "dashboard-add-wifi",
            i18n::t("ADD_WIFI_DEVICE"),
            module_content("synapse/dashboard-iot.svg", "ADD_WIFI_DEVICE", cx),
            move |window, cx| {
                let _ = wifi_owner.update(cx, |shell, cx| {
                    shell.iot_popup = Some(super::super::iot_popup::open(
                        super::super::iot_popup::DeviceKind::General,
                        window,
                        cx,
                    ));
                    cx.notify();
                });
            },
        ));
        cards.extend([
            self.dashboard_module_card(
                "syn3-profile-migration",
                "PROFILE_MIGRATION",
                "synapse/module-profile-migration.svg",
                Location::ProfileMigration,
                cx,
            ),
            DashboardCard::button(
                "dashboard-tour",
                i18n::t("TOUR"),
                module_content("synapse/dashboard-tour.svg", "TOUR", cx),
                move |window, cx| {
                    let _ = tour_owner.update(cx, |shell, cx| {
                        shell.navigate(Location::Tour(crate::shell::TourKind::Synapse), window, cx)
                    });
                },
            )
            .focus(self.tour_trigger.clone()),
        ]);
        self.dashboard_card_group(
            "module",
            "dashboard-modules",
            "MODULES_HEADER",
            layout,
            cards,
            cx,
        )
    }

    fn dashboard_module_card(
        &self,
        key: &'static str,
        label: &'static str,
        asset: &'static str,
        location: Location,
        cx: &mut Context<Self>,
    ) -> DashboardCard {
        let owner = cx.entity().downgrade();
        DashboardCard::button(
            key,
            i18n::t(label),
            module_content(asset, label, cx),
            move |window, cx| {
                let _ = owner.update(cx, |shell, cx| shell.navigate(location.clone(), window, cx));
            },
        )
    }
}

fn module_content(asset: &'static str, label: &str, cx: &App) -> AnyElement {
    v_flex()
        .size_full()
        .pt(surface::css(10.))
        .px(surface::css(20.))
        .pb(surface::css(9.))
        .child(
            img(asset)
                .size(surface::css(100.))
                .flex_shrink_0()
                .mx_auto()
                .mt(surface::css(15.))
                .mb(surface::css(25.))
                .object_fit(ObjectFit::Contain),
        )
        .child(dashboard_caption(label, None, cx))
        .into_any_element()
}

#[cfg(test)]
mod preview_artwork_tests {
    use gpui_kit::AssetSource;
    use razer_dashboard::dashboard_device_card::product_card_image;
    use razer_settings::{products, settings_page};
    use std::collections::BTreeSet;

    #[test]
    fn persisted_layout_zero_keyboard_cards_decode_to_visible_source_slot_geometry() {
        use gpui_kit::{Bounds, DevicePixels, ObjectFit, point, px, size};
        use razer_dashboard::dashboard_device_card::PRODUCT_CARD_IMAGE_CSS_SIZE;
        use razer_model::{model::Device, product_preview::registered_preview};
        mod fixtures {
            include!("../../../../testdata/dashboard_artwork.rs");
        }
        for pid in [579, 689, 720] {
            let legacy = registered_preview(pid).unwrap();
            let mut restored: Device =
                serde_json::from_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
            restored.begin_local_session();
            assert_eq!(restored.layout_id, 0);
            for scale in [1., 1.5, 2., 3.] {
                let alias = product_card_image(
                    &restored,
                    scale,
                    products::local_preview_artwork(&restored),
                )
                .unwrap_or_else(|| panic!("PID {pid}: persisted layout zero lost its image"));
                let (_, dimensions, visible) = fixtures::DASHBOARD_ARTWORK
                    .iter()
                    .find(|(fixture_alias, _, _)| *fixture_alias == alias)
                    .expect("source PNG dimensions and alpha bounds");
                let dimensions = size(DevicePixels(dimensions[0]), DevicePixels(dimensions[1]));
                let slot = Bounds::new(
                    point(px(0.), px(0.)),
                    size(
                        px(PRODUCT_CARD_IMAGE_CSS_SIZE[0]),
                        px(PRODUCT_CARD_IMAGE_CSS_SIZE[1]),
                    ),
                );
                let fitted = ObjectFit::Contain.get_bounds(slot, dimensions);
                let visible_width = fitted.size.width
                    * ((visible[2] - visible[0]) as f32 / dimensions.width.0 as f32);
                let visible_height = fitted.size.height
                    * ((visible[3] - visible[1]) as f32 / dimensions.height.0 as f32);
                assert!(
                    visible_width >= px(160.) && visible_height >= px(30.),
                    "PID {pid}: visible artwork {visible_width:?} x {visible_height:?}"
                );
                assert!(
                    fitted.size.width <= slot.size.width && fitted.size.height <= slot.size.height
                );
                eprintln!(
                    "PID {pid} scale {scale}: {alias}, source={}x{}, alpha={visible:?}, visible_css={visible_width:?}x{visible_height:?}",
                    dimensions.width.0, dimensions.height.0
                );
            }
            assert_eq!(restored.layout_id, 0);
            assert!(restored.dashboard.connection_observation.is_none());
        }
    }

    #[test]
    fn every_catalog_preview_variant_keeps_its_dashboard_artwork() {
        let mut variants = 0;
        let mut recovered = BTreeSet::new();
        let mut images = BTreeSet::new();
        let mut missing = Vec::new();
        for product in razer_catalog::registry() {
            let pid = product.id();
            for edition in settings_page::preview_editions(pid) {
                for layout in settings_page::preview_layouts_for_edition(pid, edition) {
                    variants += 1;
                    let device = products::preview_device(pid, edition, layout).unwrap();
                    let preview_image = products::local_preview_artwork(&device);
                    for scale in [1., 1.5, 2., 3.] {
                        let source = product_card_image(&device, scale, None);
                        let selected = product_card_image(&device, scale, preview_image);
                        if let Some(asset) = selected {
                            images.insert(asset);
                            if source.is_none() {
                                recovered.insert((pid, edition, layout));
                                assert_eq!(selected, preview_image);
                            } else {
                                assert_eq!(selected, source);
                            }
                        } else {
                            missing.push((pid, edition, layout, scale));
                        }
                    }
                    assert!(device.dashboard.connection_observation.is_none());
                    assert!(device.dashboard.readonly_values.is_none());
                }
            }
        }
        assert!(variants >= razer_catalog::registry().len());
        assert!(!recovered.is_empty());
        assert!(
            missing.is_empty(),
            "Missing preview Dashboard images: {missing:?}"
        );
        for image in &images {
            assert!(
                razer_assets::SynapseAssets
                    .list(image)
                    .unwrap()
                    .iter()
                    .any(|alias| alias.as_ref() == *image),
                "{image}"
            );
        }
        eprintln!(
            "{}",
            serde_json::json!({
                "registered_products": razer_catalog::registry().len(),
                "variants": variants,
                "scales_per_variant": 4,
                "recovered_variants": recovered.len(),
                "selected_images": images.len(),
                "runtime_acceptance": "unverified"
            })
        );
    }
}
