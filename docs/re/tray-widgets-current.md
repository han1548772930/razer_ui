# Current systray populated widgets

The current systray chunk 492 renders populated Synapse widgets from
`synapse.devices`. `src/shell/tray/widgets.rs` consumes parent observations.
It does not create fixture devices or report simulated device reads.

## Important correction: effective DOM and selector matching

`5492/y` inserts `children:[f(t), div.info, battery]`. `f(t)` resolves category
or subcategory from its `u` array and calls `M`, which emits a bare 40x40 SVG.
There is no `className="icon"` on that SVG. The `.icon` rule's
`margin-right:10px` therefore does not apply. Only `.info`'s `margin-left:10px`
separates the SVG and text. The previous local renderer applied both margins
and shifted the information lane 10px too far to the right.

Prepared category SVGs serialize the current `u` path and transforms through
`M`'s 40px dimensions, #ccc fill, 26.666666666666668 viewBox and grouping.
They are category outlines. Main module 5596's `Su` export gates subcategory
lookup. Unrecognised names render no SVG rather than a guessed product image.

## Effective layout

- Each device row is flex with `padding:10px 20px`.
- The category SVG is 40x40 with `min-width:40px` and no trailing margin.
- Information uses `margin-left:10px`, `max-width:220px`, and can shrink.
- Profile text is inline-block with line-height 1, max-width 100%, and 18px
  dropdown padding. Disabled affects pointer events and triangle opacity,
  not label color. The triangle has CSS borders equivalent to 10x5px.
- The title is #999, uppercase, and 12px.
- Battery uses `margin-left:auto`, `padding-left:10px`, a 14px label, and a
  26px icon lane. Ordinary artwork is 20x20 centered within that lane.
- Off uses the current manifest's 26px device-off SVG alpha mask in red,
  replacing the previous zero-battery substitution.
- Row hover is #111 and pressed is #393939.
- Empty widgets use a centered 60px row with 14px muted text.
- Section title is #222, top border 2px #111, #999 text at 10px, and 6px
  vertical padding. Inherited line-height 1.22 yields 26.2px total height.

The parent owns scroll measurement, sticky section behavior, device activation
and the profile dropdown portal. `widget_list_with_click` accepts parent
callbacks for row focus and section activation; `widget_list` is its inert
measurement wrapper. Neither fabricates a device/service result.

Source profiles must come from the host device publisher. Local draft profiles
are separate from device observations and must be identified as local.
Profile menu/hover behavior and sticky title remain explicit open items; the
static renderer alone does not complete those source features.

## Static receipts

The complete current tray branch and action read-through is maintained in
[tray-ui-current.md](tray-ui-current.md), with exact AST/CSS/host receipts in
[tray-semantic-current-evidence.json](tray-semantic-current-evidence.json).
It adds the source dedup/ready/serial rules, window-storage subscriptions,
profile BroadcastChannel payloads, per-app widget visibility and original
inconsistencies. In particular, `synapseWidgetsBatteryHidden` only annotates
`showBatteryWidget`; the current row does not consume that field. The paused
battery class contains a space before `-paused` and therefore does not match
the CSS concatenated `battery-N-paused` selector. These are source boundaries,
not claims of successful device writes or a local renderer fix.

Evidence is in [tray-widgets-current-evidence.json](tray-widgets-current-evidence.json).
It includes current manifest-declared 492 JS/CSS, 554 section-title CSS,
main 5596 category export, and source/output hashes for each prepared SVG.
Battery artwork comes from the current systray manifest, rather than assuming
the device page's similarly named images are identical.

```powershell
python -X utf8 tools/prepare-tray-widget-assets.py --check
python -X utf8 tools/audit-tray-widgets-current.py
```

Both parse source and validate static resources. No application, DLL, vendor
JavaScript, build or test was executed for this audit.
