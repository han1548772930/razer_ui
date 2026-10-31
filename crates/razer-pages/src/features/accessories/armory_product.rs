//! Independent Armory roots from each current product bundle.
//!
//! The audited audio roots mount only ProductImage. 3893/HH mounts
//! CoolerInfo (`fg`), including its three metric groups, not fan controls.
//! See docs/re/armory-product-roots-current-evidence.json and
//! docs/re/armory-audio-roots-current-evidence.json and
//! docs/re/armory-audio-next-roots-current-evidence.json.
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_assets as resources;
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::surface;
use razer_widgets::surface::css;
use razer_widgets::theme::{HanboCoolerColors as Colors, ProductSurfaceColors};

#[path = "armory_product/cooling_pad.rs"]
mod cooling_pad;

pub fn supports(device: &Device) -> bool {
    // 3894 is Head Cushion Chroma. Its current 8193.DeviceInfo.category is
    // statically ACCESSORY; the root does not inspect our discovery category.
    // The audio roots are individually audited. 1335's own DeviceInfo sets
    // specialAudio=true, so its ProductImage wrapper has a separate height.
    matches!(
        device.product_id,
        1303 | 1304 | 1313 | 1319 | 1325 | 1328 | 1330 | 1331 | 1332 | 1335 | 3893 | 3894 | 3907
    )
}

/// `BC` initializes each metric to zero. An absent observation retains that
/// presentation without recording the initialized value as a device sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct HanboRuntime {
    system_observed: bool,
    cpu_temp: Option<f64>,
    cpu_speed: Option<f64>,
    fan_speed: Option<u16>,
    pump_speed: Option<u16>,
    liquid_temp: Option<u16>,
    gpu_temp: Option<f64>,
    gpu_speed: Option<f64>,
    hardware_outdated: bool,
}

impl HanboRuntime {
    pub(crate) fn observe_system(&mut self, sample: &razer_device::system_monitor::SystemSample) {
        self.system_observed = true;
        self.cpu_temp = sample.cpu_temperature;
        self.cpu_speed = sample.cpu_clock_mhz.map(|value| value / 1000.);
        self.gpu_temp = sample.gpu_temperature;
        self.gpu_speed = sample.gpu_clock_mhz.map(|value| value / 1000.);
    }

    /// MW 34244 publishes only these three fields. CPU/GPU monitor values and
    /// the hardware warning are independent of this getter pair.
    pub(crate) fn observe_telemetry(&mut self, sample: &razer_device::hanbo_cooler::Telemetry) {
        self.fan_speed = Some(sample.fan.rpm);
        self.pump_speed = Some(sample.pump.rpm);
        self.liquid_temp = Some(sample.pump.temperature_tenths_celsius);
    }

    pub(crate) fn hardware_outdated(&self) -> bool {
        self.hardware_outdated
    }
}

#[derive(IntoElement)]
pub struct ArmoryProduct {
    device: Device,
}
impl ArmoryProduct {
    pub fn new(device: &Device) -> Self {
        Self {
            device: device.clone(),
        }
    }

    fn product_image(&self, cx: &App) -> AnyElement {
        let image_height = if self.device.product_id == 3893 { 200. } else { 250. };
        // 1335 xf/Kf changes `.widget-prod.dot-bg` to height:390px, but
        // fc still passes `this.props.height || 250` to its image. The image
        // stays centered at 250px; stretching it to the wrapper is incorrect.
        let height = if self.device.product_id == 1335 {
            390.
        } else {
            image_height
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
        product_image(artwork, height, image_height, cx)
    }
}

fn product_image(
    artwork: Option<&'static str>,
    height: f32,
    image_height: f32,
    cx: &App,
) -> AnyElement {
    div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
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
                    .w_full()
                    .h(css(image_height))
                    .flex_shrink_0()
                    .object_fit(ObjectFit::Contain),
            )
        })
        .into_any_element()
}

/// Shared `fg` CoolerInfo: `Cg` ProductImage (200px), the three metric groups
/// and `°C/°F` buttons. `on_unit` is the owner's `setCelsiusMode`.
pub(crate) type UnitHandler = std::rc::Rc<dyn Fn(bool, &mut Window, &mut App)>;

pub(crate) fn hanbo_cooler_info(
    artwork: Option<&'static str>,
    celsius: bool,
    runtime: &HanboRuntime,
    on_unit: UnitHandler,
    window: &Window,
    cx: &App,
) -> AnyElement {
    v_flex()
        .w_full()
        .min_w_0()
        .relative()
        .child(product_image(artwork, 200., 200., cx))
        .child(hanbo_metrics(celsius, runtime, on_unit, window))
        .when(runtime.hardware_outdated(), |view| {
            view.child(resources::deferred_resource(hanbo_firmware_warning()).with_priority(1))
        })
        .into_any_element()
}

fn hanbo_firmware_warning() -> AnyElement {
    div()
        .absolute()
        .top_full()
        .left_0()
        .right_0()
        .mt(css(10.))
        .child(
            v_flex()
                .w(css(315.))
                .mx_auto()
                .py(css(20.))
                .px(css(30.))
                .bg(Colors::surface())
                .border_1()
                .border_color(Colors::firmware_warning())
                .rounded(css(3.))
                .text_size(css(14.))
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .mb(css(10.))
                        .text_color(Colors::firmware_warning())
                        .font_weight(FontWeight::BOLD)
                        .child(
                            img("synapse/hanbo-warning.svg")
                                .size(css(20.))
                                .flex_shrink_0(),
                        )
                        .child(i18n::t("FIRMWARE_UPDATE_REQUIRED")),
                )
                .child(
                    div()
                        .text_center()
                        .child(i18n::t("FIRMWARE_UPDATE_REQUIRED_DESC")),
                ),
        )
        .into_any_element()
}

fn hanbo_metrics(
    celsius: bool,
    runtime: &HanboRuntime,
    on_unit: UnitHandler,
    window: &Window,
) -> AnyElement {
    let cpu = group([
        detail(
            "CPU_TEMP",
            system_temperature(runtime.cpu_temp, celsius, runtime.system_observed),
            118.,
            false,
        ),
        detail(
            "CPU_SPEED",
            system_clock(runtime.cpu_speed, runtime.system_observed),
            103.,
            true,
        ),
    ])
    .into_any_element();
    let cooling = group([
        detail(
            "FAN_SPEED",
            format!("{} RPM", initialized_integer(runtime.fan_speed)),
            141.,
            false,
        ),
        detail(
            "PUMP_SPEED",
            format!("{} RPM", initialized_integer(runtime.pump_speed)),
            141.,
            false,
        ),
        detail(
            "LIQUID_TEMP",
            liquid_temperature(runtime.liquid_temp, celsius),
            118.,
            true,
        ),
    ])
    .into_any_element();
    let gpu = group([
        detail(
            "GPU_TEMP",
            system_temperature(runtime.gpu_temp, celsius, runtime.system_observed),
            118.,
            false,
        ),
        detail(
            "GPU_SPEED",
            system_clock(runtime.gpu_speed, runtime.system_observed),
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
                let on_unit = on_unit.clone();
                BaseButton::new(label)
                    .p_0()
                    .px(css(4.))
                    .h(relative(0.5))
                    .selected(value == celsius)
                    .when(value == celsius, |b| {
                        b.bg(Colors::metric_border()).text_color(Colors::surface())
                    })
                    .child(label)
                    .on_click(move |_, window, cx| on_unit(value, window, cx))
            })),
    )
    .into_any_element();
    let compact = surface::stacked_device_columns(
        f32::from(window.viewport_size().width),
        f32::from(window.rem_size()),
    );
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
        })
        .into_any_element()
}

fn detail(label: &'static str, value: String, width: f32, last: bool) -> AnyElement {
    v_flex()
        .items_center()
        .p(css(20.))
        .text_size(css(14.))
        .when(!last, |v| {
            v.border_r_1().border_color(Colors::metric_border())
        })
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
        .bg(Colors::surface())
        .border_1()
        .border_color(Colors::metric_border())
        .rounded(css(5.))
        .children(children)
}
fn initialized_number(value: Option<f64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "0".into(),
    }
}

fn system_clock(value: Option<f64>, observed: bool) -> String {
    if observed {
        // MW 48290 formats a null clock as 0.0 without claiming a zero sample.
        format!("{:.1} GHZ", value.unwrap_or(0.))
    } else {
        "0 GHZ".into()
    }
}

fn system_temperature(value: Option<f64>, celsius: bool, observed: bool) -> String {
    if !observed {
        return temperature(None, celsius);
    }
    match (value, celsius) {
        (Some(value), true) => format!("{value:.1} °C"),
        (Some(value), false) => {
            let decimal = format!("{value:.1}")
                .parse::<f64>()
                .expect("finite temperature decimal");
            format!("{:.1} °F", decimal * 1.8 + 32.)
        }
        (None, true) => " °C".into(),
        (None, false) => "NaN °F".into(),
    }
}

fn initialized_integer(value: Option<u16>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "0".into(),
    }
}

fn temperature(value: Option<f64>, celsius: bool) -> String {
    if celsius {
        format!("{} °C", initialized_number(value))
    } else {
        let value = value.unwrap_or(0.);
        format!("{:.1} °F", value * 1.8 + 32.)
    }
}

/// MW `temperature.toFixed(1)` produces a decimal string only after a sample.
fn liquid_temperature(tenths: Option<u16>, celsius: bool) -> String {
    match tenths {
        Some(tenths) if celsius => format!("{:.1} °C", f64::from(tenths) / 10.),
        Some(tenths) => temperature(Some(f64::from(tenths) / 10.), false),
        None => temperature(None, celsius),
    }
}
impl RenderOnce for ArmoryProduct {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.device.product_id == 3907 {
            return cooling_pad::render(&self.device, window, cx);
        }
        let root = v_flex()
            .w_full()
            .min_w_0()
            .bg(ProductSurfaceColors::background())
            .font_family("Roboto")
            .text_size(css(14.))
            .text_color(ProductSurfaceColors::text())
            .child(self.product_image(cx));
        root.into_any_element()
    }
}

#[cfg(test)]
#[path = "armory_product/hanbo_tests.rs"]
mod hanbo_tests;
