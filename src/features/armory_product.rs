//! Independent Armory roots from each current product bundle.
//!
//! 1303/jg, 1304/Qf and 1313/IK mount only ProductImage. 3893/HH mounts
//! CoolerInfo (`fg`), including its three metric groups, not fan controls.
//! See docs/re/armory-product-roots-current-evidence.json.
use crate::{
    i18n,
    model::{Device, DeviceCategory},
    resources,
    ui::surface::{self, css},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};

pub(crate) fn supports(device: &Device) -> bool {
    matches!(device.product_id, 1303 | 1304 | 1313 | 3893)
        || (device.product_id == 3894
            && matches!(
                device.category,
                DeviceCategory::Accessory | DeviceCategory::Mousepad
            ))
}

/// 3893's BC.runtimeData initial values. These are source initialization,
/// never a measurement or acknowledgement from a connected device.
#[derive(Default)]
struct HanboRuntime {
    cpu_temp: f32,
    cpu_speed: f32,
    fan_speed: f32,
    pump_speed: f32,
    liquid_temp: f32,
    gpu_temp: f32,
    gpu_speed: f32,
}
struct HanboState {
    celsius: bool,
    runtime: HanboRuntime,
}

#[derive(IntoElement)]
pub(crate) struct ArmoryProduct {
    device: Device,
}
impl ArmoryProduct {
    pub(crate) fn new(device: &Device) -> Self {
        Self {
            device: device.clone(),
        }
    }

    fn product_image(&self, cx: &App) -> AnyElement {
        let height = if self.device.product_id == 3893 {
            200.
        } else {
            250.
        };
        // Source dl/ry retries edition zero. Only its `img_prods/prd` artwork
        // may fill this slot. A missing asset preserves the source empty-image
        // state; Dashboard PluginImages never replace it.
        let artwork = resources::device_image(
            self.device.product_id,
            self.device.edition_id,
            self.device.layout_id,
            resources::DeviceImage::Product,
        );
        div()
            .relative()
            .w_full()
            .min_w_0()
            .max_w(css(1220.))
            .h(css(height))
            .my(css(10.))
            .mx_auto()
            .flex_shrink_0()
            .child(surface::dot_background(cx))
            .when_some(artwork, |view, path| {
                view.child(
                    img(path)
                        .relative()
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                )
            })
            .into_any_element()
    }
}

fn detail(label: &'static str, value: String, width: f32, last: bool) -> AnyElement {
    v_flex()
        .items_center()
        .p(css(20.))
        .text_size(css(14.))
        .when(!last, |v| v.border_r_1().border_color(rgb(0x707070)))
        .child(i18n::t(label).to_uppercase())
        .child(
            div()
                .w(css(width))
                .font_family("RazerF5")
                .text_size(css(30.))
                .text_center()
                .child(value),
        )
        .into_any_element()
}
fn group(children: impl IntoIterator<Item = AnyElement>) -> Div {
    h_flex()
        .items_stretch()
        .flex_shrink_0()
        .bg(rgb(0x111111))
        .border_1()
        .border_color(rgb(0x707070))
        .rounded(css(5.))
        .children(children)
}
fn temperature(value: f32, celsius: bool) -> String {
    if celsius {
        format!("{value} °C")
    } else {
        format!("{:.1} °F", value * 1.8 + 32.)
    }
}
impl RenderOnce for ArmoryProduct {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root = v_flex()
            .w_full()
            .min_w_0()
            .bg(rgb(0x222222))
            .font_family("Roboto")
            .text_size(css(14.))
            .text_color(rgb(0xcccccc))
            .child(self.product_image(cx));
        if self.device.product_id != 3893 {
            return root;
        }
        let state = window.use_keyed_state(
            (
                ElementId::from(SharedString::from(format!(
                    "hanbo-armory-{}",
                    self.device.device_container_id
                ))),
                "runtime",
            ),
            cx,
            |_, _| HanboState {
                celsius: true,
                runtime: HanboRuntime::default(),
            },
        );
        let data = state.read(cx);
        let celsius = data.celsius;
        let runtime = &data.runtime;
        let cpu = group([
            detail(
                "CPU_TEMP",
                temperature(runtime.cpu_temp, celsius),
                118.,
                false,
            ),
            detail(
                "CPU_SPEED",
                format!("{} GHZ", runtime.cpu_speed),
                103.,
                true,
            ),
        ])
        .into_any_element();
        let cooling = group([
            detail(
                "FAN_SPEED",
                format!("{} RPM", runtime.fan_speed),
                141.,
                false,
            ),
            detail(
                "PUMP_SPEED",
                format!("{} RPM", runtime.pump_speed),
                141.,
                false,
            ),
            detail(
                "LIQUID_TEMP",
                temperature(runtime.liquid_temp, celsius),
                118.,
                true,
            ),
        ])
        .into_any_element();
        let gpu = group([
            detail(
                "GPU_TEMP",
                temperature(runtime.gpu_temp, celsius),
                118.,
                false,
            ),
            detail(
                "GPU_SPEED",
                format!("{} GHZ", runtime.gpu_speed),
                103.,
                false,
            ),
        ])
        .child(
            v_flex()
                .items_stretch()
                .font_family("RazerF5")
                .text_size(css(14.))
                .children([(true, "°C"), (false, "°F")].map(|(value, label)| {
                    BaseButton::new(label)
                        .p_0()
                        .px(css(4.))
                        .h(relative(0.5))
                        .selected(value == celsius)
                        .when(value == celsius, |b| {
                            b.bg(rgb(0x707070)).text_color(rgb(0x111111))
                        })
                        .child(label)
                        .on_click(window.listener_for(&state, move |state, _, _, cx| {
                            state.celsius = value;
                            cx.notify();
                        }))
                })),
        )
        .into_any_element();
        let compact = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        root.child(
            h_flex()
                .w_full()
                .my(css(20.))
                .mx_auto()
                .px(css(10.))
                .items_start()
                .justify_between()
                .when(compact, |v| {
                    v.flex_wrap()
                        .gap_x(css(20.))
                        .gap_y(css(10.))
                        .justify_center()
                        .max_w(css(1000.))
                })
                .children(if compact {
                    vec![cpu, gpu, cooling]
                } else {
                    vec![cpu, cooling, gpu]
                }),
        )
    }
}
