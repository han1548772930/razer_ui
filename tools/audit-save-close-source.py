"""Capture current-source receipts for save controls and window/exit behavior.

Only reads source text. Snippets locate evidence; they are not an executable
substitute for reading the full handlers in the hashed files.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
checks = [
    ('.ref/host-4.0.827/electron/components/Tab/TabUI.js', [
        ('close_button', 'closeWindow=()=>', 180)]),
    ('.ref/host-4.0.827/electron/main.js', [
        ('default_close_action', 'b.closeWindowAction=z.HIDE', 80),
        ('forced_close_option', 'null!=Me["force-close-window"]', 100),
        ('close_dispatch', 'case"handleCloseBtn":', 90),
        ('exit_dispatch', 'case D.EXIT_APPLICATION:', 230),
        ('exit_handler', 'async function Dn(', 2660)]),
    ('.ref/host-4.0.827/electron/components/Tab/common.js', [
        ('close_handler', 'static handleCloseBtn(', 240),
        ('alt_f4', 'name:"AltF4Handler"', 380)]),
    ('.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js', [
        ('conditional_header_menu', 'if(!e||0===e.length)', 400),
        ('header_buttons', 'onClick:()=>y.A.saveAll(v)', 330),
        ('unload_cleanup', 'this._onBeforeUnload=()=>', 65),
        ('window_close_callback', 'window.onclickclose=()=>', 110)]),
    ('.ref/devices/182/static/js/main.db20a7c4.js', [
        ('mapping_guard', 'this.props.isMappingChanged&&this.props.activeButton', 210),
        ('mapping_save_alert', 'className:"save-alert"', 850)]),
]

receipts = []
for path, markers in checks:
    assert path.startswith(('.ref/host-4.0.827/', '.ref/applications/synapse/dashboard/', '.ref/devices/'))
    data = (ROOT / path).read_bytes()
    source = data.decode('utf-8')
    snippets = []
    for name, marker, length in markers:
        # The conditional menu uses its actual no-items return, not a guessed
        # generic minified guard.
        if name == 'conditional_header_menu':
            marker = '0===r.length)return null;const v='
        offset = source.index(marker)
        snippets.append({'name': name, 'offset': offset,
                         'source': source[offset:offset + length]})
    receipts.append({'path': path, 'sha256': hashlib.sha256(data).hexdigest(), 'snippets': snippets})

for filename in ('crates/razer-pages/src/features/workspace.rs', 'crates/razer-pages/src/features/source_workspace.rs', 'crates/razer-shell/src/shell.rs'):
    source = (ROOT / filename).read_text(encoding='utf-8')
    for removed in ('"save-profile"', '"discard-profile"', '"source-product-save"',
                    '"source-product-discard"', '"close-save"', '"close-discard"',
                    '"保存本地更改？"'):
        assert removed not in source, (filename, removed)

result = {'schema_version': 1, 'audit_date': '2026-10-03',
          'verification': 'Static source receipts; no application, tests or vendor code executed.',
          'receipts': receipts,
          'local_behavior': 'Windows close hides the retained main window when tray initialization succeeds. Tray Exit drains already requested writes without submitting drafts or showing a generic save dialog.',
          'remaining_gap': 'Native hide retains GPUI state; Electron renderer hibernation and force-close-window override are not implemented.'}
(ROOT / 'docs/re/save-close-current-source.json').write_text(
    json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'Captured {len(receipts)} current-source files; extra save footers and close prompt absent.')
