"""Fetch the two current middleware entries and their declared scripts as inert bytes."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from html.parser import HTMLParser
import hashlib
import json
from pathlib import Path
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent.parent

class Entry(HTMLParser):
    def __init__(self):
        super().__init__()
        self.scripts = []
    def handle_starttag(self, tag, attrs):
        if tag == 'script':
            src = dict(attrs).get('src')
            if src:
                self.scripts.append(src)

def product(pid):
    base = f'https://apps.razer.com/synapse/products/{pid}/mw/'
    directory = ROOT / f'local-ui-reverse/source/official/apps.razer.com/synapse/products/{pid}/mw'
    def fetch(relative):
        url = urllib.parse.urljoin(base, relative)
        assert url.startswith(base) and '..' not in relative.split('/')
        request = urllib.request.Request(url, headers={'User-Agent':'RazerStaticSourceAudit/1.0'})
        with urllib.request.urlopen(request, timeout=35) as response:
            raw = response.read()
            assert response.status == 200 and response.url == url
        target = directory / url[len(base):]
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
        receipt = {'path': target.relative_to(ROOT).as_posix(), 'url': url,
                   'retrieved_utc': datetime.now(timezone.utc).isoformat(),
                   'sha256': hashlib.sha256(raw).hexdigest(), 'bytes':len(raw), 'http_status':200}
        target.with_name(target.name+'.http.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
        return raw,receipt
    raw,entry = fetch('index.html')
    parser = Entry();parser.feed(raw.decode('utf-8'))
    assert parser.scripts, f'No entry scripts for {pid}'
    receipts = [entry]
    shared_scripts = []
    for script in parser.scripts:
        if not urllib.parse.urljoin(base, script).startswith(base):
            # Shared libraries are separately acquired inputs. Retain their
            # URLs but fetch only this product's declared middleware here.
            shared_scripts.append(urllib.parse.urljoin(base, script))
            continue
        _,receipt = fetch(script);receipts.append(receipt)
    result = {'product_id':pid,'method':'Current official entry scripts downloaded as data, never executed','receipts':receipts,'shared_script_urls':shared_scripts}
    (ROOT / f'docs/re/gamepad-{pid}-middleware-current-acquisition.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    return {'pid':pid,'scripts':len(receipts)-1,'bytes':sum(r['bytes'] for r in receipts)}

if __name__ == '__main__':
    with ThreadPoolExecutor(max_workers=2) as pool:
        for result in pool.map(product,[2676,2684]):
            print(json.dumps(result))
