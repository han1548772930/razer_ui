//! Explicit development fixtures; these never enter a device profile or service.
use super::*;
use crate::ui::scroll::SourceScrollable as _;

const SAMPLES: [(&str, &str); 18] = [
    ("ready", "未配对"),
    ("loading", "读取中"),
    ("scanning", "扫描中"),
    ("empty", "扫描无结果"),
    ("choices", "多个候选"),
    ("pairing", "配对中"),
    ("failed", "配对失败"),
    ("paired", "已配对"),
    ("both", "双设备已配对"),
    ("confirm", "取消配对确认"),
    ("unpairing", "取消配对中"),
    ("unpair_failed", "取消配对失败"),
    ("unpaired", "取消配对完成"),
    ("just_paired", "配对完成"),
    ("unavailable", "配对服务不可用"),
    ("multi_enabled", "多设备工具可用"),
    ("multi_disabled", "多设备工具禁用"),
    ("original_dongle", "原接收器同时连接"),
];
pub(crate) fn open_preview(window: &mut Window, cx: &mut App) {
    let spec = spec(241);
    let dialog = DockDialog::open(
        spec,
        0,
        0,
        "Sample Mouse Dock V2 Pro".into(),
        sample("ready", true),
        true,
        window,
        cx,
    );
    dialog.update(cx, |view, cx| view.close(window, cx));
    let page = cx.new(|_| DockPairing {
        spec,
        edition: 0,
        layout: 0,
        name: "Sample Mouse Dock V2 Pro".into(),
        state: sample("ready", true),
        multi_pairing: false,
        dongle: false,
        original_dongle: false,
        preview: true,
        alert: None,
        modal: None,
        known_devices: Vec::new(),
    });
    let preview = cx.new(|cx| {
        let observed = cx.observe(&dialog, |_, _, cx| cx.notify());
        let opened = cx.subscribe_in(
            &page,
            window,
            |this: &mut DockPreview, _, _: &PreviewDialogRequested, window, cx| {
                this.show_page = false;
                this.dialog.update(cx, |view, cx| view.reopen(window, cx));
                cx.notify();
            },
        );
        let closed = cx.subscribe(
            &dialog,
            |this: &mut DockPreview, _, _: &dialog::DockDialogClosed, cx| {
                this.show_page = true;
                cx.notify();
            },
        );
        DockPreview {
            dialog,
            page,
            pid: 241,
            sample: "ready",
            show_page: true,
            _subscriptions: vec![observed, opened, closed],
        }
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("鼠标底座配对 · 界面预览")
            .w(window.rem_size() * (1050. / 16.))
            .child(preview.clone())
    });
}
struct DockPreview {
    dialog: Entity<DockDialog>,
    page: Entity<DockPairing>,
    pid: u32,
    sample: &'static str,
    show_page: bool,
    _subscriptions: Vec<Subscription>,
}
impl DockPreview {
    fn select(&mut self, id: &'static str, cx: &mut Context<Self>) {
        self.sample = id;
        let dual = self.pid == 241;
        let name = if dual {
            "Sample Mouse Dock V2 Pro"
        } else {
            "Sample Mouse Dock Pro"
        };
        self.dialog.update(cx, |view, cx| {
            view.spec = spec(self.pid);
            view.name = name.into();
            view.state = sample(id, dual);
            view.preview = id != "unavailable";
            view.last_request = None;
            view.alert = (id == "unavailable").then(|| "暂时无法读取配对信息。".into());
            cx.notify();
        });
        self.page.update(cx, |view, cx| {
            view.spec = spec(self.pid);
            view.name = name.into();
            view.state = sample(id, dual);
            view.multi_pairing = !dual && ["multi_enabled", "multi_disabled"].contains(&id);
            view.dongle = id == "multi_enabled";
            view.original_dongle = dual && id == "original_dongle";
            view.alert = None;
            cx.notify();
        });
        if ["multi_enabled", "multi_disabled", "original_dongle"].contains(&id) {
            self.show_page = true;
        }
        cx.notify();
    }
}
impl Render for DockPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(surface::note(
                "隔离的界面示例。候选设备均为示例；操作仅记录请求，服务结果须手动切换状态。",
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .children(
                        [(true, "设备页面"), (false, "配对弹窗")].map(|(page, label)| {
                            button::Button::new((ElementId::from("dock-preview-surface"), label))
                                .label(label)
                                .outline()
                                .selected(self.show_page == page)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.show_page = page;
                                    this.dialog.update(cx, |view, cx| {
                                        if page {
                                            view.close(window, cx);
                                        } else {
                                            view.reopen(window, cx);
                                        }
                                    });
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .children(
                        [(164, "Dock Pro"), (241, "Dock V2 Pro")].map(|(pid, label)| {
                            button::Button::new(("dock-preview-product", pid))
                                .label(label)
                                .outline()
                                .selected(self.pid == pid)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.pid = pid;
                                    this.select("ready", cx);
                                }))
                        }),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .children(SAMPLES.map(|(id, label)| {
                        button::Button::new((ElementId::from("dock-preview-sample"), id))
                            .label(label)
                            .outline()
                            .selected(self.sample == id)
                            .on_click(cx.listener(move |this, _, _, cx| this.select(id, cx)))
                    })),
            )
            .child(
                div()
                    .id("dock-preview-content")
                    .h(surface::css(500.))
                    .scrollable_both()
                    .child(
                        div()
                            .min_w(surface::css(850.))
                            .bg(Colors::panel())
                            .child(self.page.clone()),
                    ),
            )
            .when(!self.show_page, |view| view.child(self.dialog.clone()))
            .when_some(
                self.dialog.read(cx).last_request.clone(),
                |v, (kind, payload)| {
                    v.child(surface::note(format!("示例请求：{kind} {payload}"), cx))
                },
            )
    }
}
fn peer(lane: Lane, id: &str) -> Peer {
    Peer {
        id: id.into(),
        name: format!("Sample {} {id}", lane.key()),
        product_id: 0,
        dongle_id: None,
        edition: 0,
        layout: 0,
        lane,
    }
}
fn sample(id: &str, dual: bool) -> PairingState {
    let mut state = PairingState::default();
    for channel in &mut state.channels {
        channel.status = Status::Ready;
    }
    let lane = Lane::Mouse;
    let status = match id {
        "loading" | "unavailable" => Status::Loading,
        "scanning" => Status::Scanning,
        "empty" | "choices" => Status::Scanned,
        "pairing" => Status::Pairing,
        "failed" => Status::PairFailed,
        "paired" | "both" | "original_dongle" => Status::Paired,
        "confirm" => Status::ConfirmUnpair,
        "unpairing" => Status::Unpairing,
        "unpair_failed" => Status::UnpairFailed,
        "unpaired" => Status::Unpaired,
        "just_paired" => Status::JustPaired,
        _ => Status::Ready,
    };
    state.channel_mut(lane).status = status;
    if matches!(status, Status::Loading) {
        state.channel_mut(Lane::Keyboard).status = status;
    }
    if matches!(
        status,
        Status::Paired
            | Status::ConfirmUnpair
            | Status::Unpairing
            | Status::UnpairFailed
            | Status::JustPaired
    ) {
        state.channel_mut(lane).peer = Some(peer(lane, "paired"));
    }
    if id == "both" && dual {
        state.channel_mut(Lane::Keyboard).peer = Some(peer(Lane::Keyboard, "paired"));
        state.channel_mut(Lane::Keyboard).status = Status::Paired;
    }
    if id == "choices" {
        let channel = state.channel_mut(lane);
        channel.candidates = vec![peer(lane, "one"), peer(lane, "two")];
        channel.selected = Some("one".into());
    }
    state
}
