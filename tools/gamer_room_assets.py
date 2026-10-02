"""Extract Gamer Room's inline SVG literals without evaluating JavaScript."""
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path


def _attrs(text):
    names = {"fillRule": "fill-rule", "clipRule": "clip-rule"}
    return {
        names.get(key, key): str(json.loads(value)) if value.startswith('"') else value
        for key, value in re.findall(r'(\w+):("(?:\\.|[^"\\])*"|-?\d+(?:\.\d+)?)', text)
    }


def _svg(width, height):
    return ET.Element("svg", {"xmlns": "http://www.w3.org/2000/svg",
                              "width": str(width), "height": str(height),
                              "viewBox": f"0 0 {width} {height}", "fill": "none"})


def prepare(root, out, record):
    root, out = Path(root), Path(out)
    source = root / ".ref/frontend/static/js/9388.2bec5db3.chunk.js"
    text = source.read_text(encoding="utf-8")
    out.mkdir(parents=True, exist_ok=True)

    def component(start, end):
        offset = text.index(start)
        return offset, text[offset:text.index(end, offset)]

    def save(name, svg, offset, symbol, **metadata):
        destination = out / name
        ET.ElementTree(svg).write(destination, encoding="utf-8", xml_declaration=True)
        record(source, destination, module=19388, component=symbol,
               source_offset=offset, source_kind="inline_svg_literal",
               transform="Static extraction of original SVG attributes; no JavaScript evaluation",
               **metadata)

    offset, raw = component("const A=", "B=(0,r.forwardRef)(A)")
    mask_path = re.search(r'r\.createElement\("path",\{([^}]+)\}', raw)
    texture = re.search(r'xlinkHref:"(data:image/png;base64,[A-Za-z0-9+/=]+)"', raw)
    assert mask_path and texture, "Missing original Synapse mask or texture"
    svg = _svg(100, 100)
    svg.set("xmlns:xlink", "http://www.w3.org/1999/xlink")
    ET.SubElement(svg, "rect", {"width": "100", "height": "100", "rx": "50", "fill": "black"})
    mask = ET.SubElement(svg, "mask", {"id": "mask0_1307_7666", "style": "mask-type:luminance",
                                      "maskUnits": "userSpaceOnUse", "x": "3", "y": "3", "width": "94", "height": "94"})
    ET.SubElement(mask, "path", _attrs(mask_path[1]))
    group = ET.SubElement(svg, "g", {"mask": "url(#mask0_1307_7666)"})
    ET.SubElement(group, "rect", {"x": "-5.36133", "y": "-5.36011", "width": "110", "height": "110", "fill": "url(#pattern0_1307_7666)"})
    defs = ET.SubElement(svg, "defs")
    pattern = ET.SubElement(defs, "pattern", {"id": "pattern0_1307_7666", "patternContentUnits": "objectBoundingBox", "width": "1", "height": "1"})
    ET.SubElement(pattern, "use", {"xlink:href": "#image0_1307_7666", "transform": "scale(0.001002)"})
    ET.SubElement(defs, "image", {"id": "image0_1307_7666", "width": "998", "height": "998", "preserveAspectRatio": "none", "xlink:href": texture[1]})
    save("gr-device-synapse.svg", svg, offset, "A/B")

    offset, raw = component("const E=", "I=(0,r.forwardRef)(E)")
    paths = re.findall(r'r\.createElement\("path",\{([^}]+)\}', raw)
    assert len(paths) == 2, "Changed original Gamer Room icon"
    svg = _svg(32, 32)
    group = ET.SubElement(svg, "g", {"clip-path": "url(#clip0_2345_3127)"})
    for path in paths:
        ET.SubElement(group, "path", _attrs(path))
    clip = ET.SubElement(ET.SubElement(svg, "defs"), "clipPath", {"id": "clip0_2345_3127"})
    ET.SubElement(clip, "rect", {"width": "32", "height": "32", "fill": "white"})
    save("gr-device-app.svg", svg, offset, "E/I")

    offset, raw = component("const U=", "class S ")
    paths = re.findall(r'\(0,i\.jsx\)\("path",\{([^}]+)\}', raw)
    assert len(paths) == 4, "Changed original popup edges"
    for position, edge_paths in (("bottom", paths[:2]), ("top", paths[2:])):
        svg = _svg(380, 20)
        for path in edge_paths:
            ET.SubElement(svg, "path", _attrs(path))
        save(f"gr-popup-{position}.svg", svg, offset, "U", position=position,
             note="The original references an undefined clip path; omitting that inert reference preserves the paths")

    offset, raw = component("const ee=", "},te=")
    path = re.search(r'\(0,i\.jsx\)\("path",\{([^}]+)\}', raw)
    fills = re.search(r'fill:t\?"([^"]+)":"([^"]+)"', raw)
    assert path and fills, "Missing original power-state paths"
    for active, fill in ((False, fills[2]), (True, fills[1])):
        svg = _svg(24, 24)
        attrs = _attrs(path[1])
        attrs["fill"] = fill
        ET.SubElement(svg, "path", attrs)
        save("gr-device-power" + ("-active" if active else "") + ".svg", svg, offset, "ee", active=active)

    for start, end, side, name, symbol in (
        ("},te=", "class fe ", 24, "gr-device-offline.svg", "te"),
        ("class S ", "const Q=", 32, "gr-device-offline-description.svg", "S.renderDescription"),
    ):
        offset, raw = component(start, end)
        paths = re.findall(r'\(0,i\.jsx\)\("path",\{([^}]+)\}', raw)
        assert len(paths) == 1, f"Changed original offline icon in {symbol}"
        svg = _svg(side, side)
        ET.SubElement(svg, "path", _attrs(paths[0]))
        save(name, svg, offset, symbol)


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    records = []
    prepare(root, root / "assets/synapse", lambda src, dst, **meta: records.append({
        "source": src.relative_to(root).as_posix(), "output": dst.relative_to(root).as_posix(), **meta
    }))
    (root / ".work/gamer-room-assets.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    print(f"Extracted {len(records)} inline SVG assets")
