//! Source muted, looping videos decoded by GPUI's animated image support.
use gpui_kit::*;

pub fn clip(
    asset: &'static str,
    label: impl Into<SharedString>,
    ratio: f32,
) -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    div()
        .id(asset)
        .test_support()
        .role(Role::Image)
        .aria_label(label)
        .relative()
        .w_full()
        .aspect_ratio(ratio)
        .flex_shrink_0()
        // Key the cache by clip, not by tutorial. RetainAll's release observer
        // drops decoded frames and GPU images when this subtree unmounts.
        // GPUI owns timing, looping, inactive-window pause and reduced motion.
        .child(
            image_cache(retain_all(asset)).absolute().inset_0().child(
                img(asset)
                    .id("tutorial-frames")
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            ),
        )
}
