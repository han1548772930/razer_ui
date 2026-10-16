//! Current 34340/he descriptive catalog fallback, independent of live identity.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerDescription {
    dongle_id: u32,
    category: String,
    product_name: BTreeMap<String, String>,
}

impl PeerDescription {
    pub fn category(&self) -> &str {
        &self.category
    }
    pub fn product_name(&self) -> &BTreeMap<String, String> {
        &self.product_name
    }
}

pub fn description(dongle_id: u32) -> Option<&'static PeerDescription> {
    static CATALOG: OnceLock<Vec<PeerDescription>> = OnceLock::new();
    CATALOG
        .get_or_init(|| make_receiver_peer_catalog_table())
        .iter()
        .find(|entry| entry.dongle_id == dongle_id)
}

// Generated typed table: razer-device/receiver_peer_catalog.
#[rustfmt::skip]
fn make_receiver_peer_catalog_table() -> Vec<PeerDescription> {
    vec![
        PeerDescription{
            dongle_id: 604u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"RAZER BLACKWIDOW V3 PRO".into()),
                ("zh-cn".into(),"RAZER 黑寡妇蜘蛛V3无线版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 625u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"RAZER BLACKWIDOW V3 MINI HYPERSPEED".into()),
                ("zh-cn".into(),"RAZER 黑寡妇蜘蛛V3迷你无线版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 698u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"RAZER BLACKWIDOW V4 MINI HYPERSPEED".into()),
                ("zh-cn".into(),"RAZER 黑寡妇蜘蛛V4迷你无线版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 144u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER NAGA PRO".into()),
                ("zh-cn".into(),"RAZER 那伽梵蛇专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 168u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER NAGA V2 PRO".into()),
                ("zh-cn".into(),"RAZER 那伽梵蛇专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 171u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER BASILISK V3 PRO".into()),
                ("zh-cn".into(),"RAZER 巴塞利斯蛇 V3 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 212u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER BASILISK MOBILE".into()),
                ("zh-cn".into(),"RAZER 巴塞利斯蛇移动版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 205u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER BASILISK V3 PRO 35K".into()),
                ("zh-cn".into(),"RAZER 巴塞利斯蛇 V3 专业版 35K".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 227u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Basilisk V4 Pro".into()),
                ("zh-cn".into(),"Razer 巴塞利斯蛇 V4 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 234u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Basilisk V4 HyperSpeed".into()),
                ("zh-cn".into(),"Razer 巴塞利斯蛇 V4 极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 215u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Basilisk V3 Pro 35K Phantom Green Edition".into()),
                ("zh-cn".into(),"RAZER 巴塞利斯蛇 V3  专业版 35K 幻影绿".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 45067u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER BASILISK V3 PRO 35K - XBOX 25 ANNIVERSARY EDITION".into()),
                ("zh-cn".into(),"Razer 巴塞利斯蛇V3专业版35K Xbox 25周年限定款".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 125u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHADDER V2 PRO".into()),
                ("zh-cn".into(),"RAZER 炼狱蝰蛇 V2专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 148u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER OROCHI V2".into()),
                ("zh-cn".into(),"RAZER 八岐大蛇 V2".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 232u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER NAGA V3 PRO".into()),
                ("zh-cn".into(),"RAZER 那伽梵蛇 V3 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 166u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER VIPER V2 PRO".into()),
                ("zh-cn".into(),"RAZER 毒蝰V2专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 183u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHADDER V3 PRO".into()),
                ("zh-cn".into(),"RAZER炼狱蝰蛇 V3 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 156u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHADDER V2 X HYPERSPEED".into()),
                ("zh-cn".into(),"RAZER 炼狱蝰蛇 V2 X 极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 154u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER PRO CLICK MINI".into()),
                ("zh-cn".into(),"RAZER PRO CLICK MINI".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 176u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Cobra Pro".into()),
                ("zh-cn".into(),"Razer 眼镜蛇专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 184u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Viper V3 HyperSpeed".into()),
                ("zh-cn".into(),"RAZER毒蝰V3极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 635u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"RAZER PRO TYPE ULTRA".into()),
                ("zh-cn".into(),"RAZER PRO TYPE ULTRA".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 656u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHSTALKER V2 PRO".into()),
                ("zh-cn".into(),"RAZER 噬魂金蝎V2专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 662u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHSTALKER V2 PRO TENKEYLESS".into()),
                ("zh-cn".into(),"RAZER 噬魂金蝎 V2 无线竞技版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 722u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"Razer BlackWidow V4 Low-profile Tenkeyless HyperSpeed".into()),
                ("zh-cn".into(),"RAZER 黑寡妇蜘蛛V4矮轴竞技极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 725u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"Razer BlackWidow V4 Tenkeyless HyperSpeed".into()),
                ("zh-cn".into(),"RAZER 黑寡妇蜘蛛 V4 竞技极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 713u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"Razer BlackWidow V4 Low-profile HyperSpeed".into()),
                ("zh-cn".into(),"Razer 黑寡妇蜘蛛V4矮轴极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 730u32,
            category: "KEYBOARD".into(),
            product_name: vec![
                ("en".into(),"Razer Joro".into()),
                ("zh-cn".into(),"Razer 乔罗金蛛".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 164u32,
            category: "ACCESSORY".into(),
            product_name: vec![
                ("en".into(),"Razer Mouse Dock Pro".into()),
                ("zh-cn".into(),"Razer 幻彩鼠标底座".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 241u32,
            category: "ACCESSORY".into(),
            product_name: vec![
                ("en".into(),"Razer Mouse Dock V2 Pro".into()),
                ("zh-cn".into(),"RAZER 鼠标底座 V2 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 179u32,
            category: "ACCESSORY".into(),
            product_name: vec![
                ("en".into(),"Razer HyperPolling Wireless Dongle".into()),
                ("zh-cn".into(),"RAZER HYPERPOLLING 无线接收器".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 185u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Basilisk V3 X HyperSpeed".into()),
                ("zh-cn".into(),"RAZER巴塞利斯蛇 V3 X 极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 180u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Naga V2 Hyperspeed".into()),
                ("zh-cn".into(),"Razer 那伽梵蛇 V2 极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 197u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHADDER V3 HYPERSPEED".into()),
                ("zh-cn".into(),"RAZER 毒蝰V3 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 191u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"RAZER DEATHADDER V4 PRO".into()),
                ("zh-cn".into(),"RAZER 炼狱蝰蛇 V4 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 193u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Viper V3 Pro".into()),
                ("zh-cn".into(),"RAZER 毒蝰V3 专业版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 207u32,
            category: "MOUSEMAT".into(),
            product_name: vec![
                ("en".into(),"Razer HyperFlux V2 Hard Edition".into()),
                ("zh-cn".into(),"Razer HyperFlux V2 硬质版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 219u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Cobra HyperSpeed".into()),
                ("zh-cn".into(),"Razer 眼镜蛇极速版".into())
            ].into_iter().collect()
        },
        PeerDescription{
            dongle_id: 223u32,
            category: "MOUSE".into(),
            product_name: vec![
                ("en".into(),"Razer Viper V3 Pro SE".into()),
                ("zh-cn".into(),"Razer毒蝰 V3 专业版SE".into())
            ].into_iter().collect()
        }
    ]
}
