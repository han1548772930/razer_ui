# Current camera preview-source mounts

The current manifest-selected bundles mount the preview-source component on
3594, 3595 and 3596. **3592 does not mount it on CAMERA.** Its CAMERA column
contains framing, autofocus and exposure; its resolution selection belongs to
PROCESSING. Adding preview-source or the Camo promotion to 3592 would contradict
the actual root.

`tools/camera-preview.cjs`, called by the camera descriptor generator, parses
literal AST nodes without executing product JavaScript. The receipt
`camera-preview-current-evidence.json` records each mounted root, preview
component, reducer initial state, video component, shared link component, CSS
rules and the generated presentation metadata. The older camera evidence is
accepted only after the main bundle slices and hashes are checked by the caller.

Implemented source branches:

- `isCamoStudioInstalled:null` satisfies the source `!isCamoStudioInstalled`
  condition. The initial Camo disclosure includes the six original locale keys.
  3594/3595 pass `supportCamoStudioActivation:true`: their code button emits
  `SourceControlsHelpRequested`, handled by the workspace's local HELP route.
  3596 uses the source external URL `https://www.razer.com/software/camo`.
- PREVIEW SOURCE uses the shared collapsible wrapper, its 210px help tooltip,
  25px tooltip right margin, 20px horizontally reflected refresh icon and 24px
  eye icon. Header actions remain present when the body is collapsed. The source
  collapse is immediate; icon hover opacity is 0.7.
- `cameraDeviceList:[]` is represented by a retained framework SelectState with
  an empty delegate and empty placeholder. No camera names, USB IDs or selected
  third-party app are invented.
- `isPreviewEnabled:null` follows the source preview-disabled branch. Its 18px
  original camera icon, 6px icon bottom margin, 14px/16px text, 16px action top
  margin and source ENABLE translation are preserved. The eye and ENABLE action
  update this retained UI state. Enabling it removes the disabled overlay; it
  does not claim that a stream has started.

The Camo text uses the source Roboto 14px/17px rule, 10px section content gap,
5px list top margin and -10px link offset. The source `<ul>` has no additional
author override for its standard Chromium 40px start padding / 1em bottom
margin. The link's inline 30px line height, 6×10 arrow with 5px left margin,
24×24 external icon with -5px top correction, colors and active opacity come
from the shared link component and current camera CSS. Both inline SVG paths
are extracted from literal JSX; the three bundles must agree byte for byte.

The original manifest SVGs for open eye, closed eye and camera-unable are
prepared by `tools/prepare-camera-section-assets.py`, alongside the existing
four camera section resources. The asset receipt records all four current
manifests and source hashes.

Service boundary: the native replica has no media-device enumeration or video
transport here. Refresh emits `SourceControlsPreviewRefreshRequested`; no
subscriber currently returns device results. The source has no refresh-button
loading animation, so no invented spinner, completion, reconnect acknowledgement
or camera error is displayed. Third-party launch, cable-capability warning,
installed-Camo changes and live video loading/error states still require real
transport observations; they are not asserted to be reachable by this change.

Validation is static only: `validate-camera-preview.py` verifies AST slices,
metadata, initial state, mount distinctions, locale keys, SVGs and native wiring;
`validate-camera-presentation.py` verifies all 64 groups (51 collapsible), 34
tooltips and 3 resolution warning groups. Formatting and the parent task's
`cargo check --locked --all-targets` are the permitted Rust checks. No application,
build, test, installer, downloaded JavaScript or DLL was executed.

## Legacy-camera surface and media boundary

3587/3589/3590 retain `layout="legacy-camera"` and the `TAB_CUSTOMIZE` preview. The current CSS `.camera_setting .main_preview` declares a 292px height, `#222` background, `margin-top:20px`, relative positioning and `z-index:0`; it does not declare a width. The native renderer currently reserves 520×292. The containing-block width and its layout still require verification; 520px must not be presented as a width proved by this rule.

| Product | Current CSS / SHA-256 | Rule |
| --- | --- | --- |
| 3587 | `.ref/devices/3587/static/css/main.19999b36.css` / `44e4091f06c4e47f0ed850a65cd9b6dfadc0a133b4f3925420468183e30b5de2` | UTF-16 `[390074,390238)`: `.camera_setting .main_preview{align-items:center;background-color:#222;display:flex;height:292px;justify-content:center;margin-top:20px;position:relative;z-index:0}` |
| 3589 | `.ref/devices/3589/static/css/main.0273b8ab.css` / `7b247bcd0347338b1509544e2d6d86ed9f4d204c937563c0af58e640ded6437e` | UTF-16 `[390074,390238)`: `.camera_setting .main_preview{align-items:center;background-color:#222;display:flex;height:292px;justify-content:center;margin-top:20px;position:relative;z-index:0}` |
| 3590 | `.ref/devices/3590/static/css/main.0273b8ab.css` / `7b247bcd0347338b1509544e2d6d86ed9f4d204c937563c0af58e640ded6437e` | UTF-16 `[390074,390238)`: `.camera_setting .main_preview{align-items:center;background-color:#222;display:flex;height:292px;justify-content:center;margin-top:20px;position:relative;z-index:0}` |

Static descriptors and local profile defaults do not supply a camera frame or an enumeration transport. The native surfaces contain an explicit unavailable state, with no sample frame, timer or fabricated device result. Media enumeration, transport, live loading/error and source-width parity remain incomplete. Current advanced-camera mounting and geometry are recorded in [the presentation contract](camera-presentation-current-audit.md).
