//! Current 653 lg -> rg/_g -> Fh/yh; it mounts the existing brightness,
//! switch-off and effects children, without the normal product chrome.
//! See docs/re/chroma-app-current-evidence.json. Other roots stay unclaimed.
use super::{DeviceWorkspace, ProductWorkspace, settings::Effect};
use crate::ui::scroll::SourceScrollable as _;
use gpui_kit::*;

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

pub(crate) struct ChromaProduct {
    workspace: Entity<DeviceWorkspace>,
    _subscription: Subscription,
}
impl ChromaProduct {
    pub(crate) fn new(workspace: &ProductWorkspace, cx: &mut Context<Self>) -> Option<Self> {
        let workspace = workspace.chroma_lighting_workspace(cx)?;
        let subscription = cx.observe(&workspace, |_, _, cx| cx.notify());
        Some(Self {
            workspace,
            _subscription: subscription,
        })
    }
}
impl Render for ChromaProduct {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .workspace
            .update(cx, |workspace, cx| workspace.lighting_page(cx));
        div()
            .id("chroma-product-lighting")
            .size_full()
            .min_h_0()
            .scrollable_y()
            .child(content)
    }
}
