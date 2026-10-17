//! 3893 TAB_PERFORMANCE: current `xg` mounts CoolerInfo `fg` above
//! `.body-widgets.flex` with `.widget-col col-left` FAN (`Yg`) and
//! `.widget-col col-right` PUMP (`Kg`). Each widget is the shared `ce`
//! wrapper (`title`, `tips`, `extraClass: isHardwareOutdated ? "disabled"`),
//! a MODE list (`.sliderChart__preset-list`) and, for ADVANCED only, the
//! two-canvas `yg` curve editor. Selections update the typed profile owned by
//! `razer_device::hanbo_cooler`; profile capture is not a device write.
use super::*;
use razer_device::hanbo_cooler::{
    CURVE_SPEEDS, CoolerChannel, FAN_PORT_ID, FAN_PORT_NAME, PUMP_PORT_ID, PUMP_PORT_NAME,
    PortThermal, ThermalMode, ThermalProfile,
};
use std::{cell::Cell, rc::Rc};

/// `yg` canvas constants: Hg width, vg height, Bg x origin, Fg y origin.
const CHART_WIDTH: f32 = 520.;
const CHART_HEIGHT: f32 = 410.;
const ORIGIN_X: f32 = 60.;
const ORIGIN_Y: f32 = 365.;
const POINT_RADIUS: f32 = 8.;

#[derive(Clone, Copy)]
struct ChartGeometry {
    x_step: f32,
    y_step: f32,
    right: f32,
    top: f32,
    unit: f32,
}

impl ChartGeometry {
    /// `R=floor(452/(len-1))`, `d=floor(357/(len-1))`, `N=R*(len-1)+60`,
    /// `c=d*(len-1)+45`, `u=(c-45)/(max-min)`, `L=vg-c`.
    fn new(channel: CoolerChannel) -> Self {
        let columns = channel.temperatures().len() as f32 - 1.;
        let rows = CURVE_SPEEDS.len() as f32 - 1.;
        let x_step = (452. / columns).floor();
        let y_step = (357. / rows).floor();
        let height = y_step * rows + 45.;
        Self {
            x_step,
            y_step,
            right: x_step * columns + ORIGIN_X,
            top: CHART_HEIGHT - height,
            unit: (height - 45.) / 100.,
        }
    }
    fn x(&self, index: usize) -> f32 {
        ORIGIN_X + self.x_step * index as f32
    }
    fn y(&self, value: f64) -> f32 {
        ORIGIN_Y - value as f32 * self.unit
    }
    /// `+((Fg-y)/u).toFixed(1)` on the cleanup dispatch.
    fn value(&self, y: f32) -> f64 {
        (((ORIGIN_Y - y) / self.unit) as f64 * 10.).round() / 10.
    }
}

#[derive(Clone, Copy)]
pub(super) struct CurveDrag {
    channel: CoolerChannel,
    index: usize,
    start_y: f32,
    start_mouse: f32,
}

#[derive(Default)]
pub(super) struct HanboCooler {
    pub(super) profile: ThermalProfile,
    pub(super) drag: Option<CurveDrag>,
    /// Source `BC.runtimeData` initial values, including
    /// `isHardwareOutdated:false`. Only an MW `B8N` payload may replace them.
    pub(super) runtime: super::super::armory_product::HanboRuntime,
    pub(super) runtime_outdated: bool,
}

#[derive(Deserialize)]
struct SavedPort {
    name: String,
    thermal: PortThermal,
}

impl HanboCooler {
    /// Profile `ports` keep the source port identity and names.
    pub(super) fn snapshot_ports(&self) -> Value {
        json!([
            {"id": PUMP_PORT_ID, "name": PUMP_PORT_NAME, "thermal": self.profile.pump},
            {"id": FAN_PORT_ID, "name": FAN_PORT_NAME, "thermal": self.profile.fans},
        ])
    }

    pub(super) fn restore_ports(&mut self, saved: Option<&Value>) {
        self.profile = ThermalProfile::default();
        self.drag = None;
        let Some(ports) = saved
            .and_then(|saved| saved.get("ports"))
            .and_then(Value::as_array)
        else {
            return;
        };
        for port in ports {
            // Lighting-only ports have no thermal object and are not ours.
            let Ok(port) = serde_json::from_value::<SavedPort>(port.clone()) else {
                continue;
            };
            // Task runner `ko`: "Fans" is the fan controller, any other
            // thermal port is the pump controller.
            let channel = if port.name == FAN_PORT_NAME {
                CoolerChannel::Fans
            } else {
                CoolerChannel::Pump
            };
            self.profile.restore_port(channel, port.thermal);
        }
    }
}

impl AccessorySystemProductWorkspace {
    fn request_hanbo_command(&mut self, channel: CoolerChannel, cx: &mut Context<Self>) {
        self.hanbo_generation = self.hanbo_generation.wrapping_add(1);
        let generation = self.hanbo_generation;
        let command = match razer_device::hanbo_cooler::CoolerCommand::for_port(
            channel,
            self.hanbo.profile.port(channel),
        ) {
            Ok(command) => command,
            Err(error) => {
                self.hanbo_command_status = Some((generation, Err(error.to_string())));
                cx.notify();
                return;
            }
        };
        self.hanbo_command_status = Some((generation, Err("pending".into())));
        cx.emit(super::HanboCoolerRequested { generation, command });
        cx.notify();
    }

    pub(crate) fn finish_hanbo_command(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id == razer_device::hanbo_cooler::PRODUCT_ID
            && generation == self.hanbo_generation
        {
            self.hanbo_command_status = Some((generation, result));
            cx.notify();
        }
    }

    pub(crate) fn hanbo_command_matches(&self, generation: u64) -> bool {
        self.spec.product_id == razer_device::hanbo_cooler::PRODUCT_ID
            && generation == self.hanbo_generation
    }

    fn hanbo_set_mode(
        &mut self,
        channel: CoolerChannel,
        mode: ThermalMode,
        cx: &mut Context<Self>,
    ) {
        let port = self.hanbo.profile.port_mut(channel);
        // FAN `Yg` skips an unchanged mode; PUMP `Kg` dispatches every click
        // and its reducer then reports no port change.
        if port.active_thermal_indicator.mode_selected == mode {
            return;
        }
        port.active_thermal_indicator.mode_selected = mode;
        self.hanbo.drag = None;
        cx.emit(AccessorySystemProductChanged);
        self.request_hanbo_command(channel, cx);
        cx.notify();
    }

    fn hanbo_press(
        &mut self,
        channel: CoolerChannel,
        local: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let geometry = ChartGeometry::new(channel);
        let (x, y) = (f32::from(local.x), f32::from(local.y));
        let values = &self.hanbo.profile.port(channel).advanced_setting;
        let hit = values.iter().enumerate().position(|(index, value)| {
            (x - geometry.x(index)).powi(2) + (y - geometry.y(*value)).powi(2)
                <= POINT_RADIUS.powi(2)
        });
        if let Some(index) = hit {
            self.hanbo.drag = Some(CurveDrag {
                channel,
                index,
                start_y: geometry.y(values[index]),
                start_mouse: y,
            });
            cx.notify();
        }
    }

    fn hanbo_move(&mut self, channel: CoolerChannel, local: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.hanbo.drag.filter(|drag| drag.channel == channel) else {
            return;
        };
        let geometry = ChartGeometry::new(channel);
        let values = &self.hanbo.profile.port(channel).advanced_setting;
        // `maxValue:"nextValue"` / `minValue:"previousValue"` keep the curve
        // monotonic; the outer bounds are the plot top `L` and origin `Fg`.
        let mut upper = geometry.top;
        if drag.index + 1 < values.len() {
            upper = upper.max(geometry.y(values[drag.index + 1]));
        }
        let mut lower = ORIGIN_Y;
        if drag.index > 0 {
            lower = lower.min(geometry.y(values[drag.index - 1]));
        }
        let y = (drag.start_y + f32::from(local.y) - drag.start_mouse).clamp(upper, lower);
        let value = geometry.value(y);
        let port = self.hanbo.profile.port_mut(channel);
        if port.advanced_setting[drag.index] != value {
            port.advanced_setting[drag.index] = value;
            cx.emit(AccessorySystemProductChanged);
            cx.notify();
        }
    }

    fn hanbo_release(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.hanbo.drag.take() {
            self.request_hanbo_command(drag.channel, cx);
            cx.notify();
        }
    }

    fn hanbo_chart(&self, channel: CoolerChannel, cx: &Context<Self>) -> AnyElement {
        let geometry = ChartGeometry::new(channel);
        let values = self.hanbo.profile.port(channel).advanced_setting.clone();
        let selected = self
            .hanbo
            .drag
            .filter(|drag| drag.channel == channel)
            .map(|drag| drag.index);
        let temperatures = channel.temperatures();
        let (x_label, y_label) = match channel {
            CoolerChannel::Fans => ("CPU_TEMPERATURE", "FAN_SPEED"),
            CoolerChannel::Pump => ("LIQUID_TEMPERATURE", "PUMP_SPEED"),
        };
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let paint_bounds = bounds.clone();
        // Pointer events are window pixels; geometry is in source CSS units.
        let local = move |position: Point<Pixels>| -> Point<Pixels> {
            let bounds = bounds.get();
            let scale = (f32::from(bounds.size.width) / CHART_WIDTH).max(f32::EPSILON);
            point(
                px(f32::from(position.x - bounds.origin.x) / scale),
                px(f32::from(position.y - bounds.origin.y) / scale),
            )
        };
        let press_local = local.clone();
        let move_local = local;
        let mut chart = div()
            .id(SharedString::from(format!("hanbo-curve-{channel:?}")))
            .relative()
            .w(surface::css(CHART_WIDTH))
            .h(surface::css(CHART_HEIGHT))
            .child(
                canvas(
                    move |bounds, _, _| paint_bounds.set(bounds),
                    move |bounds, _, window, _| {
                        let scale = f32::from(bounds.size.width) / CHART_WIDTH;
                        let pos = |x: f32, y: f32| {
                            point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                        };
                        // Background canvas: `strokeStyle:#5d5d5d` grid.
                        let mut grid = PathBuilder::stroke(px(scale));
                        for index in 0..temperatures.len() {
                            grid.move_to(pos(geometry.x(index), ORIGIN_Y));
                            grid.line_to(pos(geometry.x(index), geometry.top));
                        }
                        for row in 0..CURVE_SPEEDS.len() {
                            let y = ORIGIN_Y - geometry.y_step * row as f32;
                            grid.move_to(pos(ORIGIN_X, y));
                            grid.line_to(pos(geometry.right, y));
                        }
                        if let Ok(path) = grid.build() {
                            window.paint_path(path, rgb(0x5d5d5d));
                        }
                        // Foreground: lineWidth 2, `#44d62c`.
                        let mut line = PathBuilder::stroke(px(2. * scale));
                        for (index, value) in values.iter().enumerate() {
                            let p = pos(geometry.x(index), geometry.y(*value));
                            if index == 0 {
                                line.move_to(p);
                            } else {
                                line.line_to(p);
                            }
                        }
                        if let Ok(path) = line.build() {
                            window.paint_path(path, rgb(0x44d62c));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.hanbo_press(channel, press_local(event.position), cx)
                }),
            )
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                this.hanbo_move(channel, move_local(event.position), cx)
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.hanbo_release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.hanbo_release(cx)),
            );
        let current = self.hanbo.profile.port(channel).advanced_setting.clone();
        for (index, value) in current.iter().enumerate() {
            // `arc(x,y,8)` filled `#44d62c`, the held point `#30961f`.
            chart = chart.child(
                div()
                    .absolute()
                    .left(surface::css(geometry.x(index) - POINT_RADIUS))
                    .top(surface::css(geometry.y(*value) - POINT_RADIUS))
                    .size(surface::css(2. * POINT_RADIUS))
                    .rounded_full()
                    .bg(if selected == Some(index) {
                        rgb(0x30961f)
                    } else {
                        rgb(0x44d62c)
                    }),
            );
        }
        // `10px Roboto` `#cccccc`: x ticks at y=375 (first left, last right
        // aligned), y ticks right-aligned at x=50.
        for (index, temperature) in temperatures.iter().enumerate() {
            let x = geometry.x(index);
            let (left, width, align) = if index == 0 {
                (x, 40., "left")
            } else if index + 1 == temperatures.len() {
                (x - 40., 40., "right")
            } else {
                (x - 20., 40., "center")
            };
            let label = div()
                .absolute()
                .left(surface::css(left))
                .top(surface::css(375.))
                .w(surface::css(width))
                .text_size(surface::css(10.))
                .text_color(rgb(0xcccccc))
                .child(format!("{temperature} °C"));
            chart = chart.child(match align {
                "left" => label.text_left(),
                "right" => label.text_right(),
                _ => label.text_center(),
            });
        }
        for (row, speed) in CURVE_SPEEDS.iter().enumerate() {
            let y = ORIGIN_Y - geometry.y_step * row as f32;
            // textBaseline middle; first row bottom, last row top.
            let top = if row == 0 {
                y - 12.
            } else if row + 1 == CURVE_SPEEDS.len() {
                y
            } else {
                y - 6.
            };
            chart = chart.child(
                div()
                    .absolute()
                    .left(surface::css(0.))
                    .top(surface::css(top))
                    .w(surface::css(50.))
                    .text_right()
                    .text_size(surface::css(10.))
                    .text_color(rgb(0xcccccc))
                    .child(format!("{speed} %")),
            );
        }
        // `14px Roboto` axis titles: x centred at the bottom (baseline vg);
        // the source rotates the y title by -90deg at the left edge.
        chart
            .child(
                div()
                    .absolute()
                    .left(surface::css(ORIGIN_X))
                    .w(surface::css(geometry.right - ORIGIN_X))
                    .top(surface::css(CHART_HEIGHT - 17.))
                    .text_center()
                    .text_size(surface::css(14.))
                    .text_color(rgb(0xcccccc))
                    .child(t(x_label)),
            )
            .child(
                div()
                    .absolute()
                    .left(surface::css(0.))
                    .top(surface::css(geometry.top))
                    .w(surface::css(14.))
                    .h(surface::css(ORIGIN_Y - geometry.top))
                    .flex()
                    .items_center()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .text_color(rgb(0xcccccc))
                    .child(t(y_label)),
            )
            .into_any_element()
    }

    fn hanbo_widget(&self, channel: CoolerChannel, cx: &Context<Self>) -> AnyElement {
        let (title, tips) = match channel {
            CoolerChannel::Fans => ("FAN", "FAN_TOOLTIP"),
            CoolerChannel::Pump => ("PUMP", "PUMP_TOOLTIP"),
        };
        let disabled = self.hanbo.runtime_outdated;
        let mode = self
            .hanbo
            .profile
            .port(channel)
            .active_thermal_indicator
            .mode_selected;
        let presets = h_flex()
            .flex_wrap()
            .gap(surface::css(12.))
            .mb(surface::css(20.))
            .children(ThermalMode::ALL.map(|option| {
                let active = option == mode;
                BaseButton::new(SharedString::from(format!(
                    "hanbo-{channel:?}-{}",
                    option.label()
                )))
                .accessibility_label(t(option.label()))
                .selected(active)
                .disabled(disabled)
                .h(surface::css(27.))
                .min_w(surface::css(90.))
                .pt(surface::css(6.))
                .pb(surface::css(7.))
                .px(surface::css(16.))
                .rounded(surface::css(3.))
                .border_1()
                .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
                .bg(if active { rgba(0xffffff1a) } else { rgb(0x111111) })
                .text_color(rgb(0xcccccc))
                .text_size(surface::css(12.))
                .line_height(surface::css(14.))
                .text_center()
                .when(!disabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(|b| b.bg(rgba(0x0000001a)).border_color(rgb(0x44d62c)))
                        .active(|b| b.bg(rgba(0xffffff1a)).border_color(rgb(0x44d62c)))
                })
                .child(t(option.label()).to_uppercase())
                .on_click(
                    cx.listener(move |this, _, _, cx| this.hanbo_set_mode(channel, option, cx)),
                )
            }));
        surface::panel_with_help(
            t(title),
            self.help_control(format!("hanbo-{channel:?}-help"), t(tips), cx),
            cx,
        )
        .when(disabled, |panel| panel.opacity(0.3))
        .child(div().mb(surface::css(8.)).child(t("MODE").to_uppercase()))
        .child(presets)
        .when(mode == ThermalMode::Advanced, |panel| {
            panel.child(self.hanbo_chart(channel, cx))
        })
        .into_any_element()
    }

    pub(super) fn hanbo(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let status = self.hanbo_command_status.as_ref().map(|(_, result)| match result {
            Ok(()) => "Hanbo device command acknowledged".to_owned(),
            Err(error) if error == "pending" => "Sending Hanbo device command…".to_owned(),
            Err(error) => format!("Hanbo device command failed: {error}"),
        });
        v_flex()
            .w_full()
            .child(super::super::armory_product::hanbo_cooler_info(
                self.hanbo_artwork,
                self.celsius,
                &self.hanbo.runtime,
                Rc::new(move |celsius, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.celsius = celsius;
                        cx.notify();
                    });
                }),
                window,
                cx,
            ))
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        self.hanbo_widget(CoolerChannel::Fans, cx),
                    ))
                    .child(surface::page_column(
                        self.hanbo_widget(CoolerChannel::Pump, cx),
                    )),
            )
            .when_some(status, |body, status| body.child(surface::note(status, cx)))
            .into_any_element()
    }
}
