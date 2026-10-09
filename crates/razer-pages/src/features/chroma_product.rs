//! Product-side `displayMode=chromaApp` root.
//!
//! The current bundles select this root in the same ternary chain that reads
//! `searchParams.get("displayMode")`. The serial predicate is product-specific:
//! 1303 compares with the running device; 1313 accepts a nonempty requested
//! serial. The local Chroma application addresses the selected device workspace
//! directly, so each popup retains that device's own profile state. The root
//! mounts the product's lighting content without the normal product navigation
//! and profile chrome, and Escape asks the parent window to close it
//! (`window.postMessage("closePopup","*")`). See
//! [display-mode audit](../../docs/re/shell-workspace-current.md),
//! [window contract](../../docs/re/display-window-contract.md) and
//! [Chroma application evidence](../../docs/re/chroma-app-current-evidence.json).
//! Audio root-to-page identity and differing body width overrides are recorded
//! in [the workspace contract](../../docs/re/shell-workspace-current.md).
//!
//! The product id table is generated from the audited static scan by
//! `tools/generate-display-mode-roots.cjs`; the branch content stays a product
//! page, so nothing is fabricated when a product has no local lighting page.
use super::{
    ProductWorkspace,
    display_mode_roots::{DisplayModeRoot, has_root_branch},
    settings::Effect,
};
use gpui_kit::*;
use razer_widgets::scroll::SourceScrollable as _;

/// Whether the current product bundle mounts a root-level `chromaApp` branch.
pub fn has_chroma_app_root(product_id: u32) -> bool {
    has_root_branch(DisplayModeRoot::ChromaApp, product_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromaPreset {
    Spectrum,
    StaticGreen,
    BreathingGreen,
    Wave,
    Fire,
    Starlight,
    Wheel,
}
impl ChromaPreset {
    pub const ALL: [Self; 7] = [
        Self::Spectrum,
        Self::StaticGreen,
        Self::BreathingGreen,
        Self::Wave,
        Self::Fire,
        Self::Starlight,
        Self::Wheel,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::Spectrum => "SPECTRUM_CYCLING",
            Self::StaticGreen => "STATIC_GREEN",
            Self::BreathingGreen => "BREATHING_GREEN",
            Self::Wave => "WAVE",
            Self::Fire => "FIRE",
            Self::Starlight => "STARLIGHT",
            Self::Wheel => "WHEEL",
        }
    }
    fn effect(self) -> Effect {
        match self {
            Self::Spectrum => Effect::Spectrum,
            Self::StaticGreen => Effect::Static,
            Self::BreathingGreen => Effect::Breathing,
            Self::Wave => Effect::Wave,
            Self::Fire => Effect::Fire,
            Self::Starlight => Effect::Starlight,
            Self::Wheel => Effect::Wheel,
        }
    }
    /// The quick-effect buttons write the shared lighting settings, which only
    /// the local editing path exposes. Products whose lighting page is a
    /// generated source page keep the preset disabled instead of pretending a
    /// write happened.
    pub fn supported(self, workspace: &ProductWorkspace, cx: &App) -> bool {
        workspace
            .chroma_lighting_workspace(cx)
            .is_some_and(|entity| {
                let device = entity.read(cx).device();
                Effect::list(device.product_id, device.use_ble, device.use_ble)
                    .contains(&self.effect())
            })
    }
    /// 62296/bs only marks a quick effect active when the actual lighting
    /// signature agrees across all device zones; a click alone is not proof.
    pub fn matches(self, workspace: &ProductWorkspace, cx: &App) -> bool {
        let Some(entity) = workspace.chroma_lighting_workspace(cx) else {
            return false;
        };
        let lighting = &entity.read(cx).settings().lighting;
        if lighting.advanced || lighting.effect != self.effect() {
            return false;
        }
        let parameters = lighting.params();
        match self {
            Self::Spectrum | Self::Fire => true,
            Self::StaticGreen => parameters.color1 == Some([0, 255, 0]),
            Self::BreathingGreen | Self::Starlight => {
                parameters.color1 == Some([0, 255, 0])
                    && parameters.color2.is_none()
                    && !parameters.random
                    && (self != Self::Starlight || parameters.duration == 2)
            }
            Self::Wave => parameters.direction == 2,
            Self::Wheel => parameters.direction == 1,
        }
    }
    pub fn apply(self, workspace: &ProductWorkspace, window: &mut Window, cx: &mut App) {
        if !self.supported(workspace, cx) {
            return;
        }
        let Some(entity) = workspace.chroma_lighting_workspace(cx) else {
            return;
        };
        entity.update(cx, |view, cx| {
            view.edit(window, cx, |settings| {
                let lighting = &mut settings.lighting;
                lighting.advanced = false;
                lighting.effect = self.effect();
                let parameters = lighting.params_mut();
                match self {
                    Self::StaticGreen | Self::BreathingGreen | Self::Starlight => {
                        parameters.color1 = Some([0, 255, 0]);
                        if self != Self::StaticGreen {
                            parameters.color2 = None;
                            parameters.random = false;
                        }
                        if self == Self::Starlight {
                            parameters.duration = 2;
                        }
                    }
                    Self::Wave => parameters.direction = 2,
                    Self::Wheel => parameters.direction = 1,
                    Self::Spectrum | Self::Fire => {}
                }
            })
        });
    }
}

pub struct ChromaLightingSnapshot {
    pub brightness: u8,
    pub enabled: bool,
    pub effect: String,
}
pub fn lighting_snapshot(workspace: &ProductWorkspace, cx: &App) -> Option<ChromaLightingSnapshot> {
    let entity = workspace.chroma_lighting_workspace(cx)?;
    let lighting = &entity.read(cx).settings().lighting;
    Some(ChromaLightingSnapshot {
        brightness: lighting.brightness,
        enabled: lighting.enabled,
        effect: if lighting.advanced {
            razer_i18n::t("ADVANCED_EFFECTS")
        } else {
            lighting.effect.label()
        },
    })
}

/// The popup root: the product's lighting content, addressed per device.
pub struct ChromaProduct {
    workspace: Entity<ProductWorkspace>,
    _subscription: Subscription,
}
impl ChromaProduct {
    pub fn new(workspace: &Entity<ProductWorkspace>, cx: &mut Context<Self>) -> Option<Self> {
        if !workspace.read(cx).has_chroma_device_page(cx) {
            return None;
        }
        let subscription = cx.observe(workspace, |_, _, cx| cx.notify());
        Some(Self {
            workspace: workspace.clone(),
            _subscription: subscription,
        })
    }
}
impl Render for ChromaProduct {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .workspace
            .update(cx, |workspace, cx| workspace.chroma_device_page(window, cx));
        // The source popup overrides the shared width rule with
        // `.body-wrapper, .main-container{ min-width: unset; }`.
        let view = div()
            .id("chroma-product-lighting")
            .size_full()
            .min_h_0()
            .min_w_0()
            .scrollable_y();
        match content {
            Some(content) => view.child(content),
            None => view,
        }
    }
}

// Receipts for the generated `displayMode` table live in
// `src/features/display_mode_roots_tests.rs`; this module only consumes it.
