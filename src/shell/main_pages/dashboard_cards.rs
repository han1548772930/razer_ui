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
            .child(self.dashboard_module_group(layout, cx))
            .child(self.dashboard_service_group(layout, cx));
        v_flex()
            .w_full()
            .when(self.dashboard_state.read(cx).banner_open(), |view| {
                view.child(
                    crate::ui::app_introduction_banner::AppIntroductionBanner::new(
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
        let mut cards =
            self.devices
                .iter()
                .map(|entity| {
                    let workspace = entity.read(cx);
                    let device = workspace.device(cx);
                    let key = workspace.identity(cx);
                    let supported = crate::features::has_product_workspace(device.product_id);
                    let name = dashboard_device::name(device);
                    let status = dashboard_device::status(device);
                    let disabled = dashboard_device::disabled(device);
                    let off = dashboard_device::power_off(device);
                    let selected_sub_device = device
                        .sub_devices
                        .as_ref()
                        .and_then(|items| {
                            items.iter().position(|item| {
                                item.get("isShownOnUI").and_then(|v| v.as_bool()) == Some(true)
                            })
                        })
                        .map(|index| index + 1)
                        .filter(|index| *index > 1);
                    let count = device
                        .sub_devices
                        .as_ref()
                        .filter(|items| items.len() >= 2)
                        .map(|items| items.len())
                        .or_else(|| {
                            device
                                .dashboard
                                .count
                                .filter(|n| *n > 0)
                                .map(|n| n as usize)
                        });
                    let owner = cx.entity().downgrade();
                    let content = v_flex()
                        .relative()
                        .size_full()
                        .pt(surface::css(10.))
                        .px(surface::css(20.))
                        .pb(surface::css(9.))
                        .when_some(selected_sub_device, |view, number| {
                            view.child(
                                div()
                                    .absolute()
                                    .top(surface::css(10.))
                                    .right(surface::css(10.))
                                    .size(surface::css(20.))
                                    .rounded(surface::css(50.))
                                    .bg(rgb(0xbbbbbb))
                                    .text_color(rgb(0x111111))
                                    .text_size(surface::css(12.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(number.to_string()),
                            )
                        })
                        .when_some(count, |view, count| {
                            view.child(
                                div()
                                    .absolute()
                                    .top(surface::css(10.))
                                    .right(surface::css(10.))
                                    .size(surface::css(24.))
                                    .rounded(surface::css(15.))
                                    .bg(rgb(0x707070))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(count.to_string()),
                            )
                        })
                        .child(div().when(disabled, |view| view.opacity(0.3)).child(
                            dashboard_image(
                                device.product_id,
                                device.edition_id,
                                device.layout_id,
                                250.,
                                140.,
                            ),
                        ))
                        .child(
                            v_flex()
                                .items_center()
                                .when(disabled, |view| view.opacity(0.3))
                                .child(
                                    div()
                                        .w_full()
                                        .px(surface::css(10.))
                                        .min_h(surface::css(17.))
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(16.))
                                        .text_center()
                                        .whitespace_normal()
                                        .child(name.to_uppercase()),
                                )
                                // K deliberately has no edition locale fallback.
                                .child(
                                    div()
                                        .px(surface::css(10.))
                                        .text_size(surface::css(12.))
                                        .line_height(surface::css(14.))
                                        .text_color(rgb(0x707070))
                                        .text_center()
                                        .child(dashboard_device::edition(device).to_uppercase()),
                                )
                                .when(!off, |tag| {
                                    tag.child(
                                        div()
                                            .w(surface::css(230.))
                                            .mx_auto()
                                            .text_size(surface::css(12.))
                                            .line_height(surface::css(14.))
                                            .text_color(
                                                if matches!(
                                                    device.setup_status,
                                                    crate::model::SetupStatus::InstallCanceled
                                                        | crate::model::SetupStatus::Error
                                                ) {
                                                    rgb(0xfd4949).into()
                                                } else {
                                                    cx.theme().primary
                                                },
                                            )
                                            .text_center()
                                            .text_ellipsis()
                                            .child(status),
                                    )
                                }),
                        )
                        .child(dashboard_device::DashboardBattery::new(
                            format!("dashboard-battery-{key}"),
                            device,
                        ));
                    DashboardCard::button(
                        format!("open-{key}"),
                        name.to_owned(),
                        content,
                        move |window, cx| {
                            let _ = owner.update(cx, |shell, cx| {
                                shell.navigate(Location::Device(key.clone()), window, cx)
                            });
                        },
                    )
                    .unavailable(
                        !supported
                            || disabled
                            || device.setup_status == crate::model::SetupStatus::Updating,
                    )
                })
                .collect::<Vec<_>>();
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
            (
                "alexa",
                "DASHBOARD_ALEXA",
                "synapse/module-alexa.svg",
                Location::Alexa,
            ),
            (
                "feedback",
                "FEEDBACK",
                "synapse/module-feedback.svg",
                Location::Feedback,
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
            self.dashboard_module_card(
                "armory",
                "DASHBOARD_WORKSHOP",
                "synapse/module-armory.svg",
                Location::Armory,
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

    fn dashboard_service_group(&self, layout: &MainLayout, cx: &mut Context<Self>) -> AnyElement {
        let cards = [
            (
                "store",
                "RAZER_STORE",
                "RAZER_STORE_DESC",
                "synapse/dashboard-store.png",
                "https://www.razer.com/store/",
            ),
            (
                "gold",
                "RAZER_GOLD_AND_SILVER",
                "RAZER_GOLD_AND_SILVER_DESC",
                "synapse/dashboard-gold.png",
                "https://gold.razer.com/",
            ),
            (
                "community",
                "RAZER_COMMUNITY",
                "RAZER_COMMUNITY_DESC",
                "synapse/dashboard-community.png",
                "https://www.razer.com/community/",
            ),
            (
                "support",
                "RAZER_SUPPORT",
                "RAZER_SUPPORT_DESC",
                "synapse/dashboard-support.png",
                "https://support.razer.com/",
            ),
        ]
        .map(|(key, label, description, asset, url)| {
            DashboardCard::link(
                format!("dashboard-service-{key}"),
                i18n::t(label),
                url,
                v_flex()
                    .size_full()
                    .child(
                        img(asset)
                            .w_full()
                            .h(surface::css(140.))
                            .flex_shrink_0()
                            .object_fit(ObjectFit::Contain)
                            .rounded_t(surface::css(5.)),
                    )
                    .child(div().mt(surface::css(10.)).child(dashboard_caption(
                        label,
                        Some(description),
                        cx,
                    ))),
            )
        })
        .into_iter()
        .collect();
        self.dashboard_card_group(
            "onlineService",
            "dashboard-services",
            "ONLINE_SERVICES_HEADER",
            layout,
            cards,
            cx,
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
