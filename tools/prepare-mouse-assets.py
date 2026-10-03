"""Resolve product illustrations from current webpack contexts; download inert images only."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path
import re
import urllib.request
import xml.etree.ElementTree as ET
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
products = json.loads((ROOT / "docs/re/mouse-product-source.json").read_text(encoding="utf-8"))["products"]
output = ROOT / "assets/synapse/mouse-products"
output.mkdir(parents=True, exist_ok=True)

def prepare(product):
    pid = product["product_id"]
    source_pid = product["config"]["exports"]["DeviceInfo"]["productId"]
    directory = ROOT / f".ref/devices/{pid}"
    sources = [(p, p.read_text(encoding="utf-8")) for p in (directory / "static/js").glob("*.js")]
    selected = None
    direct_media = None
    for relative_request in ["img_prods/prd-3x.png", "img_prods/prd-1x.png", "img_prods/prd.png", "img_prods/prd-3x.svg", "img_prods/prd-1x.svg", "svg_prods/prd.svg"]:
        request = f"./{source_pid}_0/{relative_request}"
        for context_path, source in sources:
            found = re.search(re.escape(json.dumps(request)) + r":(?:\[)?(\d+)(?=[,}])", source)
            if found:
                selected = (request, int(found[1]), context_path, found.start())
                break
        if selected:
            break
    if not selected:
        # 231's audited product configuration directly selects module 560.jm[0]
        # as topView, instead of using the shared image context. Keep this exact
        # current-source contract explicit; a changed module needs re-auditing.
        if pid == 231:
            for context_path, source in sources:
                found = re.search(r'r=\w+\.p\+"(static/media/topview\.[^"/]+\.avif)"', source)
                if found and 'jm:()=>d' in source and 'd={0:r,1:I,2:O,3:A}' in source and '{name:"topView",img:i.jm[0]}' in source:
                    selected = ('CONFIG topView -> module 560.jm[0]', 560, context_path, found.start())
                    direct_media = found[1]
                    break
        if not selected:
            return {"product_id": pid, "status": "no_default_illustration_context"}
    request, module, context_path, context_offset = selected
    matches = [(context_path, direct_media)] if direct_media is not None else []
    if direct_media is None:
        for module_path, source in sources:
            for found in re.finditer(r"(?:\{|,)" + str(module) + r":(?:(?:\([^)]*\)|[\w$]+)=>|function\([^)]*\))\{[^}]{0,300}?exports=[^+]+\+\"(static/media/[^\"]+)\"", source):
                matches.append((module_path, found[1]))
    if len(set(m[1] for m in matches)) != 1:
        return {"product_id": pid, "status": "illustration_module_unresolved", "request": request, "module": module}
    module_path, relative = matches[0]
    source_path = directory / relative
    source_url = f"https://apps.razer.com/synapse/products/{pid}/ui/{relative}"
    if source_path.exists():
        data = source_path.read_bytes()
    else:
        with urllib.request.urlopen(urllib.request.Request(source_url, headers={"User-Agent": "Razer-UI-static-resource-audit/1"}), timeout=45) as response:
            data = response.read()
        source_path.parent.mkdir(parents=True, exist_ok=True)
        source_path.write_bytes(data)
    target = output / f"{pid}{'.svg' if source_path.suffix == '.svg' else '.png'}"
    if source_path.suffix == '.svg':
        svg = ET.fromstring(data)
        assert all(node.tag.split('}')[-1] not in ('script','foreignObject') for node in svg.iter())
        target.write_bytes(data)
    else:
        with Image.open(io.BytesIO(data)) as image:
            image.convert("RGBA").save(target)
    return {"product_id": pid, "status": "prepared", "request": request, "module": module,
            "context": str(context_path.relative_to(ROOT)).replace("\\", "/"), "context_offset": context_offset,
            "module_source": str(module_path.relative_to(ROOT)).replace("\\", "/"),
            "module_sha256": hashlib.sha256(module_path.read_bytes()).hexdigest(),
            "source": str(source_path.relative_to(ROOT)).replace("\\", "/"), "source_url": source_url,
            "source_sha256": hashlib.sha256(data).hexdigest(), "output": str(target.relative_to(ROOT)).replace("\\", "/"),
            "output_sha256": hashlib.sha256(target.read_bytes()).hexdigest()}

with ThreadPoolExecutor(max_workers=6) as executor:
    receipts = list(executor.map(prepare, products))
(ROOT / "docs/re/mouse-product-assets.json").write_text(json.dumps(receipts, indent=2) + "\n", encoding="utf-8")
# The application-wide resource preparer owns embedding; this scanner records
# verified inert assets only, including SVGs that must retain their extension.
print({status: sum(r["status"] == status for r in receipts) for status in sorted({r["status"] for r in receipts})})
