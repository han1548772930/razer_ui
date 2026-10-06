# Current Studio layer operations — 2026-10-06

`chroma-studio-layers-source.json` records 13 AST slices from the current
independent Studio manifest-selected source, with file SHA-256 and UTF-16
offsets. The maintained extractor only parses JavaScript.

The native flat-layer model now follows these verified branches:

- `9286:T/Y`: default effect titles are locale keys and can repeat; renamed
  layer/group duplicates receive the first available `(1)`, `(2)`, ... suffix
  among siblings of the same type. A previous numeric suffix is replaced.
  Existing saved literal titles remain literal titles rather than being guessed
  back into translation keys. Rendering and rename initialization translate
  stored keys, as the source text component does.
- `9286:w/Y`: new empty groups use the smallest free localized group number,
  including gaps left by deletion or renaming, rather than group count plus one.
- `4264:J` and `9286:I`: Change Effect excludes the current effect and the
  unavailable Reactive/Ripple effects. It retains layer identity and visibility
  while replacing effect name, title, numeric value and parameter defaults.
  Source regions/device selections are absent from this local model.
- `9286:G/O`: the last remaining effect cannot be deleted through the layer
  menu. Deleting the selected effect picks the first remaining effect rather
  than clearing selection unconditionally.
- `5303:m`: Undo/Redo retains the selected layer when it exists and otherwise
  selects the first remaining effect.
- `9286:b`, `2904:f`: visibility changes and duplication select the first
  visible effect, falling back to the source's first-item policy. Empty groups
  are not effect selections; clicking them cannot show an effect inspector.

The native Change Effect menu uses GPUI Component's submenu. Current source
replaces the contents of the same popup. This placement difference is explicit:
the installed PopupMenu closes every ordinary item after its callback and has
no public non-dismiss item behavior. Native keyboard navigation, Escape and
focus restoration are retained instead of introducing an ad-hoc menu.

Populated groups, group children/opened state, drag placement, nested visibility
and copying group contents remain unimplemented. The local persistent Layer
schema was not expanded in this batch. No device enumeration, LED selection or
engine application is simulated.

Static checks: `node tools/prepare-chroma-studio-layers.cjs --check`,
`python tools/validate-chroma-studio-layers.py` and helper formatting. Main-file
formatting and the permitted combined cargo check belong to the parent batch.
No application, build or test was run.
