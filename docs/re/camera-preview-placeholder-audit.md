> 2026-10-06 correction: [Current camera presentation audit](camera-presentation-current-audit.md) supersedes the processing mount, resolution-selector, disclosure and tooltip claims below. In particular, 3592 mounts a resolution selector on PROCESSING, and current advanced-camera preview layout retains the empty right-hand video surface; the earlier 360?202 placeholder description is historical.

# Camera preview service boundary (2026-10-04)

The current Kiyo roots mount a live camera preview either alongside the
`CAMERA` control column (3592/3594/3595/3596) or inside the shared
`TAB_CUSTOMIZE` widget (3587/3589/3590). The static product descriptors in
`crates/razer-pages/src/features/source_controls_data.json` provide the controls and profile
defaults, but they do not provide a camera frame or a device-enumeration
transport.  The existing runtime therefore cannot claim a live frame.

`SourceControls::render_camera_column` now reserves a 360×202 content surface
on the `CAMERA` page and renders an explicit unavailable state.  The surface
uses the audited camera palette (`#000` content, `#5d5d5d` border) and labels
the missing stream service.  It contains no sample frame, timer, camera name,
or fabricated enumeration result.  A future camera transport can replace this
single child while retaining the audited controls and layout.

This is a presentation boundary only; it does not change profile persistence,
camera settings, or device service requests.

The legacy roots 3587, 3589 and 3590 use `layout="legacy-camera"`. Their
shared `TAB_CUSTOMIZE` widget has a source `.camera_setting .main_preview`
surface (`#222`, 520x292); the native renderer reserves that same frame before
the image controls and shows the same explicit unavailable boundary.
