# Wired ARGB motion follow-up

The current 778 and 3871 bundles define the shared ARGB icon choreography in
the `#multipleBrightness` stylesheet: the inactive detection glyph uses a
50ms `zoomout` track, while its green and gray expansion paths use independent
700ms `zoomoutc` and `zoomoutf` tracks. The active glyph uses a 100ms `zoomin`
track. The same stylesheet assigns 300ms `transition: all` to detection,
refresh, warning and close-icon state changes.

The source stepper component invokes its first increment immediately, then
repeats with `setInterval(..., 300)` until mouse-up or mouse-leave; the interval
is cleared on unmount and at the LED limits.

`wired_argb.rs` now keys native GPUI animation instances to each auto-detection
toggle and preserves the 50ms glyph phase plus 100ms active / 700ms inactive
timing. Its `AutoDetectionIcon` also interpolates the shared 300ms hover
transition with GPUI's motion primitive. `port.rs`
implements the source's immediate step followed by cancellable 300ms repetition
for both pointer release paths. No transport result or physical observation is
created by these motion handlers.
