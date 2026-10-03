//! Explicit fixture controls are the sole source of preview device observations.
use super::*;
use crate::ui::scroll::SourceScrollable as _;
use state::Segment;

const SCENARIOS: [(&str, &str, &str); 11] = [
    ("ready", "全部端口", "All ports"),
    ("single", "单端口", "One port"),
    ("bends", "四段灯带", "Four-sided strip"),
    ("fans", "多个风扇", "Multiple fans"),
    ("port_limit", "端口超限", "Port LED limit"),
    ("limit", "240 LED 超限", "240 LED limit"),
    ("power", "未接电源", "No external power"),
    ("protection", "过流保护", "Overcurrent protection"),
    ("empty", "无设备", "No devices"),
    ("refresh", "检测中", "Detecting"),
    ("unavailable", "服务不可用", "Service unavailable"),
];

pub(crate) fn open_preview(window: &mut Window, cx: &mut App) {
    let preview = cx.new(|cx| {
        let page = cx.new(|_| WiredArgbWorkspace::empty(spec(3871), 0));
        page.update(cx, |page, cx| apply(page, "ready", window, cx));
        let observer = cx.observe(&page, |_, _, cx| cx.notify());
        Preview {
            page,
            pid: 3871,
            edition: 0,
            scenario: "ready",
            _observer: observer,
        }
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title(local(
                "ARGB 端口 · 界面预览",
                "ARGB ports · Interface preview",
            ))
            .w(window.rem_size() * (1220. / 16.))
            .child(preview.clone())
    });
}
struct Preview {
    page: Entity<WiredArgbWorkspace>,
    pid: u32,
    edition: u32,
    scenario: &'static str,
    _observer: Subscription,
}
impl Preview {
    fn select(&mut self, scenario: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.scenario = scenario;
        self.page.update(cx, |page, cx| {
            page.spec = spec(self.pid);
            page.edition = self.edition;
            apply(page, scenario, window, cx);
        });
        cx.notify();
    }
}
impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3()
            .child(surface::note(local(
                "隔离的界面示例。端口、电源和检测数量均为示例数据；编辑不会写入硬件。刷新结果由下方状态按钮切换。",
                "Isolated interface preview. Ports, power and detected counts are sample data; edits do not write hardware. Choose a state below to supply a refresh result."), cx))
            .child(h_flex().gap_2().children([(3871, "Chroma Addressable RGB Controller"), (778, "ASRock B550 Taichi Razer Edition")].map(|(pid, label)| {
                button::Button::new(("argb-preview-product", pid)).label(spec(pid).name.clone()).tooltip(label).outline().small().selected(self.pid == pid)
                    .on_click(cx.listener(move |this, _, window, cx| { this.pid = pid; this.edition = 0; this.select("ready", window, cx); }))
            })))
            .when(self.pid == 778, |v| v.child(h_flex().gap_2().children([0, 128, 129].map(|edition| {
                button::Button::new(("argb-preview-edition", edition)).label(format!("Edition {edition}")).outline().small().selected(self.edition == edition)
                    .on_click(cx.listener(move |this, _, window, cx| { this.edition = edition; this.select(this.scenario, window, cx); }))
            }))))
            .child(h_flex().gap_2().flex_wrap().children(SCENARIOS.into_iter().filter(|(id, _, _)| self.pid == 3871 || !["limit", "power", "protection", "refresh"].contains(id)).map(|(id, zh, en)| {
                button::Button::new((ElementId::from("argb-preview-scenario"), id)).label(local(zh, en)).outline().small().selected(self.scenario == id)
                    .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)))
            })))
            .child(div().id("argb-preview-content").h(surface::css(560.)).scrollable_both()
                .child(div().min_w(surface::css(1120.)).p(surface::css(20.)).bg(cx.theme().background).child(self.page.clone())))
    }
}
fn apply(
    page: &mut WiredArgbWorkspace,
    scenario: &str,
    window: &mut Window,
    cx: &mut Context<WiredArgbWorkspace>,
) {
    page.preview = true;
    page.auto_detection = Some(false);
    page.limit_dismissed = false;
    page.last_request = None;
    page.status = match scenario {
        "power" => Status::NoPower,
        "protection" => Status::Protection,
        "empty" if !page.spec.mainboard() => Status::NoDevices,
        "refresh" => Status::Refreshing,
        "unavailable" => Status::Unavailable,
        _ => Status::Ready,
    };
    let ids = if page.spec.mainboard() {
        page.spec.mainboard_ports.clone()
    } else {
        vec![1, 2, 3, 4, 5, 6]
    };
    page.observations = ids
        .iter()
        .enumerate()
        .map(|(ix, id)| PortObservation {
            id: *id,
            active: !["empty", "unavailable"].contains(&scenario)
                && (scenario != "single" || ix == 0),
            maximum_leds: 80,
            detected_leds: 20,
        })
        .collect();
    page.draft = Draft::default();
    for (ix, id) in ids.iter().enumerate() {
        let mut draft = PortDraft {
            name: format!("ARGB {}", ix + 1),
            is_strip_mode: true,
            strip: vec![Segment {
                id: 0,
                value: if scenario == "limit" { 60 } else { 20 },
            }],
            fan: vec![Segment { id: 1, value: 20 }],
            dismissed: false,
        };
        if scenario == "bends" && ix == 0 {
            draft.strip = vec![
                Segment { id: 0, value: 10 },
                Segment { id: 2, value: 10 },
                Segment { id: 3, value: 10 },
                Segment { id: 4, value: 10 },
            ];
        }
        if ["fans", "port_limit"].contains(&scenario) && ix == 0 {
            draft.is_strip_mode = false;
            draft.fan = [1, 2, 3]
                .into_iter()
                .map(|id| Segment {
                    id,
                    value: if scenario == "port_limit" { 32 } else { 20 },
                })
                .collect();
        }
        page.draft.ports.insert(*id, draft);
    }
    page.mount_ports(window, cx);
    cx.notify();
}
