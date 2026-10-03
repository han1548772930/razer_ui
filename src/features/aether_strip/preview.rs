//! Explicit, nonpersistent fixtures; requests never turn themselves into success.
use super::*;
use crate::features::source_workspace::SourceProductWorkspace;

const SCENES: [(&str, &str); 13] = [
    ("straight", "单边"),
    ("corner", "两边"),
    ("three", "三边"),
    ("four", "四边"),
    ("remaining", "未分配 LED"),
    ("empty", "检测为 0"),
    ("refreshing", "刷新中"),
    ("offline", "离线"),
    ("locked", "其他应用控制"),
    ("off", "关灯"),
    ("external", "Gamer Room 控制"),
    ("multiple", "多个设备"),
    ("unknown", "服务不可用"),
];
pub(crate) fn open_preview(window: &mut Window, cx: &mut App) {
    let device = crate::demo::registered_preview(784).expect("registered Aether Light Strip");
    let workspace = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
    let page = workspace
        .read(cx)
        .aether_preview_page()
        .expect("Aether page attached");
    page.update(cx, |view, cx| apply(view, "straight", window, cx));
    let preview = cx.new(|cx| {
        let subscription = cx.observe(&page, |_, _, cx| cx.notify());
        StripPreview {
            page,
            workspace,
            scene: "straight",
            _subscription: subscription,
        }
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("Aether Light Strip · 界面预览")
            .w(window.rem_size() * (1100. / 16.))
            .child(preview.clone())
    });
}
fn apply(view: &mut AetherStrip, scene: &str, window: &mut Window, cx: &mut Context<AetherStrip>) {
    view.dismiss(window, cx);
    view.preview = scene != "unknown";
    view.last_request = None;
    view.observation = if scene == "unknown" {
        Observation::default()
    } else {
        Observation {
            online: Some(scene != "offline"),
            locked: Some(scene == "locked"),
            power_on: Some(scene != "off"),
            synapse_override: Some(scene != "external"),
            detected: Some(if scene == "empty" { 0 } else { 60 }),
            refreshing: scene == "refreshing",
        }
    };
    let sides = match scene {
        "corner" => 2,
        "three" => 3,
        "four" | "remaining" => 4,
        _ => 1,
    };
    view.bends = state::distribute(view.observation.detected.unwrap_or(0), sides);
    if scene == "remaining" {
        view.bends[0].value = 5;
    }
    let mut main = Peer::new("sample-aether-a".into(), "Sample Aether Light Strip".into());
    main.observation = view.observation.clone();
    view.selected = main.id.clone();
    view.peers = vec![main];
    if scene == "multiple" {
        for (id, name, online, locked) in [
            ("sample-aether-b", "Sample Desk Strip", true, true),
            ("sample-aether-c", "Sample Shelf Strip", false, false),
        ] {
            let mut peer = Peer::new(id.into(), name.into());
            peer.observation = Observation {
                online: Some(online),
                locked: Some(locked),
                power_on: Some(true),
                synapse_override: Some(true),
                detected: Some(45),
                refreshing: false,
            };
            view.peers.push(peer);
        }
    }
    view.sync_inputs(window, cx);
    cx.notify();
}
struct StripPreview {
    page: Entity<AetherStrip>,
    workspace: Entity<SourceProductWorkspace>,
    scene: &'static str,
    _subscription: Subscription,
}
impl Render for StripPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let request = self
            .page
            .read(cx)
            .last_request
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        v_flex().gap_3()
            .child(surface::note("隔离的界面示例。60/45 个 LED 与所有设备状态均为示例；操作只记录请求，不模拟设备成功响应。",cx))
            .child(h_flex().gap_2().flex_wrap().children(SCENES.map(|(scene,label)| {
                button::Button::new((ElementId::from("aether-preview-scene"),scene)).small().outline().selected(self.scene==scene).label(label)
                    .on_click(cx.listener(move |this,_,window,cx| {
                        this.scene=scene;
                        this.workspace.update(cx, |workspace, cx| workspace.set_page_key("CUSTOMIZED", window, cx));
                        this.page.update(cx,|view,cx|apply(view,scene,window,cx));
                        cx.notify();
                    }))
            })))
            .child(h_flex().gap_2().children([(device::DialogKind::Remove,"移除设备弹层"),(device::DialogKind::TakeControl,"接管设备弹层")].map(|(kind,label)| {
                button::Button::new((ElementId::from("aether-preview-layer"),label)).small().outline().label(label)
                    .on_click(cx.listener(move |this,_,window,cx| {
                        this.page.update(cx,|view,cx|view.open_device_dialog(kind,window,cx));
                    }))
            })))
            .child(div().id("aether-preview-workspace").h(surface::css(650.)).min_h_0().child(self.workspace.clone()))
            .when_some(request,|v,request|v.child(surface::note(format!("已记录请求（尚无设备响应）：{request}"),cx)))
    }
}
