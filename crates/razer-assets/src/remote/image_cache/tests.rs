//! In-memory GPUI scheduling and scene checks. No OS window, HTTP or GPU.
use super::*;
use gpui_kit::{
    AssetSource, Context, DevicePixels, HeadlessAppContext, HeadlessAtlas, ImageSource,
    IntoElement, NoopTextSystem, ParentElement as _, PlatformAtlas, PlatformHeadlessRenderer,
    PolychromeSprite, Render, Scene, SharedString, Size, Styled as _, WindowHandle, div, img, px,
    size,
};
use std::{
    borrow::Cow,
    sync::{Mutex, atomic::{AtomicUsize, Ordering}},
};

const KEY: &str = "test-resource-window/cycle.svg";
const CYCLE: &[u8] = include_bytes!("../../../testdata/mouse_customize/cycle-229.svg");

struct MutableAssets {
    bytes: Mutex<Option<&'static [u8]>>,
    loads: AtomicUsize,
}

impl AssetSource for MutableAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        assert_eq!(path, KEY, "the test must not request another product");
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.bytes.lock().unwrap().map(Cow::Borrowed))
    }

    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// Record GPUI's submitted scene; deliberately provides no screenshot pixels.
struct SceneRecorder {
    atlas: Arc<HeadlessAtlas>,
    sprites: Arc<Mutex<Vec<PolychromeSprite>>>,
}

impl PlatformHeadlessRenderer for SceneRecorder {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        _: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        *self.sprites.lock().unwrap() = scene.polychrome_sprites.clone();
        anyhow::bail!("scene recorder does not produce screenshots")
    }

    fn render_scene(&mut self, scene: &Scene, _: Size<DevicePixels>) -> anyhow::Result<()> {
        *self.sprites.lock().unwrap() = scene.polychrome_sprites.clone();
        Ok(())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

struct Fixture;

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let custom = ImageSource::Custom(Arc::new(|window, cx| {
            load_resource_image(Resource::Embedded(KEY.into()), window, cx)
        }));
        resource_scope(
            div().size_full()
                .child(img(KEY).absolute().left(px(0.)).top(px(0.)).size(px(102.)))
                .child(deferred_resource(
                    img(KEY).absolute().left(px(110.)).top(px(0.)).size(px(102.)),
                ))
                .child(img(custom).absolute().left(px(220.)).top(px(0.)).size(px(102.))),
        )
    }
}

fn draw(cx: &mut HeadlessAppContext, window: WindowHandle<Fixture>) {
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let error = window.render_to_image().unwrap_err();
        assert_eq!(error.to_string(), "scene recorder does not produce screenshots");
    }).unwrap();
}

fn test_context(
    bytes: Option<&'static [u8]>,
) -> (HeadlessAppContext, Arc<MutableAssets>, Arc<Mutex<Vec<PolychromeSprite>>>) {
    let assets = Arc::new(MutableAssets {
        bytes: Mutex::new(bytes),
        loads: AtomicUsize::new(0),
    });
    let sprites = Arc::new(Mutex::new(Vec::new()));
    let captured = sprites.clone();
    let cx = HeadlessAppContext::with_platform(
        Arc::new(NoopTextSystem), assets.clone(), move || {
            Ok(Some(Box::new(SceneRecorder {
                atlas: Arc::new(HeadlessAtlas::default()),
                sprites: captured.clone(),
            })))
        },
    );
    (cx, assets, sprites)
}

fn cached_image(cx: &mut HeadlessAppContext, window: WindowHandle<Fixture>) -> Arc<RenderImage> {
    cx.update(|cx| {
        let cache = &cx.global::<WindowResourceCaches>().by_window[&window.window_id()];
        cache.read(cx).images[&Resource::Embedded(KEY.into())]
            .get().expect("decode completed").expect("valid SVG")
    })
}

#[test]
fn normal_deferred_and_custom_images_share_one_decode_and_produce_visible_scene_sprites() {
    let (mut cx, assets, sprites) = test_context(Some(CYCLE));
    let window = cx.open_window(size(px(340.), px(120.)), |_, cx| cx.new(|_| Fixture)).unwrap();
    draw(&mut cx, window);
    cx.run_until_parked();
    draw(&mut cx, window);

    assert_eq!(assets.loads.load(Ordering::SeqCst), 1);
    let image = cached_image(&mut cx, window);
    assert_eq!((image.size(0).width.0, image.size(0).height.0), (204, 204));
    let pixels = image.as_bytes(0).unwrap();
    assert!(pixels.chunks_exact(4).any(|pixel| pixel == [17, 17, 17, 255]));
    assert!(pixels.chunks_exact(4).any(|pixel| pixel == [102, 102, 102, 255]));

    let sprites = sprites.lock().unwrap();
    assert_eq!(sprites.len(), 3, "normal, deferred and custom must all paint");
    for sprite in sprites.iter() {
        assert_eq!(sprite.opacity, 1.);
        assert_eq!(sprite.bounds.size.width.0, 204.);
        assert_eq!(sprite.bounds.size.height.0, 204.);
        assert!(!sprite.bounds.intersect(&sprite.content_mask.bounds).is_empty());
    }
    drop(sprites);

    cx.update(|cx| {
        retry_resource_images(cx);
        cx.refresh_windows();
    });
    draw(&mut cx, window);
    assert!(Arc::ptr_eq(&image, &cached_image(&mut cx, window)));
    assert_eq!(assets.loads.load(Ordering::SeqCst), 1, "success is retained on retry");

    cx.update_window(window.into(), |_, window, _| window.remove_window()).unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(cx.global::<WindowResourceCaches>().by_window.is_empty()));
}

#[test]
fn image_completion_notifies_the_owner_and_repaints_without_a_manual_draw() {
    let (mut cx, assets, sprites) = test_context(Some(CYCLE));
    let window = cx
        .open_window(size(px(340.), px(120.)), |_, cx| cx.new(|_| Fixture))
        .unwrap();

    // The first frame starts the shared decode. The completion callback from
    // GPUI's CachedLoad must notify the owning view so the test dispatcher
    // schedules a new frame; no explicit second draw is allowed here.
    draw(&mut cx, window);
    assert!(sprites.lock().unwrap().is_empty());
    assert_eq!(assets.loads.load(Ordering::SeqCst), 0);

    cx.run_until_parked();

    assert_eq!(assets.loads.load(Ordering::SeqCst), 1);
    assert_eq!(sprites.lock().unwrap().len(), 3);
}

/// Current mapping-menu icons color their paths through an embedded `<style>`
/// class (`.b{fill:#707070}`) rather than a fill attribute.
const STYLED_ICON: &[u8] = include_bytes!("../../../testdata/mapping_menu/mapping-mouse.svg");

struct IconFixture;

impl Render for IconFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        resource_scope(
            div().size_full().child(deferred_resource(
                img(KEY).absolute().left(px(10.)).top(px(10.)).size(px(20.)),
            )),
        )
    }
}

#[test]
fn style_class_svg_icon_decodes_to_visible_pixels_in_a_deferred_layer() {
    let (mut cx, _, sprites) = test_context(Some(STYLED_ICON));
    let window = cx.open_window(size(px(60.), px(60.)), |_, cx| cx.new(|_| IconFixture)).unwrap();
    let draw = |cx: &mut HeadlessAppContext| {
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            let _ = window.render_to_image();
        })
        .unwrap();
    };
    draw(&mut cx);
    cx.run_until_parked();
    draw(&mut cx);

    let image = cx.update(|cx| {
        let cache = &cx.global::<WindowResourceCaches>().by_window[&window.window_id()];
        cache.read(cx).images[&Resource::Embedded(KEY.into())]
            .get().expect("decode completed").expect("valid SVG")
    });
    let pixels = image.as_bytes(0).unwrap();
    let opaque = pixels.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    assert!(opaque > 0, "style-class fill must produce opaque pixels");
    // The `.a` rectangle is transparent; visible pixels are the #707070 path.
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[..3] == [112, 112, 112] && pixel[3] == 255));
    assert_eq!(sprites.lock().unwrap().len(), 1, "the deferred icon must paint");
}

#[test]
fn decode_error_is_reported_and_explicit_retry_reaches_valid_bytes() {
    let (mut cx, assets, sprites) = test_context(Some(b"invalid SVG"));
    let window = cx.open_window(size(px(340.), px(120.)), |_, cx| cx.new(|_| Fixture)).unwrap();
    draw(&mut cx, window);
    cx.run_until_parked();
    draw(&mut cx, window);
    cx.update(|cx| {
        let errors = resource_image_errors(cx);
        assert_eq!(errors.len(), 1);
        assert!(errors.first().unwrap().starts_with(KEY));
    });
    assert!(sprites.lock().unwrap().is_empty());
    assert_eq!(assets.loads.load(Ordering::SeqCst), 1);

    *assets.bytes.lock().unwrap() = Some(CYCLE);
    draw(&mut cx, window);
    assert_eq!(assets.loads.load(Ordering::SeqCst), 1, "failed entry remains until retry");
    cx.update(|cx| {
        retry_resource_images(cx);
        cx.refresh_windows();
    });
    draw(&mut cx, window);
    cx.run_until_parked();
    draw(&mut cx, window);

    assert_eq!(assets.loads.load(Ordering::SeqCst), 2);
    assert!(cached_image(&mut cx, window).as_bytes(0).unwrap().chunks_exact(4).any(|pixel| pixel[3] > 0));
    cx.update(|cx| assert!(resource_image_errors(cx).is_empty()));
    assert_eq!(sprites.lock().unwrap().len(), 3);
}
