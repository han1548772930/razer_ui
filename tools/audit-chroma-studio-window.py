"""Static current host/Studio ownership, chrome, rename and favicon receipts."""
import argparse
import hashlib
import json
import pathlib
import re
import urllib.request
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--fetch", action="store_true")
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
receipts = []


def sha(value):
    return hashlib.sha256(value).hexdigest()


def receipt(path, start, end, symbol):
    data = (ROOT / path).read_bytes()
    text = data.decode("utf-8")
    a = text.index(start)
    b = text.index(end, a) if end else a + len(start)
    receipts.append(dict(path=path, sha256=sha(data), symbol=symbol,
                         offset=len(text[:a].encode("utf-16-le")) // 2,
                         end=len(text[:b].encode("utf-16-le")) // 2,
                         source=text[a:b]))
    return text[a:b]


host = ".ref/host-4.0.827/source-evidence/background-current-source.js"
receipt(host, "class MC{", "const LC=new MC", "MC SubTabManager")
receipt(host, 'PC.init("chroma-app"', ")},autoOpen:", "PC Chroma host child order")
receipt(host, '{key:"chroma-studio",title:', ',{key:"chroma-connect"', "Studio title")
tab = ".ref/host-4.0.827/electron/components/Tab/Tab.js"
receipt(tab, 'this.tab.webContents.on("page-favicon-updated"', 'this.tab.webContents.on("did-start-loading"', "favicon event")
dashboard = ".ref/applications/chroma-app/dashboard/static/js/main.e3feeb1b.js"
flags = receipt(dashboard, 'D_="policy=5,tab_visible=0,app_name=chroma-app', ',L_=', "Chroma parent window flags")
assert 'width=1280,height=720,minimum_width=600,minimum_height=500' in flags
editor = ".ref/applications/synapse/chroma-studio/static/js/EditorCanvas.c3ee330f.chunk.js"
receipt(editor, "function Z(e){let{layer:", "const K=o().memo(J)", "4264 Z layer rename/visibility")
main = ".ref/applications/synapse/chroma-studio/static/js/main.6b22e9cc.js"
text = (ROOT / main).read_text(encoding="utf-8")
module = text[text.index("6257:"):]
title_limit = re.search(r'L=\{NODE_ENV.*?REACT_APP_TITLE_LIMIT\):32', module).group()
receipt(main, title_limit, None, "6257 L title limit")
css_path = ".ref/host-4.0.827/electron/index.css"
css_data = (ROOT / css_path).read_bytes()
css = [match.group() for match in re.finditer(r'[^{}]+\{[^{}]*\}', css_data.decode())
       if any(key in match.group() for key in [".etabs-tab", ".main-tab", ".etabs-content", ".etabs-window-control"])]
icon_path = ROOT / ".ref/applications/synapse/chroma-studio/favicon.svg"
url = "https://apps.razer.com/synapse/chroma-studio/favicon.svg"
if not icon_path.exists():
    assert args.fetch and not args.check, "Missing Studio favicon; use --fetch for this SVG data only"
    with urllib.request.urlopen(url, timeout=45) as response:
        data = response.read(4 * 1024 * 1024 + 1)
        assert response.status == 200 and len(data) <= 4 * 1024 * 1024
        assert ET.fromstring(data).tag.endswith("svg")
        icon_path.write_bytes(data)
icon = icon_path.read_bytes()
html = ".ref/applications/synapse/chroma-studio/index.html"
receipt(html, '<link rel="shortcut icon" href="./favicon.svg"/>', None, "Studio favicon link")
output_icon = ROOT / "assets/synapse/host-chroma-studio-favicon.svg"
record = dict(method="Current sources read statically; only favicon SVG downloaded as data",
              parent="chroma-app", child="chroma-studio", child_policy=3,
              parent_size=[1280, 720], parent_minimum_size=[600, 500],
              receipts=receipts,
              css=dict(path=css_path, sha256=sha(css_data), rules=css),
              favicon=dict(url=url, source=icon_path.relative_to(ROOT).as_posix(),
                           output=output_icon.relative_to(ROOT).as_posix(), sha256=sha(icon)))
for path, data in [(output_icon, icon), (ROOT / "docs/re/chroma-studio-window-source.json", (json.dumps(record, ensure_ascii=False, indent=2) + "\n").encode())]:
    if args.check:
        assert path.read_bytes() == data, str(path)
    else:
        path.write_bytes(data)
print(f"Validated {len(receipts)} current Studio host/rename receipts and {len(css)} host CSS rules")
