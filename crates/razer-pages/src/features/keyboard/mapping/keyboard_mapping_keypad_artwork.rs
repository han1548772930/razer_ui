//! Current three Keypad right-column assets and direction-mode submission.
//! Evidence: keyboard-keypad-artwork-current.json.gz; no vendor code executes.
use super::*;
use gpui_kit::base::motion::{Easing, Transition, transition};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProductImageKey {
    product_id: u32,
    edition_id: u32,
    layout_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in super::super::super) enum KeypadSurface {
    Product,
    Armory(EntityId),
}

#[derive(Default)]
pub(super) struct KeypadRuntime {
    observers_ready: bool,
    active: bool,
    armory_owners: BTreeSet<EntityId>,
    images: BTreeMap<KeypadSurface, ProductImageRuntime>,
}

impl KeypadRuntime {
    fn observers_installed(&mut self) {
        self.observers_ready = true;
    }

    fn set_active(&mut self, active: bool) -> bool {
        let changed = self.active != active;
        self.active = active;
        changed
    }

    fn customize_mounted(&self, surface: KeypadSurface, page: &str) -> bool {
        self.observers_ready
            && match surface {
                KeypadSurface::Product => self.active && page == "TAB_CUSTOMIZE",
                KeypadSurface::Armory(owner) => self.armory_owners.contains(&owner),
            }
    }

    fn set_armory_open(&mut self, owner: EntityId, open: bool) -> bool {
        if open {
            self.armory_owners.insert(owner)
        } else {
            self.images.remove(&KeypadSurface::Armory(owner));
            self.armory_owners.remove(&owner)
        }
    }
}

#[derive(Default)]
pub(super) struct ProductImageRuntime {
    key: Option<ProductImageKey>,
    view: Option<EntityId>,
    /// Source sessionStorage is distinct from device/profile persistence.
    sessions: BTreeMap<ProductImageKey, u64>,
    sequence: u64,
    // Keep the selected aliases after publication so an image-cache retry can
    // re-probe the same source without changing the mounted effect.
    pending: [Option<String>; 2],
    completed: [bool; 2],
    decoded: [Option<Arc<RenderImage>>; 2],
    published: [Option<Arc<RenderImage>>; 2],
    errors: [Option<String>; 2],
    effect_published: bool,
    loading: bool,
    clear_loading_on_frame: bool,
    remount: Option<Task<()>>,
}

impl ProductImageRuntime {
    fn begin_effect(&mut self, key: ProductImageKey, pending: [Option<String>; 2]) {
        if self.key.is_none() {
            self.sequence = self.sessions.get(&key).copied().unwrap_or(0);
        }
        self.key = Some(key);
        // A prop update keeps the previous srcSet and sequence until both new
        // probes finish. Its old remount timeout belongs to that published srcSet.
        self.completed = [false; 2];
        self.effect_published = false;
        self.decoded = Default::default();
        self.errors = Default::default();
        self.loading = key.product_id != 747;
        self.pending = pending;
    }

    fn unmount(&mut self) {
        self.key = None;
        self.view = None;
        self.pending = Default::default();
        self.completed = [false; 2];
        self.decoded = Default::default();
        self.published = Default::default();
        self.errors = Default::default();
        self.effect_published = false;
        self.loading = false;
        self.clear_loading_on_frame = false;
        self.remount = None;
    }

    fn probe_pending(&mut self, index: usize) {
        self.completed[index] = false;
        self.decoded[index] = None;
        self.effect_published = false;
    }

    fn record_probe(&mut self, index: usize, result: Result<Arc<RenderImage>, String>) {
        self.completed[index] = true;
        match result {
            Ok(image) => {
                self.decoded[index] = Some(image);
                self.errors[index] = None;
            }
            Err(error) => {
                self.decoded[index] = None;
                self.errors[index] = Some(error);
            }
        }
    }

    fn publish_effect(&mut self) -> bool {
        if self.effect_published || !self.completed.iter().all(|completed| *completed) {
            return false;
        }
        self.effect_published = true;
        self.remount = None;
        self.published = self.decoded.clone();
        true
    }
}

#[derive(IntoElement)]
struct FlickerFrame {
    frame: Div,
    started: Option<Instant>,
}
impl RenderOnce for FlickerFrame {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // Actual CSS flickerAnimation: transparent / #44d62c / transparent,
        // 0/50/100%, 1 second, the default ease in each keyframe interval.
        let elapsed = self.started.map(|start| start.elapsed().as_secs_f32());
        let active = elapsed.is_some_and(|time| time < 1.);
        let alpha = elapsed
            .filter(|time| *time < 1.)
            .map(|time| {
                if time <= 0.5 {
                    Easing::Ease.sample(time * 2.)
                } else {
                    1. - Easing::Ease.sample((time - 0.5) * 2.)
                }
            })
            .unwrap_or(0.);
        self.frame
            .border_color(rgba(0x44d62c00 | (alpha * 255.).round() as u32))
            .on_prepaint(move |_, window, _| {
                if active {
                    window.request_animation_frame();
                }
            })
    }
}

#[derive(Deserialize)]
struct Image {
    edition_id: u32,
    role: String,
    asset: String,
}
#[derive(Deserialize)]
struct Icon {
    role: String,
    asset: String,
}
#[derive(Deserialize)]
struct ProductImage {
    product_id: u32,
    edition_id: u32,
    density: u32,
    asset: String,
}
fn product_images() -> &'static [ProductImage] {
    static IMAGES: OnceLock<Vec<ProductImage>> = OnceLock::new();
    IMAGES.get_or_init(|| read_keyboard_keypad_device_image_catalog())
}
pub fn source_keypad_product_image(pid: u32, edition: u32, density: u32) -> Option<&'static str> {
    let density = if pid == 747 { 1 } else { density };
    [edition, 0].into_iter().find_map(|edition| {
        product_images()
            .iter()
            .find(|image| {
                image.product_id == pid && image.edition_id == edition && image.density == density
            })
            .map(|image| image.asset.as_str())
    })
}
#[derive(IntoElement)]
struct ProductImageElement {
    key: ProductImageKey,
    surface: KeypadSurface,
    owner: WeakEntity<KeyboardProductWorkspace>,
    images: [Option<Arc<RenderImage>>; 2],
    sequence: u64,
}
impl RenderOnce for ProductImageElement {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let pid = self.key.product_id;
        let index = if pid == 747 || window.scale_factor() <= 1. {
            0
        } else {
            1
        };
        div()
            .absolute()
            .left_0()
            .top_0()
            .w(surface::css(if pid == 747 { 400. } else { 350. }))
            .h(surface::css(if pid == 747 { 502. } else { 450. }))
            .children(self.images[index].clone().map(|image| {
                img(image)
                    .id(SharedString::from(format!(
                        "keypad-product-image-{}",
                        self.sequence
                    )))
                    .size_full()
            }))
            .on_prepaint(move |_, window, cx| {
                let view = window.current_view();
                let _ = self.owner.update(cx, |this, cx| {
                    // A retained element from the previous page or metadata
                    // must not remount an effect after its owner has left.
                    if !this
                        .analog_gamepad
                        .mapping
                        .keypad_runtime
                        .customize_mounted(self.surface, &this.page)
                        || this.keypad_product_image_key() != self.key
                    {
                        return;
                    }
                    // Prepaint retains the actual current view, including
                    // inline Customize, for the shared image-cache lease.
                    if this.sync_keypad_product_image(self.surface, view, window, cx) {
                        window.request_animation_frame();
                    }
                });
            })
    }
}
#[derive(Deserialize)]
struct Product {
    product_id: u32,
    images: Vec<Image>,
    icons: Vec<Icon>,
    line_parameters: Value,
    line_points: Vec<LinePoint>,
}
#[derive(Deserialize)]
struct LinePoint {
    key: String,
    x: f32,
    y: f32,
    #[serde(rename = "type")]
    kind: String,
    direction_mode: u64,
}
struct LabelPosition {
    child: AnyElement,
    body: Rc<Cell<Bounds<Pixels>>>,
    x: f32,
    y: f32,
    tx: f32,
    ty: f32,
    relative_x: f32,
    relative_y: f32,
}
impl IntoElement for LabelPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for LabelPosition {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let parent = self.body.get();
        let measured = window.layout_bounds(*child).size;
        let origin = point(
            parent.left()
                + parent.size.width * self.relative_x
                + surface::css(self.x).to_pixels(window.rem_size())
                + measured.width * self.tx,
            parent.top()
                + parent.size.height * self.relative_y
                + surface::css(self.y).to_pixels(window.rem_size())
                + measured.height * self.ty,
        );
        self.child.prepaint_at(origin, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
#[derive(IntoElement)]
struct AnimatedModeToggle {
    directional: bool,
    hypershift: bool,
}
impl RenderOnce for AnimatedModeToggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ease = Transition::new(Duration::from_millis(200)).easing(Easing::Ease);
        let left = transition(
            "keypad-toggle-left",
            if self.directional { 52_f32 } else { 0. },
            ease.clone(),
            window,
            cx,
        );
        let color = transition(
            "keypad-toggle-background",
            Hsla::from(rgb(if self.hypershift { 0xfd8611 } else { 0x44d62c })),
            ease,
            window,
            cx,
        );
        div()
            .absolute()
            .left(surface::css(left))
            .top_0()
            .w(surface::css(52.))
            .h(surface::css(25.))
            .when(self.directional, |button| {
                button
                    .rounded_tr(surface::css(3.))
                    .rounded_br(surface::css(3.))
            })
            .when(!self.directional, |button| {
                button
                    .rounded_tl(surface::css(3.))
                    .rounded_bl(surface::css(3.))
            })
            .bg(color)
    }
}
#[derive(IntoElement)]
struct ModeTip {
    value: u64,
    label: &'static str,
    shown: bool,
    trigger: Rc<Cell<Bounds<Pixels>>>,
}
struct WidgetHelpPosition {
    child: AnyElement,
    widget: Rc<Cell<Bounds<Pixels>>>,
    help: Rc<Cell<Bounds<Pixels>>>,
    viewport: Rc<Cell<Bounds<Pixels>>>,
}
#[derive(IntoElement)]
struct WidgetHelpIcon {
    shown: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    owner: WeakEntity<KeyboardProductWorkspace>,
}
impl RenderOnce for WidgetHelpIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background: Hsla = transition(
            "keypad-widget-help-background",
            if self.shown {
                rgba(0xffffff4d).into()
            } else {
                rgb(0x4a4a4a).into()
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        BaseButton::new("keypad-widget-help")
            .absolute()
            .top(surface::css(10.))
            .right(surface::css(10.))
            .size(surface::css(14.))
            .p_0()
            .rounded_full()
            .bg(background)
            .accessibility_label(t("THUMBSTICK_INPUT_MODE_TOOLTIP"))
            .child(img("synapse/keyboard-mapping-notices/help.svg").size_full())
            .on_prepaint(move |bounds, _, _| self.bounds.set(bounds))
            .on_hover(move |entered, _, cx| {
                let _ = self.owner.update(cx, |this, cx| {
                    this.analog_gamepad.mapping.keypad_help_shown = *entered;
                    cx.notify();
                });
            })
    }
}
impl IntoElement for WidgetHelpPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for WidgetHelpPosition {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let widget = self.widget.get();
        let help = self.help.get();
        let viewport = self.viewport.get();
        let measured = window.layout_bounds(*child).size;
        let css = |value| surface::css(value).to_pixels(window.rem_size());
        let mut origin = point(
            widget.right() - css(14.) - measured.width,
            widget.top() + css(34.),
        );
        // Current 747 mounted module76299's onHover, not the shell portal
        // helper: bottom overflow changes to fixed help.right+8/help.top,
        // then flips left and finally shifts up by bottom overflow+10.
        if origin.y + measured.height > viewport.bottom() {
            origin = point(help.right() + css(8.), help.top());
            if origin.x + measured.width > viewport.right() {
                origin.x = help.left() - css(8.) - measured.width;
            }
            if origin.y + measured.height > viewport.bottom() {
                origin.y = viewport.bottom() - measured.height - css(10.);
            }
        }
        self.child.prepaint_at(origin, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
impl RenderOnce for ModeTip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let opacity = transition(
            SharedString::from(format!("keypad-mode-tip-opacity-{}", self.value)),
            if self.shown { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        div().children((opacity > 0.).then(|| {
            razer_assets::deferred_resource(LabelPosition {
                child: div()
                    .px(surface::css(10.))
                    .py(surface::css(6.))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .bg(rgb(0x000000))
                    .text_color(rgb(0xcccccc))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .text_center()
                    .whitespace_nowrap()
                    .opacity(opacity)
                    .child(t(self.label))
                    .into_any_element(),
                body: self.trigger,
                x: 0.,
                y: 2.,
                tx: -0.5,
                ty: 0.,
                relative_x: 0.5,
                relative_y: 1.,
            })
            .with_priority(100)
        }))
    }
}
fn source(pid: u32) -> Option<&'static Product> {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_keypad_artwork_catalog())
        .iter()
        .find(|row| row.product_id == pid)
}

#[derive(IntoElement)]
struct DensityImage {
    pid: u32,
    edition: u32,
    role: &'static str,
    width: f32,
    height: f32,
}
impl RenderOnce for DensityImage {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let role = if self.role.ends_with(".svg") {
            self.role.to_owned()
        } else {
            format!(
                "{}-{}x.png",
                self.role,
                if window.scale_factor() > 1. { 3 } else { 1 }
            )
        };
        source(self.pid)
            .and_then(|row| {
                row.images
                    .iter()
                    .find(|image| image.edition_id == self.edition && image.role == role)
            })
            .map(|image| {
                img(image.asset.clone())
                    .w(surface::css(self.width))
                    .h(surface::css(self.height))
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .w(surface::css(self.width))
                    .h(surface::css(self.height))
                    .into_any_element()
            })
    }
}

impl KeyboardProductWorkspace {
    pub(in super::super::super) fn keypad_physical_fill_suppressed(&self, input: &str) -> bool {
        self.mapping_buttons()
            .iter()
            .find(|button| button["inputID"].as_str() == Some(input))
            .is_some_and(|button| {
                // The source's disableKeypadButton class suppresses fill,
                // despite its name; it leaves the SVG click handler enabled.
                matches!(
                    button["buttonKey"].as_str(),
                    Some("ScrollClick" | "BUTTON_DOWNRIGHT")
                )
            })
    }
    /// Physical SVG caller only: ConfigButton/sidebar callers pass their own
    /// refs and must retain the ordinary mapping selection path.
    pub(in super::super::super) fn keypad_physical_click(
        &mut self,
        input: &str,
        path: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.spec.product_id, 555 | 580 | 747) {
            return false;
        }
        let button = self.mapping_buttons().iter().find(|button| {
            button["inputID"].as_str() == Some(input) || button["buttonKey"].as_str() == Some(input)
        });
        let key = button.and_then(|button| button["buttonKey"].as_str());
        if self.spec.product_id == 747 {
            // Actual Ie=["DKM_KEYPAD_JOYSTICK"] guard precedes the dirty alert.
            return key == Some("DKM_KEYPAD_JOYSTICK")
                || (self.hypershift
                    && button
                        .is_some_and(|button| panel_truthy(&button["disableHypershiftMapping"])));
        }
        if !path {
            return false;
        }
        let index = match key {
            Some("ScrollClick") => 0,
            Some("BUTTON_DOWNRIGHT") => 1,
            _ => return false,
        };
        // classList.add while the class exists does not restart its animation.
        self.analog_gamepad.mapping.keypad_flicker[index].get_or_insert_with(Instant::now);
        let canceled = Rc::new(Cell::new(false));
        self.analog_gamepad.mapping.keypad_flicker_last_timer = Some(canceled.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            if canceled.get() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.analog_gamepad.mapping;
                state.keypad_flicker[index] = None;
                // Source callback clears this.saveTimeOut (the most recently
                // stored timeout), even after a subsequent click overwrites it.
                if let Some(latest) = state.keypad_flicker_last_timer.take() {
                    latest.set(true);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }
    fn keypad_product_image_key(&self) -> ProductImageKey {
        ProductImageKey {
            product_id: self.spec.product_id,
            edition_id: self.keypad_edition(),
            layout_id: self.analog_gamepad.layout.unwrap_or(0),
        }
    }

    pub(in super::super::super) fn keypad_product_image(
        &self,
        surface: KeypadSurface,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = self
            .analog_gamepad
            .mapping
            .keypad_runtime
            .images
            .get(&surface);
        ProductImageElement {
            key: self.keypad_product_image_key(),
            surface,
            owner: cx.weak_entity(),
            images: state
                .map(|state| state.published.clone())
                .unwrap_or_default(),
            sequence: state.map(|state| state.sequence).unwrap_or(0),
        }
        .into_any_element()
    }
    fn sync_keypad_product_image(
        &mut self,
        surface: KeypadSurface,
        view: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let pid = self.spec.product_id;
        let key = self.keypad_product_image_key();
        let state = self
            .analog_gamepad
            .mapping
            .keypad_runtime
            .images
            .entry(surface)
            .or_default();
        state.view = Some(view);
        let mut changed = false;
        if state.clear_loading_on_frame {
            state.clear_loading_on_frame = false;
            state.loading = false;
            changed = true;
        }
        if state.key != Some(key) {
            let pending = [1, if pid == 747 { 1 } else { 3 }].map(|density| {
                source_keypad_product_image(pid, key.edition_id, density).map(str::to_owned)
            });
            state.begin_effect(key, pending);
        }
        // Current for-loop probes 1x then 3x; onerror is caught independently,
        // so decode failure never requests a different edition or reports a save.
        for index in 0..2 {
            let Some(asset) = state.pending[index].as_ref() else {
                state.record_probe(
                    index,
                    Err("ProductImage current context request is absent".into()),
                );
                continue;
            };
            let resource = Resource::Embedded(SharedString::from(asset.clone()));
            let Some(result) = razer_assets::load_resource_image(resource, window, cx) else {
                // The shared image owner can evict a completed decode on an
                // explicit retry. Keep the probe pending so the next frame
                // republishes the newly decoded image instead of retaining
                // the previous error publication.
                state.probe_pending(index);
                return changed;
            };
            state.record_probe(index, result.map_err(|error| error.to_string()));
        }
        // Publish once per effect, including caught image errors.
        if !state.publish_effect() {
            return changed;
        }
        // The remount effect depends on published srcSet, not the request key:
        // a prop change alone leaves its previous timeout alive until this point.
        if pid != 747 && state.loading {
            state.clear_loading_on_frame = true;
        }
        if state.sequence == 0 && state.remount.is_none() {
            state.remount = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let runtime = &mut this.analog_gamepad.mapping.keypad_runtime;
                    if !runtime.customize_mounted(surface, &this.page) {
                        return;
                    }
                    if let Some(state) = runtime.images.get_mut(&surface)
                        && state.key.is_some()
                    {
                        state.sequence += 1;
                        state.sessions.insert(key, state.sequence);
                        cx.notify();
                        App::notify(cx, view);
                    }
                });
            }));
        }
        true
    }
    fn keypad_connection_canvas(&self) -> AnyElement {
        let metadata = source(self.spec.product_id).expect("current mounted Keypad");
        let modern = self.spec.product_id == 747;
        let mode = self.keypad_direction_mode();
        let dot_radius = metadata.line_parameters["dotRadius"]
            .as_f64()
            .expect("source dot radius") as f32;
        let circle_radius = metadata.line_parameters["circleRadius"]
            .as_f64()
            .expect("source focus ring radius") as f32;
        let rows = metadata
            .line_points
            .iter()
            .filter(|row| row.direction_mode == 0 || row.direction_mode == mode)
            .filter_map(|row| {
                let raw = self
                    .mapping_buttons()
                    .iter()
                    .find(|button| button["buttonKey"] == row.key)?;
                let button = self.mapping_active_button(raw);
                let focused = self.mapping_selection_matches(&row.key)
                    || self.hovered_key.as_deref() == button["inputID"].as_str();
                let color = if !panel_truthy(&button["isEnabled"]) {
                    rgb(0xcccccc)
                } else if focused {
                    rgb(if self.hypershift { 0xfd8611 } else { 0x44d62c })
                } else {
                    rgb(0x5d5d5d)
                };
                Some((row.x, row.y, row.kind.clone(), focused, Hsla::from(color)))
            })
            .collect::<Vec<_>>();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let unit = surface::css(1.).to_pixels(window.rem_size());
                let point_at =
                    |x: f32, y: f32| point(bounds.left() + unit * x, bounds.top() + unit * y);
                for (x, y, kind, focused, color) in &rows {
                    let endpoint = (*x + 3. * dot_radius, *y + dot_radius);
                    if modern {
                        let connector = match kind.as_str() {
                            "left" => Some(((150., endpoint.1), None)),
                            "right" => Some(((350., endpoint.1), None)),
                            "top-left" => {
                                Some(((150., 197.), Some((190. + 3. * dot_radius, 197.))))
                            }
                            "top-right" => {
                                Some(((350., 197.), Some((295. + 3. * dot_radius, 197.))))
                            }
                            "bottom-left" => {
                                Some(((150., 297.), Some((190. + 3. * dot_radius, 297.))))
                            }
                            "bottom-right" => {
                                Some(((350., 297.), Some((295. + 3. * dot_radius, 297.))))
                            }
                            "press" => Some((
                                (340. + 3. * dot_radius, 367. + dot_radius),
                                Some((300. + 3. * dot_radius, 367. + dot_radius)),
                            )),
                            _ => None,
                        };
                        if let Some((start, bend)) = connector {
                            let mut path = PathBuilder::stroke(unit * 1.5);
                            path.move_to(point_at(start.0, start.1));
                            if let Some(bend) = bend {
                                path.line_to(point_at(bend.0, bend.1));
                            }
                            path.line_to(point_at(endpoint.0, endpoint.1));
                            if let Ok(path) = path.build() {
                                window.paint_path(path, *color);
                            }
                            paint_circle(
                                window,
                                point_at(start.0, start.1),
                                unit * dot_radius,
                                None,
                                *color,
                            );
                        }
                    }
                    paint_circle(
                        window,
                        point_at(endpoint.0, endpoint.1),
                        unit * dot_radius,
                        None,
                        *color,
                    );
                    if *focused {
                        paint_circle(
                            window,
                            point_at(endpoint.0, endpoint.1),
                            unit * circle_radius,
                            Some(unit * if modern { 1.5 } else { 1. }),
                            *color,
                        );
                    }
                }
            },
        )
        .absolute()
        .left(surface::css(if modern { 0. } else { -3. }))
        .top_0()
        .w(surface::css(if modern { 500. } else { 400. }))
        .h(surface::css(if modern { 396. } else { 450. }))
        .into_any_element()
    }
    fn keypad_thumbstick(&self) -> AnyElement {
        let (x, y) = self
            .keypad_observer
            .sample()
            .map(|sample| {
                (
                    f64::from(sample.right_thumb_x()),
                    f64::from(sample.right_thumb_y()),
                )
            })
            .unwrap_or((0., 0.));
        let mut plotted_x = (x + 1000.) / 2000. * 153. + 4.5;
        let mut plotted_y = (y + 1000.) / 2000. * 153. + 4.5;
        if plotted_x == 0. {
            plotted_x = 81.;
        }
        if plotted_y == 0. {
            plotted_y = 81.;
        }
        let distance = ((plotted_x - 81.).powi(2) + (plotted_y - 81.).powi(2)).sqrt();
        if distance > 78. {
            plotted_x = 81. + (plotted_x - 81.) / distance * 78.;
            plotted_y = 81. + (plotted_y - 81.) / distance * 78.;
        }
        let percent_x = 100. * x / 1000.;
        let percent_y = 100. * -y / 1000.;
        let x_label = if percent_x == 0. {
            "0".to_owned()
        } else {
            format!("{percent_x:.1}")
        };
        let y_label = if percent_y > 0. && percent_y < 0.05 {
            "0".to_owned()
        } else if percent_y == 0. {
            "0.0".to_owned()
        } else {
            format!("{percent_y:.1}")
        };
        v_flex()
            .w(surface::css(162.))
            .mx_auto()
            .mt(surface::css(20.))
            .child(
                div()
                    .relative()
                    .w(surface::css(162.))
                    .h(surface::css(160.))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let unit = surface::css(1.).to_pixels(window.rem_size());
                                let point_at = |x: f32, y: f32| {
                                    point(bounds.left() + unit * x, bounds.top() + unit * y)
                                };
                                paint_circle(
                                    window,
                                    point_at(81., 81.),
                                    unit * 78.,
                                    Some(unit * 2.),
                                    rgb(0x707070).into(),
                                );
                                paint_circle(
                                    window,
                                    point_at(81., 81.),
                                    unit * 5.,
                                    None,
                                    rgb(0x44d62c).into(),
                                );
                                let plotted = point_at(plotted_x as f32, plotted_y as f32);
                                let mut line = PathBuilder::stroke(unit * 2.);
                                line.move_to(plotted);
                                line.line_to(point_at(81., 81.));
                                if let Ok(path) = line.build() {
                                    window.paint_path(path, rgb(0x44d62c));
                                }
                                paint_circle(
                                    window,
                                    plotted,
                                    unit * 5.,
                                    None,
                                    rgb(0x44d62c).into(),
                                );
                            },
                        )
                        .w(surface::css(162.))
                        .h(surface::css(162.)),
                    ),
            )
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .gap(surface::css(20.))
                    .justify_center()
                    .text_size(surface::css(14.))
                    .whitespace_nowrap()
                    .child(format!("X: {x_label}%"))
                    .child(format!("Y: {y_label}%")),
            )
            .into_any_element()
    }
    pub(in super::super::super) fn mount_keypad_runtime(&mut self, cx: &mut Context<Self>) {
        self.analog_gamepad
            .mapping
            .keypad_runtime
            .observers_installed();
        self.sync_keypad_artwork_runtime(cx);
        cx.notify();
    }

    pub(in super::super::super) fn notify_keypad_views(&self, cx: &mut Context<Self>) {
        let runtime = &self.analog_gamepad.mapping.keypad_runtime;
        for (surface, image) in &runtime.images {
            if runtime.customize_mounted(*surface, &self.page)
                && let Some(view) = image.view
            {
                App::notify(cx, view);
            }
        }
        cx.notify();
    }

    pub(in crate::features) fn set_keypad_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if !matches!(self.spec.product_id, 555 | 580 | 747) {
            return;
        }
        let runtime = &mut self.analog_gamepad.mapping.keypad_runtime;
        let changed = runtime.set_active(active);
        self.sync_keypad_artwork_runtime(cx);
        if changed {
            cx.notify();
        }
    }

    pub(in crate::features) fn set_armory_mapping_open(
        &mut self,
        owner: EntityId,
        open: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 555 | 580 | 747) {
            return;
        }
        if self
            .analog_gamepad
            .mapping
            .keypad_runtime
            .set_armory_open(owner, open)
        {
            self.sync_keypad_artwork_runtime(cx);
            cx.notify();
        }
    }

    pub(in super::super::super) fn sync_keypad_artwork_runtime(&mut self, cx: &mut Context<Self>) {
        let runtime = &mut self.analog_gamepad.mapping.keypad_runtime;
        if !runtime.observers_ready {
            return;
        }
        let product = runtime.customize_mounted(KeypadSurface::Product, &self.page);
        if !product && let Some(state) = runtime.images.get_mut(&KeypadSurface::Product) {
            state.unmount();
        }
        let customize = product || !runtime.armory_owners.is_empty();
        if !customize {
            self.analog_gamepad.mapping.keypad_help_shown = false;
        }
        let analog = customize && self.keypad_direction_mode() == 3;
        if self.spec.product_id != 747 {
            return;
        }
        self.request_keypad_analog_mode(analog, cx);
    }

    fn request_keypad_analog_mode(&mut self, analog: bool, cx: &mut Context<Self>) {
        self.set_keypad_sample_forwarding(analog, cx);
    }
    fn keypad_config_button(
        &self,
        key: &'static str,
        left_side: bool,
        scroll: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(raw) = self
            .mapping_buttons()
            .iter()
            .find(|row| row["buttonKey"] == key)
        else {
            return div().into_any_element();
        };
        let button = self.mapping_active_button(raw);
        let disabled = panel_truthy(&button["disabled"]) || !panel_truthy(&button["isEnabled"]);
        let modern = self.spec.product_id == 747;
        let color = if button["assignment"] == "DISABLE" {
            0xc8323c
        } else if panel_truthy(&button["isMapped"]) {
            if self.hypershift { 0xfd8611 } else { 0x44d62c }
        } else {
            0xcccccc
        };
        let input = text(&button["inputID"]);
        let hover_input = input.clone();
        let regions = self.analog_gamepad.mapping.button_regions.clone();
        let description = text(&button["buttonDescription"]);
        let label = v_flex()
            .max_w(surface::css(250.))
            .overflow_hidden()
            .text_ellipsis()
            .when(!description.is_empty(), |label| {
                label.py(surface::css(2.)).child(
                    div()
                        .text_size(surface::css(10.))
                        .line_height(surface::css(12.))
                        .text_color(rgb(if self.hypershift { 0xfd8611 } else { 0x44d62c }))
                        .child(t(&description).to_uppercase()),
                )
            })
            .child(t(&text(&button["assignmentValue"])));
        let control = BaseButton::new(SharedString::from(format!("keypad-artwork-button-{key}")))
            .h(surface::css(30.))
            .flex_shrink_0()
            .max_w(surface::css(if scroll { 150. } else { 220. }))
            .px(surface::css(if scroll { 5. } else { 10. }))
            .pt(surface::css(if scroll {
                8.
            } else if modern {
                7.
            } else {
                6.
            }))
            .pb(surface::css(if scroll {
                8.
            } else if modern {
                6.
            } else {
                7.
            }))
            .when(!modern, |button| {
                button
                    .mb(surface::css(if scroll { 10. } else { 20. }))
                    .when(left_side, |button| button.mr(surface::css(10.)))
                    .when(!left_side, |button| button.ml(surface::css(10.)))
            })
            .rounded(surface::css(3.))
            .text_size(surface::css(14.))
            .line_height(surface::css(if scroll { 14. } else { 17. }))
            .disabled(disabled)
            .text_color(if disabled {
                rgba(0xcccccc4d)
            } else {
                rgba(color << 8 | 0xff)
            })
            .overflow_hidden()
            .whitespace_nowrap()
            .bg(if self.mapping_selection_matches(key) {
                Hsla::from(rgb(0x222222))
            } else {
                Hsla::from(rgba(0x00000000))
            })
            .hover(|button| button.bg(rgb(0x383838)))
            .active(|button| button.bg(rgb(0x111111)))
            .child(label)
            .on_prepaint(move |bounds, _, _| {
                regions.borrow_mut().insert(input.clone(), bounds);
            })
            .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                if *entered {
                    this.hovered_key = Some(hover_input.clone());
                } else if this.hovered_key.as_ref() == Some(&hover_input) {
                    this.hovered_key = None;
                }
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.analog_mapping_next(Next::Key(key.into(), false), window, cx)
            }))
            .into_any_element();
        // Source positions the ConfigButton wrapper. Its child's side and
        // bottom margins participate in the wrapper's transform dimensions.
        div().child(control).into_any_element()
    }
    pub(in super::super::super) fn keypad_customize_artwork(
        &self,
        surface: KeypadSurface,
        cx: &Context<Self>,
    ) -> AnyElement {
        let modern = self.spec.product_id == 747;
        let width = if modern { 420. } else { 394. };
        let mode = self.keypad_direction_mode();
        let mut right = v_flex()
            .w(surface::css(if modern { 500. } else { 390. }))
            .mx(surface::css(30.))
            .when(modern, |column| {
                column
                    .min_w(surface::css(500.))
                    .p(surface::css(10.))
                    .self_center()
            });
        if !modern {
            right = right.child(FlickerFrame {
                started: self.analog_gamepad.mapping.keypad_flicker[0],
                frame: div()
                    .relative()
                    .w(surface::css(400.))
                    .h(surface::css(150.))
                    .mb(surface::css(20.))
                    .bg(rgb(0x111111))
                    .border_3()
                    .border_color(rgba(0x00000000))
                    .rounded(surface::css(5.))
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(128.))
                            .top(surface::css(50.))
                            .child(self.keypad_density_image("scroll", 26., 50.)),
                    )
                    .child(
                        v_flex()
                            .absolute()
                            .left(relative(0.5))
                            .top(relative(0.15))
                            .max_w(surface::css(150.))
                            .children(
                                ["ScrollUp", "ScrollClick", "ScrollDown"]
                                    .map(|key| self.keypad_config_button(key, false, true, cx)),
                            ),
                    )
                    .child(self.keypad_connection_canvas()),
            });
        }
        let body_bounds = Rc::new(Cell::new(Bounds::default()));
        let measured_body = body_bounds.clone();
        let mut body = div()
            .relative()
            .w(surface::css(width))
            .h(surface::css(if modern { 222. } else { 219. }))
            .on_prepaint(move |bounds, _, _| measured_body.set(bounds));
        if mode != 3 || !modern {
            body = body.child(
                div()
                    .absolute()
                    .left(surface::css(if modern { width / 2. - 25. } else { 180. }))
                    .top(surface::css(if modern { 55. } else { 82. }))
                    .child(self.keypad_density_image(
                        if modern { "dpad-group.svg" } else { "group" },
                        50.,
                        50.,
                    )),
            );
            let mut buttons = vec![
                (
                    "left",
                    if mode == 2 { "BUTTON_LEFT" } else { "DIRECTION_LEFT" },
                ),
                ("up", if mode == 2 { "BUTTON_UP" } else { "DIRECTION_UP" }),
                (
                    "right",
                    if mode == 2 {
                        "BUTTON_RIGHT"
                    } else {
                        "DIRECTION_RIGHT"
                    },
                ),
                (
                    "down",
                    if mode == 2 { "BUTTON_DOWN" } else { "DIRECTION_DOWN" },
                ),
            ];
            if mode == 2 {
                buttons.extend([
                    ("top-left", "BUTTON_UPLEFT"),
                    ("top-right", "BUTTON_UPRIGHT"),
                    ("bottom-left", "BUTTON_DOWNLEFT"),
                    ("bottom-right", "BUTTON_DOWNRIGHT"),
                ]);
            }
            if modern {
                buttons.push(("press", "DKM_JOYSTICK_PRESS"));
            }
            for (position, key) in buttons {
                if self
                    .mapping_buttons()
                    .iter()
                    .any(|button| button["buttonKey"] == key)
                {
                    let (x, y, tx, ty) = match position {
                        "left" => (
                            if modern { 100. } else { 130. },
                            if modern { 222. * 0.37 } else { 109.5 },
                            -1.,
                            -0.5,
                        ),
                        "right" => (
                            width - if modern { 100. } else { 130. },
                            if modern { 222. * 0.37 } else { 109.5 },
                            0.,
                            -0.5,
                        ),
                        "up" => (
                            width * if modern { 0.5 } else { 0.51 },
                            if modern { 20. } else { 40. },
                            -0.5,
                            -0.5,
                        ),
                        "down" => (
                            width * if modern { 0.5 } else { 0.51 },
                            if modern { 167. } else { 219. },
                            -0.5,
                            -1.,
                        ),
                        "top-left" => (
                            if modern { 100. } else { 130. },
                            if modern { 30. } else { 40. },
                            -1.,
                            -0.5,
                        ),
                        "top-right" => (
                            width - if modern { 100. } else { 130. },
                            if modern { 30. } else { 40. },
                            0.,
                            -0.5,
                        ),
                        "bottom-left" => (
                            if modern { 100. } else { 130. },
                            if modern { 147. } else { 219. },
                            -1.,
                            -1.,
                        ),
                        "bottom-right" => (
                            width - if modern { 100. } else { 130. },
                            if modern { 147. } else { 219. },
                            0.,
                            -1.,
                        ),
                        _ => (width - 100., 222., 0., -1.),
                    };
                    let button = self.keypad_config_button(key, position == "left", false, cx);
                    body = body.child(LabelPosition {
                        child: button.into_any_element(),
                        body: body_bounds.clone(),
                        x,
                        y,
                        tx,
                        ty,
                        relative_x: 0.,
                        relative_y: 0.,
                    });
                }
            }
        } else {
            // Source initial reducer values are zero; this initial center is
            // local source state, never a fabricated device observation.
            body = body.child(self.keypad_thumbstick());
        }
        let mut dpad = v_flex()
            .w(surface::css(if modern { width } else { 400. }))
            .when(!modern, |block| {
                block
                    .bg(rgb(0x111111))
                    .rounded(surface::css(5.))
                    .h(surface::css(272.))
                    .border_3()
                    .border_color(rgba(0x00000000))
            })
            .child(
                div()
                    .h(surface::css(47.))
                    .flex()
                    .justify_center()
                    .items_start()
                    .child(
                        div()
                            .mt(surface::css(if modern { 10. } else { 20. }))
                            .child(self.keypad_mode_control(cx)),
                    ),
            )
            .child(body);
        if modern {
            let widget_bounds = self.analog_gamepad.mapping.keypad_widget_bounds.clone();
            dpad = surface::panel(t("THUMBSTICK_INPUT_MODE"), cx)
                .relative()
                .on_prepaint(move |bounds, _, _| widget_bounds.set(bounds))
                .min_w(surface::css(500.))
                .max_w(surface::css(500.))
                .h(surface::css(410.))
                .child(
                    div()
                        .h(surface::css(34.))
                        .line_height(surface::css(17.))
                        .mb(surface::css(10.))
                        .child(t("THUMBSTICK_INPUT_MODE_DESC")),
                )
                .child(dpad)
                .child(WidgetHelpIcon {
                    shown: self.analog_gamepad.mapping.keypad_help_shown,
                    bounds: self.analog_gamepad.mapping.keypad_help_bounds.clone(),
                    owner: cx.entity().downgrade(),
                })
                .when(self.analog_gamepad.mapping.keypad_help_shown, |panel| {
                    panel.child(
                        razer_assets::deferred_resource(WidgetHelpPosition {
                            child: v_flex()
                                .max_w(surface::css(300.))
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .bg(rgb(0x000000))
                                .text_color(rgb(0xcccccc))
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(18.))
                                .child(div().child(t("THUMBSTICK_INPUT_MODE_TOOLTIP")))
                                .child(div().h(surface::css(18.)))
                                .child(
                                    h_flex()
                                        .items_start()
                                        .gap(surface::css(5.))
                                        .child(
                                            img("synapse/keyboard-mapping-notices/synapse.svg")
                                                .size(surface::css(16.)),
                                        )
                                        .child(
                                            div()
                                                .text_color(rgb(0x999999))
                                                .child(t("REQUIRES_RAZER_SYNAPSE")),
                                        ),
                                )
                                .into_any_element(),
                            widget: self.analog_gamepad.mapping.keypad_widget_bounds.clone(),
                            help: self.analog_gamepad.mapping.keypad_help_bounds.clone(),
                            viewport: self.analog_gamepad.mapping.viewport_bounds.clone(),
                        })
                        .with_priority(100),
                    )
                })
                .when(mode != 3, |panel| {
                    panel.child(self.keypad_connection_canvas())
                });
        }
        right = right.child(if modern {
            div()
                .border_3()
                .border_color(rgba(0x00000000))
                .child(dpad)
                .into_any_element()
        } else {
            FlickerFrame {
                frame: dpad,
                started: self.analog_gamepad.mapping.keypad_flicker[1],
            }
            .into_any_element()
        });
        let columns = h_flex()
            .items_start()
            .justify_center()
            .flex_wrap()
            .w(surface::css(if modern { 1040. } else { 810. }))
            .mx_auto()
            .min_h(surface::css(450.))
            .child(
                div()
                    .relative()
                    .w(surface::css(390.))
                    .h(surface::css(450.))
                    .mx(surface::css(30.))
                    .child(
                        div()
                            .when(modern, |image| {
                                // Current parent is 1040px; columns (450+560)
                                // are centered (15px), left column starts45.
                                // Actual SVG/box-img left50/top-25 therefore
                                // corresponds to this retained column's +5.
                                image
                                    .absolute()
                                    .left(surface::css(5.))
                                    .top(surface::css(-25.))
                            })
                            .child(self.keyboard_image_for_surface(true, surface, cx)),
                    ),
            )
            .child(right);
        let config_bounds = self.analog_gamepad.floating.config_bounds.clone();
        // Current PID580 main.6e07ccff.css: div.config-wrapper contains one
        // 810px config-block with both columns. Keep its dot background and
        // measured popup frame at this shared owner, outside the left image.
        surface::config_wrapper()
            .min_h(surface::css(450.))
            .on_prepaint(move |bounds, _, _| config_bounds.set(bounds))
            .child(surface::dot_background(cx))
            .child(columns)
            .into_any_element()
    }
    pub(super) fn keypad_direction_mode(&self) -> u64 {
        // Each current keymapReducer initializes directionMode to BUTTONS.
        self.draft["directionMode"].as_u64().unwrap_or(1)
    }
    fn change_keypad_direction_mode(&mut self, mode: u64, cx: &mut Context<Self>) {
        if self.customize_controls_disabled() {
            return;
        }
        if self.spec.product_id == 747 {
            // Actual setDpadMode changes analog registration before mode.
            self.request_keypad_analog_mode(mode == 3, cx);
        }
        self.draft["directionMode"] = json!(mode);
        self.analog_gamepad
            .mapping
            .button_regions
            .borrow_mut()
            .clear();
        self.request_lighting_command(
            KeyboardLightingCommand::Middleware(json!({
                "type":"ON_SET_DIRECTION_MODE",
                "timerTick":chrono::Utc::now().timestamp_millis(),
                "payload":{"directionMode":mode}
            })),
            cx,
        );
        self.sync_keypad_artwork_runtime(cx);
        // This publishes the local draft. The transport completion/error owner
        // remains separate; no device acknowledgement is manufactured.
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn keypad_edition(&self) -> u32 {
        self.source_artwork
            .observation()
            .map(|row| row.edition_id)
            .unwrap_or(0)
    }
    fn keypad_density_image(&self, role: &'static str, width: f32, height: f32) -> DensityImage {
        DensityImage {
            pid: self.spec.product_id,
            edition: self.keypad_edition(),
            role,
            width,
            height,
        }
    }
    pub(super) fn keypad_mode_control(&self, cx: &Context<Self>) -> AnyElement {
        let mode = self.keypad_direction_mode();
        if self.spec.product_id != 747 {
            let directional = mode == 2;
            return BaseButton::new("keypad-direction-mode")
                .relative()
                .w(surface::css(106.))
                .h(surface::css(27.))
                .p_0()
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .rounded(surface::css(3.))
                .disabled(self.customize_controls_disabled())
                .child(AnimatedModeToggle {
                    directional,
                    hypershift: self.hypershift,
                })
                .child(
                    h_flex().relative().children([
                        div()
                            .w(surface::css(52.))
                            .h(surface::css(25.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(self.keypad_density_image(
                                if directional { "dpad_4_1" } else { "dpad_4" },
                                20.,
                                20.,
                            )),
                        div()
                            .w(surface::css(52.))
                            .h(surface::css(25.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(self.keypad_density_image(
                                if directional { "dpad_8" } else { "dpad_8_1" },
                                20.,
                                20.,
                            )),
                    ]),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.change_keypad_direction_mode(
                        if matches!(this.keypad_direction_mode(), 1 | 3) {
                            2
                        } else {
                            1
                        },
                        cx,
                    )
                }))
                .into_any_element();
        }
        let metadata = source(self.spec.product_id).expect("actual current keypad");
        h_flex()
            .h(surface::css(36.))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(36.))
            .children(
                [
                    (3, "analog", "TRUE_ANALOG"),
                    (1, "four", "FOUR_WAY_DPAD"),
                    (2, "eight", "EIGHT_WAY_DPAD"),
                ]
                .into_iter()
                .map(|(value, role, label)| {
                    let index = match value {
                        3 => 0,
                        1 => 1,
                        _ => 2,
                    };
                    let bounds = self.analog_gamepad.mapping.keypad_mode_bounds[index].clone();
                    let measured_bounds = bounds.clone();
                    let icon = metadata
                        .icons
                        .iter()
                        .find(|icon| icon.role == role)
                        .expect("source inline mode SVG");
                    BaseButton::new(SharedString::from(format!("keypad-direction-mode-{value}")))
                        .h(surface::css(26.))
                        .px(surface::css(10.))
                        .pt(surface::css(6.))
                        .pb(surface::css(5.))
                        .mx(surface::css(5.))
                        .mt(surface::css(4.3))
                        .mb(surface::css(5.))
                        .rounded(surface::css(26.))
                        .bg(if value == mode {
                            Hsla::from(rgb(0x44d62c))
                        } else {
                            Hsla::from(rgba(0x00000000))
                        })
                        .disabled(self.customize_controls_disabled())
                        .accessibility_label(t(label))
                        .on_prepaint(move |rect, _, _| measured_bounds.set(rect))
                        .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                            let state = &mut this.analog_gamepad.mapping;
                            if *entered {
                                state.keypad_mode_hover = Some(value);
                            } else if state.keypad_mode_hover == Some(value) {
                                state.keypad_mode_hover = None;
                            }
                            cx.notify();
                        }))
                        .child(
                            razer_assets::resource_svg()
                                .path(icon.asset.clone())
                                .size(surface::css(20.))
                                .text_color(rgb(if value == mode { 0x111111 } else { 0xcccccc })),
                        )
                        .child(ModeTip {
                            value,
                            label,
                            shown: self.analog_gamepad.mapping.keypad_mode_hover == Some(value),
                            trigger: bounds,
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_keypad_direction_mode(value, cx)
                        }))
                }),
            )
            .into_any_element()
    }
}

fn paint_circle(
    window: &mut Window,
    center: Point<Pixels>,
    radius: Pixels,
    stroke: Option<Pixels>,
    color: Hsla,
) {
    let mut path = stroke
        .map(PathBuilder::stroke)
        .unwrap_or_else(PathBuilder::fill);
    path.move_to(point(center.x - radius, center.y));
    path.arc_to(
        point(radius, radius),
        px(0.),
        false,
        true,
        point(center.x + radius, center.y),
    );
    path.arc_to(
        point(radius, radius),
        px(0.),
        false,
        true,
        point(center.x - radius, center.y),
    );
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod product_image_runtime_tests {
    use super::*;

    fn key(product_id: u32, edition_id: u32, layout_id: u32) -> ProductImageKey {
        ProductImageKey {
            product_id,
            edition_id,
            layout_id,
        }
    }

    fn image() -> Arc<RenderImage> {
        Arc::new(RenderImage::new(Vec::<image::Frame>::new()))
    }

    fn requests(name: &str) -> [Option<String>; 2] {
        [Some(format!("{name}-1x")), Some(format!("{name}-3x"))]
    }

    #[::core::prelude::v1::test]
    fn keypad_product_mount_waits_for_observers_and_resumes_only_for_the_active_page() {
        let mut runtime = KeypadRuntime::default();
        let surface = KeypadSurface::Product;
        assert!(runtime.set_active(true));
        assert!(!runtime.customize_mounted(surface, "TAB_CUSTOMIZE"));
        // Deactivation can precede the deferred construction callback.
        assert!(runtime.set_active(false));
        runtime.observers_installed();
        assert!(!runtime.customize_mounted(surface, "TAB_CUSTOMIZE"));

        assert!(runtime.set_active(true));
        assert!(runtime.customize_mounted(surface, "TAB_CUSTOMIZE"));
        assert!(!runtime.customize_mounted(surface, "TAB_LIGHTING"));

        assert!(runtime.set_active(false));
        assert!(!runtime.customize_mounted(surface, "TAB_CUSTOMIZE"));
        assert!(runtime.set_active(true));
        assert!(runtime.customize_mounted(surface, "TAB_CUSTOMIZE"));
    }

    #[::core::prelude::v1::test]
    fn armory_mount_is_independent_of_the_product_page_and_closing_another_root() {
        let mut runtime = KeypadRuntime::default();
        let first = EntityId::from(1);
        let second = EntityId::from(2);
        let first_surface = KeypadSurface::Armory(first);
        let second_surface = KeypadSurface::Armory(second);
        assert!(runtime.set_armory_open(first, true));
        assert!(!runtime.customize_mounted(first_surface, "TAB_LIGHTING"));
        runtime.observers_installed();
        assert!(runtime.customize_mounted(first_surface, "TAB_LIGHTING"));
        assert!(!runtime.customize_mounted(KeypadSurface::Product, "TAB_CUSTOMIZE"));
        assert!(runtime.set_armory_open(second, true));
        assert!(!runtime.set_armory_open(second, true));

        let mut first_image = ProductImageRuntime::default();
        first_image.begin_effect(key(747, 0, 0), requests("first"));
        runtime.images.insert(first_surface, first_image);
        let mut second_image = ProductImageRuntime::default();
        second_image.begin_effect(key(747, 0, 0), requests("second"));
        let decoded = image();
        let retained = Arc::downgrade(&decoded);
        second_image.record_probe(0, Ok(decoded));
        runtime.images.insert(second_surface, second_image);

        assert!(runtime.set_armory_open(second, false));
        assert!(retained.upgrade().is_none());
        assert!(!runtime.customize_mounted(second_surface, "TAB_CUSTOMIZE"));
        assert!(runtime.customize_mounted(first_surface, "TAB_LIGHTING"));
        assert!(runtime.images.contains_key(&first_surface));
        assert!(!runtime.images.contains_key(&second_surface));
        assert!(!runtime.set_armory_open(second, false));
        assert!(runtime.set_active(true));
        assert!(runtime.customize_mounted(KeypadSurface::Product, "TAB_CUSTOMIZE"));
        assert!(runtime.set_active(false));
        assert!(runtime.customize_mounted(first_surface, "TAB_LIGHTING"));
        assert!(runtime.set_armory_open(first, false));
        assert!(runtime.images.is_empty());
    }

    #[::core::prelude::v1::test]
    fn probe_result_replaces_stale_decode_and_error_state() {
        let mut state = ProductImageRuntime::default();
        state.errors[0] = Some("previous decode error".into());
        let image = Arc::new(RenderImage::new(Vec::<image::Frame>::new()));

        state.record_probe(0, Ok(image.clone()));
        assert!(state.completed[0]);
        assert!(state.errors[0].is_none());
        assert!(
            state.decoded[0]
                .as_ref()
                .is_some_and(|value| Arc::ptr_eq(value, &image))
        );

        state.record_probe(0, Err("current decode error".into()));
        assert!(state.completed[0]);
        assert!(state.decoded[0].is_none());
        assert_eq!(state.errors[0].as_deref(), Some("current decode error"));
    }

    #[::core::prelude::v1::test]
    fn product_image_unmount_releases_both_density_decodes_and_keeps_session_sequence() {
        let mut state = ProductImageRuntime::default();
        let key = key(555, 0, 1);
        state.begin_effect(key, requests("old"));
        let first = image();
        let last = Arc::downgrade(&first);
        state.record_probe(0, Ok(first));
        state.record_probe(1, Ok(image()));
        assert!(state.publish_effect());
        state.sequence = 1;
        state.sessions.insert(key, state.sequence);

        state.unmount();
        assert!(
            last.upgrade().is_none(),
            "unmounted ProductImage cannot retain decoded pixels"
        );
        assert!(state.key.is_none());
        assert!(state.pending.iter().all(Option::is_none));
        assert!(state.decoded.iter().all(Option::is_none));
        assert!(state.published.iter().all(Option::is_none));
        assert!(state.errors.iter().all(Option::is_none));
        assert!(!state.loading);
        assert!(!state.clear_loading_on_frame);
        assert!(!state.publish_effect());
        state.unmount();

        state.begin_effect(key, requests("returning"));
        assert_eq!(state.sequence, 1);
        assert_eq!(state.key, Some(key));
        assert!(state.loading);
    }

    #[::core::prelude::v1::test]
    fn product_image_new_props_preserve_old_publication_until_both_current_probes_finish() {
        let mut state = ProductImageRuntime::default();
        state.begin_effect(key(580, 0, 1), requests("old"));
        let old = image();
        state.record_probe(0, Ok(old.clone()));
        state.record_probe(1, Ok(image()));
        assert!(state.publish_effect());

        state.begin_effect(key(580, 128, 4), requests("current"));
        assert!(Arc::ptr_eq(state.published[0].as_ref().unwrap(), &old));
        let current = image();
        state.record_probe(0, Ok(current.clone()));
        assert!(!state.publish_effect());
        assert!(Arc::ptr_eq(state.published[0].as_ref().unwrap(), &old));
        state.record_probe(1, Err("3x decoder failed".into()));
        assert!(state.publish_effect());
        assert!(Arc::ptr_eq(state.published[0].as_ref().unwrap(), &current));
        assert!(state.published[1].is_none());
        assert_eq!(state.errors[1].as_deref(), Some("3x decoder failed"));
        assert!(!state.publish_effect());
    }

    #[::core::prelude::v1::test]
    fn product_image_retry_republishes_the_current_effect_without_resetting_its_session() {
        let mut state = ProductImageRuntime::default();
        let key = key(747, 0, 0);
        state.begin_effect(key, requests("current"));
        state.record_probe(0, Err("SVG decoder failed".into()));
        state.record_probe(1, Err("SVG decoder failed".into()));
        assert!(state.publish_effect());
        assert!(
            !state.loading,
            "747 ProductImage has no body loading effect"
        );
        state.sequence = 1;
        state.sessions.insert(key, 1);

        state.probe_pending(0);
        assert!(!state.publish_effect());
        let retried = image();
        state.record_probe(0, Ok(retried.clone()));
        state.record_probe(1, Ok(retried.clone()));
        assert!(state.publish_effect());
        assert!(state.errors.iter().all(Option::is_none));
        assert!(Arc::ptr_eq(state.published[0].as_ref().unwrap(), &retried));
        assert!(Arc::ptr_eq(state.published[1].as_ref().unwrap(), &retried));
        assert_eq!(state.sequence, 1);
        assert_eq!(state.sessions.get(&key), Some(&1));
        assert_eq!(state.key, Some(key));
        assert_eq!(state.pending, requests("current"));
    }

    #[::core::prelude::v1::test]
    fn packaged_keypad_image_aliases_round_trip_through_typed_lookup() {
        let images = product_images();
        assert!(!images.is_empty());
        for image in images {
            assert!(image.asset.starts_with("synapse/"));
            assert_eq!(
                source_keypad_product_image(image.product_id, image.edition_id, image.density),
                Some(image.asset.as_str())
            );
        }
    }
}

// Generated catalog reader: razer-pages/keyboard_keypad_artwork_data.
fn read_keyboard_keypad_artwork_catalog() -> Vec<Product> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Product>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Product> {
        let value = match r.tag()? {
            0 => Product {
                product_id: r.child(decode_2)?,
                images: r.child(decode_3)?,
                icons: r.child(decode_6)?,
                line_parameters: r.child(decode_7)?,
                line_points: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Image>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Image> {
        let value = match r.tag()? {
            0 => Image {
                edition_id: r.child(decode_2)?,
                role: r.child(decode_5)?,
                asset: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Icon>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<LinePoint>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<LinePoint> {
        let value = match r.tag()? {
            0 => LinePoint {
                key: r.child(decode_5)?,
                x: r.child(decode_10)?,
                y: r.child(decode_10)?,
                kind: r.child(decode_5)?,
                direction_mode: r.child(decode_11)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Icon> {
        let value = match r.tag()? {
            0 => Icon {
                role: r.child(decode_5)?,
                asset: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_KEYPAD_ARTWORK_DATA,
        0,
        "3a104c700be00112356219b23e6ccaacaabf7cf442cd0f41b74945122e9e262b",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_keypad_artwork_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_keypad_artwork_data_decodes() {
        let _ = super::read_keyboard_keypad_artwork_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_keypad_artwork_data.

// Generated catalog reader: razer-pages/keyboard_keypad_device_image_data.
fn read_keyboard_keypad_device_image_catalog() -> Vec<ProductImage> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<ProductImage>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<ProductImage> {
        let value = match r.tag()? {
            0 => ProductImage {
                product_id: r.child(decode_2)?,
                edition_id: r.child(decode_2)?,
                density: r.child(decode_2)?,
                asset: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_KEYPAD_DEVICE_IMAGE_DATA,
        0,
        "3016eb983753e3aac372c2c7fb106e2e97fb3fbc71cba53208b766d96a5dcf7b",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_keypad_device_image_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_keypad_device_image_data_decodes() {
        let _ = super::read_keyboard_keypad_device_image_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_keypad_device_image_data.
