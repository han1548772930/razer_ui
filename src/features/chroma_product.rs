//! Product-side `displayMode=chromaApp` root.
//!
//! The current bundles select this root in the same ternary chain that reads
//! `searchParams.get("displayMode")` and only when the requested serial number
//! matches the running device (`"chromaApp"===a&&_===this.state.serialNumber`),
//! which is why the Chroma application opens it as a per-device popup. The root
//! mounts the product's lighting content without the normal product navigation
//! and profile chrome, and Escape asks the parent window to close it
//! (`window.postMessage("closePopup","*")`). See
//! [display-mode audit](../../docs/re/display-mode-audit.md),
//! [window contract](../../docs/re/display-window-contract.md) and
//! [Chroma application evidence](../../docs/re/chroma-app-current-evidence.json).
//!
//! The product id table is generated from the audited static scan by
//! `tools/generate-display-mode-roots.cjs`; the branch content stays a product
//! page, so nothing is fabricated when a product has no local lighting page.
use super::{
    ProductWorkspace,
    display_mode_roots::{DisplayModeRoot, has_root_branch},
    settings::Effect,
};
use crate::ui::scroll::SourceScrollable as _;
use gpui_kit::*;

/// Whether the current product bundle mounts a root-level `chromaApp` branch.
pub(crate) fn has_chroma_app_root(product_id: u32) -> bool {
    has_root_branch(DisplayModeRoot::ChromaApp, product_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChromaPreset {
    Spectrum,
    StaticGreen,
    BreathingGreen,
    Wave,
    Fire,
    Starlight,
    Wheel,
}
impl ChromaPreset {
    pub(crate) const ALL: [Self; 7] = [
        Self::Spectrum,
        Self::StaticGreen,
        Self::BreathingGreen,
        Self::Wave,
        Self::Fire,
        Self::Starlight,
        Self::Wheel,
    ];
    pub(crate) fn key(self) -> &'static str {
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
    pub(crate) fn supported(self, workspace: &ProductWorkspace, cx: &App) -> bool {
        workspace
            .chroma_lighting_workspace(cx)
            .is_some_and(|entity| {
                let device = entity.read(cx).device();
                Effect::list(device.product_id, device.use_ble, device.use_ble)
                    .contains(&self.effect())
            })
    }
    pub(crate) fn apply(self, workspace: &ProductWorkspace, window: &mut Window, cx: &mut App) {
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

pub(crate) struct ChromaLightingSnapshot {
    pub(crate) brightness: u8,
    pub(crate) enabled: bool,
    pub(crate) effect: String,
}
pub(crate) fn lighting_snapshot(
    workspace: &ProductWorkspace,
    cx: &App,
) -> Option<ChromaLightingSnapshot> {
    let entity = workspace.chroma_lighting_workspace(cx)?;
    let lighting = &entity.read(cx).settings().lighting;
    Some(ChromaLightingSnapshot {
        brightness: lighting.brightness,
        enabled: lighting.enabled,
        effect: if lighting.advanced {
            crate::i18n::t("ADVANCED_EFFECTS")
        } else {
            lighting.effect.label()
        },
    })
}

/// The popup root: the product's lighting content, addressed per device.
pub(crate) struct ChromaProduct {
    workspace: Entity<ProductWorkspace>,
    _subscription: Subscription,
}
impl ChromaProduct {
    pub(crate) fn new(
        workspace: &Entity<ProductWorkspace>,
        cx: &mut Context<Self>,
    ) -> Option<Self> {
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
