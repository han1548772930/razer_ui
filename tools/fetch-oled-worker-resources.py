"""Fetch and statically validate product-691 GIF-worker dependencies.

Uses exact current manifest URLs and the maintained HTTP/hash snapshot helper.
GIF frames are decoded only as media data; JS, WASM and source maps are never run.
The optional source maps may be unavailable without blocking media acquisition.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import importlib.util
import io
import json
from pathlib import Path

from PIL import Image, ImageSequence

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / ".ref/devices/691"
PREFIX = "/synapse/products/691/ui/"
KEYS = {
    **{f"static/media/{index}_15fps.gif": "gif" for index in range(1, 7)},
    "static/media/lets-go-animated_232x64.gif": "gif",
    "static/media/magick.wasm": "wasm",
    "gifWorker.91a1316a.chunk.js.map": "sourcemap",
    "9537.835bd6f0.chunk.js.map": "sourcemap",
}
spec = importlib.util.spec_from_file_location(
    "discovery", Path(__file__).with_name("discover-razer-products.py"))
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)
original_validator = discovery.validate_payload


def unsigned_leb(data, offset):
    value = 0
    for shift in range(0, 35, 7):
        byte = data[offset]
        offset += 1
        value |= (byte & 127) << shift
        if not byte & 128:
            return value, offset
    raise ValueError("Invalid WASM section length")


def validate_payload(kind, data, pid=None):
    if kind not in ("gif", "wasm", "sourcemap"):
        return original_validator(kind, data, pid)
    try:
        extra = {}
        if kind == "gif":
            if data[:6] not in (b"GIF87a", b"GIF89a"):
                raise ValueError("Expected GIF signature")
            with Image.open(io.BytesIO(data)) as image:
                dimensions = list(image.size)
                loop = image.info.get("loop")
                delays = []
                for frame in ImageSequence.Iterator(image):
                    frame.load()
                    if list(frame.size) != dimensions:
                        raise ValueError("GIF frame dimensions changed")
                    delays.append(frame.info.get("duration", 0))
            if not delays or dimensions != [232, 64] or any(delay <= 0 for delay in delays):
                raise ValueError("Expected positive-duration 232x64 OLED GIF frames")
            extra = {"dimensions": dimensions, "frames": len(delays), "loop": loop,
                     "frame_durations_ms": delays}
        elif kind == "wasm":
            if data[:8] != b"\0asm\1\0\0\0":
                raise ValueError("Expected WASM version-1 header")
            offset, sections = 8, []
            while offset < len(data):
                tag = data[offset]
                size, start = unsigned_leb(data, offset + 1)
                end = start + size
                if tag > 12 or end > len(data):
                    raise ValueError("Invalid WASM section bounds")
                sections.append({"id": tag, "offset": start, "bytes": size})
                offset = end
            if offset != len(data) or not any(section["id"] == 10 for section in sections):
                raise ValueError("Expected complete WASM code section")
            extra = {"version": 1, "sections": sections,
                     "validation_scope": "header and section bounds; no execution"}
        else:
            value = json.loads(data)
            if value.get("version") != 3 or not isinstance(value.get("sources"), list):
                raise ValueError("Expected source-map version 3")
            extra = {"sources": value["sources"],
                     "sources_content_count": len(value.get("sourcesContent", []))}
        return {"valid": True, "kind": kind, "reason": None, **extra}
    except (IndexError, KeyError, OSError, TypeError, ValueError) as error:
        return {"valid": False, "kind": kind, "reason": str(error)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--retry-errors", action="store_true")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--attempts", type=int, default=3)
    options = parser.parse_args()
    options.keepalive = True
    if options.workers < 1 or options.timeout <= 0 or options.attempts < 1:
        parser.error("workers, timeout and attempts must be positive")
    if options.offline and options.refresh:
        parser.error("offline and refresh are mutually exclusive")
    manifest = json.loads((BASE / "asset-manifest.json").read_text())
    jobs = []
    for key, kind in KEYS.items():
        declared = manifest["files"][key]
        if not declared.startswith(PREFIX):
            parser.error(f"Unexpected product resource URL: {declared}")
        relative = declared.removeprefix(PREFIX)
        target = (BASE / relative).resolve()
        if not target.is_relative_to(BASE.resolve()):
            parser.error("Resource path leaves current product snapshot")
        jobs.append(("https://apps.razer.com" + declared, target, kind))
    # Specialize validation in this helper's process only; shared tools retain
    # their own schema. The downloaded code is never loaded as Python or JS.
    discovery.validate_payload = validate_payload
    with concurrent.futures.ThreadPoolExecutor(max_workers=options.workers) as pool:
        replies = list(pool.map(lambda job: discovery.fetch_snapshot(*job, options), jobs))
    for (url, _, kind), reply in zip(jobs, replies):
        print(f"{reply['result']}: {kind} {url}", flush=True)
    if any(reply["result"] != "ok" and not (kind == "sourcemap" and reply["result"] == "not_found")
           for (_, _, kind), reply in zip(jobs, replies)):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
