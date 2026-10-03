//! Native playback of the current detecting.f2110e0d.svg SMIL timing.
use super::*;
use std::time::Duration;

pub(super) fn detecting(cx: &App) -> AnyElement {
    let mut layers = div()
        .relative()
        .w(surface::css(258.))
        .h(surface::css(270.))
        .child(
            img("synapse/wired-argb-3871-detecting-base.svg")
                .absolute()
                .inset_0()
                .size_full(),
        );
    for side in ["left", "right"] {
        for number in 1..=9 {
            let path = SharedString::from(format!(
                "synapse/wired-argb-3871-detecting-led-{side}{number}.svg"
            ));
            let led = img(path).absolute().inset_0().size_full();
            if cx.reduce_motion() {
                layers = layers.child(led.opacity(if number == 5 { 1. } else { 0. }));
            } else {
                layers = layers.child(
                    led.with_animation(
                        SharedString::from(format!("wired-argb-detection-{side}{number}")),
                        Animation::new(Duration::from_millis(1800))
                            .repeat()
                            .with_easing(linear),
                        move |led, phase| {
                            // Source: 0.5s fade in, 0.5s fade out, 0.1s stagger;
                            // next cycle starts when the ninth fade-out ends (1.8s).
                            let time = phase * 1.8 - (number - 1) as f32 * 0.1;
                            let opacity = if !(0.0..=1.0).contains(&time) {
                                0.
                            } else if time < 0.5 {
                                time / 0.5
                            } else {
                                (1. - time) / 0.5
                            };
                            led.opacity(opacity)
                        },
                    ),
                );
            }
        }
    }
    let ring = div()
        .absolute()
        .left(surface::css(103.))
        .top(surface::css(121.))
        .size(surface::css(52.));
    if cx.reduce_motion() {
        layers = layers
            .child(ring.child(img("synapse/wired-argb-3871-detecting-ring-000.svg").size_full()));
    } else {
        layers = layers.child(
            ring.with_animation(
                "wired-argb-detection-ring",
                Animation::new(Duration::from_secs(2))
                    .repeat()
                    .with_easing(linear),
                |ring, phase| {
                    let frame = ((phase * 120.).floor() as u32).min(119);
                    ring.child(
                        img(SharedString::from(format!(
                            "synapse/wired-argb-3871-detecting-ring-{frame:03}.svg"
                        )))
                        .size_full(),
                    )
                },
            ),
        );
    }
    layers.into_any_element()
}
