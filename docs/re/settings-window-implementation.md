# Current independent Settings window

2026-10-06. Implementation evidence is the current `.ref/applications/settings/`
and `.ref/host-4.0.827/electron/main.js`; obsolete source trees were not used.
`prepare-settings-window.cjs` statically parses these files (never loads their
JavaScript) and checks 29 AST receipts, 556 CSS rules and 10 locale dictionaries.
The receipt file is `settings-window-current-evidence.json`.

## Implemented boundary

`src/shell/settings_window.rs` owns the independent native window. It uses the
existing policy-5 named-window registry and the host's 1280 × 720 initial bounds.
Opening an existing window focuses it and preserves its selected page. A new
window starts on Software, as `8821/ss` does. Escape closes this window. The four
tray commands `settings`, `settings-quick-panel`, `settings-widgets` and
`settings-notifications` all open this root: the current settings bundles contain
no `settingsScrollToSection` listener. Mapping those old hints to invented tabs
would not reproduce the current program.

The quick-panel gear is a distinct launch option contract: current tray `2554/he`
calls `launchSettings("", {minimum_height:768,minimum_width:1000})`, and the SDK
merges these options. Only `settings-quick-panel` therefore supplies a 1000 × 768
minimum on first window creation. The other three entry paths retain the host
defaults; focusing an existing named window does not change its bounds/options.
The static branch check is that `command == "settings-quick-panel"` alone reaches
the `if quick_panel` 1000 × 768 branch; all four commands retain the
same named-window identity and initial Software page contract.

The three original navigation labels, Software installed-list heading, Systray
Quick Launcher / General / Widgets hierarchy, General language and About panels,
policy destinations, version formula and social destinations use current source
receipts. Reference pixel geometry is expressed through the existing rem-based
`surface::css` projection and shared source panel/navigation components; colors
use the product theme. The native Kit TitleBar supplies window controls.

The independent Settings CSS itself declares `.body-widgets .widget` with
`min-width:600px;max-width:600px;padding:30px 40px;border-radius:5px;margin:10px auto`,
and `.widget-col` with `width:600px`, so use of the shared source panel is backed
by these current receipts. Its language `.dropdown-selector>.box` is 188 × 27;
launcher `.dropdown-selector.icon>.box` is 260 wide with 35 leading inset. Slots
are vertical title/selector pairs, with 5 title gap and 20 row gap. The Software
auto-update label is a disabled plain div in `9302/ee`, styled by
`.installed-software .action{color:#707070;text-decoration:underline}` and the
separately captured `.disabled{opacity:.3;pointer-events:none}` rule.

Language changes go through `SettingsPage::set_language`, keeping one preference
owner, the existing persistence event path and both language selectors in sync.
Changing language does not claim an Razer host-service write. Base Button/Link
and the existing Kit-backed Select supply pointer and keyboard behavior.
Navigation IDs and social link IDs come from source keys. The outer content
viewport owns scrolling; panels can wrap into one column.

## Social links and toolbar presentation

`settings_window_presentation.rs` renders the current inline SVG shapes rather
than tinting the whole social icon or swapping screenshots. The resource preparer
splits the original path geometry by source class into nine alpha masks. The seven
social icons retain 28 × 28 bounds and the circle center (14,14), radius 13.
Native borders interpolate width 1 → 1.5 and source fill/stroke colors
`#999999` → `#44d62c` over 200ms ease. Their centerlines remain fixed as the
stroke grows. Insider retains its 270 × 50 bounds, two distinct text colors and
the x/y=1, 268 × 48, radius-4 outline; that outline interpolates 0 → 1.5.
Color hover uses the circle/rounded-rectangle hit region, while tooltip hover
uses the source's enclosing social-item region. Keyboard activation remains
owned by Base Link, and all links expose the current translated accessible name.

Social tooltips follow `3414/be`: immediate visibility, bottom 37, padding
8 × 10, border 1, source body line-height 1.22, and
`left=-clientWidth/2+14` after actual max-content layout measurement. Their
source has no opacity transition. The two social rows retain top margin 20,
bottom margin 10; the seven-icon row has gap 24. Insider has no tooltip in the
current source. The About wordmark uses the original multicolor-capable SVG
image path so its green is preserved.

Toolbar navigation buttons use the current 40 × 38 hit area and centered 20 × 20
images. Back/Forward remain disabled because `8821/ss` supplies an empty history;
Refresh returns the reconstructed root to the source's initial Software page.
Their labels are extracted from the current `4693` exports in all ten locales.
The source's final flex rules are navigation `auto`, title `3 1 340px`, right
`0 0 20%`; the earlier fixed 90-pixel side spacers are removed. Hover background
is `#2d2d2d`. Refresh's source attribute tooltip is placed five pixels below its
38-pixel button, left aligned, with 16px line height and 300ms linear opacity.
It paints at the toolbar's 301 priority. Disabled buttons do not show tooltips.
The source and asset preparers assert these geometry rules and mask outlines.

## Explicit remaining gaps

This is not a completed fidelity claim for the whole Settings application.
The host installed-app catalog, installation state, app/widget reorder and toggle
services, quick-launch persistence and notification service are not connected.
The Software and Widgets collections remain at their source initial empty state;
this is not evidence that the machine has no installed Razer applications. No
installed app entries, update progress, successful writes or widget records are
invented. The five empty launcher slots are visibly unavailable. The source
default systray action is shown as static text, not a functional dropdown.

The source's toolbar account/status/feedback/Settings integrations,
full launcher popup/drag behavior, runtime
catalog arrival and installed-app actions still need implementation. About uses
the captured current systray manifest version, not a claim about installed host
version. General host minimum-size defaults are traced through main.js's
`WINDOW_SIZE_DEFAULTS:M` import to current constants.js: 600 × 500. The tray
gear overrides them to 1000 × 768 on first creation. Pixel geometry, keyboard focus and native-window
behavior have not been run or visually verified under the user's prohibition.

## Verification

- `node tools/prepare-settings-window.cjs --check` passed.
- `python tools/prepare-settings-window-assets.py --check` passed: 13 SVG assets
  and 16 shared state assets.
- `rustfmt --edition 2024 src/shell/settings_window.rs src/shell/settings_page.rs`
  passed.
- Application/build/test execution was not performed. The parent task owns the
  unified permitted `cargo check --locked --all-targets` result.
