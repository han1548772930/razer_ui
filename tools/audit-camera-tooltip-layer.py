"""Parse current camera CSS and native deferred-layer wiring without running UI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_receipt(path):
    data = path.read_bytes()
    return {'path': path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.as_posix(),
            'sha256': sha(data)}


def excerpt(path, needle, end_needle=None):
    source = path.read_text(encoding='utf-8')
    start = source.index(needle)
    if end_needle:
        end = source.index(end_needle, start)
    else:
        opening = source.index('{', start)
        depth = 1
        end = opening + 1
        while depth:
            depth += (source[end] == '{') - (source[end] == '}')
            end += 1
    text = source[start:end]
    return {**file_receipt(path), 'offset': start, 'end': end,
            'excerpt_sha256': sha(text.encode()), 'source': text}


def dependency(name, relative):
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    versions = [p['version'] for p in lock['package'] if p['name'] == name]
    assert len(versions) == 1, (name, versions)
    cargo_root = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
    paths = list((cargo_root / 'registry/src').glob(f'*/{name}-{versions[0]}/{relative}'))
    assert len(paths) == 1, (name, relative)
    return paths[0]


products = []
presentation = json.loads((ROOT / 'docs/re/camera-presentation-current-evidence.json').read_text(encoding='utf-8'))
for pid in (3592, 3594, 3595, 3596):
    product = ROOT / '.ref/devices' / str(pid)
    manifest_path = product / 'asset-manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    css_path = product / manifest['files']['main.css'].removeprefix('./')
    css = css_path.read_text(encoding='utf-8')
    rules = []
    effective = {}
    for match in re.finditer(r'([^{}]+)\{([^{}]*)\}', css):
        selectors = match[1].split(',')
        for selector in ('.drop-tips', '.drop-tips.on'):
            if selector in selectors:
                declarations = dict(pair.split(':', 1) for pair in match[2].split(';') if ':' in pair)
                effective.setdefault(selector, {}).update(declarations)
        if 'z-index:' in match[2] and any(token in match[1] for token in ('drop-tips', 'modal', 'dropdown', 'advanced-camera-container')):
            rules.append({'offset': match.start(), 'end': match.end(), 'source': match[0]})
    assert effective['.drop-tips']['z-index'] == '200'
    assert effective['.drop-tips.on']['z-index'] == '9999'
    portal = next(p for p in presentation['products'] if p['product_id'] == pid)['tooltip']
    source = (ROOT / portal['path']).read_text(encoding='utf-8')
    actual = source.encode('utf-16-le')[portal['offset'] * 2:portal['end'] * 2].decode('utf-16-le')
    assert actual == portal['source'] and sha(actual.encode()) == portal['sha256']
    assert 'createPortal' in actual and 'document.body' in actual
    assert '"drop-tips on":"drop-tips"' in actual
    products.append({'product_id': pid, 'manifest': file_receipt(manifest_path),
                     'css': {**file_receipt(css_path), 'rules': rules, 'effective': effective},
                     'portal': portal})

shared_path = ROOT / 'src/ui/source_tooltip.rs'
shared = shared_path.read_text(encoding='utf-8')
priority = excerpt(shared_path, 'fn draw_priority(')
assert 'SourceTooltipKind::WidgetTip | SourceTooltipKind::ReceiverWidgetPortal' in priority['source']
assert re.search(r'\{\s*//[^\n]*\n\s*10001\s*\}\s*else\s*\{\s*200', priority['source'])
assert re.search(r'if hovered\s*\{\s*self.hovered_priority.unwrap_or\(default\)\s*\}\s*else\s*\{\s*default', priority['source'])
assert 'hovered_priority: None' in shared
assert 'let priority = self.draw_priority(hovered);' in shared and '.with_priority(priority)' in shared
camera_path = ROOT / 'src/features/source_controls/camera_sections.rs'
camera = excerpt(camera_path, 'SourceTooltip::new(', '.trigger(')
assert '.hovered_priority(9999)' in camera['source']
callers = []
for path in (ROOT / 'src').rglob('*.rs'):
    if '.hovered_priority(' in path.read_text(encoding='utf-8'):
        callers.append(path.relative_to(ROOT).as_posix())
assert callers == ['src/features/source_controls/camera_sections.rs'], callers

deferred_path = dependency('gpui-pre', 'src/elements/deferred.rs')
window_path = dependency('gpui-pre', 'src/window.rs')
framework = {
    'priority_api': excerpt(deferred_path, 'pub fn with_priority('),
    'defer_without_parent_clip': excerpt(deferred_path, 'fn prepaint('),
    'draw_order': excerpt(window_path, 'fn deferred_draw_traversal_order('),
    'paint_deferred': excerpt(window_path, 'fn paint_deferred_draws('),
    'popup': excerpt(dependency('gpui-base', 'src/popup.rs'), 'pub const POPUP_PRIORITY:', 'impl RenderOnce for Popup'),
    'dialog': excerpt(dependency('gpui-base', 'src/dialog.rs'), 'impl RenderOnce for Dialog {'),
    'window_layers': excerpt(dependency('gpui-component', 'src/root.rs'), 'impl RenderOnce for WindowStateLayers'),
    'tooltip_surface': excerpt(dependency('gpui-base', 'src/tooltip.rs'), 'impl RenderOnce for Tooltip {'),
}
assert 'self.priority = priority' in framework['priority_api']['source']
assert 'window.defer_draw(child, element_offset, self.priority, None)' in framework['defer_without_parent_clip']['source']
assert 'sort_by_key(|ix| self.next_frame.deferred_draws[*ix].priority)' in framework['draw_order']['source']
assert 'let traversal_order = self.deferred_draw_traversal_order();' in framework['paint_deferred']['source']
assert 'POPUP_PRIORITY: usize = 100' in framework['popup']['source']
assert '.with_priority(10 + self.layer)' in framework['dialog']['source']
for layer in ('sheet_layer', 'dialog_layer', 'notification_layer'):
    assert layer in framework['window_layers']['source']
assert 'deferred(' not in framework['tooltip_surface']['source']
parent_content = excerpt(ROOT / 'src/features/source_workspace.rs',
                         '.id("source-product-content")', '.children(self.profile_linked_games.clone())')
assert '.scrollable_both()' in parent_content['source'] and '.child(body)' in parent_content['source']
camera_root = excerpt(ROOT / 'src/features/source_controls.rs',
                      'if self.spec.layout.as_deref() == Some("camera") && self.page != "HELP"')
assert '.child(self.render_camera_column(page, window, cx))' in camera_root['source']
assert 'deferred(' not in camera_root['source']
profile_overlay = excerpt(ROOT / 'src/features/source_workspace/profile_transfer.rs',
                          'impl Render for SourceProfileTransfer {')
assert '.layer(3, true)' in profile_overlay['source']

result = {'schema_version': 1, 'scanner_sha256': sha(Path(__file__).read_bytes()),
          'cargo_lock': file_receipt(ROOT / 'Cargo.lock'), 'products': products,
          'native': {'policy': priority, 'camera_opt_in': camera, 'override_callers': callers,
                     'parent_content': parent_content, 'camera_root': camera_root,
                     'profile_overlay': profile_overlay},
          'framework': framework,
          'conclusion': {'camera_hovered': 9999, 'camera_unhovered': 200,
                         'other_consumers_changed': False, 'verification': 'static source only'}}
destination = ROOT / 'docs/re/camera-tooltip-layer-current-evidence.json'
parser = argparse.ArgumentParser()
parser.add_argument('--check', action='store_true', help='compare existing evidence without writing')
arguments = parser.parse_args()
if arguments.check:
    assert json.loads(destination.read_text(encoding='utf-8')) == result
else:
    destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print('Camera tooltip layers verified: hover=9999, unhover=200; other consumers retain their layers.')
