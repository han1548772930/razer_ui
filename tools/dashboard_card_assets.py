"""Original current Dashboard device-state artwork; no reference execution."""
import hashlib
import json
import shutil
import struct
import subprocess
from pathlib import Path

NAMES = (
    "arcade-controller-fw-update-disable.svg", "arcade-controller-disable.svg",
    "ps-controller-disable.a16ccaff.avif", "controller-disable.svg", "earbuds-disable.svg",
    "windows_dynamic_lighting_active_icon.svg", "windows_dynamic_lighting_icon.svg",
    "xbox-icon.svg", "icon_device_power_state_off.svg", "headset-disable.svg", "ps-icon.svg",
    "icon_firmware_update-3.svg", "warning.svg", "xbox-o-white.svg", "xbox-menu.svg",
    "xbox-a-white.svg", "xbox-y-white.svg",
)


def prepare(root, out):
    directory = root / ".ref/applications/synapse/dashboard"
    manifest = json.loads((directory / "asset-manifest.json").read_text(encoding="utf-8"))
    destination = out / "dashboard-card"
    destination.mkdir(parents=True, exist_ok=True)
    records = []
    for name in NAMES:
        relative = manifest["files"]["static/media/" + name]
        assert relative.startswith("./static/media/") and ".." not in Path(relative).parts
        source = directory / relative[2:]
        target_name = "ps-controller-disable.png" if name.endswith(".avif") else name
        target = destination / target_name
        extra = {}
        if name.endswith(".avif"):
            converter = shutil.which("magick")
            if not converter:
                raise RuntimeError("The installed ImageMagick converter is required for the current PS AVIF")
            subprocess.run([converter, str(source), "-strip", "-depth", "8", "PNG32:" + str(target)], check=True)
            png = target.read_bytes()
            assert png[:8] == b"\x89PNG\r\n\x1a\n"
            width, height = struct.unpack(">II", png[16:24])
            extra = {"conversion": "Installed ImageMagick: -strip -depth 8 PNG32", "width": width, "height": height, "mode": "RGBA"}
        else:
            shutil.copyfile(source, target)
        records.append({"source": source.relative_to(root).as_posix(),
                        "output": target.relative_to(root).as_posix(),
                        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                        "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), **extra})
    return records
