"""Resolve current product artwork through literal Webpack modules, never JS execution."""
import concurrent.futures
import hashlib
import json
import re
import urllib.request
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
MODULE = re.compile(r'(?<![\w])(?P<id>\d+):\([^)]*\)=>\{(?:"use strict";)?\w+\.exports=\w+\.p\+"(?P<path>static/media/[^"]+)";?\}')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(raw_path):
    raw = json.loads(raw_path.read_text(encoding='utf8'))
    pid = raw['product_id']
    info = raw['config']['DeviceInfo']
    product = ROOT / f'.ref/devices/{pid}'
    main = next((product/'static/js').glob('main.*.js'))
    scripts = {script:script.read_text(encoding='utf8') for script in (product/'static/js').glob('*.js') if not script.name.startswith('trans-')}
    requests = {}
    for script, source in scripts.items():
        for request, module in re.findall(r'"(\./'+str(pid)+r'_0/(?:img|svg)_prods/[^"\\]+)":(?:\[)?(\d+(?:e\d+)?)', source):
            requests[request] = (int(float(module)), script)
    category = info.get('category')
    if info.get('UseSVGForProductImage') or pid == 747:
        request = f'./{pid}_0/svg_prods/'+('prd.svg' if category == 'KEYPAD' else '1.svg')
    else:
        request = f'./{pid}_0/img_prods/'+('prd-3x.png' if category == 'KEYPAD' else '1-3x.png')
    if request not in requests:
        return {'product_id': pid, 'error': f'Exact default request unavailable: {request}'}
    module, context = requests[request]
    matches = []
    for script, source in scripts.items():
        for match in MODULE.finditer(source):
            if int(match['id']) == module:
                matches.append((match['path'], script, match.start()))
    if len(matches) != 1:
        return {'product_id': pid, 'error': f'Expected one media-export module {module}, found {len(matches)}'}
    media, script, offset = matches[0]
    path = product/media
    url = f'https://apps.razer.com/synapse/products/{pid}/ui/{media}'
    if not path.exists():
        request_http = urllib.request.Request(url, headers={'User-Agent': 'StaticSourceAudit/1.0'})
        with urllib.request.urlopen(request_http, timeout=60) as response:
            data = response.read()
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    result = dict(product_id=pid, edition_id=0, layout_id=1, request=request, module=module,
                  context=str(context.relative_to(ROOT)).replace('\\','/'), context_sha256=digest(context),
                  module_file=str(script.relative_to(ROOT)).replace('\\','/'), module_sha256=digest(script), offset=offset,
                  source=str(path.relative_to(ROOT)).replace('\\','/'), source_url=url, source_sha256=digest(path))
    if path.suffix == '.svg':
        result['svg'] = True
    else:
        output = ROOT / f'assets/synapse/keyboard-product-{pid}.png'
        with Image.open(path) as source_image:
            image = source_image.convert('RGBA')
            image.save(output, optimize=True)
            result.update(width=image.width, height=image.height)
        result.update(output=str(output.relative_to(ROOT)).replace('\\','/'), output_sha256=digest(output))
    return result


if __name__ == '__main__':
    files = sorted((ROOT/'assets/synapse/keyboard-products').glob('*.json'))
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        futures = {executor.submit(prepare, path): path for path in files}
        results = []
        for future in concurrent.futures.as_completed(futures):
            try:
                result = future.result()
            except Exception as error:
                result = {'product_id': int(futures[future].stem), 'error': str(error)}
            results.append(result)
            print(json.dumps(result, ensure_ascii=False), flush=True)
    (ROOT/'docs/re/keyboard-product-artwork.json').write_text(json.dumps(sorted(results,key=lambda r:r['product_id']),indent=2),encoding='utf8')
