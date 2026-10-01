//! Main frontend 9388/19388, IotPopupRoot/28256 and 6505/44442.
//! Browsing and navigation are local. Native discovery/install results are never
//! synthesized from clicks, timers, or the catalogue of supported products.
use crate::{i18n, model::Device, ui::surface};
use gpui_kit::component::{button::{Button, ButtonVariants}, *};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::collections::BTreeSet;

struct Product {
    name: &'static str,
    description: &'static str,
    image: &'static str,
    url: &'static str,
}
const PRODUCTS: &[Product] = &[
    Product { name: "AETHER_LIGHT_BULBS", description: "AETHER_LIGHT_BULBS_DES", image: "synapse/gr-bulb.png", url: "https://www.razer.com/gamer-room-lights/razer-aether-light-bulb" },
    Product { name: "AETHER_LIGHT_STRIP", description: "AETHER_LIGHT_STRIP_DES", image: "synapse/gr-strip.png", url: "https://www.razer.com/gamer-room-lights/razer-aether-light-strip" },
    Product { name: "AETHER_LAMP_PRO", description: "AETHER_LAMP_PRO_DES_1", image: "synapse/gr-lamp-pro.png", url: "https://www.razer.com/gamer-room-lights/razer-aether-lamp-pro" },
    Product { name: "AETHER_LAMP", description: "AETHER_LAMP_PRO_DES_2", image: "synapse/gr-lamp.png", url: "https://www.razer.com/gamer-room-lights/razer-aether-lamp" },
];

fn product_info(index: usize, cx: &App) -> AnyElement {
    let item = &PRODUCTS[index];
    v_flex().w(surface::css(284.)).max_w_full().gap(surface::css(16.)).items_center()
        .child(img(item.image).w(surface::css(250.)).max_w_full().h(surface::css(140.)).object_fit(ObjectFit::Contain))
        .child(div().font_family("RazerF5").text_size(surface::css(18.)).text_center().child(i18n::t(item.name)))
        .child(div().text_size(surface::css(14.)).text_center().whitespace_normal().child(i18n::t(item.description)))
        .child(surface::external(["gr-product-bulb", "gr-product-strip", "gr-product-lamp-pro", "gr-product-lamp"][index], i18n::t("LEARN_MORE"), item.url))
        .text_color(cx.theme().foreground)
        .into_any_element()
}

pub(super) struct GamerRoomPage {
    collapsed: [bool; 2],
    hovered_product: Option<usize>,
    tour_step: Option<usize>,
    stored_seen: Option<bool>,
}
pub(super) enum GamerRoomEvent { TutorialCompleted }
impl EventEmitter<GamerRoomEvent> for GamerRoomPage {}
impl GamerRoomPage {
    pub(super) fn new() -> Self {
        Self { collapsed: [false; 2], hovered_product: None, tour_step: None, stored_seen: None }
    }
    pub(super) fn set_tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if self.stored_seen == Some(seen) { return; }
        self.stored_seen = Some(seen);
        self.tour_step = if seen { None } else { Some(0) };
        cx.notify();
    }
    pub(super) fn reset_tutorial(&mut self, cx: &mut Context<Self>) {
        self.stored_seen = Some(false);
        self.tour_step = Some(0);
        cx.notify();
    }
    fn complete_tutorial(&mut self, cx: &mut Context<Self>) {
        self.tour_step = None;
        self.stored_seen = Some(true);
        cx.emit(GamerRoomEvent::TutorialCompleted);
        cx.notify();
    }
    fn open_add(&self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.new(|cx| AddGamerRoom { step: AddStep::Prepare, focus: cx.focus_handle() });
        window.open_dialog(cx, move |dialog, window, _| dialog
            .title(i18n::t("ADD_OTHER_WIFI_DEVICE"))
            .w((window.rem_size() * (850. / 16.)).min((window.viewport_size().width - px(40.)).max(px(260.))))
            .child(view.clone()));
    }
    fn open_product(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.hovered_product = None;
        window.open_dialog(cx, move |dialog, window, cx| dialog
            .title(i18n::t(PRODUCTS[index].name))
            .w((window.rem_size() * (if index == 2 { 700. } else { 390. } / 16.)).min((window.viewport_size().width - px(40.)).max(px(260.))))
            .child(h_flex().justify_center().flex_wrap().gap(surface::css(24.)).child(product_info(index, cx))
                .when(index == 2, |view| view.child(product_info(3, cx))))
            .footer(Button::new("gr-product-close").label("关闭").on_click(|_, w, cx| w.close_dialog(cx))));
        cx.notify();
    }
    fn banner(&self, cx: &mut Context<Self>) -> AnyElement {
        div().id("gamer-room-banner").test_support().relative().w_full().min_w(surface::css(600.)).h(surface::css(531.))
            .rounded(surface::css(5.)).overflow_hidden()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| { if !*hovered { this.hovered_product = None; cx.notify(); } }))
            .child(img("synapse/gr-background.png").absolute().size_full().object_fit(ObjectFit::Cover))
            .child(v_flex().absolute().top(surface::css(36.)).left(surface::css(40.)).w(surface::css(450.)).gap(surface::css(14.))
                .child(div().font_family("RazerF5").text_size(surface::css(20.)).child(i18n::t("RAZER_GAMER_ROOM")))
                .child(div().font_family("RazerF5").text_size(surface::css(34.)).child(i18n::t("RAZER_GAMER_ROOM_TILE")))
                .child(div().text_size(surface::css(14.)).whitespace_normal().child(i18n::t("RAZER_GAMER_ROOM_DES")))
                .child(surface::external("gamer-room-learn", i18n::t("LEARN_MORE"), "https://www.razer.com/pc/gamer-room")))
            .children([(0, 0.11, 0.57), (1, 0.39, 0.68), (2, 0.72, 0.64)].into_iter().map(|(index, x, y)| {
                Button::new(SharedString::from(format!("gr-hotspot-{index}"))).outline()
                    .absolute().left(relative(x)).top(relative(y)).h(surface::css(30.))
                    .label(i18n::t(PRODUCTS[index].name))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| { if *hovered { this.hovered_product = Some(index); cx.notify(); } }))
                    .on_click(cx.listener(move |this, _, w, cx| this.open_product(index, w, cx)))
            }))
            .when_some(self.hovered_product, |view, index| view.child(
                h_flex().id("gr-hover-product").absolute().top(surface::css(100.)).right(surface::css(20.)).p(surface::css(24.))
                    .gap(surface::css(24.)).bg(cx.theme().background.opacity(0.97)).border_1().border_color(cx.theme().border)
                    .child(product_info(index, cx)).when(index == 2, |view| view.child(product_info(3, cx)))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| { if !*hovered { this.hovered_product = None; cx.notify(); } }))
            )).into_any_element()
    }
    fn group(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let (title, tip, description) = if index == 0 {
            ("SYNAPSE_OVERRIDE_HEADER", "SYNAPSE_OVERRIDE_TIP", "SYNAPSE_OVERRIDE_MSG")
        } else { ("CONTROLLED_BY_GAMER_ROOM", "CONTROLLED_GAMER_ROOM_APP_TIP", "CONTROLLED_BY_GAMER_ROOM_MSG") };
        v_flex().id(SharedString::from(format!("gr-group-{index}"))).test_support().w_full().mt(surface::css(30.))
            .child(Button::new(SharedString::from(format!("gr-group-toggle-{index}"))).ghost().justify_start().p_0()
                .child(h_flex().gap(surface::css(10.))
                    .child(Icon::default().path("synapse/expand.svg").size(surface::css(10.))
                        .transform(Transformation::rotate(radians(if self.collapsed[index] { -std::f32::consts::FRAC_PI_2 } else { 0. }))))
                    .child(i18n::t(title)))
                .tooltip(i18n::t(tip))
                .on_click(cx.listener(move |this, _, _, cx| { this.collapsed[index] = !this.collapsed[index]; cx.notify(); })))
            .when(!self.collapsed[index], |view| view.child(
                v_flex().mt(surface::css(10.)).gap(surface::css(12.))
                    .child(surface::note(i18n::t(description), cx))
                    .when(index == 0, |view| view.child(Button::new("gamer-room-add").ghost()
                        .w(surface::css(186.)).h(surface::css(176.)).p(surface::css(15.)).border_2().border_dashed()
                        .border_color(cx.theme().border).rounded(cx.theme().font_size * (5. / 16.))
                        .child(v_flex().items_center().gap(surface::css(12.))
                            .child(img("synapse/gr-add-device.svg").size(surface::css(56.)))
                            .child(div().underline().text_center().whitespace_normal().child(i18n::t("ADD_OTHER_WIFI_DEVICE"))))
                        .on_click(cx.listener(|this, _, w, cx| this.open_add(w, cx)))))
            )).into_any_element()
    }
    fn tour(&self, step: usize, cx: &mut Context<Self>) -> AnyElement {
        let texts = ["GAMER_ROOM_TUTORIAL_HEADER_DESC_1", "GAMER_ROOM_TUTORIAL_HEADER_DESC_2"];
        let videos = [
            "https://apps.razer.com/synapse/dashboard/static/media/Gamer%20Room%20Dashboard%20Tutorial%201.080f80fb.mp4",
            "https://apps.razer.com/synapse/dashboard/static/media/Gamer%20Room%20Dashboard%20Tutorial%202.b4e338ae.mp4",
        ];
        v_flex().id("gamer-room-tour").test_support().w(surface::css(330.)).gap(surface::css(12.))
            .p(surface::css(20.)).bg(cx.theme().group_box).border_1().border_color(rgb(0xfd8611))
            .child(h_flex().justify_between().child(div().text_color(rgb(0xfd8611)).child(i18n::t("GAMER_ROOM_TUTORIAL_HEADER")))
                .child(Button::new("gr-tour-skip").ghost().label("跳过").on_click(cx.listener(|this, _, _, cx| this.complete_tutorial(cx)))))
            .child(div().whitespace_normal().child(i18n::t(texts[step])))
            .child(surface::external("gr-tour-video", "播放原教程视频", videos[step]))
            .child(h_flex().justify_between()
                .child(Button::new("gr-tour-back").label("上一步").disabled(step == 0).on_click(cx.listener(|this, _, _, cx| { this.tour_step = Some(0); cx.notify(); })))
                .child(Button::new("gr-tour-next").primary().label(if step == 0 { "下一步" } else { "完成" })
                    .on_click(cx.listener(move |this, _, _, cx| { if step == 0 { this.tour_step = Some(1); cx.notify(); } else { this.complete_tutorial(cx); } }))))
            .child(div().text_center().child(format!("{} / 2", step + 1)))
            .into_any_element()
    }
}
impl Render for GamerRoomPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().id("gamer-room").test_support().w_full().min_w(surface::css(600.))
            .child(self.banner(cx))
            .child(h_flex().mt(surface::css(14.)).justify_between()
                .child(surface::note("尚未连接 Gamer Room 设备服务。", cx))
                .child(Button::new("gamer-room-tutorial").ghost().label("查看教程")
                    .on_click(cx.listener(|this, _, _, cx| { this.tour_step = Some(0); cx.notify(); }))))
            .when_some(self.tour_step, |view, step| view.child(self.tour(step, cx)))
            .child(self.group(0, cx)).child(self.group(1, cx))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddStep { Prepare, MobileApp, Discover }
struct AddGamerRoom { step: AddStep, focus: FocusHandle }
impl AddGamerRoom {
    fn change_step(&mut self, step: AddStep, window: &mut Window, cx: &mut Context<Self>) {
        self.step = step;
        window.focus(&self.focus, cx);
        cx.notify();
    }
}
impl Render for AddGamerRoom {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.step {
            AddStep::Prepare => v_flex().gap(surface::css(22.)).items_center()
                .child(img("synapse/gr-add-device.svg").size(surface::css(56.)))
                .child(div().text_center().whitespace_normal().child(i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_DESC")))
                .child(surface::external("gr-add-how", "操作说明", "https://mysupport.razer.com/app/answers/detail/a_id/5753"))
                .child(Button::new("gr-add-mobile").outline().w_full()
                    .child(h_flex().gap(surface::css(12.)).child(img("synapse/gr-mobile.svg").size(surface::css(32.)))
                        .child(div().flex_1().whitespace_normal().child(i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_MOBILE_DEVICE_DESC"))))
                    .on_click(cx.listener(|this, _, window, cx| this.change_step(AddStep::MobileApp, window, cx))))
                .child(div().text_center().whitespace_normal().child(i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_DESC1")))
                .child(Button::new("gr-add-search").primary().label("搜索设备")
                    .on_click(cx.listener(|this, _, window, cx| this.change_step(AddStep::Discover, window, cx))))
                .into_any_element(),
            AddStep::MobileApp => v_flex().gap(surface::css(20.)).items_center()
                .child(h_flex().gap(surface::css(10.)).child(img("synapse/gr-app.svg").size(surface::css(32.))).child("Razer Gamer Room"))
                .child(div().text_center().child(i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_MOBILE_DEVICE_DESC")))
                .child(img("synapse/gr-qr.svg").size(surface::css(110.)))
                .child(surface::external("gr-app-download", "https://rzr.to/gamer-room-app", "https://rzr.to/gamer-room-app"))
                .child(surface::external("gr-app-help", "设备帮助", "https://mysupport.razer.com/app/answers/detail/a_id/5911"))
                .into_any_element(),
            AddStep::Discover => v_flex().gap(surface::css(20.)).items_center()
                .child(div().text_center().whitespace_normal().child(i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_SCAN_DESC")))
                .child(div().id("gr-discovery-unavailable").test_support().w_full().p(surface::css(24.))
                    .bg(cx.theme().group_box).child(v_flex().gap(surface::css(12.))
                        .child("设备搜索服务未连接")
                        .child(surface::note("无法读取网络中的 Gamer Room 设备。连接服务后才能搜索、识别并添加设备。", cx))))
                .child(surface::external("gr-compatible", "查看兼容设备", "https://mysupport.razer.com/app/answers/detail/a_id/5895"))
                .child(surface::external("gr-search-help", "操作说明", "https://mysupport.razer.com/app/answers/detail/a_id/5753"))
                .child(Button::new("gr-scan-refresh").primary().label("重新搜索").disabled(true)
                    .tooltip("尚未连接 IoT 设备搜索服务"))
                .into_any_element(),
        };
        v_flex().id("gamer-room-add-dialog").test_support().track_focus(&self.focus).tab_group().w_full().gap(surface::css(24.)).p(surface::css(20.))
            .child(body)
            .child(h_flex().justify_end().gap(surface::css(12.))
                .when(self.step != AddStep::Prepare, |view| view.child(Button::new("gr-add-back").label("返回")
                    .on_click(cx.listener(|this, _, window, cx| this.change_step(AddStep::Prepare, window, cx)))))
                .child(Button::new("gr-add-cancel").label("关闭").on_click(|_, w, cx| w.close_dialog(cx))))
    }
}

struct Module {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    image: Option<&'static str>,
    description: &'static str,
    url: Option<&'static str>,
}
// 6505/44442 ne + te are a static catalogue. No installed/available state is
// inferred from these records; that state belongs to the installer service.
const MODULES: &[Module] = &[
    Module { id: "macro", title: "宏", icon: "synapse/module-macro.svg", image: Some("synapse/module-macro.png"), description: "通过宏模块为你喜爱的游戏引入强大的宏功能。轻松创建一组复杂的按键敲击操作，然后只需轻轻一按，即可准确地执行致胜的按键组合。", url: None },
    Module { id: "alexa", title: "Alexa", icon: "synapse/module-alexa.svg", image: Some("synapse/module-alexa.png"), description: "对于所有支持 Chroma 幻彩的设备，Amazon Alexa 模块将完整的 Alexa Voice Service 集成到 Synapse 雷云中。需要有效的麦克风和 Amazon Alexa 账户。", url: Some("https://www.razer.com/chroma/alexa") },
    Module { id: "linked-games", title: "已关联的游戏", icon: "synapse/module-linked-games.svg", image: None, description: "原生游戏关联模块的安装状态尚未读取。设备 Profile 中的本地关联程序可在对应配置菜单中管理。", url: None },
    Module { id: "feedback", title: "反馈", icon: "synapse/module-feedback.svg", image: None, description: "原生反馈应用的安装状态尚未读取。", url: None },
    Module { id: "armory", title: "工坊", icon: "synapse/module-armory.svg", image: None, description: "原生 Armory 的安装和服务状态尚未读取。", url: None },
];
pub(super) struct ModuleCatalog { expanded: BTreeSet<&'static str> }
impl ModuleCatalog {
    pub(super) fn new() -> Self { Self { expanded: BTreeSet::new() } }
    fn module_row(&self, item: &'static Module, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.expanded.contains(item.id);
        v_flex().id(SharedString::from(format!("catalog-{}", item.id))).test_support().w_full()
            .child(h_flex().h(surface::css(80.)).px(surface::css(20.)).gap(surface::css(20.)).bg(cx.theme().group_box)
                .child(img(item.icon).size(surface::css(40.)))
                .child(div().flex_1().text_size(surface::css(16.)).child(item.title))
                .child(Button::new(SharedString::from(format!("module-details-{}", item.id))).ghost()
                    .label(if expanded { "显示较少" } else { "显示更多" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.expanded.remove(item.id) { this.expanded.insert(item.id); } cx.notify();
                    })))
                .child(surface::note("安装状态未读取", cx))
                .child(Button::new(SharedString::from(format!("module-install-{}", item.id))).label("安装").min_w(surface::css(90.))
                    .h(surface::css(27.)).disabled(true).tooltip("尚未连接模块安装服务")))
            .when(expanded, |view| view.child(h_flex().gap(surface::css(30.)).p(surface::css(20.)).bg(rgb(0x2d2d2d))
                .when_some(item.image, |view, image| view.child(img(image).w(surface::css(250.)).h(surface::css(140.)).object_fit(ObjectFit::Contain)))
                .child(v_flex().flex_1().gap(surface::css(16.))
                    .child(div().whitespace_normal().child(item.description))
                    .when_some(item.url, |view, url| view.child(surface::external("module-alexa-learn", "了解更多", url)))
                    .child(surface::note("安装包大小、版本和更新状态尚未读取。", cx)))))
            .into_any_element()
    }
}
impl Render for ModuleCatalog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().id("module-catalog").test_support().w_full().mt(surface::css(30.))
            .child(div().font_family("RazerF5").text_size(surface::css(24.)).text_color(cx.theme().primary)
                .mb(surface::css(10.)).child("模块目录"))
            .child(surface::note("查看模块说明；安装、卸载和更新需要原生模块服务。", cx))
            .children(MODULES.iter().map(|item| self.module_row(item, cx)))
    }
}

pub(super) fn open_device_details(device: Device, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, window, cx| {
        let mut values = vec![
            ("设备型号", device.product_id.to_string()),
            ("序列号", device.serial_number.clone()),
            ("固件版本（本地快照）", device.firmware_info.current_fw_version.clone()),
            ("本地配置数量", device.profiles.len().to_string()),
        ];
        if let Some(version) = &device.firmware_info.current_dock_fw_version { values.push(("接收器版本（本地快照）", version.clone())); }
        dialog.title(device.display_name())
            .w((window.rem_size() * (540. / 16.)).min((window.viewport_size().width - px(40.)).max(px(260.))))
            .child(v_flex().gap(surface::css(14.)).children(values.into_iter().map(|(label, value)| {
                h_flex().gap(surface::css(16.)).child(div().w(surface::css(180.)).child(label))
                    .child(div().flex_1().whitespace_normal().child(if value.is_empty() { "未提供".into() } else { value }))
            })).child(surface::note("未读取此设备的原生安装、连接或固件更新状态。", cx)))
            .footer(h_flex().gap(surface::css(12.))
                .child(Button::new("module-device-update").label("更新固件").disabled(true).tooltip("尚未读取目标版本和更新条件"))
                .child(Button::new("module-device-remove").label("移除设备").disabled(true).tooltip("尚未读取原生安装及可移除状态"))
                .child(Button::new("module-device-close").label("关闭").on_click(|_, w, cx| w.close_dialog(cx))))
    });
}
