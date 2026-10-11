"""Generate one complete current floating Controller evidence page per product."""
import json
from pathlib import Path
root = Path(__file__).resolve().parents[1]
receipt = json.loads((root/'docs/re/keyboard-analog-floating-controller-current-source.json').read_text(encoding='utf8'))
for record in receipt['records']:
    pid = record['product_id']
    parts = [f'# {pid} current floating Controller\n',
        'Audit date: 2026-10-11. This page is independent current-source evidence and an implementation checkpoint; runtime acceptance remains absent. Static AST/text/resource preparation only. The webpack inline entry has no numeric module ID.\n',
        f"Main: `{record['main']['path']}`\n\nSHA-256: `{record['main']['sha256']}`\n",
        '## Conditions and interaction\n',
        'The `.toggle-controller` listener toggles only local open state and CSS `open`. Close uses the same toggle. It does not clear the selected keyboard key. The controller uses the original skeleton and 24 exact path/circle hit regions. Hypershift alone selects green/orange hover, dragging and preview colors; this palette has no live device input observation.\n',
        'The drag area is fixed, horizontally centered with 30px side margins, and follows `.config-block` top/height. Panel width360, padding20, border1, radius5 and universal border-box produce a318px image width at the320×220 viewBox ratio. Height is measured after localized description layout. First opening places it left of `.keyboard-svg` and vertically centers it within the area. Hide/show retains its origin. Source clamp applies max(minX) then min(maxX), including negative origins when the area is smaller than the panel. Drawer opening shifts origin+200 only at x<=200; closing reverses only a recorded shift.\n',
        'Panel mousedown accepts every browser mouse button; document motion updates/clamps its position, document exit suppresses panel motion, mouseup commits/clamps and removes drag classes. Dragging tints the area #0000001a. A shape mousedown stops propagation and stores `{data: structuredClone(button.mapping)}` under controller-button, shows preview and global grabbing cursor. No dragCustomize/originalButton is present for palette drags. Generator refreshes every slot actuationPoint with31867.fH.createDefaultActuation(), including current profile sensitivity rules.\n',
        'Shape enter/leave controls the body tooltip. It is centered10px above the shape, moves below if above panel top, and clamps horizontally10px within the panel. Drag preview follows page pointer+10,+10 with original icon,40px height,padding10,radius5,14px bold #212121. Pointer release consumes an exact keyboard geometry hit before clearing drag state.\n',
        f"Keyboard layout padding: `{json.dumps(record['keyboard_padding'],ensure_ascii=False)}`.\n",
        '## Rust and submission connection\n',
        '`keyboard_floating_controller.rs` owns independent retained open/position/drag state, deferred absolute surfaces, source hit geometry, tooltip and preview. `keyboard_products.rs` mounts it in Customize and registers actual native keyboard regions; toggle does not mutate selected_key. Current layout geometry is selected from observed artwork/layout data when available. Page unmount clears floating state; changing a profile keeps the mounted floating origin while cancelling pending pointer interaction.\n',
        'The palette-to-key path uses `keyboard_controller_drop.rs`: source notRemapped and3342.z exclusions, both asymmetric duplicate checks (current layer and omitted layer), complete-list replacement keyed by inputID/inputType/isHyperShift. Success writes the local full mapping list, emits KeyboardProductChanged, and calls request_actuation_mapping→ON_SET_KEYMAPPING. This immediate source action has no Save confirmation. Submission responses use the existing generation/error state; no successful device operation is invented. Full drop caller/reducer spans are independently retained in `keyboard-controller-drop-current-source.json`.\n',
        '## Explicit gaps and verification limits\n',
        'The real service/device mapping adapter still reports its implementation gap; this page does not establish native writeback or successful runtime acceptance. Source MapKeyboard assignmentValue depends on the complete button-state producer and real systemKeyboardLayout; primary hyperShiftGroup is reconstructed for its specific gate, but the remaining dynamic assignment display/Windows remap branch is not claimed complete. Keyboard-to-key moving drags are not mounted by this palette work. 679 RM/DM special dial/media popovers remain an explicit gap: their source hover uses disabled DIAL_CLICK and DKM_KBMK_02 shapes plus assignmentGroup projection and independent positioning; no static card or fabricated active state is substituted. The renderer does not reproduce the source stale mouse-pressed ref after release or mismatched scroll cleanup target; native per-frame callbacks are reclaimed with their element. Tooltip300ms opacity transition, exact resize/layout300ms measurement scheduling and all host focus/cancellation paths need separate runtime/static lifecycle acceptance.\n',
        'Pure Rust tests cover clamp order/narrow areas, conditional drawer shift reversal, and close/reopen retained origin. Parent task performs cargo check and selected pure tests; application, vendor JavaScript, DLLs/helpers and real device operations were not executed.\n',
        '## Conditional curve selection evidence\n']
    for conditional in record['conditional_selections']:
        parts += [f"Selected `{conditional['selected']}` using `{json.dumps(conditional['basis'])}` from `{conditional['config_path']}` SHA-256`{conditional['config_sha256']}`. Both original branches remain retained.\n"]
        for name in ['condition','consequent','alternate']:
            span=conditional[name]
            parts += [f"{name}: UTF-8`{span['byte_range']}` SHA-256`{span['slice_sha256']}`\n\n```javascript\n{span['source']}\n```\n"]
    parts += ['## Complete floating functions, table, aliases and root\n']
    for span in record['source']:
        parts += [f"### `{span['symbol']}`\n\nUTF-8`{span['byte_range']}`, UTF-16`{span['char_range']}`, SHA-256`{span['slice_sha256']}`\n\n```javascript\n{span['source']}\n```\n"]
    parts += ['## Complete relevant current CSS\n']
    for sheet in record['css']:
        parts += [f"`{sheet['path']}` SHA-256`{sheet['sha256']}`\n"]
        for rule in sheet['rules']:
            parts += [f"UTF-8`{rule['byte_range']}` SHA-256`{rule['slice_sha256']}`\n\n```css\n{rule['source']}\n```\n"]
    parts += ['## Original SVG resources\n']
    for asset in record['resources']:
        parts += [f"### `{asset['name']}`\n\n`{asset['path']}` SHA-256`{asset['sha256']}`\n\n```xml\n{asset['source']}\n```\n"]
    (root/f'docs/re/keyboard-{pid}-floating-controller-current-source.md').write_text('\n'.join(parts),encoding='utf8')
print('Generated three independently sourced floating Controller pages')
