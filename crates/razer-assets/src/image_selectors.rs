//! Immutable image selection metadata; media bytes belong to the resource cache.
use crate::{DeviceImage, GamepadCalibrationSvg};
use anyhow::ensure;
use std::sync::OnceLock;

pub(super) struct ImageVariant {
    pub product_id: u32,
    pub edition_id: u32,
    pub layout_id: u32,
    pub asset: String,
}

pub(super) struct ProductImage {
    pub variant: ImageVariant,
    pub part: DeviceImage,
}

pub(super) struct DashboardPluginImage {
    pub variant: ImageVariant,
    pub filename_edition_id: u32,
    pub wrist: bool,
    pub density: u32,
}

pub(super) struct CalibrationSvg {
    pub product_id: u32,
    pub edition_id: u32,
    pub kind: GamepadCalibrationSvg,
    pub asset: String,
}

pub(super) struct Selectors {
    pub products: Vec<ProductImage>,
    pub dashboard: Vec<ImageVariant>,
    pub dashboard_plugins: Vec<DashboardPluginImage>,
    pub pairing: Vec<ImageVariant>,
    pub calibration: Vec<CalibrationSvg>,
}

fn snapshot() -> anyhow::Result<&'static Selectors> {
    static SNAPSHOT: OnceLock<Result<Selectors, String>> = OnceLock::new();
    SNAPSHOT
        .get_or_init(|| {
            razer_resource_db::read_catalog(|connection| {
                let mut products = Vec::new();
                let mut statement =
                    connection.prepare("SELECT * FROM product_image_selector ORDER BY position")?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    ensure!(
                        row.get::<_, usize>(0)? == products.len(),
                        "Invalid product selector order"
                    );
                    let part: String = row.get(4)?;
                    let part = match part.as_str() {
                        "Product" => DeviceImage::Product,
                        "MouseBottom" => DeviceImage::MouseBottom,
                        "KeyboardWrist" => DeviceImage::KeyboardWrist,
                        "KeyboardDial" => DeviceImage::KeyboardDial,
                        "Streamer" => DeviceImage::Streamer,
                        _ => anyhow::bail!("Unknown product image part"),
                    };
                    products.push(ProductImage {
                        variant: ImageVariant {
                            product_id: row.get(1)?,
                            edition_id: row.get(2)?,
                            layout_id: row.get(3)?,
                            asset: row.get(5)?,
                        },
                        part,
                    });
                }
                let mut dashboard = Vec::new();
                let mut statement = connection
                    .prepare("SELECT * FROM dashboard_image_selector ORDER BY position")?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    ensure!(
                        row.get::<_, usize>(0)? == dashboard.len(),
                        "Invalid dashboard selector order"
                    );
                    dashboard.push(ImageVariant {
                        product_id: row.get(1)?,
                        edition_id: row.get(2)?,
                        layout_id: row.get(3)?,
                        asset: row.get(4)?,
                    });
                }
                let mut dashboard_plugins = Vec::new();
                let mut statement = connection
                    .prepare("SELECT * FROM dashboard_plugin_image_selector ORDER BY position")?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    ensure!(
                        row.get::<_, usize>(0)? == dashboard_plugins.len(),
                        "Invalid plugin image selector order"
                    );
                    let wrist: u32 = row.get(5)?;
                    let density: u32 = row.get(6)?;
                    ensure!(
                        wrist <= 1 && matches!(density, 1 | 3),
                        "Invalid plugin image variant"
                    );
                    dashboard_plugins.push(DashboardPluginImage {
                        variant: ImageVariant {
                            product_id: row.get(1)?,
                            edition_id: row.get(2)?,
                            layout_id: row.get(4)?,
                            asset: row.get(7)?,
                        },
                        filename_edition_id: row.get(3)?,
                        wrist: wrist == 1,
                        density,
                    });
                }
                let mut pairing = Vec::new();
                let mut statement =
                    connection.prepare("SELECT * FROM pairing_image_selector ORDER BY position")?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    ensure!(
                        row.get::<_, usize>(0)? == pairing.len(),
                        "Invalid pairing selector order"
                    );
                    pairing.push(ImageVariant {
                        product_id: row.get(1)?,
                        edition_id: row.get(2)?,
                        layout_id: row.get(3)?,
                        asset: row.get(4)?,
                    });
                }
                let mut calibration = Vec::new();
                let mut statement = connection
                    .prepare("SELECT * FROM gamepad_calibration_svg_selector ORDER BY position")?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    ensure!(
                        row.get::<_, usize>(0)? == calibration.len(),
                        "Invalid calibration selector order"
                    );
                    let kind: String = row.get(3)?;
                    let kind = match kind.as_str() {
                        "prd" => GamepadCalibrationSvg::Product,
                        "ActuationDevice" => GamepadCalibrationSvg::ActuationDevice,
                        _ => anyhow::bail!("Unknown calibration SVG kind"),
                    };
                    calibration.push(CalibrationSvg {
                        product_id: row.get(1)?,
                        edition_id: row.get(2)?,
                        kind,
                        asset: row.get(4)?,
                    });
                }
                ensure!(
                    !products.is_empty()
                        && !dashboard.is_empty()
                        && !dashboard_plugins.is_empty()
                        && !pairing.is_empty()
                        && !calibration.is_empty(),
                    "Packaged image selectors are missing"
                );
                Ok(Selectors {
                    products,
                    dashboard,
                    dashboard_plugins,
                    pairing,
                    calibration,
                })
            })
            .map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

pub(super) fn validate() -> anyhow::Result<()> {
    snapshot().map(|_| ())
}

pub(super) fn get() -> &'static Selectors {
    snapshot().expect("Packaged image selectors must be validated before constructing views")
}
