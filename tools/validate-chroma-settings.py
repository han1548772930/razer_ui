"""Validate generated settings SVG/XML, hashes and embedded registration."""
from pathlib import Path
import hashlib
import json
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
evidence = json.loads((ROOT / "docs/re/chroma-settings-current-evidence.json").read_text(encoding="utf-8"))
embedded = (ROOT / "assets/synapse/chroma-settings-embedded.rs").read_text(encoding="utf-8")
resources = (ROOT / "crates/razer-assets/src/lib.rs").read_text(encoding="utf-8")
assert resources.count(".chain(CHROMA_SETTINGS_ASSETS)") == 2
assert len(evidence["outputAssets"]) == 25
for asset in evidence["outputAssets"]:
    file = ROOT / asset["path"]
    data = file.read_bytes()
    assert hashlib.sha256(data).hexdigest() == asset["sha256"], file
    svg = ET.fromstring(data)
    assert svg.tag == "{http://www.w3.org/2000/svg}svg", file
    assert svg.attrib.get("viewBox"), file
    assert not any(e.tag.endswith("}script") for e in svg.iter()), file
    assert embedded.count('include_bytes!("' + file.name + '")') == 1, file
for name in ["facebook", "ig", "twitter", "youtube", "tiktok", "twitch", "discord"]:
    for suffix in ["", "-hover"]:
        svg = ET.fromstring((ROOT / f"assets/synapse/chroma-settings-{name}{suffix}.svg").read_bytes())
        style = svg.find("{http://www.w3.org/2000/svg}style").text
        assert ".ellipse{fill:#222" in style and ".social{fill:#999999" in style
        if suffix:
            assert ".social{fill:#44D62C;}" in style
print("25 Chroma Settings SVGs: XML, hashes, shared social styles and embedded load/list validated.")
