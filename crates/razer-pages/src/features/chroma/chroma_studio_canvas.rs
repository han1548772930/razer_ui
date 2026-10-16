//! Current vt.renderGridLines / ke SVG geometry; only the move tool shows it.
use super::theme::Colors;
use gpui_kit::*;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Grid {
    rects: Vec<Rect>,
}
#[derive(Deserialize)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    major: bool,
}
fn grid() -> &'static Grid {
    static GRID: OnceLock<Grid> = OnceLock::new();
    GRID.get_or_init(|| read_chroma_studio_grid_catalog())
}
pub(super) fn render(zoom: f32, moving: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            if !moving {
                return;
            }
            let scale = f32::from(window.rem_size()) / 16.;
            let unit = zoom * scale;
            let thickness = (2. / zoom).ceil().max(2.);
            // The source canvas is 10000 x 10000, initially centered at 5000.
            let center = bounds.center();
            let left = 5000. - f32::from(bounds.size.width) / (2. * unit);
            let top = 5000. - f32::from(bounds.size.height) / (2. * unit);
            let columns = (f32::from(bounds.size.width) / (480. * unit)).ceil() as i32 + 2;
            let rows = (f32::from(bounds.size.height) / (480. * unit)).ceil() as i32 + 2;
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                for tile_y in 0..rows {
                    for tile_x in 0..columns {
                        let x = ((left / 480.).floor() + tile_x as f32) * 480.;
                        let y = ((top / 480.).floor() + tile_y as f32) * 480.;
                        for rect in &grid().rects {
                            if !rect.major && zoom <= 0.5 {
                                continue;
                            }
                            let origin = center
                                + point(
                                    px((x + rect.x - 5000.) * unit),
                                    px((y + rect.y - 5000.) * unit),
                                );
                            window.paint_quad(quad(
                                Bounds::new(
                                    origin,
                                    size(px(rect.width * unit), px(rect.height * unit)),
                                ),
                                px(0.),
                                Hsla::transparent_black(),
                                px(thickness / 2. * unit),
                                if rect.major {
                                    Colors::major_grid()
                                } else {
                                    Colors::grid()
                                },
                                BorderStyle::Solid,
                            ));
                        }
                    }
                }
                for line in [
                    Bounds::new(
                        point(bounds.left(), center.y),
                        size(bounds.size.width, px(thickness * unit)),
                    ),
                    Bounds::new(
                        point(center.x, bounds.top()),
                        size(px(thickness * unit), bounds.size.height),
                    ),
                ] {
                    window.paint_quad(quad(
                        line,
                        px(0.),
                        Colors::center_line(),
                        px(0.),
                        Colors::center_line(),
                        BorderStyle::Solid,
                    ));
                }
            });
        },
    )
    .size_full()
}

// Generated catalog reader: razer-pages/chroma_studio_grid.
fn read_chroma_studio_grid_catalog() -> Grid {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Grid> {
        let value = match r.tag()? {
            0 => Grid {
                rects: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Rect>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Rect> {
        let value = match r.tag()? {
            0 => Rect {
                x: r.child(decode_3)?,
                y: r.child(decode_3)?,
                width: r.child(decode_3)?,
                height: r.child(decode_3)?,
                major: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_CHROMA_STUDIO_GRID,
        0,
        "2f652610e4b58b3de9cbb56f0b96beb649735fac2aeb3b073a4eb0e67adae1de",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_chroma_studio_grid_decodes() {
    let _ = read_chroma_studio_grid_catalog();
}
// End generated catalog reader: razer-pages/chroma_studio_grid.
