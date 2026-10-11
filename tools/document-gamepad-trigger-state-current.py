"""Append verified current reducer source and implementation status per page."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def focus_source_section(pid):
    blocks = ['\n## Current mounted focus helper and host producers\n']
    for name in ('gamepad-calibration-focus-current-source.json',
                 'gamepad-calibration-focus-host-producers-current.json'):
        audit = json.loads((ROOT/'docs/re'/name).read_text(encoding='utf-8'))
        for item in audit['files']:
            # Product helper is specific to this page; common host producers
            # are shared only because the exact original code proves it.
            if '/products/' in item['source'] and f'/products/{pid}/' not in item['source']:
                continue
            raw = (ROOT/item['source']).read_bytes()
            assert hashlib.sha256(raw).hexdigest() == item['sha256']
            utf16 = raw.decode('utf-8').encode('utf-16-le')
            blocks.append(f"\nSource `{item['source']}`; SHA-256 `{item['sha256']}`.\n")
            for receipt in item['receipts']:
                assert utf16[receipt['offset']*2:receipt['end']*2].decode('utf-16-le') == receipt['source']
                blocks.append(f"\n{receipt['name']} ({receipt['offset']}..{receipt['end']}, UTF-16):\n\n```javascript\n{receipt['source']}\n```\n")
    blocks.append('''
Rust `host_window_focus.rs` implements the initial/blur status query, the focus(true) selected-tab query, tab-name comparisons and the popup true-then-false ref. Missing status fields are not converted to explicit false. A query failure leaves the observation unchanged; an actual absent status object follows the source false branch. Retries retain the mounted focus state. The shell initializes once and forwards actual window activation and committed tab selection events. On Windows, status uses the borrowed owning HWND's IsWindowVisible/IsIconic, not foreground focus or GPUI frame presentation. Local retained HostTab IDs provide internal frame identity; no vendor BrowserView query is claimed. No extra polling, resize or visibility event was added. Other platform status adapters, thumbstick focus connection and runtime acceptance remain gaps.
''')
    return ''.join(blocks)
marker = '\n## Trigger reducer and Rust state correspondence — 2026-10-11\n'
for pid in (2676, 2684):
    receipt_path = ROOT/f'docs/re/gamepad-{pid}-trigger-reducer-live-source.json'
    receipt = json.loads(receipt_path.read_text(encoding='utf-8'))
    raw = (ROOT/receipt['source']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == receipt['sha256']
    source = raw.decode('utf-8').encode('utf-16-le')
    for key in ('initial_state','reducer'):
        item=receipt[key]
        assert source[item['offset']*2:item['end']*2].decode('utf-16-le') == item['source']
    target = ROOT/f'docs/re/gamepad-{pid}-calibration-live-page.md'
    prior = target.read_text(encoding='utf-8').split(marker)[0]
    section = marker + f'''
Source: `{receipt['source']}`; SHA-256 `{receipt['sha256']}`. Addresses below are UTF-16 code units.

Initial state ({receipt['initial_state']['offset']}..{receipt['initial_state']['end']}):

```javascript
{receipt['initial_state']['source']}
```

Complete current calibration reducer ({receipt['reducer']['offset']}..{receipt['reducer']['end']}):

```javascript
{receipt['reducer']['source']}
```

The mounted Trigger popup is implemented separately from the thumbstick wizard. The current trigger parts are 3/4; service progress steps are 10/11/12. START locally sets step 10 and partId, STOP sets step 0, and both retain the prior isStepValid. Progress observations replace step/valid; they do not invent a new partId. The popup uses an observed trigger part when available, otherwise its selected part.

CalibrationUserMovement accepts only finite t, clamps it to 0..100, and maps marker position to 100-t with 50ms easeOutCubic. The timer resets on part/phase changes or invalidity. A valid hold phase lasts visually 2s and release phase 3s; reaching 100% never advances service progress or announces success. Done is displayed only after observed step 12. Retry starts the effective trigger part. Cleanup stops the originally selected trigger part. Focus observation permits initial inactivity and closes only after activity has been observed then lost.

Rust implementation: `gamepad_trigger_calibration_state.rs`, `gamepad_trigger_calibration.rs`, `gamepad_trigger_calibration_renderer.rs`, and `gamepad_calibration_selection.rs`. Current image/connector/meter/step resources are registered in both AssetSource load and list. Their source hashes, lexical callers and CSS evidence remain in `gamepad-trigger-selection-resources-current.json`. The selector's product wrapper background is none and its dim-corner is hidden by page-specific CSS; ordinary route entry has no displayMode. Nonempty displayMode product hiding still requires root-specific integration review.

Submission runs from the source-shaped start/stop intent through GamepadProductWorkspace, SourceProductWorkspace, ProductWorkspace and WorkspaceEvent to the shell's owned calibration session, typed IPC Start/Poll/Cancel and portable worker. Discovery selects the exact actual wired collection through retained HID identity or Windows ContainerId/path association, then requires Report10/91. The worker performs genuine raw queries, source 2s/3s valid hold, five samples, native range write and strict readback. Task acceptance never fabricates device completion. Genuine progress and movement are streamed back with generation/sequence/identity checks; a null storage removal is ignored by the popup as in tp/Rc. The source transaction counter is 0..30 after constructor zero and postincrement. Cancellation/identity protection is an owned Rust guarantee; it is not attributed to the original JS. There is no invented calibration timeout. Windows unknown interface -1 requires actual SetupAPI path/ContainerId/instance/claim1 proof rather than a VID/PID guess. See `gamepad-calibration-worker-current.md` for separate acquisition, semantics, implementation and unexecuted runtime acceptance.

Official current middleware entry and declared product script were downloaded as inert bytes on 2026-10-11. Acquisition evidence: `gamepad-{pid}-middleware-current-acquisition.json`. Current middleware/device/host receipts independently retain complete calibration classes, command wrappers and transport methods with original hashes and offsets. Pure Rust core/protocol and owned worker/shell implementations use these current sources, not vendor DLL execution.

Remaining full-scope gaps: wireless logical-device identity/calibration routing; complete thumbstick native calibration and canvas animation; host storage broadcast interoperability outside the mounted Rust owner; root-specific displayMode and focus-owner acceptance; remaining profile/device behaviors; runtime visual/interaction/device acceptance. No application, vendor JS, DLL, helper or real device operation was executed. Rust wire/core verification is separate from runtime acceptance. This page remains incomplete.
'''
    target.write_text(prior+section+focus_source_section(pid),encoding='utf-8')
print('Updated 2 page MDs with verified current reducers and explicit submission gaps')
