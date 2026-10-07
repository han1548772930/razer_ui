"""Enumerate every implemented product page for individual current-source review.

This queue is scope accounting, never a claim that a route is finished UI.
Review findings live in separately authored reports and are not inferred here.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def read(name):
    return json.loads((ROOT / name).read_text('utf-8'))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    coverage = read('docs/re/native-product-coverage.json')
    registration = {p['product_id']: p for p in read('docs/re/product-registration-audit.json')['products']}
    pages, sources = [], {}
    for product in coverage['products']:
        pid = product['product_id']
        record = registration[pid]
        categories = record['categories']
        group = 'input_products' if any(c.startswith(('MOUSE', 'KEYBOARD', 'KEYPAD', 'GAMEPAD')) for c in categories) else 'receiver_accessory_audio_products'
        navigation = {item['key']: (nav, item) for nav in record['navigation'] for item in nav['items']}
        def append(page_id, display_mode, native_status):
            nav, item = navigation[page_id]
            file = nav['source']
            assert file.startswith('.ref/devices/'), file
            if file not in sources:
                data = (ROOT / file).read_bytes()
                sources[file] = hashlib.sha256(data).hexdigest()
            assert sources[file] == nav['sha256'], f'Current source differs: {file}'
            pages.append(dict(product_id=pid, product_name=product['name'], categories=categories,
                              page_id=page_id, page_key=item['name'].get('value'), display_mode=display_mode,
                              family=product['family'], assigned_group=group,
                              source=dict(path=file, sha256=nav['sha256'], navigation_offset=nav['offset'],
                                          item_offset=item['offset'], component=item['component']),
                              previous_route_status=native_status, review_status='requires_individual_review',
                              required_review=['actual_mount', 'controls_and_conditions', 'edit_apply_save_cancel',
                                               'local_draft_boundary', 'read_observation_and_failure',
                                               'dialogs_and_independent_roots', 'test_support', 'static_validation'],
                              runtime_validation='not_run', dll_writeback='deferred'))
        for page in product['pages']:
            append(page['page_id'], 'default', page['status'])
        for mode in product['independent_modes']:
            for page_id in mode['pages']:
                append(page_id, mode['mode'], mode['status'])
    # An endpoint is a scope entry only. Its real local page roots, dialogs and
    # conditional branches are expanded by the public-pages reviewer.
    applications = [dict(route=a['route'], assigned_group='public_pages',
                         review_status='requires_local_root_and_individual_review',
                         runtime_validation='not_run')
                    for a in read('docs/re/application-catalog.json')['applications']]
    # Explicit reviewed scope, never inferred from registered routes or assets.
    for page in pages:
        if page['product_id'] == 2636 and page['page_key'] == 'TAB_CALIBRATION':
            page['review_status'] = 'partial_source_review_and_native_ui'
            page['review_evidence'] = ['docs/re/gamepad-2636-calibration-native.md']
            page['remaining'] = ['live_read_publisher', 'runtime_visual_focus_acceptance']
        if page['product_id'] in (3858, 3880) and page['page_key'] in ('TAB_GAMING', 'TAB_COLOR', 'TAB_DISPLAY'):
            page['review_status'] = 'partial_source_review_and_native_ui'
            page['review_evidence'] = ['docs/re/monitor-pages-review-2026-10-07.md']
            page['remaining'] = ['live_read_publisher', 'documented_layout_and_condition_gaps', 'runtime_visual_focus_acceptance']
    for application in applications:
        if application['route'] == '/synapse/profiles/':
            application['review_evidence'] = ['docs/re/public-ui-review-2026-10-07.md', 'docs/re/profiles-transfer-native.md']
            application['review_status'] = 'partial_source_review_and_native_ui'
        if application['route'] == '/chroma-app/settings/':
            application['review_evidence'] = ['docs/re/chroma-settings-current-audit.md']
            application['review_status'] = 'partial_source_review_and_native_ui'
        if application['route'] == '/synapse/chroma-studio/':
            application['review_evidence'] = ['docs/re/studio-reactive-ripple-starlight-native.md', 'docs/re/studio-wave-wheel-pending-review-2026-10-07.md']
            application['review_status'] = 'partial_source_review_and_native_ui'
    counts = Counter(p['assigned_group'] for p in pages)
    payload = dict(schema_version=1, scope='All registered product navigation pages, independent modes and current application endpoints; per-page reviews remain separate evidence',
                   products=len(coverage['products']), primary_pages=sum(p['display_mode']=='default' for p in pages),
                   independent_pages=sum(p['display_mode']!='default' for p in pages),
                   groups=dict(counts), source_files=len(sources), pages=pages, applications=applications,
                   limitations=['Source SHA verification and route enumeration do not prove rendered UI or behavior.',
                                'Dialog/control/animation subroots must be enumerated while reviewing each page.',
                                'No application, test, downloaded JavaScript or DLL was executed.'])
    output = ROOT / 'docs/re/ui-review-queue-2026-10-07.json'
    text = json.dumps(payload, ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert output.read_text('utf-8') == text, 'Stale full UI review queue'
    else:
        output.write_text(text, encoding='utf-8')
    print(f"Full review scope: {payload['products']} products, {payload['primary_pages']} primary pages, {payload['independent_pages']} independent pages, {len(applications)} application routes; no behavioral completion inferred")

if __name__ == '__main__':
    main()
