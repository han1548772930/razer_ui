"""Static receipts for current systray populated widgets.

This reads current extracted source as data. It never evaluates the vendor
JavaScript, starts the application, loads a DLL, or performs device I/O.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / ".ref/applications/systray/systrayv2"
MANIFEST = BASE / "asset-manifest.json"
CSS = BASE / "static/css/492.d907fb85.chunk.css"
JS = BASE / "static/js/492.2b959cf3.chunk.js"
MAIN_CSS = BASE / "static/css/main.1665f0a2.css"
SECTION_CSS = BASE / "static/css/554.7cdbd936.chunk.css"
MAIN_JS = BASE / "static/js/main.9579c403.js"
OUT = ROOT / "docs/re/tray-widgets-current-evidence.json"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def excerpt(path: Path, needle: str) -> dict[str, object]:
    source = path.read_text(encoding="utf-8")
    start = source.index(needle)
    return {"path": path.relative_to(ROOT).as_posix(), "start": start, "end": start + len(needle), "source": needle}


def main() -> None:
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    assert manifest["files"]["static/js/492.2b959cf3.chunk.js"] == "./static/js/492.2b959cf3.chunk.js"
    css = CSS.read_text(encoding="utf-8")
    js = JS.read_text(encoding="utf-8")
    main_css = MAIN_CSS.read_text(encoding="utf-8")
    section_css = SECTION_CSS.read_text(encoding="utf-8")
    main_js = MAIN_JS.read_text(encoding="utf-8")
    css_facts = [
        ".synapse>.devices>li{align-items:center;display:flex;padding:10px 20px}",
        ".synapse>.devices>li>.icon{background-position:50%;background-repeat:no-repeat;background-size:cover;flex-shrink:0;height:40px;margin-right:10px;width:40px}",
        ".synapse>.devices>li>.info{margin-left:10px;max-width:220px;min-width:0}",
        ".synapse>.devices>li>.info>.title{color:#999;font-size:12px;text-transform:uppercase}",
        ".synapse>.devices>li>.battery{align-items:center;display:flex;flex-shrink:0;margin-left:auto;padding-left:10px;text-align:right}",
        ".synapse>.devices>li>.battery>.label{font-size:14px}",
        ".synapse>.devices>li>.battery .icon{background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;width:26px}",
        ".synapse>.devices>li.active{background-color:initial}",
        ".synapse-default{align-items:center;display:flex;padding:14px 24px}",
        ".synapse>.devices>li>svg{min-width:40px}",
        ".synapse-section-title.app-section-title{z-index:2}",
        ".synapse>.devices>li>.info>.profile.disabled{pointer-events:none}",
        ".synapse>.devices>li>.info>.profile.disabled:after{opacity:.3}",
    ]
    for fact in css_facts:
        assert fact in css, fact
    for fact in [
        ".hover-bg-color:hover{background-color:#111}",
        ".hover-bg-color:active{background-color:#393939}",
        "button{background-color:initial;border:none;border-radius:0;line-height:1;outline:none;padding:0}",
    ]:
        assert fact in main_css, fact
    js_facts = [
        'e.length>0?(0,v.jsx)("ul",{className:"devices list-unstyled"',
        'C=60*e.length',
        'deviceInitStatusFail)!==T.MIXER_SYSTEM_CHECK_FAILED',
        'className:`profile ${T[s]?"active":""}',
        '"battery",{[`battery-${D(r.level)}',
        'const D=e=>{let t=0;return t=10===e?10:e>10&&e<20?20:10*Math.floor(e/10),t}',
    ]
    for fact in js_facts:
        assert fact in js, fact
    js_facts += [
        'const f=function(){let e=arguments.length>0&&void 0!==arguments[0]?arguments[0]:{};const t=Object.keys(h.Su);let a=u.find(a=>t.includes(e.category)&&e.subCategory?a.name===e.subCategory:a.name===e.category);return a?(0,v.jsx)(M,{d:a.d,width:40,height:40,transformX:a.transformX,transformY:a.transformY}):null}',
        'children:[f(t),(0,v.jsxs)("div",{className:"info"',
        'width:t,height:a,fill:s||"#ccc",viewBox:`0 0 ${t/1.5} ${a/1.5}`',
        'className:"synapse-section-title app-section-title",children:U(h.ut,t)',
    ]
    category_fact = 'C={KEYBOARD:"KEYBOARD",MOUSE:"MOUSE",MOUSEPLUSMAT:"MOUSEPLUSMAT",GAMEPAD:"GAMEPAD",MONITOR:"MONITOR",CASE:"CASE",AUDIO:"AUDIO",BROADCASTER:"BROADCASTER",EGPU:"EGPU",SYSTEM:"SYSTEM",KEYPAD:"KEYPAD",CHROMAHDK:"CHROMAHDK",ACCESSORY:"ACCESSORY",MOUSEMAT:"MOUSEMAT",IOT:"IOT"}'
    assert category_fact in main_js
    section_fact = '.systray .app-section-title{background-color:#222;border-top:2px solid #111;color:#999;font-size:10px;padding:6px 0;position:sticky;text-align:center;text-transform:uppercase;top:0;z-index:0}'
    assert section_fact in section_css
    assets = json.loads((ROOT / 'assets/synapse/tray-widget-assets.json').read_text(encoding='utf8'))
    for asset in assets:
        assert digest(ROOT / asset['source']) == asset['source_sha256']
        assert digest(ROOT / asset['output']) == asset['output_sha256']
    implementation = (ROOT / 'src/shell/tray/widgets.rs').read_text(encoding='utf8')
    assert '.mr(surface::css(10.))' not in implementation
    assert 'source_category_icon' in implementation
    assert 'widget_list_with_click' in implementation
    assert 'synapse/tray-widget-battery-off.svg' in implementation
    embedded = (ROOT / 'assets/synapse/tray-widget-embedded.rs').read_text(encoding='utf8')
    for asset in assets:
        assert asset['output'].removeprefix('assets/') in embedded
    resources = (ROOT / 'src/resources.rs').read_text(encoding='utf8')
    assert '.chain(TRAY_WIDGET_ASSETS)' in resources
    evidence = {
        "schema_version": 2,
        "source_inputs": {str(p.relative_to(ROOT)).replace("\\", "/"): digest(p) for p in [MANIFEST, JS, CSS, MAIN_CSS, MAIN_JS, SECTION_CSS]},
        "css_receipts": [excerpt(CSS, fact) for fact in css_facts],
        "main_css_receipts": [excerpt(MAIN_CSS, fact) for fact in [
            ".hover-bg-color:hover{background-color:#111}",
            ".hover-bg-color:active{background-color:#393939}",
            "button{background-color:initial;border:none;border-radius:0;line-height:1;outline:none;padding:0}",
        ]],
        "js_receipts": [excerpt(JS, fact) for fact in js_facts],
        "category_receipt": excerpt(MAIN_JS, category_fact),
        "section_title_receipt": excerpt(SECTION_CSS, section_fact),
        "assets": assets,
        "implementation": {
            "path": "src/shell/tray/widgets.rs",
            "source_contract": [
                "40px category SVG from current u/M/f; bare SVG has no .icon trailing margin",
                "only info margin-left contributes the 10px icon-to-text gap",
                "row padding 10px 20px and hover/active surfaces",
                "220px info lane with profile/title truncation",
                "battery label/icon lane and source ten-point bucket mapping",
                "empty app-section-default row height 60px",
                "app section title 10px/1.22 text + 6px vertical padding + 2px top border = 26.2px",
                "battery artwork from current tray manifest; normal 20px artwork centered in 26px lane",
                "profile disabled dims only the CSS triangle, not the label",
                "parent-owned device and title callbacks, with inert measurement wrapper",
            ],
            "runtime_validation": "not_run",
        },
        "remaining": [
            "The parent supplies observed devices; observed source profile/channel data is still required.",
            "Profile dropdown portal and row activation transport remain parent-owned.",
            "Section title sticky scrolling and all profile hover/portal interactions require parent ownership.",
        ],
    }
    serialized = json.dumps(evidence, indent=2, ensure_ascii=False) + "\n"
    if "--write" in __import__("sys").argv:
        OUT.write_text(serialized, encoding="utf-8")
    else:
        assert OUT.read_text(encoding="utf-8").replace("\r\n", "\n") == serialized
    print("Current systray widget CSS/JS receipts and source-derived widget implementation checked.")


if __name__ == "__main__":
    main()

