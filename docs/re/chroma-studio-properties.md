# Current Studio effect properties

The maintained static extractor `tools/prepare-chroma-studio-properties.cjs` reads only the current manifest-declared source under `.ref/applications/synapse/chroma-studio/`. It parses the JavaScript with Acorn and the CSS with the maintained CSS parser. It never imports or evaluates vendor factories. Run `node tools/prepare-chroma-studio-properties.cjs --check` to compare both generated artifacts with current source.

- `src/features/chroma_studio_properties_data.json`: reviewed controls for all 13 effect roots, shared component identities, source constants, gradient presets and working-buffer rules. This data is preparation for the native inspector; its presence does not mean every control is implemented.
- `docs/re/chroma-studio-properties-source.json`: exact module/function receipts with current-file SHA-256 and UTF-16 offsets, JSX control inventory with original property expressions and safely resolved literals, and relevant rules from 16 manifest-declared CSS files. All declared CSS files are locally present. A literal `null` represents source `undefined` in the extraction format, especially the no-color swatch; it is not RGB black.

## Working parameters and persistence

`9286:R` (export `HZ`) dispatches `UA` and optionally previews; its second argument defaults to true. The preview call occurs only when the editor tool is `select`. Sliders send false during changes and true after interaction. `1638:j` merges the patch into **working `effectLayer.params`**, removes the patched keys from `paramsMixed`, and sets `isActive`. It does not update default objects, layer parameters, `params2`, or persistent device regions. When both `effectConfigList` and `currentConfigId` appear in one patch, it derives `currentConfig` from that list/index, falling back to `{}`.

`9286:N` (export `XF`) dispatches `RZ` and previews. `RZ` merges into the separate `effectLayer.params2` buffer, which is used by wheel/tidal direction and center points. `9286:U` applies parameters to device regions. `9870:d` preserves working parameters between pen and bucket; transitions involving select or move perform the source reset/merge behavior. The source reset action `9286:P` uses current-effect paint defaults and, for wheel/tidal, separately resets center-point parameters. These buffers must not be mirrored into one another or into a persisted layer on every input change.

`4264:se` disables the inspector when no devices are selected and the active tool is select or move. Pen and bucket can edit working paint parameters with no selected device. Actual screen capture, screen-region selection, eyedropper sampling, device-region application, and center-point device choice require real native/backend responses.

## Important control differences

Ambient uses screen presets `full`, `left`, `top`, `right`, `bottom`, a native-selected custom rectangle, normalized screen coordinates 0–65535, and a blur slider 1–9, default 5, inherited step 1, with no tooltip. Its native screen actions must not report successful sampling without a backend.

Static embeds the shared color editor. Fire has hot/cold colors. Breathing and tidal have two colors and random-color disabling; their absent color fallback is the source sentinel 1677721600. Reactive has a color, random color, and duration, but no Playback section. Tidal exposes speed, center point, direction, and Playback; stored angle/split defaults are not visible controls in its root.

Wave pause is a **number input**, 0–60 seconds, stored in milliseconds, rather than a second speed slider. Its angle dial is accompanied by an input bounded 0–359. Ripple width steps by 100 over 100–400. Duration selectors map three UI positions to effect-specific milliseconds. Audio exposes boost 0.25–4 in 0.25 steps (disabled by auto boost) and decay 0.1–2 in 0.1 steps. Spectrum, starlight and audio do not render the shared Playback component.

Playback start options are `random`, `onPress`, `onSelectedKeys`; end options are `never`, `after`, `toggle`. Toggle is omitted for random start. Cycles 1–100 are shown only for `after`; other modes store -1. Changing start to random chooses never; changing to another start chooses after and restores remembered cycles or 1.

Generate accepts `image/*, .mp4` and checks dropped MIME types for image/video. It enables generation only with media, allows at most five configs, bounds previous/next navigation, and clears media/list/index when removing. Receipts contain the source generation/config code; preparation of this schema does not establish native media processing support.

## Color and gradient editor

Module 1698 draws a horizontal red/yellow/green/cyan/blue/magenta/red hue field with a transparent-to-white vertical overlay. It is not a conventional hue/saturation plane shaded to black. Brightness is separate and uses the source threshold 0.7490196078431373. The normal canvas is 228×136; dropdown use overrides the width to 208. The picker marker offsets by -7 on both axes. HEX is six characters with non-hex input removed; RGB channels clamp to 0–255. Enter/Escape blur the input and thereby commit. There are eight fixed swatches including undefined/no color, plus at most eight custom slots. Custom swatch context removal only applies to custom indexes. Eyedropper uses native `showColorPicker`, `onColorPicker`, and `onColorPickerDone`.

Module 3690 selects separate current-source gradient presets for default, audio and spectrum effects. Spectrum requires at least two stops; others require one. Maximum stops derive from the respective preset arrays. The editor's inner color canvas is 208 pixels wide; effect roots use gradient canvas width 180. Dragged stop positions are rounded to two decimals, constrained by neighbors and a pixel-derived minimum distance. Adding a stop samples the actual gradient pixel. `9220` converts normalized positions to rounded percentages and RGB triplets to 24-bit colors before sorting. Mixed/undefined gradient values hide the picker; unrelated product color palettes are not evidence for this editor.
