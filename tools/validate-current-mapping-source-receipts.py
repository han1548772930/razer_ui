"""Validate named current-source receipts; no indexing or vendor execution."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
NAMES = (
    "mouse-190-mapping-current-source.json",
    "keyboard-analog-shared-assets-current-source.json",
    "keyboard-two-tap-current-source.json",
    "keyboard-analog-floating-controller-current-source.json",
    "keyboard-controller-drop-current-source.json",
    "mouse-button-state-current-source.json",
    "mouse-190-matcher-current-source.json",
    "keyboard-679-source-artwork-current.json",
    "mapping-engine-current-source.json",
    "mapping-submission-190-current-source.json",
    "mapping-submission-678-current-source.json",
    "mapping-submission-679-current-source.json",
    "mapping-submission-688-current-source.json",
    "gamepad-calibration-focus-current-source.json",
    "gamepad-calibration-focus-host-producers-current.json",
)
CACHE = {}
UTF16_CACHE = {}
FILES_CHECKED = set()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    assert path.startswith("local-ui-reverse/source/"), path
    if path not in CACHE:
        CACHE[path] = (ROOT / path).read_bytes()
        FILES_CHECKED.add(path)
    return CACHE[path]


def walk(node, context=None):
    checked = 0
    if isinstance(node, list):
        return sum(walk(child, context) for child in node)
    if not isinstance(node, dict):
        return 0
    # Helpers and root calls belong to the containing record's main bundle.
    # A main child is not itself the parent of those sibling receipts.
    if isinstance(node.get("main"), dict):
        context = node["main"].get("path", context)
    for key in ("file", "path", "source_file", "source"):
        value = node.get(key)
        if isinstance(value, str) and value.startswith("local-ui-reverse/source/"):
            context = value
            data = load(value)
            expected = node.get("file_sha256") or node.get("source_sha256") or node.get("sha256")
            if expected:
                assert digest(data) == expected.lower(), (value, "file hash")
            break
    if context:
        raw = load(context)
        span = node.get("utf8_byte_range") or node.get("byte_range")
        if span:
            assert 0 <= span[0] <= span[1] <= len(raw), (context, span, "byte bounds")
            segment = raw[span[0]:span[1]]
        else:
            span = node.get("utf16_range") or node.get("char_range")
            if not span and "offset" in node and "end" in node:
                span = [node["offset"], node["end"]]
            if span:
                if context not in UTF16_CACHE:
                    UTF16_CACHE[context] = raw.decode("utf-8").encode("utf-16-le")
                encoded = UTF16_CACHE[context]
                assert 0 <= span[0] <= span[1] <= len(encoded) // 2, (context, span, "UTF-16 bounds")
                segment = encoded[span[0]*2:span[1]*2].decode("utf-16-le").encode("utf-8")
            elif isinstance(node.get("source"), str) and "byte_offset" in node:
                start = node["byte_offset"]
                segment = node["source"].encode("utf-8")
                span = [start, start + len(segment)]
                assert 0 <= start <= span[1] <= len(raw), (context, span, "byte bounds")
                segment = raw[start:span[1]]
        if span:
            expected = node.get("slice_sha256")
            if expected:
                assert digest(segment) == expected.lower(), (context, span, "slice hash")
            text = node.get("source")
            if isinstance(text, str) and not text.startswith("local-ui-reverse/source/"):
                assert segment.decode("utf-8") == text, (context, span, "source text")
            checked += 1
    for value in node.values():
        if isinstance(value, (list, dict)):
            checked += walk(value, context)
    return checked


for name in NAMES:
    record = json.loads((ROOT / "docs/re" / name).read_text(encoding="utf-8-sig"))
    count = walk(record)
    assert count > 0, (name, "no source spans validated")
    print(json.dumps({"receipt": name, "checked_spans": count}))
    # Bound memory to one receipt instead of keeping the entire corpus loaded.
    CACHE.clear()
    UTF16_CACHE.clear()
print(json.dumps({"current_source_files_checked": len(FILES_CHECKED)}))
