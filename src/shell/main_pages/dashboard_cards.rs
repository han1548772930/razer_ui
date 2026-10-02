//! Source Dashboard groups, distinct from the Devices & Modules route.
use super::*;

impl AppShell {
    pub(in crate::shell) fn dashboard(
        &self,
        layout: &MainLayout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .id("dashboard")
            .w(surface::css(layout.dashboard_width))
            .flex_shrink_0()
            .when(!layout.narrow, |view| view.mx_auto())
            .child(self.dashboard_device_group(layout, cx))
            .child(self.dashboard_module_group(layout, cx))
            .child(self.dashboard_service_group(layout, cx))
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
        let mut cards = self
            .devices
            .iter()
            .map(|entity| {
                let workspace = entity.read(cx);
                let device = workspace.device();
                let key = workspace.identity();
                let supported = !Tab::for_product(device.product_id).is_empty();
                let owner = cx.entity().downgrade();
                let content = v_flex()
                    .size_full()
                    .pt(surface::css(10.))
                    .px(surface::css(20.))
                    .pb(surface::css(9.))
                    .child(dashboard_image(
                        device.product_id,
                        device.edition_id,
                        device.layout_id,
                        250.,
                        140.,
                    ))
                    .child(
                        div()
                            .w_full()
                            .px(surface::css(10.))
                            .min_h(surface::css(17.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .text_center()
                            .whitespace_normal()
                            .child(device.display_name().to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .text_color(MainPageColors.card_caption())
                            .text_center()
                            .child(if device.serial_number.starts_with("PREVIEW-") {
                                "预览设备"
                            } else {
                                "本地快照"
                            }),
                    );
                DashboardCard::button(
                    format!("open-{key}"),
                    format!("打开 {}", device.display_name()),
                    content,
                    move |window, cx| {
                        let _ = owner.update(cx, |shell, cx| {
                            shell.navigate(Location::Device(key.clone()), window, cx)
                        });
                    },
                )
                .unavailable(!supported)
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
        // AVAILABLE_MODULES retains IoT and Tour without installer results.
        let wifi_owner = cx.entity().downgrade();
        let tour_owner = wifi_owner.clone();
        let cards = vec![
            DashboardCard::button(
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
        ];
        self.dashboard_card_group(
            "module",
            "dashboard-modules",
            "MODULES_HEADER",
            layout,
            cards,
            cx,
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
