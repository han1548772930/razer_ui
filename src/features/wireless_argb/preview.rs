//! Isolated state fixtures. Requests are recorded; service success is never inferred.
use super::*;
use crate::ui::scroll::SourceScrollable as _;

pub(crate) fn open_preview(window: &mut Window, cx: &mut App) {
    let page = cx.new(|cx| WirelessArgb::for_product(3884, window, cx));
    page.update(cx, |view, cx| {
        view.preview = true;
        view.observation = Observation::Ready;
        view.active_ports = vec![1, 2, 3];
        view.detected = [(1, 40), (2, 40), (3, 40)].into();
        view.auto_detection = true;
        cx.notify();
    });
    let preview = cx.new(|cx| {
        let observed = cx.observe(&page, |_, _, cx| cx.notify());
        ArgbPreview {
            page,
            pid: 3884,
            sample: "ready",
            _observed: observed,
        }
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("无线 ARGB 控制器 · 界面预览")
            .w(window.rem_size() * (1150. / 16.))
            .child(preview.clone())
    });
}
struct ArgbPreview {
    page: Entity<WirelessArgb>,
    pid: u32,
    sample: &'static str,
    _observed: Subscription,
}
const SAMPLES: [(&str, &str); 12] = [
    ("ready", "三个端口"),
    ("single", "单个端口"),
    ("detecting", "正在检测"),
    ("empty", "未检测到设备"),
    ("standby", "待机"),
    ("mobile", "手机同步"),
    ("bluetooth", "蓝牙模式"),
    ("dc", "直流电源断开"),
    ("protection", "过流保护"),
    ("limit", "超出 LED 上限"),
    ("unavailable", "服务不可用"),
    ("auto_off", "自动检测关闭"),
];
impl ArgbPreview {
    fn select(&mut self, sample: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.sample = sample;
        self.page.update(cx, |page, cx| {
            page.spec = spec(self.pid);
            page.preview = true;
            page.observation = match sample {
                "detecting" => Observation::Detecting,
                "empty" => Observation::Empty,
                "standby" => Observation::Standby,
                "mobile" => Observation::Mobile,
                "bluetooth" => Observation::Bluetooth,
                "dc" => Observation::DcRequired,
                "protection" => Observation::Protection,
                "limit" => Observation::LedLimit,
                "unavailable" => Observation::Unavailable,
                _ => Observation::Ready,
            };
            if self.pid == 3884 && sample == "dc" {
                page.observation = Observation::Unavailable;
            }
            page.active_ports = if sample == "single" {
                vec![1]
            } else {
                vec![1, 2, 3]
            };
            page.detected = [(1, 40), (2, 40), (3, 40)].into();
            page.auto_detection = sample != "auto_off";
            page.last_request = None;
            page.alert = None;
            page.limit_dismissed = false;
            page.restore(None, window, cx);
            cx.notify();
        });
        cx.notify();
    }
}
impl Render for ArgbPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reveal = self.page.read(cx).reveal_editor;
        v_flex().gap_3()
            .child(surface::note("隔离的界面示例。端口及检测数均为示例；编辑仅影响此预览。设备命令只记录请求，结果须手动切换状态。",cx))
            .child(h_flex().gap_2().children([(3884,"Chroma Wireless ARGB Controller 3884"),(3886,"Chroma Wireless ARGB Controller 3886")].map(|(pid,label)|Button::new(("argb-preview-product",pid)).outline().small().selected(self.pid==pid).label(label).on_click(cx.listener(move |this,_,window,cx|{this.pid=pid;this.select("ready",window,cx);})))))
            .child(h_flex().flex_wrap().gap_2().children(SAMPLES.into_iter().filter(|(key,_)|self.pid==3886||*key!="dc").map(|(key,label)|Button::new(SharedString::from(format!("argb-preview-{key}"))).outline().small().label(label).selected(self.sample==key).on_click(cx.listener(move |this,_,window,cx|this.select(key,window,cx))))))
            .when(self.pid==3886,|view|view.child(v_flex().gap_2().child(surface::note("3886 当前源码的端口渲染条件恒为 false。下方按钮仅展示源码中已定义、但未实际挂载的端口编辑器。",cx)).child(Button::new("argb-preview-unmounted").outline().small().selected(reveal).label("展示未挂载的端口编辑器").on_click(cx.listener(|this,_,_,cx|{this.page.update(cx,|p,cx|{p.reveal_editor=!p.reveal_editor;cx.notify();});cx.notify();})))))
            .child(div().id("argb-preview-scroll").h(surface::css(500.)).scrollable_both().child(self.page.clone()))
            .when_some(self.page.read(cx).last_request.clone(),|view,request|view.child(surface::note(format!("示例请求：{request}"),cx)))
    }
}
