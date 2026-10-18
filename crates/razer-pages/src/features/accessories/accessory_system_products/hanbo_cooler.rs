//! 3893 TAB_PERFORMANCE: current `xg` mounts CoolerInfo `fg` above
//! `.body-widgets.flex` with `.widget-col col-left` FAN (`Yg`) and
//! `.widget-col col-right` PUMP (`Kg`). Each widget is the shared `ce`
//! wrapper (`title`, `tips`, `extraClass: isHardwareOutdated ? "disabled"`),
//! a MODE list (`.sliderChart__preset-list`) and, for ADVANCED only, the
//! two-canvas `yg` curve editor. Selections update the typed profile owned by
//! `razer_device::hanbo_cooler`; profile capture is not a device write.
use super::*;
use razer_assets::rotated_label::{LabelGeometry, LabelRequest, LabelState};
use razer_device::hanbo_cooler::{
    CURVE_SPEEDS, CoolerChannel, FAN_PORT_NAME, PortThermal, ThermalMode, ThermalProfile,
};
use razer_device::hanbo_settings::LocalChange;
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::theme::{HanboCoolerColors as Colors, ProductSurfaceColors};
use std::{cell::Cell, rc::Rc};

/// `yg` canvas constants: Hg width, vg height, Bg x origin, Fg y origin.
const CHART_WIDTH: f32 = 520.;
const CHART_HEIGHT: f32 = 410.;
const ORIGIN_X: f32 = 60.;
const ORIGIN_Y: f32 = 365.;
const POINT_RADIUS: f32 = 8.;

/// Resource presentation changed without changing any saved device settings.
pub(crate) struct HanboResourceChanged;
impl EventEmitter<HanboResourceChanged> for AccessorySystemProductWorkspace {}

#[derive(Clone, Copy)]
struct ChartGeometry {
    x_step: f32,
    y_step: f32,
    right: f32,
    top: f32,
    unit: f32,
}

#[cfg(test)]
#[path = "hanbo_cooler_tests.rs"]
mod tests;

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
    /// Canvas points retain their pixel precision until the cleanup dispatch.
    fn value(&self, y: f32) -> f64 {
        f64::from((ORIGIN_Y - y) / self.unit)
    }
    fn title_anchor(&self) -> f32 {
        ORIGIN_Y - (CHART_HEIGHT - self.top) / 2.
    }
}

#[derive(Clone, Copy)]
pub(super) struct CurveDrag {
    channel: CoolerChannel,
    index: usize,
    start_y: f32,
    start_mouse: f32,
}

struct CurveEditor {
    values: Vec<f64>,
    locale: String,
    axis_label: Option<Entity<LabelState>>,
    axis_observer: Option<Subscription>,
}

struct ThermalCommit {
    channel: CoolerChannel,
    thermal: PortThermal,
}

/// `.sliderChart__preset-list`: selected/pressed rules occur after hover in
/// the source stylesheet, so their background wins while the border stays green.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModeBackground {
    Normal,
    Hover,
    Selected,
}

fn mode_presentation(
    selected: bool,
    disabled: bool,
    hovered: bool,
    pressed: bool,
) -> (ModeBackground, bool) {
    if selected || (!disabled && hovered && pressed) {
        (ModeBackground::Selected, true)
    } else if !disabled && hovered {
        (ModeBackground::Hover, true)
    } else {
        (ModeBackground::Normal, false)
    }
}

#[derive(IntoElement)]
struct ModeButton {
    owner: WeakEntity<AccessorySystemProductWorkspace>,
    channel: CoolerChannel,
    option: ThermalMode,
    selected: bool,
    disabled: bool,
}

impl RenderOnce for ModeButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = SharedString::from(format!("hanbo-{:?}-{}", self.channel, self.option.label()));
        let pointer = surface::pointer_state(id.clone(), window, cx);
        let (hovered, pressed) = pointer.read(cx).sample();
        let (background, green_border) =
            mode_presentation(self.selected, self.disabled, hovered, pressed);
        let background = match background {
            ModeBackground::Normal => Colors::surface(),
            ModeBackground::Hover => Colors::mode_hover(),
            ModeBackground::Selected => Colors::selected_mode(),
        };
        let background = surface::fade_color(
            SharedString::from(format!("{id}-background")),
            background,
            300,
            window,
            cx,
        );
        let border = surface::fade_color(
            SharedString::from(format!("{id}-border")),
            if green_border {
                Colors::curve()
            } else {
                Colors::curve_grid()
            },
            300,
            window,
            cx,
        );
        let button = BaseButton::new(id)
            .accessibility_label(t(self.option.label()))
            .selected(self.selected)
            .disabled(self.disabled)
            .h(surface::css(27.))
            .min_w(surface::css(90.))
            .pt(surface::css(6.))
            .pb(surface::css(7.))
            .px(surface::css(16.))
            .rounded(surface::css(3.))
            .border_1()
            .border_color(border)
            .bg(background)
            .text_color(Colors::text())
            .text_size(surface::css(12.))
            .line_height(surface::css(14.))
            .text_center()
            .whitespace_nowrap()
            .when(!self.disabled, |button| button.cursor_pointer())
            .child(t(self.option.label()).to_uppercase())
            .on_click(move |_, _, cx| {
                let _ = self.owner.update(cx, |this, cx| {
                    this.hanbo_set_mode(self.channel, self.option, cx)
                });
            });
        surface::track_pointer(button, &pointer, window)
    }
}

#[derive(Default)]
pub(super) struct HanboCooler {
    pub(super) profile: ThermalProfile,
    pub(super) drag: Option<CurveDrag>,
    fan_curve: Option<CurveEditor>,
    pump_curve: Option<CurveEditor>,
    /// Shared performance/Armory observations, separate from profile choices.
    pub(super) runtime: super::super::armory_product::HanboRuntime,
}

#[derive(Deserialize)]
struct SavedPort {
    name: String,
    thermal: PortThermal,
}

impl HanboCooler {
    fn editor(&self, channel: CoolerChannel) -> Option<&CurveEditor> {
        match channel {
            CoolerChannel::Fans => self.fan_curve.as_ref(),
            CoolerChannel::Pump => self.pump_curve.as_ref(),
        }
    }

    fn editor_slot(&mut self, channel: CoolerChannel) -> &mut Option<CurveEditor> {
        match channel {
            CoolerChannel::Fans => &mut self.fan_curve,
            CoolerChannel::Pump => &mut self.pump_curve,
        }
    }

    pub(super) fn mount_curves(&mut self) {
        for channel in [CoolerChannel::Fans, CoolerChannel::Pump] {
            if self
                .profile
                .port(channel)
                .active_thermal_indicator
                .mode_selected
                == ThermalMode::Advanced
                && self.editor(channel).is_none()
            {
                let values = self.profile.port(channel).advanced_setting.clone();
                *self.editor_slot(channel) = Some(CurveEditor {
                    values,
                    locale: i18n::locale(),
                    axis_label: None,
                    axis_observer: None,
                });
            }
        }
    }

    fn curve_values(&self, channel: CoolerChannel) -> &[f64] {
        self.editor(channel)
            .map(|editor| editor.values.as_slice())
            .unwrap_or(&self.profile.port(channel).advanced_setting)
    }

    fn capture(&self, channel: CoolerChannel) -> ThermalCommit {
        ThermalCommit {
            channel,
            thermal: self.profile.port(channel).clone(),
        }
    }

    fn commit_curve(&mut self, channel: CoolerChannel, values: &[f64]) -> ThermalCommit {
        // `+((Fg-y)/u).toFixed(1)` is applied when `yg`'s effect cleans up.
        let values = values
            .iter()
            .map(|value| (value * 10.).round() / 10.)
            .collect();
        let port = self.profile.port_mut(channel);
        port.advanced_setting = values;
        self.capture(channel)
    }

    fn cleanup_curve(&mut self, channel: CoolerChannel) -> Option<ThermalCommit> {
        let values = self.editor(channel)?.values.clone();
        Some(self.commit_curve(channel, &values))
    }

    fn unmount_curve(&mut self, channel: CoolerChannel) -> Option<ThermalCommit> {
        let editor = self.editor_slot(channel).take()?;
        if self.drag.is_some_and(|drag| drag.channel == channel) {
            self.drag = None;
        }
        Some(self.commit_curve(channel, &editor.values))
    }

    fn set_mode(&mut self, channel: CoolerChannel, mode: ThermalMode) -> Vec<ThermalCommit> {
        let port = self.profile.port_mut(channel);
        let previous = port.active_thermal_indicator.mode_selected;
        if channel == CoolerChannel::Fans && previous == mode {
            return Vec::new();
        }
        port.active_thermal_indicator.mode_selected = mode;
        let mut commits = vec![self.capture(channel)];
        if previous == ThermalMode::Advanced && mode != ThermalMode::Advanced {
            // The mode dispatch precedes the chart's passive effect cleanup;
            // its final curve is submitted with the now-selected mode.
            if let Some(commit) = self.unmount_curve(channel) {
                commits.push(commit);
            }
        } else if mode == ThermalMode::Advanced {
            self.mount_curves();
        }
        commits
    }

    fn press_curve(&mut self, channel: CoolerChannel, x: f32, y: f32) -> Option<ThermalCommit> {
        if self.runtime.hardware_outdated() {
            return None;
        }
        let geometry = ChartGeometry::new(channel);
        let values = &self.editor(channel)?.values;
        let index = values.iter().enumerate().position(|(index, value)| {
            (x - geometry.x(index)).powi(2) + (y - geometry.y(*value)).powi(2)
                <= POINT_RADIUS.powi(2)
        })?;
        let start_y = geometry.y(values[index]);
        let cleanup = self.cleanup_curve(channel);
        self.drag = Some(CurveDrag {
            channel,
            index,
            start_y,
            start_mouse: y,
        });
        cleanup
    }

    fn move_curve(&mut self, channel: CoolerChannel, mouse_y: f32) -> Option<ThermalCommit> {
        if self.runtime.hardware_outdated() {
            return None;
        }
        let drag = self.drag.filter(|drag| drag.channel == channel)?;
        let geometry = ChartGeometry::new(channel);
        let values = &self.editor(channel)?.values;
        let mut upper = geometry.top;
        if drag.index + 1 < values.len() {
            upper = upper.max(geometry.y(values[drag.index + 1]));
        }
        let mut lower = ORIGIN_Y;
        if drag.index > 0 {
            lower = lower.min(geometry.y(values[drag.index - 1]));
        }
        let y = (drag.start_y + mouse_y - drag.start_mouse).clamp(upper, lower);
        // A changed `m` first cleans up the previous effect, then installs the
        // new points. Releasing/unmounting submits the last visible points.
        let cleanup = self.cleanup_curve(channel);
        self.editor_slot(channel).as_mut()?.values[drag.index] = geometry.value(y);
        cleanup
    }

    fn release_curve(&mut self) -> Option<ThermalCommit> {
        let drag = self.drag.take()?;
        self.cleanup_curve(drag.channel)
    }

    fn unmount_curves(&mut self) -> Vec<ThermalCommit> {
        [CoolerChannel::Fans, CoolerChannel::Pump]
            .into_iter()
            .filter_map(|channel| self.unmount_curve(channel))
            .collect()
    }

    fn refresh_curve_locale(&mut self, locale: &str) -> Vec<ThermalCommit> {
        let mut commits = Vec::new();
        for channel in [CoolerChannel::Fans, CoolerChannel::Pump] {
            if self
                .editor(channel)
                .is_some_and(|editor| editor.locale != locale)
            {
                // `yg`'s paint callback depends on language; changing it cleans
                // up the preceding effect without replacing its retained points.
                if let Some(commit) = self.cleanup_curve(channel) {
                    commits.push(commit);
                }
                if let Some(editor) = self.editor_slot(channel) {
                    editor.locale = locale.to_owned();
                }
            }
        }
        commits
    }

    /// Snapshot only the owned thermal fields. Lighting, port identity, order
    /// and extension fields remain in the local document boundary.
    pub(super) fn snapshot_ports(&self, existing: Option<&Value>) -> Value {
        let mut snapshot = existing
            .cloned()
            .unwrap_or_else(razer_device::hanbo_settings::default_ports);
        let Some(ports) = snapshot.as_array_mut() else {
            // Retain malformed external data for the checked persistence
            // boundary; capture cannot turn it into a successful empty list.
            return snapshot;
        };
        for port in ports {
            let Some(port) = port.as_object_mut() else {
                continue;
            };
            let Some(name) = port.get("name").and_then(Value::as_str) else {
                continue;
            };
            let channel = if name == FAN_PORT_NAME {
                CoolerChannel::Fans
            } else {
                CoolerChannel::Pump
            };
            let Some(thermal) = port.get_mut("thermal").and_then(Value::as_object_mut) else {
                continue;
            };
            let profile = self.profile.port(channel);
            thermal.insert("advancedSetting".into(), json!(profile.advanced_setting));
            if let Some(indicator) = thermal
                .get_mut("activeThermalIndicator")
                .and_then(Value::as_object_mut)
            {
                indicator.insert(
                    "modeSelected".into(),
                    json!(profile.active_thermal_indicator.mode_selected),
                );
            }
        }
        snapshot
    }

    pub(super) fn restore_ports(&mut self, saved: Option<&Value>) {
        self.profile = ThermalProfile::default();
        self.drag = None;
        self.fan_curve = None;
        self.pump_curve = None;
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
    pub(crate) fn refresh_hanbo_locale(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return;
        }
        for commit in self.hanbo.refresh_curve_locale(&i18n::locale()) {
            self.publish_hanbo_thermal(commit, cx);
        }
        self.sync_hanbo_axis_labels(cx);
        cx.notify();
    }

    /// Called after the actual performance page mounts or restores its
    /// advanced editors. A retained editor owns its label and background work.
    pub(crate) fn sync_hanbo_axis_labels(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return;
        }
        let locale = i18n::locale();
        for channel in [CoolerChannel::Fans, CoolerChannel::Pump] {
            let Some(editor) = self.hanbo.editor_slot(channel) else {
                continue;
            };
            let key = match channel {
                CoolerChannel::Fans => "FAN_SPEED",
                CoolerChannel::Pump => "PUMP_SPEED",
            };
            let request = LabelRequest::new(
                t(key),
                locale.clone(),
                LabelGeometry::new(
                    14.,
                    size(20., CHART_HEIGHT),
                    point(0., ChartGeometry::new(channel).title_anchor()),
                ),
            );
            if let Some(label) = &editor.axis_label {
                label.update(cx, |label, cx| label.set_request(request, cx));
            } else {
                let label = LabelState::new(request, cx);
                editor.axis_observer = Some(cx.observe(&label, |_, _, cx| {
                    cx.emit(HanboResourceChanged);
                }));
                editor.axis_label = Some(label);
            }
        }
    }

    pub(crate) fn hanbo_label_errors(&self, cx: &App) -> Vec<String> {
        [CoolerChannel::Fans, CoolerChannel::Pump]
            .into_iter()
            .filter_map(|channel| {
                let label = self.hanbo.editor(channel)?.axis_label.as_ref()?;
                match label.read(cx).status() {
                    razer_assets::rotated_label::LabelStatus::Failed(error) => {
                        Some(format!("Hanbo {channel:?} axis label: {error}"))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    pub(crate) fn retry_hanbo_labels(&mut self, cx: &mut Context<Self>) {
        for channel in [CoolerChannel::Fans, CoolerChannel::Pump] {
            if let Some(label) = self
                .hanbo
                .editor(channel)
                .and_then(|editor| editor.axis_label.clone())
            {
                label.update(cx, |label, cx| label.retry(cx));
            }
        }
    }

    pub(crate) fn observe_hanbo_system_sample(
        &mut self,
        sample: &razer_device::system_monitor::SystemSample,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return;
        }
        let previous = self.hanbo.runtime;
        self.hanbo.runtime.observe_system(sample);
        if self.hanbo.runtime != previous {
            cx.notify();
        }
    }

    pub(crate) fn observe_hanbo_telemetry(
        &mut self,
        telemetry: &razer_device::hanbo_cooler::Telemetry,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return;
        }
        let previous = self.hanbo.runtime;
        self.hanbo.runtime.observe_telemetry(telemetry);
        if self.hanbo.runtime != previous {
            cx.notify();
        }
    }

    pub(crate) fn restore_hanbo_celsius(&mut self, celsius: bool, cx: &mut Context<Self>) {
        if self.spec.product_id == razer_device::hanbo_cooler::PRODUCT_ID && self.celsius != celsius
        {
            self.celsius = celsius;
            cx.notify();
        }
    }

    fn set_hanbo_celsius(&mut self, celsius: bool, cx: &mut Context<Self>) {
        self.celsius = celsius;
        cx.emit(super::HanboLocalSettingsChanged {
            change: LocalChange::TemperatureUnit { celsius },
            command: None,
        });
        cx.notify();
    }

    fn hanbo_cooler_info(
        &self,
        artwork: Option<&'static str>,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        super::super::armory_product::hanbo_cooler_info(
            artwork,
            self.celsius,
            &self.hanbo.runtime,
            Rc::new(move |celsius, _, cx| {
                let _ = this.update(cx, |this, cx| this.set_hanbo_celsius(celsius, cx));
            }),
            window,
            cx,
        )
    }

    pub(crate) fn hanbo_armory_element(
        &self,
        device: &Device,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let artwork = razer_assets::device_image(
            device.product_id,
            device.edition_id,
            device.layout_id,
            razer_assets::DeviceImage::Product,
        );
        v_flex()
            .w_full()
            .min_w_0()
            .bg(ProductSurfaceColors::background())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(ProductSurfaceColors::text())
            .child(self.hanbo_cooler_info(artwork, window, cx))
            .into_any_element()
    }

    fn capture_hanbo_thermal(&mut self, commit: ThermalCommit) -> super::HanboLocalSettingsChanged {
        self.hanbo_generation = self.hanbo_generation.wrapping_add(1);
        let generation = self.hanbo_generation;
        let command = match razer_device::hanbo_cooler::CoolerCommand::for_port(
            commit.channel,
            &commit.thermal,
        ) {
            Ok(command) => {
                self.hanbo_command_status = None;
                Some(super::HanboCoolerRequested {
                    generation,
                    command,
                })
            }
            Err(error) => {
                self.hanbo_command_status = Some((generation, Err(error.to_string())));
                None
            }
        };
        super::HanboLocalSettingsChanged {
            change: LocalChange::Thermal {
                channel: commit.channel,
                thermal: commit.thermal,
            },
            command,
        }
    }

    fn publish_hanbo_thermal(&mut self, commit: ThermalCommit, cx: &mut Context<Self>) {
        let request = self.capture_hanbo_thermal(commit);
        cx.emit(request);
        cx.notify();
    }

    /// Return source chart cleanup requests directly so exit can accept them
    /// before closing its asynchronous event subscriptions.
    pub(crate) fn flush_hanbo_curve(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<super::HanboLocalSettingsChanged> {
        if self.spec.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return Vec::new();
        }
        let commits = self.hanbo.unmount_curves();
        let requests = commits
            .into_iter()
            .map(|commit| self.capture_hanbo_thermal(commit))
            .collect();
        cx.notify();
        requests
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
        if self.hanbo.runtime.hardware_outdated() {
            return;
        }
        for commit in self.hanbo.set_mode(channel, mode) {
            self.publish_hanbo_thermal(commit, cx);
        }
        self.sync_hanbo_axis_labels(cx);
    }

    fn hanbo_press(
        &mut self,
        channel: CoolerChannel,
        local: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let (x, y) = (f32::from(local.x), f32::from(local.y));
        if let Some(commit) = self.hanbo.press_curve(channel, x, y) {
            self.publish_hanbo_thermal(commit, cx);
        }
    }

    fn hanbo_move(&mut self, channel: CoolerChannel, local: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(commit) = self.hanbo.move_curve(channel, f32::from(local.y)) {
            self.publish_hanbo_thermal(commit, cx);
        }
    }

    fn hanbo_release(&mut self, cx: &mut Context<Self>) {
        if let Some(commit) = self.hanbo.release_curve() {
            self.publish_hanbo_thermal(commit, cx);
        }
    }

    fn hanbo_chart(&self, channel: CoolerChannel, cx: &Context<Self>) -> AnyElement {
        let geometry = ChartGeometry::new(channel);
        let values = self.hanbo.curve_values(channel).to_vec();
        let selected = self
            .hanbo
            .drag
            .filter(|drag| drag.channel == channel)
            .map(|drag| drag.index);
        let temperatures = channel.temperatures();
        let x_label = match channel {
            CoolerChannel::Fans => "CPU_TEMPERATURE",
            CoolerChannel::Pump => "LIQUID_TEMPERATURE",
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
        let drag_pointer = cx.entity().downgrade();
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
                            window.paint_path(path, Colors::curve_grid());
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
                            window.paint_path(path, Colors::curve());
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
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.hanbo_release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.hanbo_release(cx)),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        let Some(entity) = drag_pointer.upgrade() else {
                            return;
                        };
                        if !entity
                            .read(cx)
                            .hanbo
                            .drag
                            .is_some_and(|drag| drag.channel == channel)
                        {
                            return;
                        }
                        let motion = entity.clone();
                        let move_local = move_local.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase.capture() {
                                motion.update(cx, |this, cx| {
                                    this.hanbo_move(channel, move_local(event.position), cx)
                                });
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase.capture() && event.button == MouseButton::Left {
                                entity.update(cx, |this, cx| this.hanbo_release(cx));
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
        let current = self.hanbo.curve_values(channel).to_vec();
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
                        Colors::held_point()
                    } else {
                        Colors::curve()
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
                .text_color(Colors::text())
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
                    .text_color(Colors::text())
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
                    .text_color(Colors::text())
                    .child(t(x_label)),
            )
            .when_some(
                self.hanbo
                    .editor(channel)
                    .and_then(|editor| editor.axis_label.clone()),
                |chart, label| {
                    chart.child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .w(surface::css(20.))
                            .h(surface::css(CHART_HEIGHT))
                            .text_color(Colors::text())
                            .child(label),
                    )
                },
            )
            .into_any_element()
    }

    fn hanbo_widget(&self, channel: CoolerChannel, cx: &Context<Self>) -> AnyElement {
        let (title, tips) = match channel {
            CoolerChannel::Fans => ("FAN", "FAN_TOOLTIP"),
            CoolerChannel::Pump => ("PUMP", "PUMP_TOOLTIP"),
        };
        let disabled = self.hanbo.runtime.hardware_outdated();
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
            .children(ThermalMode::ALL.map(|option| ModeButton {
                owner: cx.entity().downgrade(),
                channel,
                option,
                selected: option == mode,
                disabled,
            }));
        let panel = surface::panel_with_help(
            t(title),
            self.help_control(format!("hanbo-{channel:?}-help"), t(tips), cx),
            cx,
        )
        .my_0()
        .bg(Colors::surface())
        .when(disabled, |panel| panel.opacity(0.3))
        .child(div().mb(surface::css(8.)).child(t("MODE").to_uppercase()))
        .child(presets)
        .when(mode == ThermalMode::Advanced, |panel| {
            panel.child(self.hanbo_chart(channel, cx))
        });
        // The source dims `.widget.disabled > div`, preserving the card's
        // opaque background. Keep the shared panel composition and supply an
        // equal backdrop under its faded children; occlude pointer input to
        // the help control as well as the already-disabled semantic buttons.
        div()
            .relative()
            .my(surface::css(10.))
            .bg(Colors::surface())
            .rounded(surface::css(5.))
            .child(panel)
            .when(disabled, |panel| {
                panel.child(div().absolute().inset_0().occlude())
            })
            .into_any_element()
    }

    pub(super) fn hanbo(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .child(self.hanbo_cooler_info(self.hanbo_artwork, window, cx))
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        self.hanbo_widget(CoolerChannel::Fans, cx),
                    ))
                    .child(surface::page_column(
                        self.hanbo_widget(CoolerChannel::Pump, cx),
                    )),
            )
            .into_any_element()
    }
}
