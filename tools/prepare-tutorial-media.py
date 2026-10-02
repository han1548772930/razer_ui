"""Convert source muted looping tutorials to GPUI's native animated WebP format.

Preserves decoded frame dimensions and timing; never runs downloaded JavaScript.
Requires the resource Python environment and requirements-tutorial-media.txt.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import importlib.util
import json
import shutil
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.parse import quote, unquote, urljoin, urlparse

import imageio_ffmpeg
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("discovery", Path(__file__).with_name("discover-razer-products.py"))
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)
SOURCES = (
    ("tutorial-dashboard", "frontend", "Synapse Dashboard Tutorial.mp4"),
    ("tutorial-gamer-room-1", "frontend", "Gamer Room Dashboard Tutorial 1.mp4"),
    ("tutorial-gamer-room-2", "frontend", "Gamer Room Dashboard Tutorial 2.mp4"),
    ("tour-quick-effects", "applications/synapse/introduction-tour", "quick_effect_advanced_effect.mp4"),
    ("tour-devices-modules", "applications/synapse/introduction-tour", "devices_and_modules_tab.mp4"),
    ("tour-razer-apps", "applications/synapse/introduction-tour", "razer_more_apps.mp4"),
    ("tour-macros", "applications/synapse/introduction-tour", "macros.mp4"),
    ("tour-linked-games", "applications/synapse/introduction-tour", "linked_games.mp4"),
    ("tour-background", "applications/synapse/introduction-tour", "large_background_image.cd5f51c6.avif"),
    ("tour-app-icon", "applications/synapse/introduction-tour", "icon_app.svg"),
    ("tour-chroma-quick-effects", "applications/synapse/introduction-tour", "quick_effects.9f9806eb.avif"),
    ("tour-chroma-advanced-effects", "applications/synapse/introduction-tour", "chroma_studio.abd5d541.avif"),
    ("tour-chroma-apps", "applications/synapse/introduction-tour", "chroma_apps.ca901740.avif"),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--download-only", action="store_true")
    parser.add_argument("--offline", action="store_true")
    options = parser.parse_args()
    options.refresh, options.retry_errors, options.keepalive = False, True, True
    options.attempts, options.timeout = 3, 45
    jobs = []
    for name, folder, key in SOURCES:
        manifest_path = ROOT / ".ref" / folder / "asset-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
        matches = [value for request, value in manifest["files"].items() if request == "static/media/" + key]
        if len(matches) != 1:
            raise ValueError(f"Missing or ambiguous media request: {folder}/{key}")
        route = "synapse/dashboard/" if folder == "frontend" else "synapse/introduction-tour/"
        url = urljoin("https://apps.razer.com/" + route, quote(matches[0], safe="/%:._-"))
        source = ROOT / ".ref/tutorial-media" / unquote(Path(urlparse(url).path).name)
        jobs.append((name, url, source, manifest_path))
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        replies = list(pool.map(lambda job: discovery.fetch_snapshot(job[1], job[2],
            "svg" if job[2].suffix == ".svg" else "media", options), jobs))
    for job, reply in zip(jobs, replies):
        if reply["result"] != "ok":
            raise RuntimeError(f"Media download failed: {job[1]} ({reply['result']})")
        print(f"Source {job[0]}: {reply['bytes']} bytes", flush=True)
    if options.download_only:
        return
    executable = imageio_ffmpeg.get_ffmpeg_exe()
    output = ROOT / "assets/synapse"
    prior_path = output / "tutorial-media-manifest.json"
    prior = {entry["output"]: entry for entry in json.loads(prior_path.read_text(encoding="utf-8"))["entries"]} if prior_path.is_file() else {}
    entries = []
    for name, url, source, manifest in jobs:
        destination = output / (name + {".avif": ".png", ".svg": ".svg"}.get(source.suffix, ".webp"))
        previous = prior.get(destination.relative_to(ROOT).as_posix())
        if previous and destination.is_file() and previous["source_sha256"] == discovery.sha256(source.read_bytes()) and previous["sha256"] == discovery.sha256(destination.read_bytes()):
            entries.append(previous)
            print(f"Verified existing {name}", flush=True)
            continue
        if source.suffix == ".svg":
            shutil.copyfile(source, destination)
            entries.append({"source": source.relative_to(ROOT).as_posix(), "source_url": url,
                "source_sha256": discovery.sha256(source.read_bytes()),
                "output": destination.relative_to(ROOT).as_posix(),
                "sha256": discovery.sha256(destination.read_bytes()),
                "resource_manifest": manifest.relative_to(ROOT).as_posix()})
            continue
        if source.suffix == ".avif":
            with Image.open(source) as im:
                im.convert("RGBA").save(destination, optimize=True)
        else:
            subprocess.run([executable, "-v", "error", "-y", "-i", str(source), "-an",
                "-c:v", "libwebp_anim", "-lossless", "1", "-compression_level", "4",
                "-loop", "0", "-fps_mode", "passthrough", str(destination)], check=True)
        with Image.open(destination) as im:
            delays = []
            for frame in range(im.n_frames):
                im.seek(frame)
                im.load()
                if im.n_frames > 1:
                    delays.append(im.info["duration"])
            entry = {"source": source.relative_to(ROOT).as_posix(), "source_url": url,
                "source_sha256": discovery.sha256(source.read_bytes()),
                "output": destination.relative_to(ROOT).as_posix(),
                "sha256": discovery.sha256(destination.read_bytes()),
                "width": im.width, "height": im.height, "frames": im.n_frames,
                "frame_durations_ms": delays, "loop": im.info.get("loop"),
                "resource_manifest": manifest.relative_to(ROOT).as_posix(),
                "conversion": "Decoded original frames, lossless animated WebP, original dimensions/timing, audio omitted because source video is muted" if delays else "AVIF to RGBA PNG"}
        entries.append(entry)
        print(f"Converted {name}: {entry['width']}x{entry['height']}, {entry['frames']} frames, {sum(delays)}ms, {destination.stat().st_size} bytes", flush=True)
    # The rich text parser supports image geometry but not CSS inline margins.
    # Expand only the SVG viewport: 8px + 16px source icon + 8px at native size.
    source = next(job[2] for job in jobs if job[0] == "tour-app-icon")
    svg = ET.fromstring(source.read_bytes())
    svg.set("viewBox", "-10 0 40 20")
    svg.set("width", "40")
    ET.register_namespace("", "http://www.w3.org/2000/svg")
    destination = output / "tour-app-inline.svg"
    ET.ElementTree(svg).write(destination, encoding="utf-8", xml_declaration=True)
    entries.append({"source": source.relative_to(ROOT).as_posix(),
        "source_sha256": discovery.sha256(source.read_bytes()),
        "output": destination.relative_to(ROOT).as_posix(),
        "sha256": discovery.sha256(destination.read_bytes()),
        "conversion": "Original icon paths; expanded SVG viewport preserves u-ml-2/u-mr-2 around the 16px inline image"})
    # CSS .fake-background is a black 100px band at y=600, blurred by 25px.
    # A one-column alpha mask retains that geometry when stretched horizontally.
    css = ROOT / ".ref/applications/synapse/introduction-tour/static/css/main.adb3bb78.css"
    assert "filter:blur(25px);height:100px" in css.read_text(encoding="utf-8")
    mask = Image.new("L", (1, 250))
    mask.paste(255, (0, 100, 1, 200))
    fade = Image.new("RGBA", mask.size, (0, 0, 0, 0))
    fade.putalpha(mask.filter(ImageFilter.GaussianBlur(25)))
    destination = output / "tour-background-fade.png"
    fade.save(destination, optimize=True)
    entries.append({"source": css.relative_to(ROOT).as_posix(),
        "source_sha256": discovery.sha256(css.read_bytes()),
        "output": destination.relative_to(ROOT).as_posix(),
        "sha256": discovery.sha256(destination.read_bytes()), "width": 1, "height": 250,
        "conversion": "Black alpha mask for source .fake-background: y=600, height=100, blur=25; rendered at y=500"})
    discovery.write_json(output / "tutorial-media-manifest.json", {"entries": entries,
        "ffmpeg": imageio_ffmpeg.get_ffmpeg_version()})


if __name__ == "__main__":
    main()
