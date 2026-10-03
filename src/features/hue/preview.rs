//! Explicit development samples. Nothing here is written to a device/profile.
use super::*;
use crate::ui::scroll::SourceScrollable as _;

const SAMPLES: [(&str, &str); 18] = [
    ("init", "开始扫描"),
    ("scan", "扫描中"),
    ("manual_scan", "IP 搜索中"),
    ("manual", "扫描失败 / 手动 IP"),
    ("found", "找到网桥"),
    ("wait_pair", "已请求配对"),
    ("pair", "等待按网桥按钮"),
    ("failed", "配对失败"),
    ("retry", "重试等待"),
    ("paired", "已连接"),
    ("empty", "娱乐区为空"),
    ("offline", "网桥关闭"),
    ("busy", "被其他程序占用"),
    ("loading", "刷新中"),
    ("per_light", "逐灯亮度"),
    ("brightness_off", "亮度关闭"),
    ("advanced", "Chroma 未安装"),
    ("advanced_installed", "Chroma 无配置"),
];
pub(crate) fn open_preview(window: &mut Window, cx: &mut App) {
    let workspace = cx.new(|cx| HueWorkspace::new(window, cx));
    workspace.update(cx, |workspace, cx| {
        workspace.preview = true;
        workspace.sample("init", window, cx);
    });
    let preview = cx.new(|_| HuePreview {
        workspace,
        sample: "init",
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("Philips Hue · 界面预览")
            .w(window.rem_size() * (1280. / 16.))
            .child(preview.clone())
    });
}
struct HuePreview {
    workspace: Entity<HueWorkspace>,
    sample: &'static str,
}
impl Render for HuePreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3()
            .child(surface::note("以下为隔离的状态示例。设备与娱乐区均为示例数据；操作不会扫描网络或控制灯具。服务回应需通过状态选项切换。",cx))
            .child(h_flex().gap_2().flex_wrap().children(SAMPLES.map(|(id,label)|{
                button::Button::new((ElementId::from("hue-sample"),id)).label(label).outline().selected(self.sample==id)
                    .on_click(cx.listener(move |this,_,window,cx|{this.sample=id;this.workspace.update(cx,|view,cx|view.sample(id,window,cx));cx.notify();}))
            })))
            .child(div().id("hue-preview-scroll").h(surface::css(500.)).scrollable_both().child(self.workspace.clone()))
    }
}
impl HueWorkspace {
    pub(super) fn sample(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.alert = None;
        self.last_command = None;
        self.bridge = spec().initial.hue.clone();
        self.brightness = spec().initial.brightness.clone();
        self.draft = json!({});
        self.selected_effect = 3;
        self.effect_settings.clear();
        self.advanced = ["advanced", "advanced_installed"].contains(&id);
        self.chroma_installed = Some(id == "advanced_installed");
        for input in &self.ip {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.integration = match id {
            "scan" => Integration::Scanning,
            "manual_scan" => Integration::ScanningIp,
            "manual" => Integration::ScanFailed,
            "found" => Integration::ScanSuccessful,
            "wait_pair" => Integration::WaitUserClickPair,
            "pair" => Integration::Pairing,
            "failed" => Integration::PairFailed,
            "retry" => Integration::RetryPair,
            _ => Integration::Init,
        };
        if [
            "paired",
            "empty",
            "offline",
            "busy",
            "loading",
            "per_light",
            "brightness_off",
            "advanced",
            "advanced_installed",
        ]
        .contains(&id)
        {
            self.bridge.is_paired = true;
            self.bridge.bridge_enabled = !["offline", "busy"].contains(&id);
            self.bridge.is_control = id != "busy";
            self.bridge.is_loading = id == "loading";
            self.brightness.is_enabled = id != "brightness_off";
            self.brightness.value = 75;
            self.brightness.is_global_brightness = Some(id != "per_light");
            if id != "empty" {
                self.bridge.groups = vec![
                    Group {
                        id: "sample-area-desk".into(),
                        name: "Sample desk area".into(),
                    },
                    Group {
                        id: "sample-area-room".into(),
                        name: "Sample room area".into(),
                    },
                ];
                self.bridge.active_group = "sample-area-desk".into();
                self.bridge.devices = vec![
                    Light {
                        device_container_id: "sample-bulb-1".into(),
                        region_id: 17,
                        is_on: true,
                        name: "Sample channel 1".into(),
                        product_name: "Sample Hue light".into(),
                        raw_data: LightIdentity {
                            physical_name: "Sample desk light".into(),
                            physical_arche_type: "sultanbulb".into(),
                        },
                    },
                    Light {
                        device_container_id: "sample-strip-2".into(),
                        region_id: 42,
                        is_on: false,
                        name: "Sample channel 2".into(),
                        product_name: "Sample Hue lightstrip".into(),
                        raw_data: LightIdentity {
                            physical_name: "Sample lightstrip".into(),
                            physical_arche_type: "huelightstrip".into(),
                        },
                    },
                ];
                self.draft["ports"] = json!([{"id":17,"brightness":{"value":60}},{"id":42,"brightness":{"value":50}}]);
            }
        }
        let choices = self
            .bridge
            .groups
            .iter()
            .map(|g| Choice::new(&g.id, g.name.clone()))
            .collect::<Vec<_>>();
        self.groups.update(cx, |groups, cx| {
            groups.set_items(choices, window, cx);
            groups.set_selected_value(&self.bridge.active_group, window, cx);
        });
        self.rebuild_lights(window, cx);
        self.sync_effects(window, cx);
        cx.notify();
    }
}
