"""Acquire all missing JS declared by the already pinned middleware manifests.

Reads vendor data and downloads HTTPS bytes only. Never refreshes manifests,
overwrites existing source bytes, or evaluates downloaded JavaScript.
"""
import argparse
import concurrent.futures
from datetime import datetime, timezone
import hashlib
import http.client
import json
from pathlib import Path
import re
import time
import threading

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / '.ref/middleware'
OUTPUT = ROOT / 'docs/re/middleware-source-acquisition-current.json'
REQUESTS = BASE / 'ALL-MANIFEST-SOURCE-REQUESTS.json'
PROGRESS = BASE / 'ALL-MANIFEST-SOURCE-PROGRESS.json'
MAX_FILE_BYTES = 16 * 1024 * 1024
HTTP_STATE = threading.local()


def utc():
    return datetime.now(timezone.utc).isoformat()


def digest(body):
    return hashlib.sha256(body).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding='utf8'))


def write_json(path, value):
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
    temporary.replace(path)


def collect():
    manifests, jobs, existing = [], [], []
    for path in sorted(BASE.glob('*/webpackManifest.json'), key=lambda p: int(p.parent.name)):
        product = int(path.parent.name)
        body = path.read_bytes()
        manifest = json.loads(body)
        manifests.append({'product_id': product, 'path': path.relative_to(ROOT).as_posix(), 'sha256': digest(body)})
        for name in sorted(set(manifest.values())):
            if not isinstance(name, str) or not name.endswith('.js'):
                continue
            if not re.fullmatch(r'[A-Za-z0-9_.-]+\.js', name):
                raise ValueError('Unsafe manifest name: ' + name)
            request = {'product_id': product, 'file': name}
            (existing if (path.parent / name).exists() else jobs).append(request)
    return manifests, jobs, existing


def existing_receipt(product, name):
    directory = BASE / str(product)
    target, receipt_path = directory / name, directory / (name + '.http.json')
    if not target.exists():
        return None
    url = f'https://apps.razer.com/synapse/products/{product}/mw/{name}'
    if not receipt_path.is_file():
        return {'status': 'preserved_existing_without_receipt', 'source_url': url}
    receipt, body = read_json(receipt_path), target.read_bytes()
    if (receipt.get('source_url') != url or receipt.get('final_url') != url
            or receipt.get('http_status') != 200 or receipt.get('sha256') != digest(body)
            or receipt.get('bytes') != len(body)):
        return {'status': 'preserved_existing_invalid_receipt', 'source_url': url}
    return {'status': 'verified_existing', 'source_url': url, 'sha256': receipt['sha256'], 'bytes': len(body)}


def fetch(job):
    product, name = job['product_id'], job['file']
    existing = existing_receipt(product, name)
    if existing:
        return {**job, **existing}
    directory = BASE / str(product)
    target = directory / name
    url = f'https://apps.razer.com/synapse/products/{product}/mw/{name}'
    failure = {}
    for attempt in range(2):
        response = None
        try:
            connection = getattr(HTTP_STATE, 'connection', None)
            if connection is None:
                connection = http.client.HTTPSConnection('apps.razer.com', timeout=20)
                HTTP_STATE.connection = connection
            connection.request('GET', f'/synapse/products/{product}/mw/{name}', headers={
                'User-Agent': 'RazerSourceAudit/1.0 (static bytes only)', 'Cache-Control': 'no-cache'})
            response = connection.getresponse()
            body = response.read(MAX_FILE_BYTES + 1)
            if response.status != 200:
                failure = {'http_status': response.status, 'error': f'HTTP {response.status}: {response.reason}'}
                if response.status in (403, 404, 410):
                    break
                raise ValueError(f'Unexpected HTTP status {response.status}; redirects are not followed')
            if len(body) > MAX_FILE_BYTES:
                raise ValueError('Source exceeds per-file 16MiB cap')
            if not body.strip() or body.lstrip().lower().startswith((b'<!doctype', b'<html')):
                raise ValueError('HTML/empty response for JavaScript')
            receipt = {'source_url': url, 'final_url': url, 'http_status': response.status,
                'fetched_at_utc': utc(), 'sha256': digest(body), 'bytes': len(body),
                'etag': response.getheader('ETag'), 'last_modified': response.getheader('Last-Modified'),
                'content_type': response.getheader('Content-Type'), 'body_file': name,
                'manifest_path': f'.ref/middleware/{product}/webpackManifest.json'}
            # The workspace may be shared with other static acquisitions. Preserve
            # their bytes; never replace even an equal existing source file.
            with target.open('xb') as stream:
                stream.write(body)
            write_json(directory / (name + '.http.json'), receipt)
            return {**job, 'status': 'downloaded', 'source_url': url, 'sha256': receipt['sha256'], 'bytes': len(body)}
        except FileExistsError:
            return {**job, **(existing_receipt(product, name) or {'status': 'preserved_concurrent_source'})}
        except (OSError, ValueError, http.client.HTTPException) as exc:
            connection = getattr(HTTP_STATE, 'connection', None)
            if connection:
                connection.close()
                HTTP_STATE.connection = None
            failure = {'http_status': getattr(response, 'status', None), 'error': str(exc)}
        if attempt == 0:
            time.sleep(0.5)
    result = {**job, 'source_url': url, 'checked_at_utc': utc(),
        'status': 'not_found' if failure.get('http_status') in (404, 410) else 'error', **failure}
    write_json(directory / (name + '.fetch-error.json'), result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workers', type=int, default=24)
    parser.add_argument('--batch-size', type=int, default=512)
    parser.add_argument('--limit', type=int, help='Sample only this many outstanding requests; explicitly recorded as incomplete')
    parser.add_argument('--plan-only', action='store_true')
    parser.add_argument('--verify-only', action='store_true', help='No network; validate acquired bytes and update the aggregate report')
    args = parser.parse_args()
    if not 1 <= args.workers <= 24 or not 1 <= args.batch_size <= 2048:
        parser.error('workers must be 1..24 and batch-size 1..2048')
    manifests, missing, existing = collect()
    if REQUESTS.exists():
        pinned = read_json(REQUESTS)
        if pinned['manifests'] != manifests:
            raise ValueError('Pinned middleware manifest set changed; existing request baseline preserved')
        original_jobs = pinned['requests']
    else:
        original_jobs = missing
        write_json(REQUESTS, {'method': 'Static manifest values only; no vendor execution',
            'planned_at_utc': utc(), 'manifests': manifests, 'requests': original_jobs,
            'existing_js_at_plan': len(existing), 'unique_missing_filenames_at_plan': len({j['file'] for j in missing})})
    print(f'Pinned manifests={len(manifests)}, original missing={len(original_jobs)}, now missing={len(missing)}', flush=True)
    if args.plan_only:
        return
    baseline = {(j['product_id'], j['file']) for j in original_jobs}
    outstanding = [j for j in missing if (j['product_id'], j['file']) in baseline]
    if args.verify_only:
        outstanding = []
    if args.limit is not None:
        outstanding = outstanding[:args.limit]
    results = read_json(PROGRESS).get('results', []) if PROGRESS.exists() else []
    by_job = {(r['product_id'], r['file']): r for r in results}
    started = time.monotonic()
    completed = 0
    counts = {}
    for batch_start in range(0, len(outstanding), args.batch_size):
        jobs = outstanding[batch_start:batch_start + args.batch_size]
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
            futures = [pool.submit(fetch, job) for job in jobs]
            for future in concurrent.futures.as_completed(futures):
                result = future.result()
                by_job[(result['product_id'], result['file'])] = result
                counts[result['status']] = counts.get(result['status'], 0) + 1
                completed += 1
                if completed % 100 == 0 or completed == len(outstanding):
                    print(f'JS {completed}/{len(outstanding)}, seconds={time.monotonic()-started:.1f}, {counts}', flush=True)
        write_json(PROGRESS, {'updated_at_utc': utc(), 'results': list(by_job.values())})
    # Final verification is local: downloaded bytes are matched to per-file
    # source receipts, and every pinned manifest SHA is checked again.
    current_manifests, now_missing, _ = collect()
    if manifests != current_manifests:
        raise ValueError('Manifest changed during static source acquisition')
    verified, unresolved = [], []
    for job in original_jobs:
        status = existing_receipt(job['product_id'], job['file'])
        if status and status['status'] == 'verified_existing':
            verified.append({**job, **status})
        else:
            unresolved.append(by_job.get((job['product_id'], job['file']), {**job, 'status': 'not_attempted'}))
    report = {'schema': 1, 'checked_at_utc': utc(),
        'method': 'Pinned manifest-declared missing JavaScript bytes over official HTTPS; no vendor execution or source overwrite',
        'source_version_scope': 'Already acquired current middleware manifests, not a new live version refresh',
        'manifest_count': len(manifests), 'manifests': manifests, 'requests_path': REQUESTS.relative_to(ROOT).as_posix(),
        'original_missing_paths': len(original_jobs), 'unique_missing_filenames_at_plan': len({j['file'] for j in original_jobs}),
        'verified_paths': len(verified), 'verified_bytes': sum(r['bytes'] for r in verified),
        'unresolved_paths': len(unresolved), 'currently_missing_manifest_js_paths': len(now_missing),
        'all_original_missing_acquired': not unresolved, 'workers': args.workers, 'per_file_byte_cap': MAX_FILE_BYTES,
        'results': verified, 'unresolved': unresolved,
        'boundaries': ['Downloaded source inventory is not all-function semantic reverse engineering',
            'No DLL or application execution', 'Manifest hashed filenames do not substitute for independent expected full-byte hashes']}
    write_json(OUTPUT, report)
    print(f'Wrote {OUTPUT}: verified={len(verified)}, bytes={report["verified_bytes"]}, unresolved={len(unresolved)}', flush=True)


if __name__ == '__main__':
    main()
