"""Acquire current middleware catalog sources as bytes; never execute vendor code.

The initial pass gets HTML/manifests and the declared main script for every
official catalog or already acquired current UI product. Further source files
must be requested by a static parser and listed in that product's manifest.
"""
import argparse
import concurrent.futures
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / '.ref/middleware'


def read_json(path):
    return json.loads(path.read_text(encoding='utf8'))


def fetch(product, name):
    if not re.fullmatch(r'[A-Za-z0-9_.-]+\.(?:html|json|js)', name):
        raise ValueError('Unsafe source name: ' + name)
    directory = BASE / str(product)
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / name
    receipt_path = directory / (name + '.http.json')
    url = f'https://apps.razer.com/synapse/products/{product}/mw/{name}'
    if target.is_file() and receipt_path.is_file():
        receipt = read_json(receipt_path)
        body = target.read_bytes()
        if receipt.get('source_url') != url or receipt.get('final_url') != url or receipt.get('http_status') != 200 or receipt.get('sha256') != hashlib.sha256(body).hexdigest() or receipt.get('bytes') != len(body):
            raise ValueError(f'Invalid existing acquisition: {target}')
        return {'product_id': product, 'file': name, 'status': 'verified_existing', 'sha256': receipt['sha256']}
    error = None
    for attempt in range(3):
        try:
            request = urllib.request.Request(url, headers={'User-Agent': 'RazerSourceAudit/1.0 (static bytes only)', 'Cache-Control': 'no-cache'})
            with urllib.request.urlopen(request, timeout=30) as response:
                if response.url != url or response.status != 200:
                    raise ValueError('Unexpected source redirect/status: ' + response.url)
                body = response.read()
                if name.endswith('.json'):
                    json.loads(body)
                if name.endswith('.js') and (not body.strip() or body.lstrip().lower().startswith((b'<!doctype', b'<html'))):
                    raise ValueError('HTML/empty response for JavaScript')
                receipt = {'source_url': url, 'final_url': response.url, 'http_status': response.status,
                    'fetched_at_utc': datetime.now(timezone.utc).isoformat(), 'sha256': hashlib.sha256(body).hexdigest(),
                    'bytes': len(body), 'etag': response.headers.get('ETag'), 'last_modified': response.headers.get('Last-Modified'),
                    'content_type': response.headers.get('Content-Type'), 'body_file': name}
            if target.exists() and target.read_bytes() != body:
                raise ValueError('Official bytes differ from existing source; preserved: ' + str(target))
            target.write_bytes(body)
            receipt_path.write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf8')
            return {'product_id': product, 'file': name, 'status': 'downloaded', 'sha256': receipt['sha256']}
        except urllib.error.HTTPError as exc:
            error = {'http_status': exc.code, 'error': str(exc)}
            if exc.code == 404:
                break
        except (OSError, ValueError) as exc:
            error = {'http_status': None, 'error': str(exc)}
        if attempt < 2:
            time.sleep(0.3 * (attempt + 1))
    failure = {'product_id': product, 'file': name, 'source_url': url, 'checked_at_utc': datetime.now(timezone.utc).isoformat(),
        'status': 'not_found' if error['http_status'] == 404 else 'error', **error}
    (directory / (name + '.fetch-error.json')).write_text(json.dumps(failure, indent=2) + '\n', encoding='utf8')
    return failure


def batch(jobs, workers):
    results = []
    counts = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(fetch, product, name) for product, name in sorted(set(jobs))]
        for index, future in enumerate(concurrent.futures.as_completed(futures), 1):
            result = future.result()
            results.append(result)
            counts[result['status']] = counts.get(result['status'], 0) + 1
            if index % 100 == 0 or index == len(futures):
                print(f'Source bytes {index}/{len(futures)}: {counts}', flush=True)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--requests', type=Path)
    parser.add_argument('--workers', type=int, default=16)
    args = parser.parse_args()
    if not 1 <= args.workers <= 24:
        parser.error('workers must be 1..24')
    if args.requests:
        requests = read_json(args.requests)
        jobs = []
        for request in requests['requests']:
            product, name = request['product_id'], request['file']
            if not isinstance(product, int) or product <= 0:
                raise ValueError('Invalid product')
            manifest = read_json(BASE / str(product) / 'webpackManifest.json')
            if name not in manifest.values():
                raise ValueError('Requested file absent from source manifest: ' + name)
            jobs.append((product, name))
        output = {'stage': 'parser_requested_sources', 'results': batch(jobs, args.workers)}
        destination = BASE / (args.requests.stem + '-acquisition.json')
    else:
        catalog = read_json(ROOT / '.ref/applications/synapse/dashboard/AvailableDevices.json')
        products = {item['productId'] for item in catalog}
        products.update(int(path.name) for path in (ROOT / '.ref/devices').iterdir() if path.is_dir() and path.name.isdigit())
        print(f'Current source scope: {len(products)} products', flush=True)
        results = batch([(product, name) for product in products for name in ('index.html', 'manifest.json', 'webpackManifest.json')], args.workers)
        main_jobs = []
        for product in sorted(products):
            directory = BASE / str(product)
            if not all((directory / name).is_file() for name in ('index.html', 'webpackManifest.json')):
                continue
            manifest = read_json(directory / 'webpackManifest.json')
            main_file = manifest.get('main.js')
            if not isinstance(main_file, str) or main_file not in (directory / 'index.html').read_text(encoding='utf8'):
                raise ValueError(f'HTML/main manifest mismatch: {product}')
            main_jobs.append((product, main_file))
        results.extend(batch(main_jobs, args.workers))
        output = {'stage': 'current_catalog_entrypoints', 'products': sorted(products), 'results': results}
        destination = BASE / 'CATALOG-ACQUISITION.json'
    output['method'] = 'Official HTTPS source byte acquisition and SHA-256 verification only; no vendor execution'
    destination.write_text(json.dumps(output, indent=2) + '\n', encoding='utf8')
    print('Wrote ' + str(destination), flush=True)


if __name__ == '__main__':
    main()
