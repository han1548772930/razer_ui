"""Prepare exact 32-bit ICO frames as PNG using only the standard library.

No decoder DLLs, application or reference code are executed. The existing
independently prepared 20px tray RGBA is also checked against the host frame.
"""
import argparse
import hashlib
import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def decode_frame(data, size):
    reserved, kind, count = struct.unpack_from("<HHH", data)
    assert (reserved, kind) == (0, 1)
    frames = []
    for i in range(count):
        width, height, colors, reserved, planes, bits, length, offset = struct.unpack_from("<BBBBHHII", data, 6 + i * 16)
        if (width or 256, height or 256) == (size, size):
            frames.append((bits, length, offset))
    assert len(frames) == 1, "Expected one exact-size ICO frame"
    bits, length, offset = frames[0]
    frame = data[offset:offset + length]
    header, width, height, planes, bpp, compression, image_size, xp, yp, used, important = struct.unpack_from("<IiiHHIIiiII", frame)
    assert (header, width, height, planes, bpp, compression, used) == (40, size, size * 2, 1, 32, 0, 0)
    assert len(frame) == 40 + size * size * 4 + ((size + 31) // 32 * 4) * size
    # ICO BI_RGB color plane: bottom-up, BGRA; 32bpp requires no row padding.
    rgba = bytearray()
    for y in reversed(range(size)):
        row = frame[40 + y * size * 4:40 + (y + 1) * size * 4]
        for x in range(size):
            blue, green, red, alpha = row[x * 4:x * 4 + 4]
            rgba.extend((red, green, blue, alpha))
    assert any(rgba[3::4]), "No alpha plane; a mask-only icon requires a separate decoder"
    # Both selected sources have an alpha plane. As with ICO's 32-bit alpha
    # interpretation, retain these bytes instead of imposing the legacy mask.
    return bytes(rgba), dict(offset=offset, bytes=length, frame_sha256=digest(frame))


def png(rgba, size):
    def chunk(kind, payload):
        body = kind + payload
        return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))
    raw = b"".join(b"\0" + rgba[y * size * 4:(y + 1) * size * 4] for y in range(size))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def prepare(root=ROOT, output=None, check=False):
    output = output or root / "assets/synapse"
    records = []
    for source, name, size, url in [
        (".ref/host-4.0.827/electron/resources/images/rzAppEngine.ico", "host-default-tab.png", 20, None),
        (".ref/applications/synapse/alexa/favicon.ico", "host-alexa-favicon.png", 63, "https://apps.razer.com/synapse/alexa/favicon.ico"),
    ]:
        original = (root / source).read_bytes()
        rgba, frame = decode_frame(original, size)
        if size == 20:
            assert rgba == (root / "assets/synapse/tray-app.rgba").read_bytes(), "Existing independently decoded host pixels differ"
        encoded = png(rgba, size)
        target = output / name
        if check:
            assert target.read_bytes() == encoded, f"Stale converted icon: {name}"
        else:
            target.write_bytes(encoded)
        entry = dict(source=source, output=target.relative_to(root).as_posix(), source_sha256=digest(original),
                     sha256=digest(encoded), width=size, height=size, mode="RGBA",
                     conversion="ico-bgra32-to-png", rgba_sha256=digest(rgba), ico_frame=frame)
        if url:
            entry["source_url"] = url
        records.append(entry)
    return records


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    records = prepare(check=args.check)
    receipt = ROOT / "docs/re/host-tab-ico-current-conversion.json"
    rendered = json.dumps(dict(method="Static ICO32 BGRA frame extraction and PNG encoding; host pixels match existing tray-app.rgba; no resampling", entries=records), indent=2) + "\n"
    if args.check:
        assert receipt.read_text(encoding="utf8") == rendered
    else:
        receipt.write_text(rendered, encoding="utf8")
    print("Prepared/validated host 20px and Alexa 63px exact ICO frames.")


if __name__ == "__main__":
    main()
