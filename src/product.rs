//! Product differences verified against each cached Synapse module 8193.
//! These entries describe the UI contract, not detected hardware capabilities.

mod registry;
pub(crate) use registry::{ProductPage, ProductPageId, ProductPageRole, registered, registry};

pub(crate) const AUDITED_MOUSE_MAT_IDS: [u32; 7] = [3072, 3073, 3074, 3076, 3077, 3078, 3080];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseMatWaveDirection {
    ClockwiseCounterclockwise,
    LeftRight,
}

impl MouseMatWaveDirection {
    // Module 9228: hz = { CWCCW: 12, UPDOWN: 4, LEFTRIGHT: 2 }.
    pub(crate) fn default_value(self) -> u8 {
        match self {
            Self::ClockwiseCounterclockwise => 12,
            Self::LeftRight => 2,
        }
    }
}

pub(crate) struct MouseMat {
    id: u32,
    name: &'static str,
    brightness: u8,
    support_url: &'static str,
    guide_prefix: &'static str,
    wave_direction: Option<MouseMatWaveDirection>,
}

impl MouseMat {
    pub(crate) fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn brightness(&self) -> u8 {
        self.brightness
    }

    pub(crate) fn support_url(&self) -> &'static str {
        self.support_url
    }

    pub(crate) fn guide_prefix(&self) -> &'static str {
        self.guide_prefix
    }

    pub(crate) fn wave_direction(&self) -> Option<MouseMatWaveDirection> {
        self.wave_direction
    }
}

static MOUSE_MATS: [MouseMat; 7] = [
    MouseMat {
        id: 3072,
        name: "Razer Firefly Hard Edition",
        brightness: 100,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/3665/",
        guide_prefix: "http://dl.razerzone.com/master-guides/RazerSynapse3/FireFly-00003072-",
        wave_direction: Some(MouseMatWaveDirection::ClockwiseCounterclockwise),
    },
    MouseMat {
        id: 3073,
        name: "Razer Goliathus Chroma",
        brightness: 100,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/3728/",
        guide_prefix: "http://dl.razer.com/master-guides/RazerSynapse3/GoliathusChroma-00003073-",
        wave_direction: None,
    },
    MouseMat {
        id: 3074,
        name: "Razer Goliathus Extended Chroma",
        brightness: 100,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/3754/kw/Razer%20Goliathus%20Extended%20Chroma",
        guide_prefix: "http://dl.razer.com/master-guides/RazerSynapse3/GoliathusChromaExtended-00003074-",
        wave_direction: None,
    },
    MouseMat {
        id: 3076,
        name: "Razer Firefly V2",
        brightness: 66,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/3668/",
        guide_prefix: "http://dl.razer.com/master-guides/RazerSynapse3/RAZERFIREFLYV2-00003076-",
        wave_direction: Some(MouseMatWaveDirection::ClockwiseCounterclockwise),
    },
    MouseMat {
        id: 3077,
        name: "Razer Strider Chroma",
        brightness: 66,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/6191/kw/strider",
        guide_prefix: "https://dl.razerzone.com/master-guides/RazerSynapse3/STRIDERCHROMA-00003077-",
        wave_direction: Some(MouseMatWaveDirection::ClockwiseCounterclockwise),
    },
    MouseMat {
        id: 3078,
        name: "Razer Goliathus Chroma 3XL",
        brightness: 100,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/6189/kw/goliathus%203xl",
        guide_prefix: "http://dl.razer.com/master-guides/RazerSynapse3/RAZERGOLIATHUSCHROMA3XL-00003078-",
        wave_direction: None,
    },
    MouseMat {
        id: 3080,
        name: "Razer Firefly V2 Pro",
        brightness: 66,
        support_url: "https://mysupport.razer.com/app/answers/detail/a_id/14141",
        guide_prefix: "http://dl.razer.com/master-guides/RazerSynapse3/FIREFLYV2PRO-00003080-",
        wave_direction: Some(MouseMatWaveDirection::LeftRight),
    },
];

pub(crate) fn audited_mouse_mat(pid: u32) -> Option<&'static MouseMat> {
    MOUSE_MATS.iter().find(|product| product.id == pid)
}
