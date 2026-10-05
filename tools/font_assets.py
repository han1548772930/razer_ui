"""Preserve current font outlines while applying CSS @font-face metadata."""
from hashlib import sha256
from fontTools.ttLib import TTFont

# Only faces used by currently mounted native pages. Both current Dashboard and
# host CSS declare these aliases; current Alexa also uses RazerF5 Thin (100).
FACES = (
    ("Roboto-Light", "Roboto", "Light", 300),
    ("Roboto-Regular", "Roboto", "Regular", 400),
    ("Roboto-Medium", "Roboto", "Medium", 500),
    ("Roboto-Bold", "Roboto", "Bold", 700),
    ("RazerF5-Thin", "RazerF5", "Thin", 100),
    ("RazerF5-Regular", "RazerF5", "Regular", 400),
    ("RazerF5-Bold", "RazerF5", "Bold", 700),
)


def prepare(root, out):
    records = []
    for filename, family, style, weight in FACES:
        source = root / ".ref/host-4.0.827/electron/assets/fonts" / (filename + ".woff2")
        target = out / (filename + ".ttf")
        font = TTFont(source, recalcTimestamp=False, recalcBBoxes=False)
        original = {"family": font["name"].getDebugName(1),
                    "subfamily": font["name"].getDebugName(2),
                    "preferred_family": font["name"].getDebugName(16),
                    "preferred_subfamily": font["name"].getDebugName(17),
                    "weight": font["OS/2"].usWeightClass}
        preserved = {tag: font.getTableData(tag) for tag in font.keys()
                     if tag not in {"GlyphOrder", "name", "OS/2", "head"}}
        normalize = original["family"] != family or original["weight"] != weight
        if normalize:
            names = {1: family, 2: style, 16: family, 17: style}
            for record in list(font["name"].names):
                if record.nameID in names:
                    font["name"].setName(names[record.nameID], record.nameID,
                                         record.platformID, record.platEncID, record.langID)
            for name_id, value in names.items():
                font["name"].setName(value, name_id, 3, 1, 0x409)
            font["OS/2"].usWeightClass = weight
            # CSS normal/bold is separate from the outline file's legacy label.
            font["OS/2"].fsSelection &= ~((1 << 5) | (1 << 6))
            font["OS/2"].fsSelection |= (1 << 5) if weight >= 700 else (1 << 6)
            font["head"].macStyle = (font["head"].macStyle & ~1) | (weight >= 700)
        font.flavor = None
        font.save(target)
        converted = TTFont(target, recalcTimestamp=False, recalcBBoxes=False)
        assert all(converted.getTableData(tag) == data for tag, data in preserved.items()), filename
        assert converted["name"].getDebugName(1) == family, filename
        assert converted["OS/2"].usWeightClass == weight, filename
        records.append({
            "source": source.relative_to(root).as_posix(),
            "output": target.relative_to(root).as_posix(),
            "source_sha256": sha256(source.read_bytes()).hexdigest(),
            "sha256": sha256(target.read_bytes()).hexdigest(),
            "css_face": {"family": family, "style": "normal", "weight": weight},
            "original_metadata": original,
            "metadata_normalized": normalize,
            "unchanged_tables": {tag: sha256(data).hexdigest() for tag, data in preserved.items()},
        })
    return records
