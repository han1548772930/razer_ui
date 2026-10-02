"""Extract the current Dashboard pairing SVG literals without executing JS."""
import hashlib
import json
import re
from pathlib import Path
from urllib.parse import unquote
from xml.sax.saxutils import quoteattr


def prepare(root, output):
    folder = root / ".ref/applications/synapse/dashboard"
    manifest = json.loads((folder / "asset-manifest.json").read_text(encoding="utf-8"))
    css = folder / "static/css/7861.a49b4dc6.chunk.css"
    script = folder / "static/js/7861.1b0e99a4.chunk.js"
    declared = {value.removeprefix("./") for value in manifest["files"].values()}
    assert css.relative_to(folder).as_posix() in declared
    assert script.relative_to(folder).as_posix() in declared
    records = []

    def save(name, source, content, **evidence):
        destination = output / ("pairing-" + name + ".svg")
        destination.write_text(content, encoding="utf-8")
        records.append({"asset": "synapse/" + destination.name,
            "output": destination.relative_to(root).as_posix(),
            "source": source.relative_to(root).as_posix(),
            "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            **evidence, "sha256": hashlib.sha256(destination.read_bytes()).hexdigest()})

    text = css.read_text(encoding="utf-8")
    for name, suffix in [("keyboard", "J1Q8r"), ("mouse", "Ijr7F")]:
        selector = f".DeviceCard_categoryIcon__HomVd.DeviceCard_{name}__{suffix}"
        rule = re.search(re.escape(selector) + r"\{([^}]+)\}", text)
        assert rule, selector
        data = re.search(r"url\('data:image/svg\+xml;charset=utf-8,([^']+)'\)", rule[1])
        assert data, selector
        save(name, css, unquote(data[1]), selector=selector,
            source_character_offset=rule.start(), transform="decode original CSS SVG data URL")

    text = script.read_text(encoding="utf-8")
    for name, symbol, forwarded in [("paired", "Os", "_s"), ("unpair", "Ls", "Ts")]:
        function = re.search(r"const " + symbol + r"=\(e,t\)=>\{(.*?)\}," + forwarded
                             + r"=\(0,i.forwardRef\)\(" + symbol + r"\)", text)
        assert function, symbol
        svg = re.search(r'createElement\("svg",\w+\(\{(.*?)\},\w+\)', function[1])
        path = re.search(r'createElement\("path",\{d:("[^"]+"),fill:("[^"]+")\}\)', function[1])
        assert svg and path, symbol
        attributes = {}
        for key in ["width", "height", "viewBox", "fill", "xmlns"]:
            value = re.search(r'(?:^|,)' + key + r':("[^"]*"|[0-9]+)(?:,|$)', svg[1])
            assert value, (symbol, key)
            attributes[key] = str(json.loads(value[1]))
        content = '<svg ' + ' '.join(key + '=' + quoteattr(value)
            for key, value in attributes.items()) + '><path d=' + quoteattr(json.loads(path[1]))
        content += ' fill=' + quoteattr(json.loads(path[2])) + '/></svg>'
        save(name, script, content, source_symbol=symbol,
            source_character_offset=function.start() + len("const "),
            transform="serialize original React SVG attributes and path without geometry changes")
    (output / "pairing-manifest.json").write_text(
        json.dumps({"assets": records}, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    prepare(root, root / "assets/synapse")
