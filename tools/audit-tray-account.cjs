// Read-only source receipts and native wiring checks. Never evaluate vendor JS.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const assert = (condition, message) => { if (!condition) throw Error(message); };
const evidence = JSON.parse(read('docs/re/tray-account-current-evidence.json'));
for (const receipt of [evidence.manifest, evidence.js, evidence.css, evidence.gear, evidence.geometry]) {
  assert(receipt.path.startsWith('.ref/applications/systray/systrayv2/') || receipt === evidence.gear,
    'Unexpected provenance');
  assert(hash(read(receipt.path)) === receipt.sha256, 'Stale source: ' + receipt.path);
}
const js = read(evidence.js.path);
for (const receipt of evidence.nodes) {
  assert(js.slice(receipt.start, receipt.end) === receipt.source, 'Stale AST excerpt: ' + receipt.symbol);
}
const node = symbol => evidence.nodes.find(n => n.symbol === symbol)?.source || '';
assert(node('oe').includes('t.isGuest?RazerApp.logOut()'), 'Guest command changed');
assert(node('oe').includes('t.isGuest?J.FR:J.Il'), 'Guest/authenticated label condition changed');
assert(node('de').includes('showClassName:"show"})},100)'), 'Tooltip show delay changed');
assert(node('de').includes('shouldRender:!1,position:e.props.position})},100)'), 'Tooltip unmount delay changed');
assert(node('de').includes('window.addEventListener("blur",this.hideTooltip)'), 'Tooltip blur contract changed');
assert(node('_e').includes('t?e.length>0?') && node('W').includes('hasItems:!1'), 'Notification loading/empty distinction changed');
const css = evidence.rules.join('\n');
for (const fact of ['transition:color .1s ease-in-out,opacity .1s linear',
  'pointer-events:none;position:absolute;transition:opacity .1s linear;width:300px',
  'border:1px solid #5d5d5d', 'padding:7px 8px',
  '.tooltip-razer.bottom-left>.main', 'right:0']) assert(css.includes(fact), 'Missing source CSS: ' + fact);
const native = read('src/shell/tray/account.rs');
const tray = read('src/shell/tray.rs');
for (const fact of ['self.settings_tip_task = None', 'self.settings_tip_mounted = true',
  'self.settings_tip_visible = false', 'Duration::from_millis(100)',
  'self.settings_tip_mounted', '.with_priority(1060)', 'TrayColors::tooltip_border()',
  '"settings-quick-panel"', 'if self.notifications_loaded_empty',
  '"text-color"', '"press-opacity"', 'Easing::EaseInOut', 'Easing::Linear']) {
  assert(native.includes(fact), 'Missing native wiring: ' + fact);
}
assert(tray.includes('this.hover_settings(false, cx)'), 'Window blur does not dismiss settings tooltip');
assert(tray.includes('"login" | "account-guest-logout"'), 'Guest command silently dropped');
assert(tray.includes('session: TraySession::SignedOut'), 'Unconnected host must remain signed out');
assert(node('he').includes('minimum_height:768,minimum_width:1e3'), 'Tray settings minimum override changed');
assert(tray.includes('command == "settings-quick-panel"'), 'Settings gear option lost');
const settings = read('src/shell/settings_window.rs');
assert(/if quick_panel\s*\{\s*size\(px\(1000\.\), px\(768\.\)\)\s*\}\s*else\s*\{\s*size\(px\(600\.\), px\(500\.\)\)/.test(settings),
  'Settings default/gear minimum size branches changed');
console.log('Tray account: 12 current AST receipts, tooltip/empty-state CSS, native timer and command wiring checked.');
